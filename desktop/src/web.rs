//! A loopback browser host for the same durable project, sessions, and task ledger as the TUI.
//! The browser is a view/controller; this process remains the sole owner of every PTY it starts.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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
    chunks: VecDeque<OutputChunk>,
    base: u64,
    next: u64,
    buffered: usize,
    final_screen: Vec<u8>,
    exit_seen_at: Option<Instant>,
    failure: Option<String>,
}

impl WebTerminal {
    fn new(mut hosted: Hosted) -> Self {
        hosted.capture_web_output();
        Self {
            hosted: Some(hosted),
            chunks: VecDeque::new(),
            base: 0,
            next: 0,
            buffered: 0,
            final_screen: Vec::new(),
            exit_seen_at: None,
            failure: None,
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
        // durable ledger instead, so these volatile notices must not accumulate here.
        let _ = hosted.take_structural();
        for chunk in hosted.take_web_output() {
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

struct WebHost {
    project: PathBuf,
    project_id: String,
    token: String,
    origin: String,
    terminals: HashMap<String, WebTerminal>,
    phone_shares: HashMap<String, PhoneShare>,
    checks: Arc<std::sync::Mutex<ChecksCache>>,
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

struct Reply {
    status: u16,
    mime: &'static str,
    body: Vec<u8>,
}

impl Reply {
    fn json(status: u16, value: Value) -> Self {
        Self {
            status,
            mime: "application/json; charset=utf-8",
            body: value.to_string().into_bytes(),
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
    let token = secure_token()?;
    let identity = project::identity(project)?;
    let mut host = WebHost {
        project: project.to_path_buf(),
        project_id: identity.id,
        token: token.clone(),
        origin: format!("http://127.0.0.1:{}", address.port()),
        terminals: HashMap::new(),
        phone_shares: HashMap::new(),
        checks: Arc::default(),
    };
    let running = Arc::new(AtomicBool::new(true));
    let signal_flag = Arc::clone(&running);
    ctrlc::set_handler(move || signal_flag.store(false, Ordering::SeqCst))
        .map_err(|error| format!("could not install shutdown handler: {error}"))?;
    // One write, and nothing else on stdout afterwards: whoever launched us may read the URL line
    // and close the pipe (`verb web | head -1`, a test harness, a launcher script). A second
    // `println!` would then panic on EPIPE and take every hosted terminal down with it.
    {
        let mut stdout = std::io::stdout().lock();
        stdout
            .write_all(
                format!(
                    "Verb web UI: {}/#{}\nLocal only. Close this process to stop its hosted agent sessions.\n",
                    host.origin, token
                )
                .as_bytes(),
            )
            .and_then(|()| stdout.flush())
            .map_err(|error| format!("could not print web URL: {error}"))?;
    }
    while running.load(Ordering::SeqCst) {
        for terminal in host.terminals.values_mut() {
            if let Err(error) = terminal.poll() {
                eprintln!("Verb stopped one web terminal: {error}");
                terminal.fail(error);
            }
        }
        host.phone_shares.retain(|id, _| {
            host.terminals
                .get(id)
                .is_some_and(|terminal| terminal.hosted.is_some())
        });
        if let Some(request) = server
            .recv_timeout(Duration::from_millis(30))
            .map_err(|error| format!("web server stopped: {error}"))?
        {
            host.respond(request);
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

impl WebHost {
    fn respond(&mut self, mut request: Request) {
        let reply = self.dispatch(&mut request);
        let mut response =
            Response::from_data(reply.body).with_status_code(StatusCode(reply.status));
        for (name, value) in [
            ("Content-Type", reply.mime),
            ("Cache-Control", "no-store"),
            ("X-Content-Type-Options", "nosniff"),
            ("X-Frame-Options", "DENY"),
            ("Referrer-Policy", "no-referrer"),
            ("Cross-Origin-Resource-Policy", "same-origin"),
            ("Content-Security-Policy", "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'; font-src 'self'; img-src 'self' data:; frame-ancestors 'none'; base-uri 'none'"),
        ] {
            response.add_header(Header::from_bytes(name, value).expect("static HTTP header"));
        }
        let _ = request.respond(response);
    }

    fn dispatch(&mut self, request: &mut Request) -> Reply {
        if !request
            .remote_addr()
            .is_some_and(|addr| addr.ip().is_loopback())
        {
            return Reply::error(403, "local browser access only");
        }
        let expected_host = self.origin.trim_start_matches("http://");
        if header(request, "Host") != Some(expected_host) {
            return Reply::error(403, "unexpected host");
        }
        if let Some(origin) = header(request, "Origin") {
            if origin != self.origin {
                return Reply::error(403, "unexpected origin");
            }
        }
        let url = request.url().to_owned();
        let path = url.split('?').next().unwrap_or("");
        if request.method() == &Method::Get {
            match path {
                "/" => {
                    return Reply::asset(
                        "text/html; charset=utf-8",
                        include_bytes!("../web/index.html"),
                    )
                }
                "/app.js" => {
                    return Reply::asset(
                        "text/javascript; charset=utf-8",
                        include_bytes!("../web/dist/app.js"),
                    )
                }
                "/app.css" => {
                    return Reply::asset(
                        "text/css; charset=utf-8",
                        include_bytes!("../web/dist/app.css"),
                    )
                }
                "/favicon.svg" => {
                    return Reply::asset("image/svg+xml", include_bytes!("../web/favicon.svg"))
                }
                _ => {}
            }
        }
        if !path.starts_with("/api/") {
            return Reply::error(404, "not found");
        }
        if header(request, "X-Verb-Token") != Some(self.token.as_str()) {
            return Reply::error(403, "open the URL printed by verb web");
        }
        let method = request.method().clone();
        match self.api(&method, &url, path, request) {
            Ok(reply) => reply,
            Err(error) => Reply::error(400, error),
        }
    }

    fn api(
        &mut self,
        method: &Method,
        url: &str,
        path: &str,
        request: &mut Request,
    ) -> Result<Reply, String> {
        if method == &Method::Get && path == "/api/state" {
            return Ok(Reply::json(200, self.state()?));
        }
        // Not folded into /api/state: it runs runtime `--version` probes, so the page asks for it on
        // load and on demand rather than every few seconds.
        if method == &Method::Get && path == "/api/checks" {
            return Ok(Reply::json(200, self.checks()));
        }
        if method == &Method::Post && path == "/api/good/mark" {
            let mark = crate::good::mark(&self.project)?;
            self.invalidate_checks();
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
            return Ok(Reply::json(200, json!({"message": message})));
        }
        if method == &Method::Post && path == "/api/terminals" {
            let input: LaunchRequest = read_json(request)?;
            let session = self.launch(input)?;
            return Ok(Reply::json(201, json!({"sessionId": session})));
        }
        let parts: Vec<_> = path.trim_start_matches('/').split('/').collect();
        match parts.as_slice() {
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
                let terminal = self
                    .terminals
                    .get(*id)
                    .ok_or("terminal is not hosted here")?;
                Ok(Reply::json(200, terminal.output(after)))
            }
            ["api", "terminals", id, "input"] if method == &Method::Post => {
                let input: InputRequest = read_json(request)?;
                if input.data.len() > 8192 {
                    return Err("terminal input is too long".to_owned());
                }
                let terminal = self
                    .terminals
                    .get_mut(*id)
                    .ok_or("terminal is not hosted here")?;
                let hosted = terminal.hosted.as_mut().ok_or("terminal has ended")?;
                hosted.write(input.data.as_bytes())?;
                Ok(Reply::json(200, json!({"ok": true})))
            }
            // The desktop user takes input back from a paired phone. Only reachable with the page's
            // token, which the hosted program does not have.
            ["api", "terminals", id, "control"] if method == &Method::Post => {
                let terminal = self
                    .terminals
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
                let terminal = self
                    .terminals
                    .get(*id)
                    .ok_or("terminal is not hosted here")?;
                if terminal.hosted.is_none() {
                    return Err("terminal has ended".to_owned());
                }
                if self.phone_shares.contains_key(*id) {
                    return Err("stop the current phone share before creating a new one".to_owned());
                }
                let share = PhoneShare::start(id)?;
                let links = share.links.clone();
                self.phone_shares.insert((*id).to_owned(), share);
                Ok(Reply::json(200, json!({"links": links})))
            }
            ["api", "terminals", id, "phone"] if method == &Method::Get => {
                self.terminals
                    .get(*id)
                    .ok_or("terminal is not hosted here")?;
                let links = self.phone_shares.get(*id).map(|share| share.links.clone());
                Ok(Reply::json(200, json!({"links": links})))
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
                let terminal = self
                    .terminals
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
                    .remove(*id)
                    .ok_or("terminal is not hosted here")?;
                if let Some(hosted) = terminal.hosted.take() {
                    hosted.stop()?;
                }
                Ok(Reply::json(200, json!({"ok": true})))
            }
            _ => Ok(Reply::error(404, "not found")),
        }
    }

    fn launch(&mut self, input: LaunchRequest) -> Result<String, String> {
        if input.args.len() > 32 || input.args.iter().any(|arg| arg.len() > 4096) {
            return Err("too many or oversized agent arguments".to_owned());
        }
        if input.resume_id.is_none()
            && !matches!(
                input.agent.as_str(),
                "claude" | "codex" | "opencode" | "shell" | "custom"
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
                "claude" => crate::begin_session(&workspace, Agent::Claude, input.args),
                "codex" => crate::begin_session(&workspace, Agent::Codex, input.args),
                "opencode" => crate::begin_session(&workspace, Agent::OpenCode, input.args),
                "shell" => crate::begin_session(&workspace, Agent::Shell, input.args),
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
        let hosted = Hosted::start(
            &workspace,
            start.session,
            &start.command,
            &start.args,
            &start.env,
            start.is_new,
            28,
            100,
        )?;
        let id = hosted.session.id.clone();
        self.terminals.insert(id.clone(), WebTerminal::new(hosted));
        Ok(id)
    }

    fn state(&self) -> Result<Value, String> {
        let identity = project::identity(&self.project)?;
        let git = crate::git_snapshot(&self.project);
        let hosted_ids: Vec<_> = self.terminals.keys().map(String::as_str).collect();
        let sessions = crate::read_sessions_except(&hosted_ids)?;
        let mut session_rows = Vec::new();
        for session in sessions {
            if !crate::session_in_project_with_id(&session, &self.project, &identity.id)
                || session.agent.is_none()
            {
                continue;
            }
            let inbox = workbench::inbox_snapshot(&self.project, &session.id)?;
            session_rows.push(json!({
                "id": session.id, "agent": session.display_agent(), "state": session.state.as_str(),
                "workspace": session.project_id, "isolated": session.project_id != identity.anchor,
                "lastSeenAt": session.last_seen_at, "hostedHere": self.terminals.get(&session.id)
                    .is_some_and(|terminal| terminal.hosted.is_some()),
                "hasTerminal": self.terminals.contains_key(&session.id),
                "canResume": session.state == SessionState::Recoverable && session.project_id.is_dir(),
                "attention": inbox.items.len(), "contextState": inbox.context_state,
            }));
        }
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
        Ok(json!({
            "project": {"id": identity.id, "name": identity.anchor.file_name()
                .map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
                "anchor": identity.anchor, "workspace": self.project,
                "branch": git.branch, "changedFiles": git.changed_files},
            "memory": workbench::shared_memory(&self.project)?,
            "tasks": tasks, "sessions": session_rows,
        }))
    }
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
            chunks: VecDeque::new(),
            base: 0,
            next: 0,
            buffered: 0,
            final_screen: b"last screen".to_vec(),
            exit_seen_at: None,
            failure: None,
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
