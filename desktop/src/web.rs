//! A loopback browser host for the same durable project, sessions, and task ledger as the TUI.
//! The browser is a view/controller; this process remains the sole owner of every PTY it starts.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use crate::phone::PhoneShare;
use crate::tui::term::Hosted;
use crate::{project, workbench, Agent, SessionState};

const MAX_BODY: usize = 64 * 1024;
const MAX_BUFFER: usize = 2 * 1024 * 1024;
const MAX_REPLY: usize = 128 * 1024;

struct OutputChunk {
    from: u64,
    bytes: Vec<u8>,
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
    failure: Option<String>,
    /// For the observer, in memory only: when this terminal last produced output, and recent
    /// failed commands (shell integration's volatile label, never written anywhere).
    last_output: Instant,
    failures: VecDeque<(String, Instant)>,
}

impl WebTerminal {
    fn new(mut hosted: Hosted, sink: Arc<crate::stream::TerminalStreamSink>) -> Self {
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
            let _ = hosted.stop();
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
}

impl crate::stream::StreamTerminalHandler for WebTerminalsManager {
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

struct WebHost {
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
    identity: project::ProjectIdentity,
    deployment: Value,
    state_cache: Arc<std::sync::Mutex<Option<StateCache>>>,
    state_generation: Arc<AtomicU64>,
}

struct StateCache {
    body: Vec<u8>,
    cached_at: Instant,
    generation: u64,
}

struct StateJob {
    project: PathBuf,
    identity: project::ProjectIdentity,
    deployment: Value,
    hosted: HashMap<String, bool>,
    cache: Arc<std::sync::Mutex<Option<StateCache>>>,
    generation: u64,
    current_generation: Arc<AtomicU64>,
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

    fn invalidate_checks(&self) {
        let mut cache = self
            .checks
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        cache.report = None;
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
    body: Vec<u8>,
    set_cookie: Option<String>,
    cache_control: Option<&'static str>,
    state_job: Option<StateJob>,
    /// Slow, read-only work answered off the request loop, which also pumps live terminals.
    deferred: Option<DeferredWork>,
}

impl Reply {
    fn json(status: u16, value: Value) -> Self {
        Self {
            status,
            mime: "application/json; charset=utf-8",
            body: value.to_string().into_bytes(),
            set_cookie: None,
            cache_control: None,
            state_job: None,
            deferred: None,
        }
    }

    fn error(status: u16, message: impl AsRef<str>) -> Self {
        Self::json(status, json!({"error": message.as_ref()}))
    }

    fn asset(mime: &'static str, bytes: &[u8]) -> Self {
        Self {
            status: 200,
            mime,
            body: bytes.to_vec(),
            set_cookie: None,
            cache_control: None,
            state_job: None,
            deferred: None,
        }
    }

    fn asset_cached(mime: &'static str, bytes: &[u8]) -> Self {
        Self {
            status: 200,
            mime,
            body: bytes.to_vec(),
            set_cookie: None,
            cache_control: None,
            state_job: None,
            deferred: None,
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
        identity,
        deployment: deployment_info(),
        state_cache: Arc::default(),
        state_generation: Arc::new(AtomicU64::new(0)),
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
        let is_authorized = header(&request, "X-Verb-Token") == Some(self.token.as_str())
            || token_in_query == Some(self.token.as_str())
            || session_valid
            || has_valid_session
            || cf_authenticated;

        if !is_authorized {
            send_reply(
                request,
                Reply::error(403, "open the URL printed by verb web"),
            );
            return;
        }

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
        });

        std::thread::spawn(move || {
            crate::stream::handle_ws_connection(tcp_stream, handler, conn_id);
        });
    }

    fn respond(&mut self, mut request: Request) {
        let mut reply = self.dispatch(&mut request);
        if let Some(work) = reply.deferred.take() {
            std::thread::spawn(move || {
                match work() {
                    Ok(body) => reply.body = body,
                    Err(error) => {
                        reply.status = 400;
                        reply.body = json!({"error": error}).to_string().into_bytes();
                    }
                }
                send_reply(request, reply);
            });
            return;
        }
        if let Some(job) = reply.state_job.take() {
            // Only secondary state work leaves the PTY loop. Authentication remains in dispatch.
            std::thread::spawn(move || {
                reply.body = match job.run() {
                    Ok(body) => body,
                    Err(error) => {
                        reply.status = 400;
                        json!({"error": error}).to_string().into_bytes()
                    }
                };
                send_reply(request, reply);
            });
        } else {
            send_reply(request, reply);
        }
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
                if let Some(cookie) = session_cookie {
                    reply.set_cookie = Some(cookie);
                }
                return reply;
            }
        }
        if !path.starts_with("/api/") {
            return Reply::error(404, "not found");
        }
        let is_authorized = header(request, "X-Verb-Token") == Some(self.token.as_str())
            || has_valid_session
            || cf_authenticated;
        if !is_authorized {
            return Reply::error(403, "open the URL printed by verb web");
        }
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
            let mark = crate::good::mark(&self.project)?;
            self.invalidate_checks();
            self.invalidate_state();
            return Ok(Reply::json(
                200,
                json!({
                    "message": format!(
                        "Marked last-known-good at {}.",
                        mark.short_head().unwrap_or("this state")
                    )
                }),
            ));
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
            let session = self.launch(input)?;
            self.invalidate_state();
            return Ok(Reply::json(201, json!({"sessionId": session})));
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
            let signals = if settings.enabled {
                let terminals = self.terminals.lock().unwrap();
                let owned: Vec<_> = terminals
                    .iter()
                    .filter_map(|(id, terminal)| {
                        let hosted = terminal.hosted.as_ref()?;
                        let session = &hosted.session;
                        let is_agent = !matches!(
                            session.agent.as_ref(),
                            None | Some(Agent::Shell) | Some(Agent::Custom(_))
                        );
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
                    .collect();
                drop(terminals);
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
                crate::observer::evaluate(&facts, &settings.muted)
            } else {
                Vec::new()
            };
            return Ok(Reply::json(
                200,
                json!({"enabled": settings.enabled, "muted": settings.muted, "kinds": crate::observer::KINDS, "signals": signals}),
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
                let home = std::env::var_os("HOME")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_default();
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
            return Ok(Reply::json(200, json!(crate::hub::files(&self.project)?)));
        }
        if method == &Method::Get && path == "/api/files/preview" {
            let wanted = url
                .split('?')
                .nth(1)
                .and_then(|query| query.split('&').find_map(|pair| pair.strip_prefix("path=")))
                .map(crate::shell::percent_decode)
                .ok_or("which file?")?;
            return Ok(Reply::json(
                200,
                json!(crate::hub::preview(&self.project, &wanted)?),
            ));
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
                &crate::specs::actor(&self.project, "Verb web"),
            )?;
            self.invalidate_state();
            return Ok(Reply::json(
                201,
                json!({"path": crate::hub::BRIEF_PATH, "message": format!("Created {}", crate::hub::BRIEF_PATH)}),
            ));
        }
        if method == &Method::Post && path == "/api/hub/sync" {
            let changed = crate::hub::sync_agent_files(
                &self.project,
                &crate::specs::actor(&self.project, "Verb web"),
            )?;
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
            return Ok(Reply::json(
                200,
                json!({"specs": crate::specs::list(&self.project), "stages": crate::specs::STAGES}),
            ));
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
                &crate::specs::actor(&self.project, "Verb web"),
            )?;
            self.invalidate_state();
            return Ok(Reply::json(201, json!(spec)));
        }
        if method == &Method::Get && path == "/api/git" {
            return Ok(Reply::json(
                200,
                json!(crate::specs::git_summary(&self.project)?),
            ));
        }
        if method == &Method::Get && path == "/api/git/diff" {
            let wanted = url
                .split('?')
                .nth(1)
                .and_then(|query| query.split('&').find_map(|pair| pair.strip_prefix("path=")))
                .map(crate::shell::percent_decode)
                .ok_or("which file?")?;
            return Ok(Reply::json(
                200,
                json!(crate::diff::file_diff(&self.project, &wanted)?),
            ));
        }
        if method == &Method::Post && path == "/api/git/commit" {
            let input: CommitRequest = read_json(request)?;
            let sha = crate::specs::commit(
                &self.project,
                &input.message,
                input.spec_id.as_deref(),
                &crate::specs::actor(&self.project, "Verb web"),
            )?;
            self.invalidate_state();
            return Ok(Reply::json(
                200,
                json!({"sha": sha, "message": format!("Committed {sha}")}),
            ));
        }
        let parts: Vec<_> = path.trim_start_matches('/').split('/').collect();
        match parts.as_slice() {
            ["api", "specs", id] if method == &Method::Get => Ok(Reply::json(
                200,
                json!(crate::specs::find(&self.project, id)?.1),
            )),
            ["api", "specs", id, "stage"] if method == &Method::Post => {
                let input: SpecStageRequest = read_json(request)?;
                let (spec, warnings) = crate::specs::set_stage(
                    &self.project,
                    id,
                    &input.stage,
                    &input.note,
                    &crate::specs::actor(&self.project, "Verb web"),
                )?;
                self.invalidate_state();
                Ok(Reply::json(
                    200,
                    json!({"spec": spec, "warnings": warnings}),
                ))
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
                    &crate::specs::actor(&self.project, "Verb web"),
                )?;
                self.invalidate_state();
                Ok(Reply::json(200, json!(spec)))
            }
            ["api", "specs", id, "branch"] if method == &Method::Post => {
                let branch = crate::specs::switch_to_branch(
                    &self.project,
                    id,
                    &crate::specs::actor(&self.project, "Verb web"),
                )?;
                self.invalidate_state();
                Ok(Reply::json(
                    200,
                    json!({"branch": branch, "message": format!("Now on {branch}")}),
                ))
            }
            ["api", "specs", id, "agent"] if method == &Method::Post => {
                let input: SpecAgentRequest = read_json(request)?;
                let session = self.start_on_spec(id, &input.agent)?;
                self.invalidate_state();
                Ok(Reply::json(201, json!({"sessionId": session})))
            }
            ["api", "specs", id, "handoff"] if method == &Method::Post => {
                let input: SpecHandoffRequest = read_json(request)?;
                if input.note.len() > 2000 {
                    return Err("keep the handoff note under 2000 characters".to_owned());
                }
                if !SPEC_AGENTS.contains(&input.to.as_str()) {
                    return Err("choose a supported agent".to_owned());
                }
                let note = crate::specs::handoff(
                    &self.project,
                    id,
                    &input.to,
                    &input.note,
                    &crate::specs::actor(&self.project, "Verb web"),
                )?;
                let session = self.start_on_spec(id, &input.to)?;
                self.invalidate_state();
                Ok(Reply::json(
                    201,
                    json!({"sessionId": session, "note": note}),
                ))
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
                self.phone_shares.remove(*id);
                let mut terminal = self
                    .terminals
                    .lock()
                    .unwrap()
                    .remove(*id)
                    .ok_or("terminal is not hosted here")?;
                if let Some(hosted) = terminal.hosted.take() {
                    hosted.stop()?;
                }
                self.invalidate_state();
                Ok(Reply::json(200, json!({"ok": true})))
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

    /// Starts an agent (or a shell) on a spec and records it in the spec's audit trail, which is
    /// how the session board and Ask Verb know who worked on what.
    fn start_on_spec(&mut self, id: &str, agent: &str) -> Result<String, String> {
        let (path, spec) = crate::specs::find(&self.project, id)?;
        // Agents whose CLI takes an opening prompt as its first argument get the spec brief.
        let args = match agent {
            "claude" | "codex" | "gemini" => vec![crate::specs::agent_brief(&spec)],
            "agy" | "opencode" | "shell" => Vec::new(),
            _ => return Err("choose a supported agent".to_owned()),
        };
        let session = self.launch(LaunchRequest {
            agent: agent.to_owned(),
            isolated: false,
            command: None,
            args,
            resume_id: None,
        })?;
        crate::specs::record(
            &path,
            &crate::specs::actor(&self.project, "Verb web"),
            &format!(
                "started {agent} on this spec (session {})",
                &session[..session.len().min(8)]
            ),
        )?;
        Ok(session)
    }

    fn launch(&mut self, input: LaunchRequest) -> Result<String, String> {
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
                "custom" => crate::begin_external_session(
                    &workspace,
                    input.command.ok_or("enter a CLI executable")?,
                    input.args,
                )?,
                _ => return Err("choose a supported agent or a custom CLI".to_owned()),
            }
        };
        let workspace = start.session.project_id.clone();
        if !crate::session_in_project_with_id(&start.session, &self.project, &self.project_id) {
            return Err("selected session belongs to another project".to_owned());
        }
        let sink = crate::stream::TerminalStreamSink::new(start.session.id.clone());
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
        self.terminals
            .lock()
            .unwrap()
            .insert(id.clone(), WebTerminal::new(hosted, sink));
        self.invalidate_state();
        Ok(id)
    }

    fn invalidate_state(&self) {
        self.state_generation.fetch_add(1, Ordering::SeqCst);
    }

    fn workspace(&self) -> Value {
        let terminals = self.terminals.lock().unwrap();
        let sessions = terminals
            .values()
            .filter_map(|terminal| {
                let hosted = terminal.hosted.as_ref()?;
                let session = &hosted.session;
                Some(json!({"id": session.id, "agent": session.display_agent(),
                "state": "live", "isolated": session.project_id != self.identity.anchor,
                "hostedHere": true, "hasTerminal": true, "canResume": false, "attention": 0}))
            })
            .collect::<Vec<_>>();
        json!({"project": {"id": self.project_id, "name": self.identity.anchor.file_name()
            .map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
            "anchor": self.identity.anchor, "workspace": self.project,
            "branch": null, "changedFiles": null},
            "deployment": self.deployment, "sessions": sessions, "tasks": [], "memory": ""})
    }
}

impl StateJob {
    fn run(&self) -> Result<Vec<u8>, String> {
        // Concurrent state requests share work. The PTY thread only uses try_lock.
        let mut cache = self.cache.lock().map_err(|_| "state cache unavailable")?;
        if let Some(cached) = cache.as_ref().filter(|cached| {
            cached.generation == self.generation
                && cached.cached_at.elapsed() < Duration::from_millis(1500)
        }) {
            return Ok(cached.body.clone());
        }
        let body = self.state()?.to_string().into_bytes();
        if self.current_generation.load(Ordering::SeqCst) == self.generation {
            *cache = Some(StateCache {
                body: body.clone(),
                cached_at: Instant::now(),
                generation: self.generation,
            });
        }
        Ok(body)
    }

    fn state(&self) -> Result<Value, String> {
        let identity = &self.identity;
        let git = crate::git_snapshot(&self.project);
        let hosted_ids: Vec<_> = self.hosted.keys().map(String::as_str).collect();
        let sessions = crate::read_sessions_except(&hosted_ids)?;
        let tasks = workbench::snapshots(&self.project)?
            .into_iter()
            .map(|task| {
                json!({
                    "id": task.id, "title": task.title, "brief": task.brief, "status": task.status,
                    "owner": task.owner, "needsHelp": task.needs_help, "createdAt": task.created_at,
                    "events": task.events.into_iter().map(|event| json!({
                        "at": event.at, "kind": event.kind.label(), "sessionId": event.session_id,
                        "note": event.note,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>();
        let mut session_rows = Vec::new();
        for session in sessions {
            if !crate::session_in_project_with_id(&session, &self.project, &identity.id) {
                continue;
            }
            let attention = if session.agent.is_some() {
                tasks
                    .iter()
                    .filter(|task| {
                        task.get("owner").and_then(|o| o.as_str()) == Some(&session.id)
                            && task
                                .get("needsHelp")
                                .and_then(|h| h.as_bool())
                                .unwrap_or(false)
                    })
                    .count()
            } else {
                0
            };
            session_rows.push(json!({
                "id": session.id, "agent": session.display_agent(), "state": session.state.as_str(),
                "workspace": session.project_id, "isolated": session.project_id != identity.anchor,
                "lastSeenAt": session.last_seen_at, "hostedHere": self.hosted.get(&session.id).copied().unwrap_or(false),
                "hasTerminal": self.hosted.contains_key(&session.id),
                "canResume": session.state == SessionState::Recoverable && session.project_id.is_dir(),
                "attention": attention, "contextState": "not applicable",
            }));
        }
        Ok(json!({
            "project": {"id": identity.id, "name": identity.anchor.file_name()
                .map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
                "anchor": identity.anchor, "workspace": self.project,
                "branch": git.branch, "changedFiles": git.changed_files},
            "deployment": self.deployment,
            "memory": workbench::shared_memory(&self.project)?,
            "tasks": tasks, "sessions": session_rows,
        }))
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
    let mut response = Response::from_data(reply.body).with_status_code(StatusCode(reply.status));
    let cache_control = reply.cache_control.unwrap_or("no-store");
    for (name, value) in [
            ("Content-Type", reply.mime),
            ("Cache-Control", cache_control),
            ("X-Content-Type-Options", "nosniff"),
            ("X-Frame-Options", "DENY"),
            ("Referrer-Policy", "no-referrer"),
            ("Cross-Origin-Resource-Policy", "same-origin"),
            // 'wasm-unsafe-eval' lets the terminal's image addon compile its WebAssembly Sixel
            // decoder. It permits WebAssembly compilation only, not JavaScript eval. Without it every
            // terminal threw a CSP error and inline images never worked (caught by browser tests).
            ("Content-Security-Policy", "default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; connect-src 'self'; font-src 'self'; img-src 'self' data:; frame-ancestors 'none'; base-uri 'none'"),
        ] {
            response.add_header(Header::from_bytes(name, value).expect("static HTTP header"));
        }
    if let Some(cookie) = reply.set_cookie {
        if let Ok(header) = Header::from_bytes("Set-Cookie", cookie) {
            response.add_header(header);
        }
    }
    let _ = request.respond(response);
}

fn header<'a>(request: &'a Request, name: &'static str) -> Option<&'a str> {
    request
        .headers()
        .iter()
        .find(|header| header.field.equiv(name))
        .map(|header| header.value.as_str())
}

fn read_json<T: for<'de> Deserialize<'de>>(request: &mut Request) -> Result<T, String> {
    if !header(request, "Content-Type").is_some_and(|value| value.starts_with("application/json")) {
        return Err("send application/json".to_owned());
    }
    if request.body_length().is_none_or(|length| length > MAX_BODY) {
        return Err("request body exceeds limit or has no length".to_owned());
    }
    let mut body = Vec::new();
    request
        .as_reader()
        .take((MAX_BODY + 1) as u64)
        .read_to_end(&mut body)
        .map_err(|error| format!("could not read request: {error}"))?;
    if body.len() > MAX_BODY {
        return Err("request body exceeds limit".to_owned());
    }
    serde_json::from_slice(&body).map_err(|error| format!("invalid request: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

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
