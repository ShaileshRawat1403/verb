//! A loopback browser host for the same durable project, sessions, and task ledger as the TUI.
//! The browser is a view/controller; this process remains the sole owner of every PTY it starts.

mod state;
use state::{terminal_status, StateCache, StateJob};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use crate::phone::PhoneShare;
use crate::tui::term::Hosted;
use crate::{project, workbench, Agent, SessionState};

const MAX_BODY: usize = 64 * 1024;
const MAX_BUFFER: usize = 2 * 1024 * 1024;
/// How long a launch waits for the execution boundary to admit or refuse an external terminal.
/// The boundary client retries a capacity refusal for up to 3 s before giving up.
const ADMISSION_WAIT: Duration = Duration::from_secs(6);
/// Shown when the execution boundary has no free terminal slot.
const TERMINAL_CAPACITY: &str =
    "All terminal slots on this host are in use. End a terminal, then start another.";
/// Shown when the execution boundary gave no verdict in time; the terminal has been closed.
const TERMINAL_START_TIMEOUT: &str =
    "The terminal did not start in time and was closed. Try again in a moment.";
const MAX_REPLY: usize = 128 * 1024;

const MAX_BACKGROUND_REQUESTS: usize = 8;
const TERMINAL_CLEANUP_UNCONFIRMED: &str =
    "The terminal ended, but the execution boundary has not \
    confirmed its cleanup yet. Its slot stays reserved until it does.";
const HOST_BUSY: &str = "desktop host is busy; retry shortly";
const MAX_TERMINAL_CONNECTIONS: usize = 32;
struct RequestPermit(Arc<AtomicUsize>);
impl RequestPermit {
    fn acquire(counter: &Arc<AtomicUsize>, limit: usize) -> Option<Self> {
        let mut count = counter.load(Ordering::Acquire);
        while count < limit {
            match counter.compare_exchange_weak(
                count,
                count + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return Some(Self(Arc::clone(counter))),
                Err(actual) => count = actual,
            }
        }
        None
    }
}
impl Drop for RequestPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

struct OutputChunk {
    from: u64,
    bytes: Vec<u8>,
}

/// How an external terminal's start resolved, short of refusal or timeout (which are errors).
#[derive(Debug, PartialEq, Eq)]
enum Admission {
    /// The boundary issued the session's lease: the terminal started.
    Confirmed,
    /// The terminal ended (with this exit status) before any admission was observed. It is shown
    /// so its status and output stay available, but no start is claimed: the client may have
    /// failed before the boundary admitted anything (for example exit 126 on a controller error).
    EndedUnconfirmed(i32),
}

/// Waits until the execution boundary admits the external terminal `launch`, or refuses it. Admission is the controller's lease receipt, so two simultaneous launches cannot both
/// be reported as running when only one slot is free.
///
/// - Admitted (a receipt was observed at any point): `Confirmed`, whatever its later exit code.
/// - Refused: exit 75 with the boundary's own refusal text and no observed admission. Never shown;
///   dropped once the terminal loop has closed out its record.
/// - Any other exit without observed admission: `EndedUnconfirmed`. Shown with its status and
///   output; the absence of refusal text is not taken as evidence that it was admitted.
/// - No verdict before `wait`: stopped and dropped, and reported as a start timeout. A terminal is
///   never reported as running without acknowledgement.
fn await_admission(
    terminals: &std::sync::Mutex<HashMap<String, WebTerminal>>,
    id: &str,
    launch: &crate::isolation::Launch,
    wait: Duration,
    admitted: impl Fn(&crate::isolation::Launch) -> bool,
) -> Result<Admission, String> {
    let deadline = Instant::now() + wait;
    let mut seen = false;
    loop {
        seen = seen || admitted(launch);
        let mut map = terminals.lock().unwrap();
        let Some(terminal) = map.get_mut(id) else {
            return Err("the terminal closed before it started".to_owned());
        };
        if seen {
            terminal.pending_admission = None;
            return Ok(Admission::Confirmed);
        }
        if let Some(code) = terminal.exit_code {
            if code != crate::isolation::REFUSED_EXIT || !terminal.reported_refusal() {
                terminal.pending_admission = None;
                return Ok(Admission::EndedUnconfirmed(code));
            }
            // Wait for the terminal loop to close out the session record, then forget the
            // never-presented terminal so nothing hidden lingers in the host.
            if terminal.hosted.is_none() || Instant::now() >= deadline + Duration::from_secs(3) {
                map.remove(id);
                return Err(TERMINAL_CAPACITY.to_owned());
            }
        } else if Instant::now() >= deadline {
            let terminal = map.remove(id);
            drop(map);
            let cleanup = terminal
                .and_then(|mut terminal| terminal.hosted.take())
                .map_or(Ok(()), |hosted| hosted.stop().map(|_| ()));
            return Err(match cleanup {
                Ok(()) => TERMINAL_START_TIMEOUT.to_owned(),
                Err(error) => {
                    format!("{TERMINAL_START_TIMEOUT} Cleanup was not confirmed: {error}")
                }
            });
        }
        drop(map);
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Work that must happen only after a launched terminal is confirmed started (for example the spec
/// record of an agent start). It runs once, after admission, and never for a refused launch.
type AfterAdmission = Box<dyn FnOnce() -> Result<(), String> + Send>;

/// The spec audit line for a confirmed agent start. A handoff's intent is recorded separately.
fn spec_start_line(agent: &str, session: &str, continues_talk: bool) -> String {
    format!(
        "started {agent} on this spec (session {}){}",
        &session[..session.len().min(8)],
        if continues_talk {
            " to continue the talk"
        } else {
            ""
        }
    )
}

/// Runs the post-admission work for a terminal that is running. A failure is reported in the reply
/// rather than failing the launch, so the running terminal stays tracked.
fn after_admission(mut body: Value, after: Option<AfterAdmission>) -> Value {
    if let Some(after) = after {
        if let Err(error) = after() {
            body["warning"] = json!(format!(
                "The terminal is running, but its spec record could not be written: {error}"
            ));
        }
    }
    body
}

/// Reply for a terminal that ended before the boundary confirmed it: it stays visible for
/// diagnosis, and nothing records it as started.
fn unconfirmed_start(mut body: Value, code: i32) -> Value {
    body["warning"] = json!(format!(
        "The terminal ended (exit {code}) before the execution boundary confirmed it started. \
         Its output is kept for diagnosis; no start was recorded."
    ));
    body
}

/// What a launch's `start` produces: the session, the route's own reply body (for example a spec
/// handoff note, sent only after admission) and any work that must follow confirmed admission.
type Started = (String, Value, Option<AfterAdmission>);

/// Every terminal launch answers through this. `start` runs on the worker reserved here, never on the
/// request loop: it may cross the execution boundary before the client exists, and an external
/// terminal's answer then waits for the boundary's verdict. A saturated host refuses before `start`
/// runs, so no terminal starts without a worker to acknowledge it.
fn launch_reply(
    counter: &Arc<AtomicUsize>,
    terminals: &Arc<std::sync::Mutex<HashMap<String, WebTerminal>>>,
    generation: &Arc<AtomicU64>,
    start: impl FnOnce() -> Result<Started, String> + Send + 'static,
    admitted: fn(&crate::isolation::Launch) -> bool,
) -> Reply {
    let Some(permit) = RequestPermit::acquire(counter, MAX_BACKGROUND_REQUESTS) else {
        return Reply::error(503, HOST_BUSY);
    };
    let terminals = Arc::clone(terminals);
    let generation = Arc::clone(generation);
    let mut reply = Reply::json(201, Value::Null);
    reply.permit = Some(permit);
    reply.deferred = Some(Box::new(move || {
        let (session, body, after) = start()?;
        answer_launch(&terminals, &generation, &session, body, after, admitted)
    }));
    reply
}

/// The answer for a started terminal: at once when it does not cross the execution boundary,
/// otherwise once the boundary admits or refuses it (or the wait times out).
fn answer_launch(
    terminals: &std::sync::Mutex<HashMap<String, WebTerminal>>,
    generation: &AtomicU64,
    session: &str,
    body: Value,
    after: Option<AfterAdmission>,
    admitted: fn(&crate::isolation::Launch) -> bool,
) -> Result<Vec<u8>, String> {
    let pending = terminals
        .lock()
        .unwrap()
        .get(session)
        .and_then(|terminal| terminal.pending_admission.clone());
    let Some(launch) = pending else {
        return Ok(after_admission(body, after).to_string().into_bytes());
    };
    let result = await_admission(terminals, session, &launch, ADMISSION_WAIT, admitted);
    generation.fetch_add(1, Ordering::SeqCst);
    result.map(|admission| {
        match admission {
            Admission::Confirmed => after_admission(body, after),
            Admission::EndedUnconfirmed(code) => unconfirmed_start(body, code),
        }
        .to_string()
        .into_bytes()
    })
}

/// Status and body for slow work that failed. A full execution boundary is temporary congestion
/// (503, `code: "capacity"`), never a Git or request failure; everything else stays 400.
fn deferred_error(error: &str) -> (u16, Vec<u8>) {
    if let Some(message) = error.strip_prefix(crate::studio::CONFLICT) {
        (
            409,
            json!({"error": message, "code": "file_conflict"})
                .to_string()
                .into_bytes(),
        )
    } else if error.starts_with(TERMINAL_START_TIMEOUT) {
        (
            503,
            json!({"error": error, "code": "startup_timeout"})
                .to_string()
                .into_bytes(),
        )
    } else if error == crate::project_read::TIMEOUT {
        (
            503,
            json!({"error": error, "code": "read_timeout"})
                .to_string()
                .into_bytes(),
        )
    } else if error == TERMINAL_CAPACITY || crate::isolation::capacity_message(error) {
        (
            503,
            json!({"error": error, "code": "capacity"})
                .to_string()
                .into_bytes(),
        )
    } else {
        (400, json!({"error": error}).to_string().into_bytes())
    }
}

/// Stops a terminal the request loop gave up on without holding the loop: its cleanup may wait on
/// the execution boundary. One reaper thread, so a stalled controller holds one cleanup at a time.
fn stop_in_background(hosted: Hosted) {
    static REAPER: std::sync::OnceLock<std::sync::mpsc::Sender<Hosted>> =
        std::sync::OnceLock::new();
    let reaper = REAPER.get_or_init(|| {
        let (sender, stops) = std::sync::mpsc::channel::<Hosted>();
        std::thread::spawn(move || {
            for hosted in stops {
                if let Err(error) = hosted.stop() {
                    eprintln!("Verb could not confirm a stopped web terminal: {error}");
                }
            }
        });
        sender
    });
    if let Err(std::sync::mpsc::SendError(hosted)) = reaper.send(hosted) {
        let _ = hosted.stop();
    }
}

struct WebTerminal {
    hosted: Option<Hosted>,
    sink: Arc<crate::stream::TerminalStreamSink>,
    chunks: VecDeque<OutputChunk>,
    base: u64,
    next: u64,
    buffered: usize,
    final_screen: Vec<u8>,
    exit_seen_at: Option<Instant>,
    /// Exit status once the process has ended, kept for the launch acknowledgement.
    exit_code: Option<i32>,
    /// For an external terminal not yet admitted by the execution boundary: its launch identity.
    /// A pending terminal is not presented as a session until admission is confirmed.
    pending_admission: Option<crate::isolation::Launch>,
    failure: Option<String>,
    /// For the observer, in memory only: when this terminal last produced output, and recent
    /// failed commands (shell integration's volatile label, never written anywhere).
    last_output: Instant,
    failures: VecDeque<(String, Instant)>,
}

impl WebTerminal {
    fn new(
        mut hosted: Hosted,
        sink: Arc<crate::stream::TerminalStreamSink>,
        pending_admission: Option<crate::isolation::Launch>,
    ) -> Self {
        hosted.capture_web_output();
        Self {
            hosted: Some(hosted),
            sink,
            chunks: VecDeque::new(),
            base: 0,
            next: 0,
            buffered: 0,
            final_screen: Vec::new(),
            exit_seen_at: None,
            exit_code: None,
            pending_admission,
            failure: None,
            last_output: Instant::now(),
            failures: VecDeque::new(),
        }
    }

    fn push(&mut self, mut bytes: Vec<u8>) {
        if bytes.len() > MAX_BUFFER {
            self.next += (bytes.len() - MAX_BUFFER) as u64;
            bytes = bytes.split_off(bytes.len() - MAX_BUFFER);
            self.chunks.clear();
            self.buffered = 0;
            self.base = self.next;
        }
        self.buffered += bytes.len();
        self.next += bytes.len() as u64;
        self.chunks.push_back(OutputChunk {
            from: self.next - bytes.len() as u64,
            bytes,
        });
        while self.buffered > MAX_BUFFER {
            if let Some(oldest) = self.chunks.pop_front() {
                self.buffered -= oldest.bytes.len();
                self.base = oldest.from + oldest.bytes.len() as u64;
            }
        }
    }

    /// Whether this terminal's own output carries the boundary client's pre-lease refusal text.
    fn reported_refusal(&self) -> bool {
        let start: Vec<u8> = self
            .chunks
            .iter()
            .flat_map(|chunk| chunk.bytes.iter().copied())
            .take(4096)
            .collect();
        crate::isolation::capacity_message(&String::from_utf8_lossy(&start))
    }

    fn poll(&mut self) -> Result<(), String> {
        let Some(hosted) = &mut self.hosted else {
            return Ok(());
        };
        let (exit, _) = hosted.poll()?;
        // The TUI consumes structural notices for its activity view. The browser reads the
        // durable ledger instead; here only failed commands are kept, briefly and in memory, for
        // the observer's failing-loop signal.
        for notice in hosted.take_structural() {
            if let crate::pty::Structural::CommandFinished {
                exit_code,
                label: Some(label),
                ..
            } = notice
            {
                if exit_code != 0 {
                    self.failures.push_back((label, Instant::now()));
                    while self.failures.len() > 20 {
                        self.failures.pop_front();
                    }
                }
            }
        }
        for chunk in hosted.take_web_output() {
            self.last_output = Instant::now();
            self.push(chunk);
        }
        if let Some(code) = exit {
            self.exit_code = Some(code);
            let seen = self.exit_seen_at.get_or_insert_with(Instant::now);
            if self
                .hosted
                .as_ref()
                .is_some_and(|hosted| hosted.output_drained())
                || seen.elapsed() >= Duration::from_secs(2)
            {
                let hosted = self.hosted.take().ok_or("terminal host disappeared")?;
                self.final_screen = hosted.screen().contents_formatted();
                hosted.finish(code)?;
            }
        }
        Ok(())
    }

    fn fail(&mut self, error: String) {
        self.failure = Some(error);
        if let Some(hosted) = self.hosted.take() {
            self.final_screen = hosted.screen().contents_formatted();
            stop_in_background(hosted);
        }
    }

    fn output(&self, after: u64) -> Value {
        let (controller, phone_connected) = self
            .hosted
            .as_ref()
            .and_then(|hosted| hosted.phone_control_status().ok())
            .unwrap_or((crate::mobile::Controller::Desktop, false));
        if after < self.base || after > self.next {
            let screen = self.hosted.as_ref().map_or_else(
                || self.final_screen.clone(),
                |hosted| hosted.screen().contents_formatted(),
            );
            return json!({"reset": true, "data": BASE64.encode(screen), "cursor": self.next,
                "running": self.hosted.is_some(), "error": self.failure,
                "controller": controller, "phoneConnected": phone_connected});
        }
        let mut bytes = Vec::new();
        let mut cursor = after;
        for chunk in &self.chunks {
            let end = chunk.from + chunk.bytes.len() as u64;
            if end <= cursor {
                continue;
            }
            let start = cursor.saturating_sub(chunk.from) as usize;
            let room = MAX_REPLY.saturating_sub(bytes.len());
            if room == 0 {
                break;
            }
            let part = &chunk.bytes[start..];
            let count = room.min(part.len());
            bytes.extend_from_slice(&part[..count]);
            cursor += count as u64;
            if count < part.len() {
                break;
            }
        }
        json!({"reset": false, "data": BASE64.encode(bytes), "cursor": cursor,
            "running": self.hosted.is_some(), "error": self.failure,
            "controller": controller, "phoneConnected": phone_connected})
    }
}

pub struct WebTerminalsManager {
    terminals: Arc<std::sync::Mutex<HashMap<String, WebTerminal>>>,
    access_token: Option<String>,
    expires_at: Option<Instant>,
}

impl crate::stream::StreamTerminalHandler for WebTerminalsManager {
    fn is_authorized(&self) -> bool {
        self.expires_at.is_none_or(|expiry| Instant::now() < expiry)
            && self.access_token.as_deref().is_none_or(|token| {
                crate::tokens::verify(token)
                    .is_some_and(|access| access.allows(crate::tokens::Scope::Drive))
            })
    }
    fn write_terminal_input(&self, id: &str, data: &[u8]) -> Result<(), String> {
        let mut terminals = self.terminals.lock().map_err(|e| e.to_string())?;
        let terminal = terminals.get_mut(id).ok_or("terminal not found")?;
        let hosted = terminal.hosted.as_mut().ok_or("terminal has ended")?;
        hosted.write(data)
    }

    fn attach_terminal(
        &self,
        id: &str,
        conn_id: u64,
        queue: Arc<crate::stream::WsQueue>,
        rows: u16,
        cols: u16,
    ) -> Option<crate::stream::TerminalAttachInfo> {
        let mut terminals = self.terminals.lock().ok()?;
        let terminal = terminals.get_mut(id)?;
        terminal.sink.add_listener(conn_id, queue);
        let (screen, running, controller, phone_connected) =
            if let Some(hosted) = &mut terminal.hosted {
                if rows >= 4 && cols >= 20 {
                    hosted.resize(rows, cols);
                }
                let (ctrl, phone) = hosted
                    .phone_control_status()
                    .unwrap_or((crate::mobile::Controller::Desktop, false));
                let mut screen_bytes = hosted.screen().contents_formatted();
                if hosted.full_screen_app() {
                    let mut prefixed = b"\x1b[?1049h".to_vec();
                    prefixed.extend(screen_bytes);
                    screen_bytes = prefixed;
                }
                (
                    screen_bytes,
                    true,
                    match ctrl {
                        crate::mobile::Controller::Desktop => "desktop".to_owned(),
                        crate::mobile::Controller::Phone => "phone".to_owned(),
                    },
                    phone,
                )
            } else {
                (
                    terminal.final_screen.clone(),
                    false,
                    "desktop".to_owned(),
                    false,
                )
            };
        Some(crate::stream::TerminalAttachInfo {
            screen,
            running,
            controller,
            phone_connected,
        })
    }

    fn detach_terminal(&self, id: &str, conn_id: u64) {
        if let Ok(terminals) = self.terminals.lock() {
            if let Some(terminal) = terminals.get(id) {
                terminal.sink.remove_listener(conn_id);
            }
        }
    }

    fn resize_terminal(&self, id: &str, rows: u16, cols: u16) {
        if rows < 4 || cols < 20 {
            return;
        }
        if let Ok(mut terminals) = self.terminals.lock() {
            if let Some(terminal) = terminals.get_mut(id) {
                if let Some(hosted) = &mut terminal.hosted {
                    hosted.resize(rows, cols);
                }
            }
        }
    }

    fn ack_terminal(&self, id: &str, bytes: usize) {
        if let Ok(terminals) = self.terminals.lock() {
            if let Some(terminal) = terminals.get(id) {
                terminal.sink.ack(bytes);
            }
        }
    }

    fn signal_terminal(&self, id: &str, signal: &str) {
        if let Ok(terminals) = self.terminals.lock() {
            if let Some(terminal) = terminals.get(id) {
                if let Some(hosted) = &terminal.hosted {
                    let sig_num = match signal {
                        "SIGINT" | "INT" => 2,
                        "SIGQUIT" | "QUIT" => 3,
                        "SIGTERM" | "TERM" => 15,
                        "SIGKILL" | "KILL" => 9,
                        "SIGHUP" | "HUP" => 1,
                        "SIGWINCH" | "WINCH" => 28,
                        s => s.parse::<std::os::raw::c_int>().unwrap_or(0),
                    };
                    if sig_num > 0 {
                        let _ = hosted.signal(sig_num);
                    }
                }
            }
        }
    }
}

/// How long the owner's name for audit lines is used before it is read again from Git config.
const OWNER_FRESH: Duration = Duration::from_secs(60);

struct OwnerName {
    name: String,
    read_at: Instant,
    refreshing: bool,
}

impl OwnerName {
    fn read(name: String) -> Self {
        Self {
            name,
            read_at: Instant::now(),
            refreshing: false,
        }
    }
}

struct WebHost {
    background_requests: Arc<AtomicUsize>,
    terminal_connections: Arc<AtomicUsize>,
    project: PathBuf,
    project_id: String,
    allowed_origin: Option<String>,
    allowed_email: Option<String>,
    cf_access_iss: Option<String>,
    cf_access_aud: Option<String>,
    sessions: HashMap<String, Instant>,
    jwks_cache: Arc<RwLock<JwksCache>>,
    token: String,
    origin: String,
    terminals: Arc<std::sync::Mutex<HashMap<String, WebTerminal>>>,
    phone_shares: HashMap<String, PhoneShare>,
    checks: Arc<std::sync::Mutex<ChecksCache>>,
    owner: Arc<std::sync::Mutex<OwnerName>>,
    identity: project::ProjectIdentity,
    deployment: Value,
    state_cache: Arc<std::sync::Mutex<Option<StateCache>>>,
    state_generation: Arc<AtomicU64>,
    talks: crate::talk::Talks,
    studio_previews: Arc<std::sync::Mutex<crate::studio::Previews>>,
    /// Who is making the request being handled: the owner, or a named access token.
    access: crate::tokens::Access,
    stream_clients: HashMap<String, StreamClient>,
    browser: Arc<std::sync::Mutex<crate::browser::Browser>>,
    browser_cancel: Arc<AtomicBool>,
    browser_identity: Arc<std::sync::Mutex<Option<String>>>,
}

/// `verb check` runs Git and several `--version` probes, which can take seconds. The web host has one
/// request loop that also pumps every hosted terminal, so checks are computed on their own thread and
/// the page is answered from the latest finished report.
#[derive(Default)]
struct ChecksCache {
    report: Option<(Instant, Value)>,
    running: bool,
}

/// A report younger than this is served as it is; an older one is served while a new one is read.
const CHECKS_FRESH: Duration = Duration::from_secs(20);

impl WebHost {
    fn record_stream_ack(&mut self, id: &str, label: Option<&str>, generation: u64) {
        let Some(id) = stream_client_id(id) else {
            return;
        };
        let now = crate::now_millis();
        let entry = self
            .stream_clients
            .entry(id.to_owned())
            .or_insert_with(|| StreamClient {
                id: id.to_owned(),
                label: String::new(),
                acked_generation: 0,
                acked_at: now,
                seen_at: now,
            });
        entry.label = stream_client_label(label);
        entry.seen_at = now;
        if generation >= entry.acked_generation {
            entry.acked_generation = generation;
            entry.acked_at = now;
        }
        while self.stream_clients.len() > MAX_STREAM_CLIENTS {
            let oldest = self
                .stream_clients
                .values()
                .min_by_key(|client| client.seen_at)
                .map(|client| client.id.clone());
            match oldest {
                Some(oldest) => self.stream_clients.remove(&oldest),
                None => break,
            };
        }
    }

    /// The switch, its generation, and every browser that has acknowledged a generation. A browser
    /// behind the current generation has not confirmed that it cleared.
    fn stream_status(&self, settings: &crate::transcript::Settings) -> Value {
        let mut clients: Vec<&StreamClient> = self.stream_clients.values().collect();
        clients.sort_by_key(|client| std::cmp::Reverse(client.seen_at));
        json!({"enabled": settings.enabled, "generation": settings.generation, "clients": clients})
    }

    fn checks(&self) -> Value {
        let mut cache = self
            .checks
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let fresh = cache
            .report
            .as_ref()
            .is_some_and(|(at, _)| at.elapsed() < CHECKS_FRESH);
        if !fresh && !cache.running {
            cache.running = true;
            let shared = Arc::clone(&self.checks);
            let project = self.project.clone();
            std::thread::spawn(move || {
                let report = crate::checks::assemble(&project)
                    .and_then(|report| {
                        serde_json::from_str::<Value>(&report.to_json())
                            .map_err(|error| error.to_string())
                    })
                    .unwrap_or_else(|error| json!({"error": error}));
                let mut cache = shared.lock().unwrap_or_else(|poison| poison.into_inner());
                cache.report = Some((Instant::now(), report));
                cache.running = false;
            });
        }
        match &cache.report {
            Some((_, report)) => {
                let mut report = report.clone();
                if let Value::Object(map) = &mut report {
                    map.insert("refreshing".to_owned(), Value::Bool(cache.running));
                }
                report
            }
            None => json!({"pending": true}),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LaunchRequest {
    agent: String,
    #[serde(default)]
    isolated: bool,
    command: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    resume_id: Option<String>,
}

#[derive(Deserialize)]
struct TalkMessage {
    text: String,
}

#[derive(Deserialize)]
struct StreamRequest {
    enabled: bool,
    #[serde(default, rename = "clientId")]
    client_id: Option<String>,
    #[serde(default)]
    label: Option<String>,
}

#[derive(Deserialize)]
struct StreamAckRequest {
    #[serde(rename = "clientId")]
    client_id: String,
    #[serde(default)]
    label: Option<String>,
    generation: u64,
}

#[derive(Deserialize)]
struct ChatRequest {
    #[serde(rename = "messageId")]
    message_id: String,
    text: String,
}

/// A browser that has shown this project's conversation, and the newest stream generation it
/// has said it applied. Memory only; a restart forgets every client.
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct StreamClient {
    id: String,
    label: String,
    acked_generation: u64,
    acked_at: u128,
    /// The last time this browser said anything, cleared or not.
    seen_at: u128,
}

const MAX_STREAM_CLIENTS: usize = 16;

fn stream_client_id(id: &str) -> Option<&str> {
    (8..=64).contains(&id.len()).then_some(id).filter(|id| {
        id.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    })
}

fn stream_client_label(label: Option<&str>) -> String {
    let label: String = label
        .unwrap_or("A browser")
        .chars()
        .filter(|c| !c.is_control())
        .take(40)
        .collect();
    if label.trim().is_empty() {
        "A browser".to_owned()
    } else {
        label
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ObserverRequest {
    enabled: Option<bool>,
    mute: Option<String>,
    unmute: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AskRequest {
    question: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BriefRequest {
    #[serde(default)]
    problem: String,
    #[serde(default)]
    users: String,
    #[serde(default)]
    scope: String,
    #[serde(default)]
    constraints: String,
    #[serde(default)]
    glossary: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecCreateRequest {
    title: String,
    #[serde(default)]
    problem: String,
    #[serde(default)]
    users: String,
    #[serde(default)]
    criteria: Vec<String>,
    #[serde(default)]
    out_of_scope: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecStageRequest {
    stage: String,
    #[serde(default)]
    note: String,
    #[serde(default, rename = "expectedRevision")]
    expected_revision: Option<String>,
    #[serde(default, rename = "expectedContext")]
    expected_context: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecCriterionRequest {
    done: bool,
    #[serde(default)]
    evidence: String,
}

const SPEC_AGENTS: [&str; 6] = ["claude", "codex", "gemini", "agy", "opencode", "shell"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecHandoffRequest {
    to: String,
    #[serde(default)]
    note: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecAgentRequest {
    agent: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommitRequest {
    message: String,
    selection: crate::git_commit::Request,
    #[serde(default, rename = "specId")]
    spec_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskRequest {
    title: String,
    #[serde(default)]
    brief: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoteRequest {
    note: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionRequest {
    action: String,
    session_id: String,
    note: Option<String>,
    expected_revision: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputRequest {
    data: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResizeRequest {
    rows: u16,
    cols: u16,
}

/// Read-only work that produces a reply body on a background thread.
type DeferredWork = Box<dyn FnOnce() -> Result<Vec<u8>, String> + Send>;

struct Reply {
    status: u16,
    mime: &'static str,
    sandbox_preview: bool,
    studio_frames: bool,
    body: Vec<u8>,
    set_cookie: Option<String>,
    cache_control: Option<&'static str>,
    state_job: Option<StateJob>,
    /// Slow, read-only work answered off the request loop, which also pumps live terminals.
    deferred: Option<DeferredWork>,
    /// A worker permit reserved before the request had side effects (a terminal launch).
    permit: Option<RequestPermit>,
}

impl Reply {
    fn json(status: u16, value: Value) -> Self {
        Self {
            status,
            sandbox_preview: false,
            studio_frames: false,
            mime: "application/json; charset=utf-8",
            body: value.to_string().into_bytes(),
            set_cookie: None,
            cache_control: None,
            state_job: None,
            deferred: None,
            permit: None,
        }
    }

    fn error(status: u16, message: impl AsRef<str>) -> Self {
        Self::json(status, json!({"error": message.as_ref()}))
    }

    fn asset(mime: &'static str, bytes: &[u8]) -> Self {
        Self {
            status: 200,
            sandbox_preview: false,
            studio_frames: false,
            mime,
            body: bytes.to_vec(),
            set_cookie: None,
            cache_control: None,
            state_job: None,
            deferred: None,
            permit: None,
        }
    }

    fn asset_cached(mime: &'static str, bytes: &[u8]) -> Self {
        Self {
            status: 200,
            sandbox_preview: false,
            studio_frames: false,
            mime,
            body: bytes.to_vec(),
            set_cookie: None,
            cache_control: None,
            state_job: None,
            deferred: None,
            permit: None,
        }
    }
}

pub(super) fn run(project: &Path, args: &[String]) -> Result<(), String> {
    let port = match args {
        [] => 0,
        [flag, value] if flag == "--port" => value
            .parse::<u16>()
            .map_err(|_| "web port must be between 0 and 65535".to_owned())?,
        _ => return Err("usage: verb web [--port PORT]".to_owned()),
    };
    let server = Server::http(("127.0.0.1", port))
        .map_err(|error| format!("could not start local web UI: {error}"))?;
    let address = server
        .server_addr()
        .to_ip()
        .ok_or("web server did not get a TCP address")?;
    let (token, token_is_custom) = match std::env::var("VERB_TOKEN") {
        Ok(val) if !val.trim().is_empty() => (val.trim().to_owned(), true),
        _ => (secure_token()?, false),
    };
    let identity = project::identity(project)?;
    crate::host::mark_started();
    let mut host = WebHost {
        project: project.to_path_buf(),
        project_id: identity.id.clone(),
        allowed_origin: std::env::var("VERB_ALLOWED_ORIGIN")
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty()),
        allowed_email: std::env::var("VERB_ALLOWED_EMAIL")
            .ok()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty()),
        cf_access_iss: std::env::var("VERB_CF_ACCESS_ISS")
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty()),
        cf_access_aud: std::env::var("VERB_CF_ACCESS_AUD")
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty()),
        sessions: HashMap::new(),
        jwks_cache: Arc::new(RwLock::new(JwksCache::default())),
        token: token.clone(),
        origin: format!("http://127.0.0.1:{}", address.port()),
        terminals: Arc::default(),
        phone_shares: HashMap::new(),
        checks: Arc::default(),
        owner: Arc::new(std::sync::Mutex::new(OwnerName::read(
            crate::specs::owner_name(project),
        ))),
        identity,
        deployment: deployment_info(),
        background_requests: Arc::default(),
        terminal_connections: Arc::default(),
        state_cache: Arc::default(),
        state_generation: Arc::new(AtomicU64::new(0)),
        talks: crate::talk::Talks::default(),
        studio_previews: Arc::default(),
        stream_clients: HashMap::new(),
        browser: Arc::default(),
        browser_cancel: Arc::default(),
        browser_identity: Arc::default(),
        access: crate::tokens::Access::owner(),
    };
    if let Some(jwks) = load_initial_jwks() {
        if let Ok(mut w) = host.jwks_cache.write() {
            w.jwks = Some(jwks);
            w.fetched_at = Some(Instant::now());
        }
    }
    let running = Arc::new(AtomicBool::new(true));
    if host.cf_access_iss.is_some() {
        let bg_cache = Arc::clone(&host.jwks_cache);
        let bg_iss = host.cf_access_iss.clone();
        let bg_running = Arc::clone(&running);
        std::thread::spawn(move || {
            background_jwks_refresh(bg_cache, bg_iss, bg_running);
        });
    }
    let signal_flag = Arc::clone(&running);
    ctrlc::set_handler(move || signal_flag.store(false, Ordering::SeqCst))
        .map_err(|error| format!("could not install shutdown handler: {error}"))?;
    // One write, and nothing else on stdout afterwards: whoever launched us may read the URL line
    // and close the pipe (`verb web | head -1`, a test harness, a launcher script). A second
    // `println!` would then panic on EPIPE and take every hosted terminal down with it.
    {
        let mut stdout = std::io::stdout().lock();
        let message = if token_is_custom {
            format!(
                "Verb web UI: {} (configured token in use)\nLocal only. Close this process to stop its hosted agent sessions.\n",
                host.origin
            )
        } else {
            format!(
                "Verb web UI: {}/#{}\nLocal only. Close this process to stop its hosted agent sessions.\n",
                host.origin, token
            )
        };
        stdout
            .write_all(message.as_bytes())
            .and_then(|()| stdout.flush())
            .map_err(|error| format!("could not print web URL: {error}"))?;
    }
    while running.load(Ordering::SeqCst) {
        {
            let mut terminals = host.terminals.lock().unwrap();
            for terminal in terminals.values_mut() {
                if let Err(error) = terminal.poll() {
                    eprintln!("Verb stopped one web terminal: {error}");
                    terminal.fail(error);
                }
            }
        }
        {
            let terminals = host.terminals.lock().unwrap();
            host.phone_shares.retain(|id, _| {
                terminals
                    .get(id)
                    .is_some_and(|terminal| terminal.hosted.is_some())
            });
        }
        if let Some(request) = server
            .recv_timeout(Duration::from_millis(30))
            .map_err(|error| format!("web server stopped: {error}"))?
        {
            let url = request.url().to_owned();
            let path = url.split('?').next().unwrap_or("");
            if path == "/api/terminals/ws" {
                host.handle_ws_upgrade(request);
            } else {
                host.respond(request);
            }
        }
    }
    Ok(())
}

fn secure_token() -> Result<String, String> {
    let mut bytes = [0_u8; 32];
    File::open("/dev/urandom")
        .and_then(|mut random| random.read_exact(&mut bytes))
        .map_err(|error| format!("could not create a secure local web token: {error}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[derive(Default)]
struct JwksCache {
    jwks: Option<jsonwebtoken::jwk::JwkSet>,
    fetched_at: Option<Instant>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CfAccessClaims {
    aud: Value,
    email: Option<String>,
    iss: String,
    exp: u64,
    #[serde(default)]
    nbf: Option<u64>,
}

fn cookie<'a>(request: &'a Request, name: &str) -> Option<&'a str> {
    let header_val = header(request, "Cookie")?;
    for part in header_val.split(';') {
        let trimmed = part.trim();
        if let Some((k, v)) = trimmed.split_once('=') {
            if k.trim() == name {
                return Some(v.trim().trim_matches('"'));
            }
        }
    }
    None
}

impl WebHost {
    fn get_jwk(&self, kid: &str) -> Option<jsonwebtoken::jwk::Jwk> {
        let cache = self.jwks_cache.read().ok()?;
        cache.jwks.as_ref()?.find(kid).cloned()
    }
}

fn load_initial_jwks() -> Option<jsonwebtoken::jwk::JwkSet> {
    if let Ok(raw) = std::env::var("VERB_CF_JWKS_JSON") {
        if let Ok(jwks) = serde_json::from_str::<jsonwebtoken::jwk::JwkSet>(&raw) {
            return Some(jwks);
        }
    }

    let cache_file = std::env::var("VERB_CF_JWKS_FILE")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".verb/cf_jwks.json"))
        })
        .or_else(|| {
            let p = PathBuf::from("/root/.verb/cf_jwks.json");
            if p.exists() {
                Some(p)
            } else {
                None
            }
        });
    if let Some(path) = &cache_file {
        if let Ok(raw) = std::fs::read_to_string(path) {
            if let Ok(jwks) = serde_json::from_str::<jsonwebtoken::jwk::JwkSet>(&raw) {
                return Some(jwks);
            }
        }
    }
    None
}

fn fetch_jwks_network(iss: Option<&str>) -> Option<jsonwebtoken::jwk::JwkSet> {
    let url = std::env::var("VERB_CF_CERTS_URL").ok().unwrap_or_else(|| {
        let iss_clean = iss.unwrap_or("").trim_end_matches('/');
        format!("{iss_clean}/cdn-cgi/access/certs")
    });

    if !url.starts_with("http://") && !url.starts_with("https://") {
        return None;
    }

    let body = ureq::get(&url)
        .timeout(Duration::from_secs(5))
        .call()
        .ok()?
        .into_string()
        .ok()?;

    let jwks = serde_json::from_str::<jsonwebtoken::jwk::JwkSet>(&body).ok()?;
    let cache_file = std::env::var("VERB_CF_JWKS_FILE")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".verb/cf_jwks.json"))
        })
        .or_else(|| {
            let p = PathBuf::from("/root/.verb/cf_jwks.json");
            if p.parent().is_some_and(|d| d.exists()) {
                Some(p)
            } else {
                None
            }
        });
    if let Some(path) = cache_file {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, &body);
    }
    Some(jwks)
}

fn background_jwks_refresh(
    cache: Arc<RwLock<JwksCache>>,
    iss: Option<String>,
    running: Arc<AtomicBool>,
) {
    let needs_initial = cache.read().map(|c| c.jwks.is_none()).unwrap_or(true);
    if needs_initial {
        if let Some(jwks) = fetch_jwks_network(iss.as_deref()) {
            if let Ok(mut w) = cache.write() {
                w.jwks = Some(jwks);
                w.fetched_at = Some(Instant::now());
            }
        }
    }
    while running.load(Ordering::SeqCst) {
        for _ in 0..720 {
            if !running.load(Ordering::SeqCst) {
                return;
            }
            std::thread::sleep(Duration::from_secs(5));
        }
        if let Some(jwks) = fetch_jwks_network(iss.as_deref()) {
            if let Ok(mut w) = cache.write() {
                w.jwks = Some(jwks);
                w.fetched_at = Some(Instant::now());
            }
        }
    }
}

impl WebHost {
    /// The owner's token, compared in equal time.
    fn is_owner_token(&self, presented: Option<&str>) -> bool {
        presented
            .is_some_and(|p| crate::tokens::constant_time_eq(p.as_bytes(), self.token.as_bytes()))
    }

    /// Who the audit trail names for this request: the owner, or the access token by name.
    fn actor(&self) -> String {
        match &self.access.token {
            Some(name) => format!("{name} (access token) via Verb web"),
            None => format!("{} via Verb web", self.owner_name()),
        }
    }

    /// The owner's name for audit lines. Reading Git config can cross the execution boundary, so the
    /// request loop only reads this cache: filled before the host serves, refreshed off the loop.
    fn owner_name(&self) -> String {
        let mut owner = self
            .owner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if owner.read_at.elapsed() >= OWNER_FRESH && !owner.refreshing {
            owner.refreshing = true;
            let (shared, project) = (Arc::clone(&self.owner), self.project.clone());
            std::thread::spawn(move || {
                let name = crate::specs::owner_name(&project);
                *shared.lock().unwrap_or_else(|poison| poison.into_inner()) = OwnerName::read(name);
            });
        }
        owner.name.clone()
    }

    fn authenticate_cf_access(&mut self, request: &Request) -> bool {
        let (Some(expected_email), Some(expected_iss), Some(expected_aud)) = (
            self.allowed_email.clone(),
            self.cf_access_iss.clone(),
            self.cf_access_aud.clone(),
        ) else {
            return false;
        };

        // 1. Verify Cf-Access-Authenticated-User-Email header matches configured allowed identity if present
        if let Some(email_hdr) = header(request, "Cf-Access-Authenticated-User-Email") {
            if email_hdr.trim().to_lowercase() != *expected_email {
                return false;
            }
        }

        // 2. Extract JWT from Cf-Access-Jwt-Assertion header or CF_Authorization cookie
        let jwt_str = header(request, "Cf-Access-Jwt-Assertion")
            .or_else(|| cookie(request, "CF_Authorization"));
        let Some(jwt_str) = jwt_str else {
            return false;
        };

        // 3. Decode header, verify RS256 algorithm, extract kid
        let Ok(hdr) = jsonwebtoken::decode_header(jwt_str) else {
            return false;
        };
        if hdr.alg != jsonwebtoken::Algorithm::RS256 {
            return false;
        }
        let Some(kid) = hdr.kid.as_deref() else {
            return false;
        };

        // 4. Retrieve matching JWK from cache or Cloudflare endpoint (fail closed if not found)
        let Some(jwk) = self.get_jwk(kid) else {
            return false;
        };

        // 5. Construct DecodingKey from the JWK public key components (n, e)
        let Ok(decoding_key) = jsonwebtoken::DecodingKey::from_jwk(&jwk) else {
            return false;
        };

        // 6. Cryptographically verify RS256 signature and standard claims
        let mut validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::RS256);
        validation.set_audience(&[expected_aud]);
        validation.set_issuer(&[expected_iss]);
        validation.validate_exp = true;
        validation.leeway = 60; // 60s clock skew allowance

        let Ok(token_data) =
            jsonwebtoken::decode::<CfAccessClaims>(jwt_str, &decoding_key, &validation)
        else {
            return false;
        };

        // 7. Verify email claim inside cryptographically signed token payload matches expected identity
        let email_claim = token_data
            .claims
            .email
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_lowercase();
        if email_claim != *expected_email {
            return false;
        }

        // 8. Verify nbf (not before) if present
        if let Some(nbf) = token_data.claims.nbf {
            let now = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                Ok(dur) => dur.as_secs(),
                Err(_) => return false,
            };
            if nbf > now + 60 {
                return false;
            }
        }

        true
    }

    fn has_valid_session(&mut self, request: &Request) -> bool {
        self.prune_sessions();
        let Some(token) = cookie(request, "verb_session") else {
            return false;
        };
        self.sessions
            .get(token)
            .is_some_and(|expiry| *expiry > Instant::now())
    }

    fn prune_sessions(&mut self) {
        let now = Instant::now();
        self.sessions.retain(|_, expiry| *expiry > now);
    }

    fn handle_ws_upgrade(&mut self, request: Request) {
        if !request
            .remote_addr()
            .is_some_and(|addr| addr.ip().is_loopback())
        {
            send_reply(request, Reply::error(403, "local browser access only"));
            return;
        }
        let expected_host = self.origin.trim_start_matches("http://");
        let allowed_host = self.allowed_origin.as_deref().map(|origin| {
            origin
                .trim_start_matches("https://")
                .trim_start_matches("http://")
        });
        let host_allowed = header(&request, "Host").is_some_and(|host| {
            host == expected_host || allowed_host.is_some_and(|allowed| host == allowed)
        });
        if !host_allowed {
            send_reply(request, Reply::error(403, "unexpected host"));
            return;
        }
        let origin = header(&request, "Origin");
        let origin_allowed = origin.is_some_and(|origin| {
            origin == self.origin
                || self
                    .allowed_origin
                    .as_deref()
                    .is_some_and(|allowed| origin == allowed)
        });
        if !origin_allowed {
            send_reply(request, Reply::error(403, "unexpected origin"));
            return;
        }

        let url = request.url().to_owned();
        let token_in_query = url.split('?').nth(1).and_then(|query| {
            for pair in query.split('&') {
                if let Some((k, v)) = pair.split_once('=') {
                    if k == "token" {
                        return Some(v);
                    }
                }
            }
            None
        });

        let cf_authenticated = self.authenticate_cf_access(&request);
        let has_valid_session = self.has_valid_session(&request);
        let session_valid = token_in_query.is_some_and(|tok| {
            self.sessions
                .get(tok)
                .is_some_and(|expiry| *expiry > Instant::now())
        });
        // A terminal stream carries keystrokes: it needs drive scope.
        let access = if self.is_owner_token(header(&request, "X-Verb-Token"))
            || self.is_owner_token(token_in_query)
            || session_valid
            || has_valid_session
            || cf_authenticated
        {
            Some(crate::tokens::Access::owner())
        } else {
            header(&request, "X-Verb-Token")
                .or(token_in_query)
                .and_then(crate::tokens::verify)
        };
        let is_authorized = access
            .as_ref()
            .is_some_and(|a| a.allows(crate::tokens::Scope::Drive));

        if !is_authorized {
            send_reply(
                request,
                Reply::error(403, "open the URL printed by verb web"),
            );
            return;
        }

        let stream_access_token = access
            .as_ref()
            .and_then(|access| access.token.as_ref())
            .and_then(|_| {
                header(&request, "X-Verb-Token")
                    .or(token_in_query)
                    .map(str::to_owned)
            });
        let stream_expiry = if session_valid {
            token_in_query.and_then(|token| self.sessions.get(token).copied())
        } else if has_valid_session {
            cookie(&request, "verb_session").and_then(|token| self.sessions.get(token).copied())
        } else {
            None
        };
        let Some(permit) =
            RequestPermit::acquire(&self.terminal_connections, MAX_TERMINAL_CONNECTIONS)
        else {
            send_reply(
                request,
                Reply::error(503, "too many terminal connections; retry shortly"),
            );
            return;
        };
        let Some(key) = header(&request, "Sec-WebSocket-Key") else {
            send_reply(request, Reply::error(400, "missing Sec-WebSocket-Key"));
            return;
        };
        let accept_key = tungstenite::handshake::derive_accept_key(key.as_bytes());

        let mut response = Response::empty(StatusCode(101));
        if let Ok(hdr) = Header::from_bytes("Sec-WebSocket-Accept", accept_key) {
            response.add_header(hdr);
        }

        let tcp_stream = match request.upgrade_tcp("websocket", response) {
            Ok(stream) => stream,
            Err(_) => return,
        };

        static NEXT_CONN_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let conn_id = NEXT_CONN_ID.fetch_add(1, Ordering::SeqCst);
        let handler = Arc::new(WebTerminalsManager {
            terminals: Arc::clone(&self.terminals),
            access_token: stream_access_token,
            expires_at: stream_expiry,
        });

        std::thread::spawn(move || {
            let _permit = permit;
            crate::stream::handle_ws_connection(tcp_stream, handler, conn_id);
        });
    }

    fn respond(&mut self, mut request: Request) {
        // A stalled HTTP client must not keep the controller waiting indefinitely.
        if let Some(stream) = request.tcp_stream() {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
        }
        let mut reply = self.dispatch(&mut request);
        let permit = if let Some(permit) = reply.permit.take() {
            Some(permit)
        } else if reply.deferred.is_some() || reply.state_job.is_some() {
            let Some(permit) =
                RequestPermit::acquire(&self.background_requests, MAX_BACKGROUND_REQUESTS)
            else {
                send_reply(request, Reply::error(503, HOST_BUSY));
                return;
            };
            Some(permit)
        } else {
            None
        };
        if let Some(work) = reply.deferred.take() {
            std::thread::spawn(move || {
                let _permit = permit;
                match work() {
                    Ok(body) => reply.body = body,
                    Err(error) => (reply.status, reply.body) = deferred_error(&error),
                }
                send_reply(request, reply);
            });
            return;
        }
        if let Some(job) = reply.state_job.take() {
            // Only secondary state work leaves the PTY loop. Authentication remains in dispatch.
            std::thread::spawn(move || {
                let _permit = permit;
                match job.run() {
                    Ok(body) => reply.body = body,
                    Err(error) => (reply.status, reply.body) = deferred_error(&error),
                }
                send_reply(request, reply);
            });
        } else {
            send_reply(request, reply);
        }
    }

    fn studio_native(&self) -> bool {
        matches!(crate::isolation::profile(), Ok(None))
            && self.allowed_origin.is_none()
            && self.cf_access_iss.is_none()
            && self.cf_access_aud.is_none()
            && self.deployment["mode"] == "LOCAL"
    }

    fn studio_connected(&self) -> bool {
        self.studio_native() || crate::studio_bridge::configured()
    }

    fn dispatch(&mut self, request: &mut Request) -> Reply {
        if !request
            .remote_addr()
            .is_some_and(|addr| addr.ip().is_loopback())
        {
            return Reply::error(403, "local browser access only");
        }
        let expected_host = self.origin.trim_start_matches("http://");
        let allowed_host = self.allowed_origin.as_deref().map(|origin| {
            origin
                .trim_start_matches("https://")
                .trim_start_matches("http://")
        });
        let host_allowed = header(request, "Host").is_some_and(|host| {
            host == expected_host || allowed_host.is_some_and(|allowed| host == allowed)
        });
        if !host_allowed {
            return Reply::error(403, "unexpected host");
        }
        if let Some(origin) = header(request, "Origin") {
            let origin_allowed = origin == self.origin
                || self
                    .allowed_origin
                    .as_deref()
                    .is_some_and(|allowed| origin == allowed);
            if !origin_allowed {
                return Reply::error(403, "unexpected origin");
            }
        }
        // One-use, short-lived document capabilities. These authorize only this immutable
        // snapshot, never an API or file read. No owner token or session cookie is embedded.
        if request.method() == &Method::Get {
            if let Some(ticket) = request.url().strip_prefix("/studio-preview/") {
                if !self.studio_connected() || header(request, "Sec-Fetch-Dest") != Some("iframe") {
                    return Reply::error(
                        403,
                        "Studio preview requires a connected workspace and iframe navigation",
                    );
                }
                let html = self
                    .studio_previews
                    .lock()
                    .ok()
                    .and_then(|mut p| p.take(ticket));
                return match html {
                    Some(html) => {
                        let mut reply = Reply::asset("text/html; charset=utf-8", &[]);
                        reply.sandbox_preview = true;
                        reply.deferred = Some(Box::new(move || Ok(html)));
                        reply
                    }
                    None => Reply::error(
                        410,
                        "Preview expired or already opened. Create a new preview.",
                    ),
                };
            }
        }
        let cf_authenticated = self.authenticate_cf_access(request);
        let has_valid_session = self.has_valid_session(request);
        let mut session_cookie = None;

        if cf_authenticated && !has_valid_session {
            if let Ok(new_token) = secure_token() {
                self.sessions.insert(
                    new_token.clone(),
                    Instant::now() + Duration::from_secs(86400),
                );
                session_cookie = Some(format!(
                    "verb_session={}; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=86400",
                    new_token
                ));
            }
        }

        let url = request.url().to_owned();
        let path = url.split('?').next().unwrap_or("");
        if request.method() == &Method::Get && path == "/healthz" {
            // Minimal HTTP readiness after host/origin validation. No project/session or token data.
            return Reply::json(
                200,
                serde_json::json!({"status":"serving","version":env!("CARGO_PKG_VERSION"),"sourceRevision":env!("VERB_BUILD_REVISION"),"sourceDirty":match env!("VERB_BUILD_DIRTY") {"true"=>Some(true),"false"=>Some(false),_=>None}}),
            );
        }
        if request.method() == &Method::Get {
            let asset_reply = match path {
                "/" => Some(Reply::asset(
                    "text/html; charset=utf-8",
                    include_bytes!("../web/index.html"),
                )),
                "/app.js" => Some(Reply::asset_cached(
                    "text/javascript; charset=utf-8",
                    include_bytes!("../web/dist/app.js"),
                )),
                "/app.css" => Some(Reply::asset_cached(
                    "text/css; charset=utf-8",
                    include_bytes!("../web/dist/app.css"),
                )),
                "/favicon.svg" => Some(Reply::asset_cached(
                    "image/svg+xml",
                    include_bytes!("../web/favicon.svg"),
                )),
                _ => None,
            };
            if let Some(mut reply) = asset_reply {
                reply.studio_frames = path == "/" && self.studio_connected();
                if let Some(cookie) = session_cookie {
                    reply.set_cookie = Some(cookie);
                }
                return reply;
            }
        }
        if !path.starts_with("/api/") {
            return Reply::error(404, "not found");
        }
        let presented = header(request, "X-Verb-Token");
        let access = if self.is_owner_token(presented) || has_valid_session || cf_authenticated {
            crate::tokens::Access::owner()
        } else if let Some(access) = presented.and_then(crate::tokens::verify) {
            access
        } else {
            return Reply::error(403, "open the URL printed by verb web");
        };
        let needed = crate::tokens::required_scope(request.method().as_str(), path);
        if !access.allows(needed) {
            return Reply::error(
                403,
                format!(
                    "the access token `{}` has {} scope; this needs {}",
                    access.token.as_deref().unwrap_or("?"),
                    access.scope.name(),
                    needed.name()
                ),
            );
        }
        self.access = access;
        let method = request.method().clone();
        let mut reply = match self.api(&method, &url, path, request) {
            Ok(reply) => reply,
            Err(error) => Reply::error(400, error),
        };
        if let Some(cookie) = session_cookie {
            reply.set_cookie = Some(cookie);
        }
        reply
    }

    fn api(
        &mut self,
        method: &Method,
        url: &str,
        path: &str,
        request: &mut Request,
    ) -> Result<Reply, String> {
        if path == "/api/browser" {
            // Initial browser runtime is owner-only, native-local, and explicitly configured.
            if !self.access.allows(crate::tokens::Scope::Owner) || !self.studio_native() {
                return Ok(Reply::error(
                    403,
                    "Browser runtime needs the owner on a native-local host.",
                ));
            }
            let configured = crate::browser::Browser::configured();
            if method == &Method::Get {
                return Ok(Reply::json(
                    200,
                    json!({"available":configured,"location":"This Verb host", "reason": if configured { "Start a fresh, isolated browser session." } else { "A browser runtime has not been configured on this host." }}),
                ));
            }
            if method != &Method::Post {
                return Ok(Reply::error(405, "Use GET or POST."));
            }
            let input: Value = read_json(request)?;
            let stopping = input["op"] == "stop";
            if stopping {
                let identity = self
                    .browser_identity
                    .lock()
                    .map_err(|_| "Browser identity unavailable.")?;
                if identity.is_some() && input["session"].as_str() != identity.as_deref() {
                    return Ok(Reply::error(
                        409,
                        "Browser session changed. Reopen it before stopping it.",
                    ));
                }
                self.browser_cancel.store(true, Ordering::SeqCst);
            }
            let cancel = self.browser_cancel.clone();
            let identity = self.browser_identity.clone();
            let browser = self.browser.clone();
            let origin = self.origin.clone();
            let mut reply = Reply::json(200, Value::Null);
            reply.deferred = Some(Box::new(move || {
                let mut browser = if stopping {
                    browser.lock().map_err(|_| "Browser lock is unavailable.")?
                } else {
                    browser.try_lock().map_err(|_| {
                        "Browser is busy. Wait for the current action; it was not queued."
                    })?
                };
                let view = browser.command(&origin, input, &cancel, &identity)?;
                serde_json::to_vec(&view).map_err(|e| e.to_string())
            }));
            return Ok(reply);
        }
        if method == &Method::Get && path == "/api/studio/workspace" {
            let native = self.studio_native();
            let boundary = crate::studio_bridge::configured();
            let drive = self.access.allows(crate::tokens::Scope::Drive);
            let project = self.project.clone();
            let id = self.project_id.clone();
            return Ok(self.off_loop(false, move || {
                let probe = if boundary && drive { crate::studio_bridge::probe(&project) } else { Ok(()) };
                let available = (native || boundary) && drive && probe.is_ok();
                let reason = if available {
                    "UTF-8 edits up to 200 KB and saved single-file HTML previews. Saves do not commit or publish.".to_owned()
                } else if let Err(error) = probe { error } else {
                    "Editing and app previews require drive access and a connected workspace bridge.".to_owned()
                };
                Ok(json!({"schemaVersion":1,"workspaceId":id,
                    "editing":{"available":available,"maxBytes":crate::studio::MAX_BYTES},
                    "preview":{"available":available,"kind":"single-file-html","expiresSeconds":120},
                    "transport":if boundary {"monitored-v1"} else if native {"native-local"} else {"unavailable"},
                    "reason":reason}))
            }));
        }

        if matches!(
            path,
            "/api/studio/file" | "/api/studio/preview" | "/api/studio/image"
        ) {
            if !self.studio_connected() {
                return Ok(Reply::error(403, "Studio editing and app preview are not connected through this execution boundary"));
            }
            let project = self.project.clone();
            let id = self.project_id.clone();
            let boundary = crate::studio_bridge::configured();
            if method == &Method::Get && path == "/api/studio/image" {
                let wanted = url
                    .split('?')
                    .nth(1)
                    .and_then(|q| q.split('&').find_map(|p| p.strip_prefix("path=")))
                    .map(crate::shell::percent_decode)
                    .ok_or("which image?")?;
                return Ok(self.off_loop(false, move || {
                    let image = if boundary {
                        crate::studio_bridge::image(&project, &wanted)?
                    } else {
                        crate::studio::image(&project, &wanted)?
                    };
                    Ok(json!({"workspaceId":id,"image":image}))
                }));
            }
            if method == &Method::Get && path == "/api/studio/file" {
                let wanted = url
                    .split('?')
                    .nth(1)
                    .and_then(|q| q.split('&').find_map(|p| p.strip_prefix("path=")))
                    .map(crate::shell::percent_decode)
                    .ok_or("which file?")?;
                return Ok(self.off_loop(false, move || {
                    let file = if boundary {
                        crate::studio_bridge::read(&project, &wanted)?
                    } else {
                        crate::studio::read(&project, &wanted)?
                    };
                    Ok(json!({"workspaceId":id,"file":file}))
                }));
            }
            if method == &Method::Post && path == "/api/studio/file" {
                let input: crate::studio::SaveRequest =
                    read_json_limit(request, crate::studio::MAX_BYTES * 6 + 1024)?;
                if input.workspace_id != id {
                    return Ok(Reply::error(
                        409,
                        "Workspace changed; reopen the file before saving",
                    ));
                }
                return Ok(self.off_loop(true, move || {
                    let file = if boundary {
                        crate::studio_bridge::save(&project, input)?
                    } else {
                        crate::studio::save(&project, input)?
                    };
                    Ok(json!({"workspaceId":id,"file":file}))
                }));
            }
            if method == &Method::Post && path == "/api/studio/preview" {
                let input: crate::studio::FileRequest = read_json(request)?;
                if input.workspace_id != id {
                    return Ok(Reply::error(
                        409,
                        "Workspace changed; reopen the file before previewing",
                    ));
                }
                let previews = Arc::clone(&self.studio_previews);
                let ticket = secure_token()?;
                return Ok(self.off_loop(false, move || {
                    let doc = if boundary { crate::studio_bridge::read(&project, &input.path)? } else { crate::studio::read(&project, &input.path)? };
                    crate::studio::expect_revision(&doc.revision, &input.expected_revision)?;
                    let revision = doc.revision.clone();
                    previews.lock().map_err(|_| "Preview registry unavailable")?.insert(ticket.clone(), doc)?;
                    Ok(json!({"url":format!("/studio-preview/{ticket}"),"workspaceId":id,"revision":revision,"expiresSeconds":120}))
                }));
            }
            return Ok(Reply::error(405, "unsupported Studio method"));
        }
        if method == &Method::Get && path == "/api/agents" {
            return Ok(self.off_loop(false, || {
                let programs = crate::isolation::external_programs();
                let agents: serde_json::Map<String, Value> = crate::installed_agents()
                    .into_iter()
                    .map(|(name, installed)| (name.to_owned(), json!(installed)))
                    .collect();
                Ok(json!({ "installed": agents, "restricted": programs.is_some(), "programs": programs,
                    "isolatedCheckouts": project::isolated_checkouts_supported() }))
            }));
        }
        if method == &Method::Get && path == "/api/whoami" {
            return Ok(Reply::json(200, json!(self.access)));
        }
        if method == &Method::Get && path == "/api/state" {
            if let Ok(cache) = self.state_cache.try_lock() {
                if let Some(cache) = cache.as_ref().filter(|cache| {
                    cache.generation == self.state_generation.load(Ordering::SeqCst)
                        && cache.cached_at.elapsed() < Duration::from_millis(1500)
                }) {
                    return Ok(Reply::asset("application/json; charset=utf-8", &cache.body));
                }
            }
            let mut reply = Reply::json(200, Value::Null);
            reply.state_job = Some(StateJob {
                project: self.project.clone(),
                identity: self.identity.clone(),
                deployment: self.deployment.clone(),
                hosted: self
                    .terminals
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|(id, terminal)| (id.clone(), terminal.hosted.is_some()))
                    .collect(),
                cache: Arc::clone(&self.state_cache),
                generation: self.state_generation.load(Ordering::SeqCst),
                current_generation: Arc::clone(&self.state_generation),
            });
            return Ok(reply);
        }
        if method == &Method::Get && path == "/api/workspace" {
            return Ok(Reply::json(200, self.workspace()));
        }
        // Not folded into /api/state: it runs runtime `--version` probes, so the page asks for it on
        // load and on demand rather than every few seconds.
        if method == &Method::Get && path == "/api/checks" {
            return Ok(Reply::json(200, self.checks()));
        }
        if method == &Method::Post && path == "/api/good/mark" {
            let (project, checks) = (self.project.clone(), Arc::clone(&self.checks));
            return Ok(self.off_loop(true, move || {
                let mark = crate::good::mark(&project)?;
                checks
                    .lock()
                    .unwrap_or_else(|poison| poison.into_inner())
                    .report = None;
                Ok(json!({
                    "message": format!(
                        "Marked last-known-good at {}.",
                        mark.short_head().unwrap_or("this state")
                    )
                }))
            }));
        }
        if method == &Method::Post && path == "/api/tasks" {
            let input: TaskRequest = read_json(request)?;
            let id = workbench::create_ui_task(&self.project, &input.title, &input.brief)?;
            self.invalidate_state();
            return Ok(Reply::json(
                201,
                json!({
                    "id": id, "message": format!("Created task: {}", input.title.trim())
                }),
            ));
        }
        if method == &Method::Post && path == "/api/memory" {
            let input: NoteRequest = read_json(request)?;
            let message = workbench::append_ui_memory(&self.project, &input.note)?;
            self.invalidate_state();
            return Ok(Reply::json(200, json!({"message": message})));
        }
        if method == &Method::Post && path == "/api/terminals" {
            let input: LaunchRequest = read_json(request)?;
            let launcher = self.launcher();
            return Ok(self.launch_reply(move || {
                let session = launcher.launch(input)?;
                Ok((session.clone(), json!({"sessionId": session}), None))
            }));
        }
        if method == &Method::Post && path == "/api/ask" {
            let input: AskRequest = read_json(request)?;
            if input.question.trim().is_empty() || input.question.len() > 500 {
                return Err("ask a question of up to 500 characters".to_owned());
            }
            let live: Vec<crate::ask::LiveSession> = self.workspace()["sessions"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|s| {
                    Some(crate::ask::LiveSession {
                        id: s["id"].as_str()?.to_owned(),
                        agent: s["agent"].as_str().unwrap_or("Session").to_owned(),
                    })
                })
                .collect();
            let project = self.project.clone();
            let mut reply = Reply::json(200, Value::Null);
            reply.deferred = Some(Box::new(move || {
                serde_json::to_vec(&crate::ask::answer(&project, &input.question, &live))
                    .map_err(|e| e.to_string())
            }));
            return Ok(reply);
        }
        if path == "/api/stream" && (method == &Method::Get || method == &Method::Post) {
            let store = self.identity.store.clone();
            let mut settings = crate::transcript::load(&store);
            if method == &Method::Post {
                let input: StreamRequest = read_json(request)?;
                settings = crate::transcript::set_enabled(&store, input.enabled)?;
                // Every hosted terminal clears or rereads now, not at its next settings check.
                for terminal in self.terminals.lock().unwrap().values_mut() {
                    if let Some(hosted) = terminal.hosted.as_mut() {
                        hosted.apply_stream(settings.clone());
                    }
                }
                // The browser that turned it off cleared itself before asking.
                if let Some(id) = input.client_id.as_deref() {
                    self.record_stream_ack(id, input.label.as_deref(), settings.generation);
                }
            }
            return Ok(Reply::json(200, self.stream_status(&settings)));
        }
        if path == "/api/stream/ack" && method == &Method::Post {
            let input: StreamAckRequest = read_json(request)?;
            let settings = crate::transcript::load(&self.identity.store);
            if input.generation > settings.generation {
                return Err("that stream generation does not exist yet".to_owned());
            }
            self.record_stream_ack(&input.client_id, input.label.as_deref(), input.generation);
            return Ok(Reply::json(200, self.stream_status(&settings)));
        }
        if path == "/api/observer" && (method == &Method::Get || method == &Method::Post) {
            let store = self.identity.store.clone();
            let mut settings = crate::observer::load(&store);
            if method == &Method::Post {
                let input: ObserverRequest = read_json(request)?;
                if let Some(enabled) = input.enabled {
                    settings.enabled = enabled;
                }
                if let Some(kind) = input.mute {
                    if !crate::observer::KINDS.contains(&kind.as_str()) {
                        return Err("unknown signal".to_owned());
                    }
                    if !settings.muted.contains(&kind) {
                        settings.muted.push(kind);
                    }
                }
                if let Some(kind) = input.unmute {
                    settings.muted.retain(|k| *k != kind);
                }
                crate::observer::save(&store, &settings)?;
            }
            // Facts are gathered for every terminal, but only one thing is reported without the
            // observer: which agents are explicitly asking permission ("Needs you" on their tab).
            // That is the agent addressing the person, not Verb watching; every other signal still
            // needs the observer turned on.
            let owned: Vec<_> = {
                let terminals = self.terminals.lock().unwrap();
                terminals
                    .iter()
                    .filter_map(|(id, terminal)| {
                        let hosted = terminal.hosted.as_ref()?;
                        let session = &hosted.session;
                        let is_agent = !matches!(
                            session.agent.as_ref(),
                            None | Some(Agent::Shell) | Some(Agent::Custom(_))
                        );
                        if !settings.enabled && !is_agent {
                            return None;
                        }
                        let failures: Vec<(String, u64)> = terminal
                            .failures
                            .iter()
                            .map(|(label, at)| (label.clone(), at.elapsed().as_secs()))
                            .collect();
                        Some((
                            id.clone(),
                            session.display_agent().to_owned(),
                            is_agent,
                            terminal.last_output.elapsed().as_secs(),
                            hosted.screen().contents(),
                            failures,
                        ))
                    })
                    .collect()
            };
            let facts: Vec<crate::observer::TerminalFacts> = owned
                .iter()
                .map(|(id, agent, is_agent, idle, screen, failures)| {
                    crate::observer::TerminalFacts {
                        id,
                        agent,
                        is_agent: *is_agent,
                        idle_secs: *idle,
                        screen,
                        failures,
                    }
                })
                .collect();
            let asking = crate::observer::asking(&facts);
            let signals = if settings.enabled {
                crate::observer::evaluate(&facts, &settings.muted)
            } else {
                Vec::new()
            };
            return Ok(Reply::json(
                200,
                json!({"enabled": settings.enabled, "muted": settings.muted, "kinds": crate::observer::KINDS, "signals": signals, "asking": asking}),
            ));
        }
        if method == &Method::Get && path == "/api/meters" {
            // For each running Claude or Codex session: what to read, gathered under the lock and
            // read on a background thread, since it touches the agents' log files.
            let wanted: Vec<_> = self
                .terminals
                .lock()
                .unwrap()
                .iter()
                .filter_map(|(id, terminal)| {
                    let session = &terminal.hosted.as_ref()?.session;
                    let record = match session.agent.as_ref()? {
                        crate::Agent::Claude => crate::observe::Record::Claude,
                        crate::Agent::Codex => crate::observe::Record::Codex,
                        _ => return None,
                    };
                    Some((
                        id.clone(),
                        record,
                        session.created_at,
                        session.resume_identity.clone(),
                    ))
                })
                .collect();
            let project = self.project.clone();
            let mut reply = Reply::json(200, Value::Null);
            reply.deferred = Some(Box::new(move || {
                let home = crate::isolation::agent_home().unwrap_or_default();
                let meters: serde_json::Map<String, Value> = wanted
                    .into_iter()
                    .map(|(id, record, created, conversation)| {
                        let meter = crate::meter::for_session(
                            record,
                            &home,
                            &project,
                            created,
                            conversation.as_deref(),
                        );
                        (id, json!(meter))
                    })
                    .collect();
                serde_json::to_vec(&json!({ "meters": meters })).map_err(|e| e.to_string())
            }));
            return Ok(reply);
        }
        if method == &Method::Get && path == "/api/host" {
            let mut reply = Reply::json(200, Value::Null);
            reply.deferred = Some(Box::new(|| {
                serde_json::to_vec(&crate::host::report()).map_err(|e| e.to_string())
            }));
            return Ok(reply);
        }
        if method == &Method::Get && path == "/api/hub" {
            return Ok(Reply::json(200, json!(crate::hub::hub(&self.project))));
        }
        if method == &Method::Get && path == "/api/files" {
            let project = self.project.clone();
            return Ok(self.off_loop(false, move || Ok(json!(crate::hub::files(&project)?))));
        }
        if method == &Method::Get && path == "/api/files/preview" {
            let wanted = url
                .split('?')
                .nth(1)
                .and_then(|query| query.split('&').find_map(|pair| pair.strip_prefix("path=")))
                .map(crate::shell::percent_decode)
                .ok_or("which file?")?;
            let project = self.project.clone();
            return Ok(self.off_loop(false, move || {
                Ok(json!(crate::hub::preview(&project, &wanted)?))
            }));
        }
        if method == &Method::Post && path == "/api/hub/brief" {
            let input: BriefRequest = read_json(request)?;
            crate::hub::create_brief(
                &self.project,
                crate::hub::NewBrief {
                    problem: input.problem,
                    users: input.users,
                    scope: input.scope,
                    constraints: input.constraints,
                    glossary: input.glossary,
                },
                &self.actor(),
            )?;
            self.invalidate_state();
            return Ok(Reply::json(
                201,
                json!({"path": crate::hub::BRIEF_PATH, "message": format!("Created {}", crate::hub::BRIEF_PATH)}),
            ));
        }
        if method == &Method::Post && path == "/api/hub/sync" {
            let changed = crate::hub::sync_agent_files(&self.project, &self.actor())?;
            self.invalidate_state();
            return Ok(Reply::json(
                200,
                json!({
                    "changed": changed,
                    "message": if changed.is_empty() {
                        "Agent context was already up to date.".to_owned()
                    } else {
                        format!("Updated {}", changed.join(" and "))
                    }
                }),
            ));
        }
        if method == &Method::Get && path == "/api/specs" {
            let project = self.project.clone();
            let project_id = self.identity.id.clone();
            let cache = Arc::clone(&self.state_cache);
            return Ok(self.off_loop(false, move || {
                let workspace = cache.try_lock().ok().and_then(|cache| {
                    cache
                        .as_ref()
                        .filter(|c| c.cached_at.elapsed() < Duration::from_secs(30))
                        .and_then(|c| serde_json::from_slice::<Value>(&c.body).ok())
                });
                let specs = crate::specs::list_checked(&project)?;
                let mut summaries =
                    crate::spec_summary::resolve_all(&specs, workspace.as_ref(), &project);
                crate::sdlc::enrich_all(&project, &project_id, &specs, &mut summaries);
                Ok(json!({"specs": specs, "summaries": summaries, "stages": crate::specs::STAGES}))
            }));
        }
        if method == &Method::Post && path == "/api/specs" {
            let input: SpecCreateRequest = read_json(request)?;
            if input.criteria.len() > 50 {
                return Err("keep a spec to 50 acceptance criteria or fewer".to_owned());
            }
            let spec = crate::specs::create(
                &self.project,
                crate::specs::NewSpec {
                    title: input.title,
                    problem: input.problem,
                    users: input.users,
                    criteria: input.criteria,
                    out_of_scope: input.out_of_scope,
                },
                &self.actor(),
            )?;
            self.invalidate_state();
            return Ok(Reply::json(201, json!(spec)));
        }
        if method == &Method::Get && path == "/api/git" {
            // Git may cross an execution boundary or scan a large checkout.
            // Keep it off the PTY/request loop, including while opening a terminal.
            let project = self.project.clone();
            let mut reply = Reply::json(200, Value::Null);
            reply.deferred = Some(Box::new(move || {
                serde_json::to_vec(&crate::specs::git_summary(&project)?)
                    .map_err(|error| error.to_string())
            }));
            return Ok(reply);
        }
        if method == &Method::Get && path == "/api/git/diff" {
            let wanted = url
                .split('?')
                .nth(1)
                .and_then(|query| query.split('&').find_map(|pair| pair.strip_prefix("path=")))
                .map(crate::shell::percent_decode)
                .ok_or("which file?")?;
            let project = self.project.clone();
            let mut reply = Reply::json(200, Value::Null);
            reply.deferred = Some(Box::new(move || {
                serde_json::to_vec(&crate::diff::file_diff(&project, &wanted)?)
                    .map_err(|error| error.to_string())
            }));
            return Ok(reply);
        }
        if method == &Method::Get && path == "/api/git/commit-review" {
            let project = self.project.clone();
            return Ok(self.off_loop(false, move || {
                Ok(json!(crate::git_commit::review(&project)?))
            }));
        }
        if method == &Method::Post && path == "/api/git/commit-recovery" {
            let input: crate::git_commit::RecoveryRequest = read_json(request)?;
            let project = self.project.clone();
            return Ok(self.off_loop(true, move || {
                Ok(json!({ "message": crate::git_commit::repair(&project, &input)? }))
            }));
        }
        if method == &Method::Post && path == "/api/git/commit" {
            let input: CommitRequest = read_json(request)?;
            let (project, actor) = (self.project.clone(), self.actor());
            return Ok(self.off_loop(true, move || {
                let sha = crate::specs::commit_selected(
                    &project,
                    &input.message,
                    input.spec_id.as_deref(),
                    &actor,
                    &input.selection,
                )?;
                Ok(json!({"sha": sha, "message": format!("Committed {sha}")}))
            }));
        }
        if method == &Method::Get && path == "/api/git/transfer" {
            let project = self.project.clone();
            return Ok(self.off_loop(false, move || Ok(json!(crate::git_sync::review(&project)?))));
        }
        if method == &Method::Post && path == "/api/git/transfer" {
            let input: crate::git_sync::Request = read_json(request)?;
            let project = self.project.clone();
            return Ok(self.off_loop(true, move || {
                Ok(json!(crate::git_sync::transfer(&project, input)?))
            }));
        }
        let parts: Vec<_> = path.trim_start_matches('/').split('/').collect();
        match parts.as_slice() {
            ["api", "specs", id, "sdlc-diff"] if method == &Method::Get => {
                let (project, id) = (self.project.clone(), id.to_string());
                Ok(self.off_loop(false, move || {
                    crate::specs::find(&project, &id)?;
                    crate::sdlc::implementation_diff(&project)
                }))
            }
            ["api", "specs", id, "sdlc-export"] if method == &Method::Get => {
                let (project, project_id, id) = (
                    self.project.clone(),
                    self.identity.id.clone(),
                    id.to_string(),
                );
                Ok(self.off_loop(false, move || {
                    crate::sdlc::export(&project_id, &crate::specs::find(&project, &id)?.1)
                }))
            }
            ["api", "specs", id, "sdlc"] if method == &Method::Get => {
                let (project, project_id, id) = (
                    self.project.clone(),
                    self.identity.id.clone(),
                    id.to_string(),
                );
                Ok(self.off_loop(false, move || {
                    crate::sdlc::view(&project, &project_id, &crate::specs::find(&project, &id)?.1)
                }))
            }
            ["api", "specs", id, "sdlc"] if method == &Method::Post => {
                let input: crate::sdlc::Request = read_json(request)?;
                let (project, project_id, id, actor) = (
                    self.project.clone(),
                    self.identity.id.clone(),
                    id.to_string(),
                    self.actor(),
                );
                Ok(self.off_loop(true, move || {
                    crate::sdlc::act(&project, &project_id, &id, input, &actor)
                }))
            }
            ["api", "specs", id] if method == &Method::Get => Ok(Reply::json(
                200,
                json!(crate::specs::find(&self.project, id)?.1),
            )),
            ["api", "specs", id, "stage-check"] if method == &Method::Get => {
                let (_, spec) = crate::specs::find(&self.project, id)?;
                let to = url
                    .split('?')
                    .nth(1)
                    .and_then(|q| q.split('&').find_map(|pair| pair.strip_prefix("to=")))
                    .unwrap_or(&spec.stage);
                let target = crate::specs::STAGES
                    .iter()
                    .position(|s| *s == to)
                    .ok_or("unknown stage")?;
                let project = self.project.clone();
                let project_id = self.identity.id.clone();
                Ok(self.off_loop(false, move || {
                    let ship = crate::specs::ship_state(&project, &spec);
                    let flow = crate::sdlc::view(&project, &project_id, &spec)?;
                    let mut gate_spec = spec.clone();
                    gate_spec.criteria_done = flow["accepted"].as_u64().unwrap_or(0) as usize;
                    let mut warnings = crate::specs::stage_warnings(&project, &gate_spec, target);
                    warnings.extend(crate::sdlc::warnings(&flow, crate::specs::STAGES[target]));
                    Ok(json!({
                        "warnings": warnings, "context": flow["context"],
                        "ship": ship,
                    }))
                }))
            }
            ["api", "specs", id, "stage"] if method == &Method::Post => {
                let input: SpecStageRequest = read_json(request)?;
                let (project, actor, id) = (self.project.clone(), self.actor(), id.to_string());
                let project_id = self.identity.id.clone();
                Ok(self.off_loop(true, move || {
                    let current = crate::specs::find(&project, &id)?.1;
                    if input.expected_revision.as_deref().is_some_and(|r| r != current.revision) { return Err("This spec changed while you were reviewing it. Refresh the spec and review the move again.".into()); }
                    let flow = crate::sdlc::view(&project, &project_id, &current)?;
                    if input.expected_context.as_deref().is_some_and(|context| flow["context"].as_str() != Some(context)) {
                        return Err("The source or evidence changed while this move was open. Refresh and review it again.".into());
                    }
                    let extra = crate::sdlc::warnings(&flow, &input.stage);
                    let note = if extra.is_empty() { input.note.clone() } else { format!("{}; SDLC readiness warnings: {}", input.note, extra.join("; ")) };
                    let (spec, mut warnings) = crate::specs::set_stage(
                        &project,
                        &id,
                        &input.stage,
                        &note,
                        &actor,
                        input.expected_revision.as_deref(),
                        flow["accepted"].as_u64().map(|n| n as usize),
                    )?;
                    warnings.extend(extra);
                    Ok(json!({"spec": spec, "warnings": warnings}))
                }))
            }
            ["api", "specs", id, "criteria", index] if method == &Method::Post => {
                let input: SpecCriterionRequest = read_json(request)?;
                let index = index
                    .parse::<usize>()
                    .map_err(|_| "invalid criterion".to_owned())?;
                let spec = crate::specs::set_criterion(
                    &self.project,
                    id,
                    index,
                    input.done,
                    &input.evidence,
                    &self.actor(),
                )?;
                self.invalidate_state();
                Ok(Reply::json(200, json!(spec)))
            }
            ["api", "specs", id, "branch"] if method == &Method::Post => {
                let (project, actor, id) = (self.project.clone(), self.actor(), id.to_string());
                Ok(self.off_loop(true, move || {
                    let branch = crate::specs::switch_to_branch(&project, &id, &actor)?;
                    Ok(json!({"branch": branch, "message": format!("Now on {branch}")}))
                }))
            }
            ["api", "specs", id, "talk"] if method == &Method::Post => {
                // A talk with Antigravity on this spec, through agy's JSON mode (crate::talk).
                let (path, spec) = crate::specs::find(&self.project, id)?;
                let talk = format!(
                    "talk-{:x}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis())
                        .unwrap_or(0)
                );
                self.talks.create(talk.clone(), Some(spec.id.clone()));
                crate::specs::record(
                    &path,
                    &self.actor(),
                    &format!("started a talk with agy on this spec ({talk})"),
                )?;
                let opening = format!(
                    "{} We are planning first: read the spec and the code, then say how you will meet \
                     each acceptance criterion.",
                    crate::specs::agent_brief(&spec)
                );
                self.talks.send(
                    &talk,
                    &self.project,
                    &opening,
                    &crate::talk::program().to_string_lossy(),
                )?;
                self.invalidate_state();
                Ok(Reply::json(201, json!({"talkId": talk})))
            }
            ["api", "talk"] if method == &Method::Get => {
                Ok(Reply::json(200, json!({"talks": self.talks.list()})))
            }
            ["api", "talk", id] if method == &Method::Get => {
                let view = self.talks.view(id).ok_or("no such talk")?;
                Ok(Reply::json(200, json!(view)))
            }
            ["api", "talk", id] if method == &Method::Delete => {
                self.talks.remove(id);
                Ok(Reply::json(200, json!({"ok": true})))
            }
            ["api", "talk", id, "message"] if method == &Method::Post => {
                let input: TalkMessage = read_json(request)?;
                self.talks.send(
                    id,
                    &self.project,
                    &input.text,
                    &crate::talk::program().to_string_lossy(),
                )?;
                Ok(Reply::json(202, json!({"ok": true})))
            }
            ["api", "talk", id, "stop"] if method == &Method::Post => {
                self.talks.stop(id);
                Ok(Reply::json(200, json!({"ok": true})))
            }
            ["api", "talk", id, "terminal"] if method == &Method::Post => {
                // Continue the same conversation in agy's terminal, where it can ask permission.
                let view = self.talks.view(id).ok_or("no such talk")?;
                let conversation = view.conversation_id.ok_or(
                    "Antigravity has not started this conversation yet; wait for its first reply",
                )?;
                let spec = view.spec_id.ok_or("this talk is not on a spec")?;
                let launcher = self.launcher();
                Ok(self.launch_reply(move || {
                    let (session, record) =
                        launcher.start_on_spec_with(&spec, "agy", Some(&conversation))?;
                    Ok((session.clone(), json!({"sessionId": session}), Some(record)))
                }))
            }
            ["api", "specs", id, "agent"] if method == &Method::Post => {
                let input: SpecAgentRequest = read_json(request)?;
                let (launcher, id) = (self.launcher(), id.to_string());
                Ok(self.launch_reply(move || {
                    let (session, record) = launcher.start_on_spec(&id, &input.agent)?;
                    Ok((session.clone(), json!({"sessionId": session}), Some(record)))
                }))
            }
            ["api", "specs", id, "handoff"] if method == &Method::Post => {
                let input: SpecHandoffRequest = read_json(request)?;
                if input.note.len() > 2000 {
                    return Err("keep the handoff note under 2000 characters".to_owned());
                }
                if !SPEC_AGENTS.contains(&input.to.as_str()) {
                    return Err("choose a supported agent".to_owned());
                }
                let (launcher, id) = (self.launcher(), id.to_string());
                // The handoff note records intent when the request is accepted; the agent start is
                // recorded separately, and only once its terminal is confirmed started.
                Ok(self.launch_reply(move || {
                    let note = crate::specs::handoff(
                        &launcher.project,
                        &id,
                        &input.to,
                        &input.note,
                        &launcher.actor,
                    )?;
                    let (session, record) = launcher.start_on_spec(&id, &input.to)?;
                    let body = json!({"sessionId": session, "note": note});
                    Ok((session, body, Some(record)))
                }))
            }
            ["api", "tasks", id, "handoff-revision"] if method == &Method::Get => {
                let revision = workbench::handoff_revision(&self.project, id)?;
                Ok(Reply::json(200, json!({"revision": revision})))
            }
            ["api", "tasks", id, "actions"] if method == &Method::Post => {
                let input: ActionRequest = read_json(request)?;
                let action = match input.action.as_str() {
                    "claim" => workbench::TaskAction::Claim,
                    "reassign" => workbench::TaskAction::Reassign,
                    "help" => workbench::TaskAction::RequestHelp,
                    "reply" => workbench::TaskAction::Reply,
                    "handoff" => workbench::TaskAction::Handoff,
                    "done" => workbench::TaskAction::Done,
                    _ => return Err("unknown task action".to_owned()),
                };
                let message = workbench::apply_ui_action(
                    &self.project,
                    id,
                    &input.session_id,
                    action,
                    input.note,
                    input.expected_revision.as_deref(),
                )?;
                self.invalidate_state();
                Ok(Reply::json(200, json!({"message": message})))
            }
            ["api", "inbox", id] if method == &Method::Get => {
                let inbox = workbench::inbox_snapshot(&self.project, id)?;
                Ok(Reply::json(200, json!(inbox)))
            }
            // The conversation, as `crate::conversation` reads it: content only with the
            // project's stream on, and never from a generation older than the current setting.
            ["api", "terminals", id, "conversation"] if method == &Method::Get => {
                let fresh = crate::transcript::load(&self.identity.store);
                let mut terminals = self.terminals.lock().unwrap();
                let terminal = terminals
                    .get_mut(*id)
                    .ok_or("terminal is not hosted here")?;
                let Some(hosted) = terminal.hosted.as_mut() else {
                    return Ok(Reply::json(
                        200,
                        json!({"available": false, "running": false, "stream": fresh,
                            "reason": "The agent's process has ended, so Verb no longer follows its conversation here."}),
                    ));
                };
                let mut view = hosted
                    .conversation_view(crate::transcript::MAX_ITEMS)
                    .ok_or("This terminal has no agent conversation.")?;
                if view["stream"]["generation"].as_u64() != Some(fresh.generation) {
                    hosted.apply_stream(fresh);
                    view = hosted
                        .conversation_view(crate::transcript::MAX_ITEMS)
                        .ok_or("This terminal has no agent conversation.")?;
                }
                view["available"] = json!(true);
                view["running"] = json!(true);
                Ok(Reply::json(200, view))
            }
            // A chat message: never raw keystrokes, never sent twice for one id, and refused unless
            // the agent is ready on evidence. Terminal input keeps its own route, ungated.
            ["api", "terminals", id, "chat"] if method == &Method::Post => {
                let input: ChatRequest = read_json(request)?;
                let mut terminals = self.terminals.lock().unwrap();
                let terminal = terminals
                    .get_mut(*id)
                    .ok_or("terminal is not hosted here")?;
                let hosted = terminal.hosted.as_mut().ok_or("terminal has ended")?;
                match hosted.chat_send(
                    &input.message_id,
                    &input.text,
                    crate::conversation::Origin::Desktop,
                ) {
                    Ok(()) => Ok(Reply::json(
                        200,
                        hosted
                            .conversation_view(crate::transcript::MAX_ITEMS)
                            .unwrap_or(Value::Null),
                    )),
                    Err(refusal) => Ok(Reply::json(
                        409,
                        json!({"error": refusal.reason, "code": refusal.code}),
                    )),
                }
            }
            ["api", "terminals", id, "chat", message, action @ ("check" | "discard")]
                if method == &Method::Post =>
            {
                let mut terminals = self.terminals.lock().unwrap();
                let terminal = terminals
                    .get_mut(*id)
                    .ok_or("terminal is not hosted here")?;
                let hosted = terminal.hosted.as_mut().ok_or("terminal has ended")?;
                let outcome = if *action == "check" {
                    hosted.chat_check(message)
                } else {
                    hosted.chat_discard(message)
                };
                match outcome {
                    Ok(()) => Ok(Reply::json(
                        200,
                        hosted
                            .conversation_view(crate::transcript::MAX_ITEMS)
                            .unwrap_or(Value::Null),
                    )),
                    Err(error) => Ok(Reply::json(
                        404,
                        json!({"error": error, "code": "unknown_message"}),
                    )),
                }
            }
            ["api", "terminals", id, "output"] if method == &Method::Get => {
                let after = url
                    .split('?')
                    .nth(1)
                    .and_then(|query| query.strip_prefix("after="))
                    .unwrap_or("0")
                    .parse::<u64>()
                    .map_err(|_| "invalid terminal cursor".to_owned())?;
                let terminals = self.terminals.lock().unwrap();
                let terminal = terminals.get(*id).ok_or("terminal is not hosted here")?;
                Ok(Reply::json(200, terminal.output(after)))
            }
            ["api", "terminals", id, "input"] if method == &Method::Post => {
                let input: InputRequest = read_json(request)?;
                if input.data.len() > 8192 {
                    return Err("terminal input is too long".to_owned());
                }
                let mut terminals = self.terminals.lock().unwrap();
                let terminal = terminals
                    .get_mut(*id)
                    .ok_or("terminal is not hosted here")?;
                let hosted = terminal.hosted.as_mut().ok_or("terminal has ended")?;
                hosted.write(input.data.as_bytes())?;
                Ok(Reply::json(200, json!({"ok": true})))
            }
            // The desktop user takes input back from a paired phone. Only reachable with the page's
            // token, which the hosted program does not have.
            ["api", "terminals", id, "control"] if method == &Method::Post => {
                let mut terminals = self.terminals.lock().unwrap();
                let terminal = terminals
                    .get_mut(*id)
                    .ok_or("terminal is not hosted here")?;
                let hosted = terminal.hosted.as_mut().ok_or("terminal has ended")?;
                hosted.take_input_back()?;
                Ok(Reply::json(
                    200,
                    json!({"message": "Input is back with this desktop."}),
                ))
            }
            ["api", "terminals", id, "phone"] if method == &Method::Post => {
                let terminals = self.terminals.lock().unwrap();
                let terminal = terminals.get(*id).ok_or("terminal is not hosted here")?;
                if terminal.hosted.is_none() {
                    return Err("terminal has ended".to_owned());
                }
                if self.phone_shares.contains_key(*id) {
                    return Err("stop the current phone share before creating a new one".to_owned());
                }
                let share = PhoneShare::start(id)?;
                let links = share.links.clone();
                let expires_at = share.offer_expires_at;
                let status = share.status()?;
                self.phone_shares.insert((*id).to_owned(), share);
                Ok(Reply::json(
                    200,
                    json!({"links": links, "expiresAt": expires_at, "status": status}),
                ))
            }
            ["api", "terminals", id, "phone"] if method == &Method::Get => {
                self.terminals
                    .lock()
                    .unwrap()
                    .get(*id)
                    .ok_or("terminal is not hosted here")?;
                let share = self.phone_shares.get(*id);
                let status = share.map(|share| share.status()).transpose()?;
                Ok(Reply::json(
                    200,
                    json!({"links": share.map(|share| &share.links),
                    "expiresAt": share.map(|share| share.offer_expires_at), "status": status}),
                ))
            }
            ["api", "terminals", id, "phone", "renew"] if method == &Method::Post => {
                let share = self
                    .phone_shares
                    .get_mut(*id)
                    .ok_or("this terminal is not shared")?;
                share.renew_offer()?;
                let status = share.status()?;
                Ok(Reply::json(
                    200,
                    json!({"links": share.links, "expiresAt": share.offer_expires_at, "status": status}),
                ))
            }
            ["api", "terminals", id, "phone"] if method == &Method::Delete => {
                self.phone_shares
                    .remove(*id)
                    .ok_or("this terminal is not shared")?;
                Ok(Reply::json(200, json!({"message":"Phone access revoked."})))
            }
            ["api", "terminals", id, "resize"] if method == &Method::Post => {
                let size: ResizeRequest = read_json(request)?;
                if !(4..=200).contains(&size.rows) || !(20..=400).contains(&size.cols) {
                    return Err("terminal size is out of range".to_owned());
                }
                let mut terminals = self.terminals.lock().unwrap();
                let terminal = terminals
                    .get_mut(*id)
                    .ok_or("terminal is not hosted here")?;
                let hosted = terminal.hosted.as_mut().ok_or("terminal has ended")?;
                hosted.resize(size.rows, size.cols);
                Ok(Reply::json(200, json!({"ok": true})))
            }
            ["api", "terminals", id] if method == &Method::Delete => {
                // Stopping may wait on the execution boundary: do it on a worker, reserved before
                // anything changes so a saturated host refuses without losing track of the terminal.
                let Some(permit) =
                    RequestPermit::acquire(&self.background_requests, MAX_BACKGROUND_REQUESTS)
                else {
                    return Ok(Reply::error(503, HOST_BUSY));
                };
                let mut terminal = self
                    .terminals
                    .lock()
                    .unwrap()
                    .remove(*id)
                    .ok_or("terminal is not hosted here")?;
                self.phone_shares.remove(*id);
                self.invalidate_state();
                let hosted = terminal.hosted.take();
                let generation = Arc::clone(&self.state_generation);
                let mut reply = Reply::json(200, Value::Null);
                reply.permit = Some(permit);
                reply.deferred = Some(Box::new(move || {
                    let mut body = json!({"ok": true});
                    if let Some(hosted) = hosted {
                        let (pid, session) = (hosted.pid(), hosted.session.id.clone());
                        hosted.stop()?;
                        if crate::isolation::cleanup_unconfirmed(pid, &session) {
                            body["warning"] = json!(TERMINAL_CLEANUP_UNCONFIRMED);
                        }
                    }
                    generation.fetch_add(1, Ordering::SeqCst);
                    Ok(body.to_string().into_bytes())
                }));
                Ok(reply)
            }
            ["api", "sessions", "clear-ended"] if method == &Method::Post => {
                let records = crate::read_session_records()?;
                let mut cleared = 0;
                let mut terminals = self.terminals.lock().unwrap();
                for session in records {
                    if session.state == SessionState::Ended
                        && crate::session_in_project_with_id(
                            &session,
                            &self.project,
                            &self.project_id,
                        )
                        && terminals
                            .get(&session.id)
                            .is_none_or(|terminal| terminal.hosted.is_none())
                        && crate::forget_session(&session.id).is_ok()
                    {
                        terminals.remove(&session.id);
                        cleared += 1;
                    }
                }
                self.invalidate_state();
                Ok(Reply::json(200, json!({"cleared": cleared})))
            }
            ["api", "sessions", id] if method == &Method::Delete => {
                let mut terminals = self.terminals.lock().unwrap();
                if terminals
                    .get(*id)
                    .is_some_and(|terminal| terminal.hosted.is_some())
                {
                    return Err("cannot delete a currently hosted terminal session".to_owned());
                }
                let session = crate::load_session_by_id(id)?.ok_or("session not found")?;
                if session.state != SessionState::Ended
                    || !crate::session_in_project_with_id(&session, &self.project, &self.project_id)
                {
                    return Err("only ended sessions in this project can be removed".to_owned());
                }
                crate::forget_session(id)?;
                terminals.remove(*id);
                self.invalidate_state();
                Ok(Reply::json(200, json!({"ok": true})))
            }
            _ => Ok(Reply::error(404, "not found")),
        }
    }

    fn launcher(&self) -> Launcher {
        Launcher {
            project: self.project.clone(),
            project_id: self.project_id.clone(),
            identity: self.identity.clone(),
            terminals: Arc::clone(&self.terminals),
            generation: Arc::clone(&self.state_generation),
            actor: self.actor(),
        }
    }

    /// Answers `work` off the request loop, on a bounded worker: anything that may cross the
    /// execution boundary (Git, file lists) must not hold the loop that serves health, workspace
    /// reads and live terminal output. A change to project state is announced once `work` is done.
    fn off_loop(
        &self,
        changes_state: bool,
        work: impl FnOnce() -> Result<Value, String> + Send + 'static,
    ) -> Reply {
        let generation = changes_state.then(|| Arc::clone(&self.state_generation));
        let mut reply = Reply::json(200, Value::Null);
        reply.deferred = Some(Box::new(move || {
            let result = work();
            if let Some(generation) = generation {
                generation.fetch_add(1, Ordering::SeqCst);
            }
            result.map(|value| value.to_string().into_bytes())
        }));
        reply
    }

    fn launch_reply(
        &self,
        start: impl FnOnce() -> Result<Started, String> + Send + 'static,
    ) -> Reply {
        launch_reply(
            &self.background_requests,
            &self.terminals,
            &self.state_generation,
            start,
            crate::isolation::admitted,
        )
    }

    fn invalidate_state(&self) {
        self.state_generation.fetch_add(1, Ordering::SeqCst);
    }

    fn workspace(&self) -> Value {
        let terminals = self.terminals.lock().unwrap();
        let sessions = terminals
            .values()
            .filter(|terminal| terminal.pending_admission.is_none())
            .filter_map(|terminal| {
                let hosted = terminal.hosted.as_ref()?;
                let session = &hosted.session;
                Some(json!({"id": session.id, "agent": session.display_agent(),
                "state": "live", "isolated": session.project_id != self.identity.anchor,
                "hostedHere": true, "hasTerminal": true, "terminal": terminal_status(Some(true)), "canResume": false, "attention": 0}))
            })
            .collect::<Vec<_>>();
        json!({"project": {"id": self.project_id, "name": self.identity.anchor.file_name()
            .map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
            "anchor": self.identity.anchor, "workspace": self.project,
            "branch": null, "gitStatus": "loading", "changedFiles": null},
            "deployment": self.deployment, "sessions": sessions,
            "terminalStates": terminals.iter().filter(|(_, terminal)| terminal.pending_admission.is_none()).map(|(id, terminal)|
                (id.clone(), terminal_status(Some(terminal.hosted.is_some()))))
                .collect::<serde_json::Map<String, Value>>(),
            "tasks": [], "memory": ""})
    }
}

/// What starting a terminal needs from the host, detached from the request loop. A launch can
/// cross the execution boundary before its client even starts (Git for a handoff note, an isolated
/// checkout), so it runs on the worker that later answers it, never on the loop that serves health,
/// workspace reads and live terminal output.
#[derive(Clone)]
struct Launcher {
    project: PathBuf,
    project_id: String,
    identity: project::ProjectIdentity,
    terminals: Arc<std::sync::Mutex<HashMap<String, WebTerminal>>>,
    generation: Arc<AtomicU64>,
    /// Who asked, resolved while the request was authenticated.
    actor: String,
}

impl Launcher {
    /// Starts an agent (or a shell) on a spec. The returned action records the start in the spec's
    /// audit trail (how the session board and Ask Verb know who worked on what); callers run it only
    /// after the terminal is confirmed started.
    fn start_on_spec(&self, id: &str, agent: &str) -> Result<(String, AfterAdmission), String> {
        self.start_on_spec_with(id, agent, None)
    }

    /// `conversation`: continue an Antigravity talk in its terminal instead of starting afresh.
    /// Returns the launched session and the spec record of its start, which the caller runs only
    /// once the terminal is confirmed started: a refused or timed-out launch records nothing.
    fn start_on_spec_with(
        &self,
        id: &str,
        agent: &str,
        conversation: Option<&str>,
    ) -> Result<(String, AfterAdmission), String> {
        let (path, spec) = crate::specs::find(&self.project, id)?;
        // Every agent whose CLI can take an opening prompt gets the spec brief: as the first
        // argument for Claude, Codex and Gemini, through `-i` (run it, then stay interactive) for
        // Antigravity. OpenCode has no such option, so it starts empty and the spec view says so.
        let args = match agent {
            "claude" | "codex" | "gemini" => vec![crate::specs::agent_brief(&spec)],
            "agy" => match conversation {
                Some(c) => vec!["--conversation".to_owned(), c.to_owned()],
                None => vec!["-i".to_owned(), crate::specs::agent_brief(&spec)],
            },
            "opencode" | "shell" => Vec::new(),
            _ => return Err("choose a supported agent".to_owned()),
        };
        let session = self.launch(LaunchRequest {
            agent: agent.to_owned(),
            isolated: false,
            command: None,
            args,
            resume_id: None,
        })?;
        let actor = self.actor.clone();
        let line = spec_start_line(agent, &session, conversation.is_some());
        let record: AfterAdmission = Box::new(move || crate::specs::record(&path, &actor, &line));
        Ok((session, record))
    }

    fn launch(&self, input: LaunchRequest) -> Result<String, String> {
        if input.args.len() > 32 || input.args.iter().any(|arg| arg.len() > 4096) {
            return Err("too many or oversized agent arguments".to_owned());
        }
        if input.resume_id.is_none()
            && !matches!(
                input.agent.as_str(),
                "claude" | "codex" | "gemini" | "agy" | "opencode" | "shell" | "custom"
            )
        {
            return Err("choose a supported agent or a custom CLI".to_owned());
        }
        if input.resume_id.is_none()
            && input.agent == "custom"
            && input
                .command
                .as_deref()
                .is_none_or(|command| command.trim().is_empty())
        {
            return Err("enter a CLI executable".to_owned());
        }
        // Say plainly when an agent is not on this machine, instead of starting a terminal that
        // fails at exec and shows an empty tile.
        let missing = match input.resume_id.as_deref() {
            Some(id) => crate::recorded_agent_installed(id)
                .filter(|(_, installed)| !installed)
                .map(|(agent, _)| agent),
            None => (input.agent != "shell" && input.agent != "custom")
                .then(|| crate::Agent::parse(&input.agent))
                .filter(|agent| !agent.installed())
                .map(|agent| agent.label().to_owned()),
        };
        if let Some(agent) = missing {
            return Err(format!(
                "{agent} is unavailable in this execution environment. Choose an available agent or ask the host operator to provision it."
            ));
        }
        let start = if let Some(id) = input.resume_id {
            if input.isolated || !input.args.is_empty() || input.command.is_some() {
                return Err(
                    "resume uses the selected session's original checkout and arguments".to_owned(),
                );
            }
            crate::begin_resume(&self.project, Some(&id)).map_err(|error| error.message)?
        } else {
            let workspace = if input.isolated {
                project::create_isolated_checkout(&self.project)?
            } else {
                self.project.clone()
            };
            match input.agent.as_str() {
                "claude" => crate::begin_session_with_identity(
                    &workspace,
                    Agent::Claude,
                    input.args,
                    Some(&self.identity),
                ),
                "codex" => crate::begin_session_with_identity(
                    &workspace,
                    Agent::Codex,
                    input.args,
                    Some(&self.identity),
                ),
                "gemini" => crate::begin_session_with_identity(
                    &workspace,
                    Agent::Gemini,
                    input.args,
                    Some(&self.identity),
                ),
                "agy" => crate::begin_session_with_identity(
                    &workspace,
                    Agent::Agy,
                    input.args,
                    Some(&self.identity),
                ),
                "opencode" => crate::begin_session_with_identity(
                    &workspace,
                    Agent::OpenCode,
                    input.args,
                    Some(&self.identity),
                ),
                "shell" => crate::begin_session_with_identity(
                    &workspace,
                    Agent::Shell,
                    input.args,
                    Some(&self.identity),
                ),
                "custom" => crate::begin_external_session_with_identity(
                    &workspace,
                    input.command.ok_or("enter a CLI executable")?,
                    input.args,
                    Some(&self.identity),
                )?,
                _ => return Err("choose a supported agent or a custom CLI".to_owned()),
            }
        };
        let workspace = start.session.project_id.clone();
        if !crate::session_in_project_with_id(&start.session, &self.project, &self.project_id) {
            return Err("selected session belongs to another project".to_owned());
        }
        let sink = crate::stream::TerminalStreamSink::new(start.session.id.clone());
        // Before the spawn: the controller's receipt for this launch cannot be older.
        let spawned_at = std::time::SystemTime::now();
        let hosted = Hosted::start_with_sink(
            &workspace,
            start.session,
            &start.command,
            &start.args,
            &start.env,
            start.is_new,
            28,
            100,
            Some(Arc::clone(&sink)),
        )?;
        let id = hosted.session.id.clone();
        let pending = crate::isolation::launch_identity(hosted.pid(), &workspace, &id, spawned_at);
        self.terminals
            .lock()
            .unwrap()
            .insert(id.clone(), WebTerminal::new(hosted, sink, pending));
        self.generation.fetch_add(1, Ordering::SeqCst);
        Ok(id)
    }
}

fn deployment_info() -> Value {
    let remote = std::env::var("VERB_DEPLOYMENT")
        .map(|value| value == "remote")
        .unwrap_or_else(|_| {
            std::env::var("VERB_ALLOWED_ORIGIN").is_ok_and(|value| !value.trim().is_empty())
        });
    let name = std::env::var("VERB_NODE_NAME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            if remote {
                "PocketFabric Node 1"
            } else {
                "Running on this computer"
            }
            .to_owned()
        });
    json!({"name": name, "mode": if remote { "REMOTE" } else { "LOCAL" }})
}

fn send_reply(request: Request, reply: Reply) {
    // Early authentication/capacity failures deliberately do not drain bodies. Closing
    // before releasing the reader prevents a keep-alive parser from adopting those bytes.
    let retire_connection = reply.status >= 400
        && (request.body_length().is_some_and(|length| length > 0)
            || header(&request, "Transfer-Encoding").is_some());
    let mut response = Response::from_data(reply.body).with_status_code(StatusCode(reply.status));
    // Saved sandbox snapshots must not acquire CDN-injected scripts or HTML rewrites.
    // Keep no-store; no-transform is response-local and does not change Access/WAF policy.
    let cache_control = if reply.sandbox_preview {
        "no-store, no-transform"
    } else {
        reply.cache_control.unwrap_or("no-store")
    };
    let csp = if reply.sandbox_preview {
        "sandbox allow-scripts; default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:; connect-src 'none'; form-action 'none'; base-uri 'none'; frame-src 'none'; frame-ancestors 'self'"
    } else if reply.studio_frames {
        "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; connect-src 'self'; font-src 'self'; img-src 'self' data:; frame-src 'self'; frame-ancestors 'none'; base-uri 'none'"
    } else {
        "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; connect-src 'self'; font-src 'self'; img-src 'self' data:; frame-ancestors 'none'; base-uri 'none'"
    };
    for (name, value) in [
        ("Content-Type", reply.mime),
        ("Cache-Control", cache_control),
        ("X-Content-Type-Options", "nosniff"),
        (
            "X-Frame-Options",
            if reply.sandbox_preview {
                "SAMEORIGIN"
            } else {
                "DENY"
            },
        ),
        ("Referrer-Policy", "no-referrer"),
        ("Cross-Origin-Resource-Policy", "same-origin"),
        // 'wasm-unsafe-eval' lets the terminal's image addon compile its WebAssembly Sixel
        // decoder. It permits WebAssembly compilation only, not JavaScript eval. Without it every
        // terminal threw a CSP error and inline images never worked (caught by browser tests).
        ("Content-Security-Policy", csp),
    ] {
        response.add_header(Header::from_bytes(name, value).expect("static HTTP header"));
    }
    if let Some(cookie) = reply.set_cookie {
        if let Ok(header) = Header::from_bytes("Set-Cookie", cookie) {
            response.add_header(header);
        }
    }
    if retire_connection {
        let _ = request.respond_and_close(response);
    } else {
        let _ = request.respond(response);
    }
}

fn header<'a>(request: &'a Request, name: &'static str) -> Option<&'a str> {
    request
        .headers()
        .iter()
        .find(|header| header.field.equiv(name))
        .map(|header| header.value.as_str())
}

struct DeadlineReader<'a> {
    inner: &'a mut dyn Read,
    until: Instant,
}
impl Read for DeadlineReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if Instant::now() >= self.until {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "request body deadline exceeded",
            ));
        }
        self.inner.read(buffer)
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(request: &mut Request) -> Result<T, String> {
    read_json_limit(request, MAX_BODY)
}

fn read_json_limit<T: for<'de> Deserialize<'de>>(
    request: &mut Request,
    limit: usize,
) -> Result<T, String> {
    if !header(request, "Content-Type").is_some_and(|value| value.starts_with("application/json")) {
        return Err("send application/json".to_owned());
    }
    if request.body_length().is_none_or(|length| length > limit) {
        return Err("request body exceeds limit or has no length".to_owned());
    }
    let mut body = Vec::new();
    DeadlineReader {
        inner: request.as_reader(),
        until: Instant::now() + Duration::from_secs(2),
    }
    .take((limit + 1) as u64)
    .read_to_end(&mut body)
    .map_err(|error| format!("could not read request: {error}"))?;
    if body.len() > limit {
        return Err("request body exceeds limit".to_owned());
    }
    serde_json::from_slice(&body).map_err(|error| format!("invalid request: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_capacity_is_bounded_and_recovers_when_work_finishes() {
        let counter = Arc::new(AtomicUsize::new(0));
        let first = RequestPermit::acquire(&counter, 2).unwrap();
        let second = RequestPermit::acquire(&counter, 2).unwrap();
        assert!(RequestPermit::acquire(&counter, 2).is_none());
        drop(first);
        assert!(RequestPermit::acquire(&counter, 2).is_some());
        drop(second);
        assert_eq!(counter.load(Ordering::Acquire), 0);
    }

    fn never(_: &crate::isolation::Launch) -> bool {
        false
    }

    fn always(_: &crate::isolation::Launch) -> bool {
        true
    }

    fn test_launch(pid: i32) -> crate::isolation::Launch {
        crate::isolation::Launch {
            pid,
            started: Some(1),
            project: "p".to_owned(),
            session: "t".to_owned(),
            spawned_at: std::time::SystemTime::now(),
        }
    }

    fn spec_reply(
        terminal: WebTerminal,
        body: Value,
        after: Option<AfterAdmission>,
        admitted: fn(&crate::isolation::Launch) -> bool,
    ) -> (Reply, Arc<std::sync::Mutex<HashMap<String, WebTerminal>>>) {
        let terminals = Arc::new(std::sync::Mutex::new(HashMap::from([(
            "spec".to_owned(),
            terminal,
        )])));
        let generation = Arc::new(AtomicU64::new(0));
        let counter = Arc::new(AtomicUsize::new(0));
        let reply = launch_reply(
            &counter,
            &terminals,
            &generation,
            move || Ok(("spec".to_owned(), body, after)),
            admitted,
        );
        (reply, terminals)
    }

    fn spec_handoff_reply(
        terminal: WebTerminal,
        admitted: fn(&crate::isolation::Launch) -> bool,
    ) -> (Reply, Arc<std::sync::Mutex<HashMap<String, WebTerminal>>>) {
        spec_reply(
            terminal,
            json!({"sessionId": "spec", "note": "handoff note"}),
            None,
            admitted,
        )
    }

    /// A spec start record that only counts how often it is written.
    fn counted_record(fail: bool) -> (AfterAdmission, Arc<AtomicUsize>) {
        let writes = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&writes);
        let record: AfterAdmission = Box::new(move || {
            seen.fetch_add(1, Ordering::SeqCst);
            if fail {
                Err("spec file is read-only".to_owned())
            } else {
                Ok(())
            }
        });
        (record, writes)
    }

    /// Bodies of the three spec launch routes: agent, handoff and talk-to-terminal.
    fn spec_route_bodies() -> [(&'static str, Value); 3] {
        [
            ("agent", json!({"sessionId": "spec"})),
            (
                "handoff",
                json!({"sessionId": "spec", "note": "handoff note"}),
            ),
            ("talk", json!({"sessionId": "spec"})),
        ]
    }

    #[test]
    fn admitted_spec_launches_record_the_start_once_and_keep_their_body() {
        for (route, body) in spec_route_bodies() {
            let (record, writes) = counted_record(false);
            let (mut reply, terminals) = spec_reply(
                bare_terminal(None, Some(4242)),
                body.clone(),
                Some(record),
                always,
            );
            let sent = (reply.deferred.take().unwrap())().unwrap();
            let sent: Value = serde_json::from_slice(&sent).unwrap();
            assert_eq!(writes.load(Ordering::SeqCst), 1, "{route}");
            assert_eq!(sent["sessionId"], "spec", "{route}");
            assert_eq!(sent.get("note"), body.get("note"), "{route}");
            assert!(sent.get("warning").is_none(), "{route}");
            assert!(terminals.lock().unwrap()["spec"]
                .pending_admission
                .is_none());
        }
    }

    #[test]
    fn refused_spec_launches_record_no_start() {
        for (route, body) in spec_route_bodies() {
            let (record, writes) = counted_record(false);
            let (mut reply, terminals) = spec_reply(refused_terminal(), body, Some(record), never);
            let error = (reply.deferred.take().unwrap())().unwrap_err();
            assert_eq!(error, TERMINAL_CAPACITY, "{route}");
            assert_eq!(writes.load(Ordering::SeqCst), 0, "{route}");
            assert!(terminals.lock().unwrap().is_empty(), "{route}");
        }
    }

    #[test]
    fn timed_out_spec_launches_record_no_start() {
        // Run the three routes concurrently: each waits out the real admission deadline.
        let runs: Vec<_> = spec_route_bodies()
            .into_iter()
            .map(|(route, body)| {
                std::thread::spawn(move || {
                    let (record, writes) = counted_record(false);
                    let (mut reply, terminals) =
                        spec_reply(bare_terminal(None, Some(4242)), body, Some(record), never);
                    let error = (reply.deferred.take().unwrap())().unwrap_err();
                    assert_eq!(error, TERMINAL_START_TIMEOUT, "{route}");
                    assert_eq!(writes.load(Ordering::SeqCst), 0, "{route}");
                    assert!(terminals.lock().unwrap().is_empty(), "{route}");
                })
            })
            .collect();
        for run in runs {
            run.join().unwrap();
        }
    }

    #[test]
    fn client_exit_126_before_admission_records_no_start_and_stays_visible() {
        for (route, body) in spec_route_bodies() {
            let mut failed = bare_terminal(Some(126), Some(4242));
            failed.push(b"Verb execution unavailable: unavailable\r\n".to_vec());
            let (record, writes) = counted_record(false);
            let (mut reply, terminals) = spec_reply(failed, body, Some(record), never);
            let sent = (reply.deferred.take().unwrap())().unwrap();
            let sent: Value = serde_json::from_slice(&sent).unwrap();
            assert_eq!(writes.load(Ordering::SeqCst), 0, "{route}");
            let warning = sent["warning"].as_str().unwrap();
            assert!(
                warning.contains("exit 126") && warning.contains("no start was recorded"),
                "{route}: {warning}"
            );
            // Kept for diagnosis: listed, with its exit status and output.
            let map = terminals.lock().unwrap();
            assert!(map["spec"].pending_admission.is_none(), "{route}");
            assert_eq!(map["spec"].exit_code, Some(126), "{route}");
            let kept: Vec<u8> = map["spec"]
                .chunks
                .iter()
                .flat_map(|chunk| chunk.bytes.clone())
                .collect();
            assert!(
                String::from_utf8_lossy(&kept).contains("unavailable: unavailable"),
                "{route}: client output must be kept for diagnosis"
            );
        }
    }

    #[test]
    fn confirmed_admission_then_exit_126_or_75_records_exactly_once() {
        for code in [126, crate::isolation::REFUSED_EXIT] {
            for (route, body) in spec_route_bodies() {
                let (record, writes) = counted_record(false);
                let (mut reply, _) = spec_reply(
                    bare_terminal(Some(code), Some(4242)),
                    body,
                    Some(record),
                    always,
                );
                let sent = (reply.deferred.take().unwrap())().unwrap();
                let sent: Value = serde_json::from_slice(&sent).unwrap();
                assert_eq!(writes.load(Ordering::SeqCst), 1, "{route} exit {code}");
                assert!(sent.get("warning").is_none(), "{route} exit {code}");
            }
        }
    }

    #[test]
    fn quick_exit_without_confirmed_admission_makes_no_start_claim() {
        for code in [0, crate::isolation::REFUSED_EXIT] {
            let mut quick = bare_terminal(Some(code), Some(4242));
            quick.push(b"done\r\n".to_vec());
            let (record, writes) = counted_record(false);
            let (mut reply, terminals) =
                spec_reply(quick, json!({"sessionId": "spec"}), Some(record), never);
            let sent = (reply.deferred.take().unwrap())().unwrap();
            let sent: Value = serde_json::from_slice(&sent).unwrap();
            assert_eq!(writes.load(Ordering::SeqCst), 0, "exit {code}");
            assert!(
                sent["warning"]
                    .as_str()
                    .unwrap()
                    .contains("no start was recorded"),
                "exit {code}"
            );
            assert!(terminals.lock().unwrap()["spec"]
                .pending_admission
                .is_none());
        }
    }

    #[test]
    fn failed_start_record_is_reported_and_the_running_terminal_stays_tracked() {
        let (record, writes) = counted_record(true);
        let (mut reply, terminals) = spec_reply(
            bare_terminal(None, Some(4242)),
            json!({"sessionId": "spec", "note": "handoff note"}),
            Some(record),
            always,
        );
        let sent = (reply.deferred.take().unwrap())().unwrap();
        let sent: Value = serde_json::from_slice(&sent).unwrap();
        assert_eq!(writes.load(Ordering::SeqCst), 1);
        assert_eq!(sent["sessionId"], "spec");
        assert_eq!(sent["note"], "handoff note");
        let warning = sent["warning"].as_str().unwrap();
        assert!(
            warning.contains("terminal is running") && warning.contains("read-only"),
            "{warning}"
        );
        assert!(terminals.lock().unwrap()["spec"]
            .pending_admission
            .is_none());
    }

    #[test]
    fn non_boundary_spec_launch_records_the_start_without_waiting() {
        let (record, writes) = counted_record(false);
        let (mut reply, _) = spec_reply(
            bare_terminal(None, None),
            json!({"sessionId": "spec"}),
            Some(record),
            never,
        );
        let started = Instant::now();
        (reply.deferred.take().unwrap())().unwrap();
        assert!(started.elapsed() < Duration::from_millis(500));
        assert_eq!(writes.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn start_record_names_the_agent_and_marks_a_continued_talk() {
        assert_eq!(
            spec_start_line("codex", "0123456789abcdef", false),
            "started codex on this spec (session 01234567)"
        );
        assert_eq!(
            spec_start_line("agy", "abc", true),
            "started agy on this spec (session abc) to continue the talk"
        );
    }

    #[test]
    fn admitted_spec_launch_answers_with_its_own_body_and_is_shown() {
        let (mut reply, terminals) = spec_handoff_reply(bare_terminal(None, Some(4242)), always);
        let body = (reply
            .deferred
            .take()
            .expect("external launch must wait for admission"))()
        .unwrap();
        let body: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["sessionId"], "spec");
        assert_eq!(body["note"], "handoff note");
        assert!(terminals.lock().unwrap()["spec"]
            .pending_admission
            .is_none());
    }

    #[test]
    fn refused_spec_launch_is_a_capacity_error_and_leaves_nothing_hidden() {
        let (mut reply, terminals) = spec_handoff_reply(refused_terminal(), never);
        let error = (reply.deferred.take().unwrap())().unwrap_err();
        assert_eq!(error, TERMINAL_CAPACITY);
        assert!(terminals.lock().unwrap().is_empty());
    }

    #[test]
    fn saturated_workers_refuse_a_launch_before_any_process_starts() {
        let counter = Arc::new(AtomicUsize::new(0));
        let terminals = Arc::new(std::sync::Mutex::new(HashMap::new()));
        let generation = Arc::new(AtomicU64::new(0));
        let started = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let start = |started: &Arc<std::sync::atomic::AtomicBool>| {
            let started = Arc::clone(started);
            move || {
                started.store(true, Ordering::SeqCst);
                Ok(("t".to_owned(), json!({"sessionId": "t"}), None))
            }
        };
        let held: Vec<_> = (0..MAX_BACKGROUND_REQUESTS)
            .map(|_| RequestPermit::acquire(&counter, MAX_BACKGROUND_REQUESTS).unwrap())
            .collect();
        let refused = launch_reply(&counter, &terminals, &generation, start(&started), never);
        assert_eq!(refused.status, 503);
        assert!(refused.deferred.is_none());
        assert!(
            !started.load(Ordering::SeqCst),
            "no terminal may start without a worker to acknowledge it"
        );
        drop(held);
        let mut reply = launch_reply(&counter, &terminals, &generation, start(&started), never);
        assert_eq!(counter.load(Ordering::Acquire), 1);
        // The start itself runs on that worker, never on the request loop.
        assert!(!started.load(Ordering::SeqCst));
        (reply.deferred.take().unwrap())().unwrap();
        assert!(started.load(Ordering::SeqCst));
        drop(reply);
        assert_eq!(counter.load(Ordering::Acquire), 0);
    }

    #[test]
    fn a_reserved_launch_worker_travels_with_the_deferred_reply() {
        let (reply, _) = spec_handoff_reply(bare_terminal(None, Some(4242)), always);
        assert!(reply.permit.is_some() && reply.deferred.is_some());
    }

    #[test]
    fn non_external_launch_is_answered_without_an_admission_wait() {
        let (mut reply, _) = spec_handoff_reply(bare_terminal(None, None), never);
        assert_eq!(reply.status, 201);
        let started = Instant::now();
        let body: Value =
            serde_json::from_slice(&(reply.deferred.take().unwrap())().unwrap()).unwrap();
        assert!(started.elapsed() < Duration::from_millis(500));
        assert_eq!(body["note"], "handoff note");
    }

    fn refused_terminal() -> WebTerminal {
        let mut terminal = bare_terminal(Some(crate::isolation::REFUSED_EXIT), Some(4242));
        terminal.push(b"Verb execution unavailable: capacity\r\n".to_vec());
        terminal
    }

    fn bare_terminal(exit_code: Option<i32>, pending: Option<i32>) -> WebTerminal {
        WebTerminal {
            hosted: None,
            sink: crate::stream::TerminalStreamSink::new("test".to_owned()),
            chunks: VecDeque::new(),
            base: 0,
            next: 0,
            buffered: 0,
            final_screen: Vec::new(),
            exit_seen_at: None,
            exit_code,
            pending_admission: pending.map(test_launch),
            failure: None,
            last_output: Instant::now(),
            failures: VecDeque::new(),
        }
    }

    #[test]
    fn refused_external_terminal_is_a_capacity_error_not_a_session() {
        let terminals =
            std::sync::Mutex::new(HashMap::from([("t".to_owned(), refused_terminal())]));
        let error = await_admission(
            &terminals,
            "t",
            &test_launch(4242),
            Duration::from_secs(5),
            never,
        )
        .unwrap_err();
        assert_eq!(error, TERMINAL_CAPACITY);
        // Never presented as a session, and nothing hidden is left behind once it is closed out.
        assert!(!terminals.lock().unwrap().contains_key("t"));
        let (status, body) = deferred_error(&error);
        assert_eq!(status, 503);
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap()["code"],
            "capacity"
        );
    }

    #[test]
    fn other_early_exits_are_shown_and_closed_terminals_are_not_capacity() {
        let terminals = std::sync::Mutex::new(HashMap::from([(
            "t".to_owned(),
            bare_terminal(Some(126), Some(4242)),
        )]));
        assert_eq!(
            await_admission(
                &terminals,
                "t",
                &test_launch(4242),
                Duration::from_secs(5),
                never
            )
            .unwrap(),
            Admission::EndedUnconfirmed(126)
        );
        assert!(terminals.lock().unwrap()["t"].pending_admission.is_none());
        let error = await_admission(
            &terminals,
            "missing",
            &test_launch(4242),
            Duration::ZERO,
            never,
        )
        .unwrap_err();
        assert!(error.contains("closed"), "{error}");
        assert_eq!(deferred_error(&error).0, 400);
    }

    #[test]
    fn no_verdict_in_time_is_a_start_timeout_with_cleanup_never_running() {
        let terminals = std::sync::Mutex::new(HashMap::from([(
            "t".to_owned(),
            bare_terminal(None, Some(4242)),
        )]));
        let error = await_admission(
            &terminals,
            "t",
            &test_launch(4242),
            Duration::from_millis(100),
            never,
        )
        .unwrap_err();
        assert_eq!(error, TERMINAL_START_TIMEOUT);
        assert!(terminals.lock().unwrap().is_empty());
        let (status, body) = deferred_error(&error);
        assert_eq!(status, 503);
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap()["code"],
            "startup_timeout"
        );
    }

    #[test]
    fn delayed_admission_is_awaited_and_then_shown() {
        let terminals = std::sync::Mutex::new(HashMap::from([(
            "t".to_owned(),
            bare_terminal(None, Some(4242)),
        )]));
        let from = Instant::now();
        let later =
            move |_: &crate::isolation::Launch| from.elapsed() >= Duration::from_millis(200);
        assert_eq!(
            await_admission(
                &terminals,
                "t",
                &test_launch(4242),
                Duration::from_secs(3),
                later
            )
            .unwrap(),
            Admission::Confirmed
        );
        assert!(from.elapsed() >= Duration::from_millis(200));
        assert!(terminals.lock().unwrap()["t"].pending_admission.is_none());
    }

    #[test]
    fn delayed_refusal_is_awaited_and_never_shown() {
        let terminals = Arc::new(std::sync::Mutex::new(HashMap::from([(
            "t".to_owned(),
            bare_terminal(None, Some(4242)),
        )])));
        let writer = Arc::clone(&terminals);
        let refuse = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            let mut map = writer.lock().unwrap();
            let terminal = map.get_mut("t").unwrap();
            terminal.push(b"Verb execution unavailable: capacity\r\n".to_vec());
            terminal.exit_code = Some(crate::isolation::REFUSED_EXIT);
        });
        let error = await_admission(
            &terminals,
            "t",
            &test_launch(4242),
            Duration::from_secs(3),
            never,
        )
        .unwrap_err();
        refuse.join().unwrap();
        assert_eq!(error, TERMINAL_CAPACITY);
        assert!(terminals.lock().unwrap().is_empty());
    }

    #[test]
    fn admitted_command_that_quickly_exits_75_is_not_a_capacity_refusal() {
        // Admission observed, then the program itself exits 75.
        let terminals = std::sync::Mutex::new(HashMap::from([(
            "t".to_owned(),
            bare_terminal(Some(crate::isolation::REFUSED_EXIT), Some(4242)),
        )]));
        assert_eq!(
            await_admission(
                &terminals,
                "t",
                &test_launch(4242),
                Duration::from_secs(3),
                always
            )
            .unwrap(),
            Admission::Confirmed
        );
        assert!(terminals.lock().unwrap()["t"].pending_admission.is_none());
        // Admission too brief to observe and no refusal text: shown, but not claimed as started.
        let mut quick = bare_terminal(Some(crate::isolation::REFUSED_EXIT), Some(4242));
        quick.push(b"tool: temporary failure\r\n".to_vec());
        let terminals = std::sync::Mutex::new(HashMap::from([("t".to_owned(), quick)]));
        assert_eq!(
            await_admission(
                &terminals,
                "t",
                &test_launch(4242),
                Duration::from_secs(3),
                never
            )
            .unwrap(),
            Admission::EndedUnconfirmed(crate::isolation::REFUSED_EXIT)
        );
        assert!(terminals.lock().unwrap()["t"].pending_admission.is_none());
    }

    #[test]
    fn boundary_capacity_text_is_congestion_and_git_errors_stay_errors() {
        let busy = "Verb execution unavailable: busy";
        assert_eq!(deferred_error(busy).0, 503);
        assert_eq!(deferred_error("fatal: not a git repository").0, 400);
    }

    #[test]
    fn output_cursor_replays_bytes_and_reports_overflow() {
        let mut terminal = WebTerminal {
            hosted: None,
            sink: crate::stream::TerminalStreamSink::new("test".to_owned()),
            chunks: VecDeque::new(),
            base: 0,
            next: 0,
            buffered: 0,
            final_screen: b"last screen".to_vec(),
            exit_seen_at: None,
            exit_code: None,
            pending_admission: None,
            failure: None,
            last_output: Instant::now(),
            failures: VecDeque::new(),
        };
        terminal.push(b"hello".to_vec());
        terminal.push(b" world".to_vec());
        let output = terminal.output(0);
        assert_eq!(output["data"], BASE64.encode(b"hello world"));
        assert_eq!(output["cursor"], 11);
        assert_eq!(terminal.output(5)["data"], BASE64.encode(b" world"));
        terminal.base = 5;
        let reset = terminal.output(0);
        assert_eq!(reset["reset"], true);
        assert_eq!(reset["data"], BASE64.encode(b"last screen"));
    }
}
