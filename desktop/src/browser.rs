//! Optional local browser worker. All content is transient; the public/external host never starts it.
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;
use std::time::Instant;

const MAX_REPLY: u64 = 3 * 1024 * 1024;
struct Worker {
    child: Child,
    input: Option<ChildStdin>,
    replies: Receiver<Result<Vec<u8>, String>>,
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.input.take(); // EOF tells the worker to close Chromium and its fresh profile.
        for _ in 0..20 {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        unsafe {
            libc::kill(self.child.id() as i32, libc::SIGTERM);
        }
        for _ in 0..20 {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
#[derive(Default)]
pub(crate) struct Browser {
    worker: Option<Worker>,
}
fn configured_path(name: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(
        std::env::var_os(name).ok_or("Browser runtime is not configured on this host.")?,
    );
    if !path.is_absolute() || !path.is_file() {
        return Err("Browser runtime paths must name existing absolute files.".to_owned());
    }
    Ok(path)
}
impl Browser {
    pub(crate) fn configured() -> bool {
        [
            "VERB_BROWSER_NODE",
            "VERB_BROWSER_WORKER",
            "VERB_BROWSER_EXECUTABLE",
        ]
        .iter()
        .all(|name| configured_path(name).is_ok())
    }
    fn launch(&mut self, origin: &str) -> Result<(), String> {
        let mut child = Command::new(configured_path("VERB_BROWSER_NODE")?)
            .arg(configured_path("VERB_BROWSER_WORKER")?)
            .arg(origin)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env(
                "VERB_BROWSER_EXECUTABLE",
                configured_path("VERB_BROWSER_EXECUTABLE")?,
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "Could not start the browser worker.".to_owned())?;
        let input = child.stdin.take().ok_or("Browser input unavailable.")?;
        let output = child.stdout.take().ok_or("Browser output unavailable.")?;
        let (sender, replies) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                let mut line = Vec::new();
                let result = Read::by_ref(&mut reader)
                    .take(MAX_REPLY + 1)
                    .read_until(b'\n', &mut line);
                let result = match result {
                    Ok(0) => Err(
                        "Browser worker ended. Start a new session; actions are never retried."
                            .to_owned(),
                    ),
                    Ok(_) if line.len() as u64 > MAX_REPLY || !line.ends_with(b"\n") => {
                        Err("Browser reply exceeded its limit.".to_owned())
                    }
                    Ok(_) => Ok(line),
                    Err(_) => Err("Browser output could not be read.".to_owned()),
                };
                let failed = result.is_err();
                if sender.send(result).is_err() || failed {
                    break;
                }
            }
        });
        self.worker = Some(Worker {
            child,
            input: Some(input),
            replies,
        });
        Ok(())
    }
    pub(crate) fn command(
        &mut self,
        origin: &str,
        input: Value,
        cancel: &AtomicBool,
        identity: &std::sync::Mutex<Option<String>>,
    ) -> Result<Value, String> {
        let op = input["op"].as_str().ok_or("Browser operation required.")?;
        if !matches!(op, "start" | "status") {
            let current = identity
                .lock()
                .map_err(|_| "Browser identity is unavailable.")?;
            if current.is_some() && input["session"].as_str() != current.as_deref() {
                return Err("Browser session changed. Reopen it before acting.".to_owned());
            }
        }
        if op == "stop" {
            self.worker = None;
            *identity
                .lock()
                .map_err(|_| "Browser identity is unavailable.")? = None;
            cancel.store(false, Ordering::SeqCst);
            return Ok(json!({"running":false,"generation":0,"tabs":[]}));
        }
        if op == "start" {
            cancel.store(false, Ordering::SeqCst);
        }
        if !Self::configured() {
            return Err("Browser runtime is not configured on this host.".to_owned());
        }
        if self.worker.is_none() {
            if op == "status" || op == "stop" {
                return Ok(json!({"running":false,"generation":0,"tabs":[]}));
            }
            if op != "start" {
                return Err("Start a browser session first.".to_owned());
            }
            self.launch(origin)?;
            *identity
                .lock()
                .map_err(|_| "Browser identity is unavailable.")? = Some(crate::new_id());
        }
        let mut bytes = serde_json::to_vec(&input).map_err(|e| e.to_string())?;
        if bytes.len() > 3500 {
            return Err("Browser command is too large.".to_owned());
        }
        bytes.push(b'\n');
        let worker = self.worker.as_mut().ok_or("Browser is unavailable.")?;
        let outcome = worker.input.as_mut().ok_or("Browser input is closed.")?.write_all(&bytes)
            .map_err(|_| "Browser connection lost; the action outcome is unknown. Do not retry automatically.".to_owned())
            .and_then(|_| {
                let deadline = Instant::now() + Duration::from_secs(25);
                loop {
                    if cancel.load(Ordering::SeqCst) { return Err("Browser stopped by you; the last action outcome may be unknown.".to_owned()); }
                    match worker.replies.recv_timeout(Duration::from_millis(100)) {
                        Ok(result) => return result,
                        Err(mpsc::RecvTimeoutError::Disconnected) => return Err("Browser worker ended; no action was retried.".to_owned()),
                        Err(_) if Instant::now() >= deadline => return Err("Browser timed out; action outcome unknown. Start a fresh session.".to_owned()),
                        Err(_) => {}
                    }
                }
            })
            .and_then(|line| serde_json::from_slice::<Value>(&line).map_err(|_| "Invalid browser response.".to_owned()));
        match outcome {
            Ok(mut view) => {
                if view["running"] == false {
                    self.worker = None;
                    *identity
                        .lock()
                        .map_err(|_| "Browser identity is unavailable.")? = None;
                }
                view["session"] = json!(*identity
                    .lock()
                    .map_err(|_| "Browser identity is unavailable.")?);
                redact_view(&mut view);
                if op == "stop" {
                    self.worker = None;
                }
                Ok(view)
            }
            Err(error) => {
                self.worker = None;
                *identity
                    .lock()
                    .map_err(|_| "Browser identity is unavailable.")? = None;
                Err(error)
            }
        }
    }
}

pub(crate) fn redact_view(view: &mut Value) {
    for key in ["text", "error", "navigationError"] {
        if let Some(text) = view[key].as_str() {
            view[key] = json!(crate::transcript::redact(text));
        }
    }
    if let Some(errors) = view["errors"].as_array_mut() {
        for error in errors {
            if let Some(text) = error.as_str() {
                *error = json!(crate::transcript::redact(text));
            }
        }
    }
}
