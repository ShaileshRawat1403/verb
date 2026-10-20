//! One bounded boundary entry per read-only project request, rather than one per Git probe.
use serde::de::DeserializeOwned;
use std::io::Read;
use std::path::Path;
use std::process::Stdio;
use std::sync::mpsc;
use std::time::{Duration, Instant};

const MAX_REPLY: usize = 8 * 1024 * 1024;
pub(crate) const TIMEOUT: &str = "Project read timed out. Retry when the host is less busy.";

pub(crate) fn request<T: DeserializeOwned>(
    project: &Path,
    action: &str,
    args: &[&str],
) -> Result<T, String> {
    let mut command = crate::isolation::command(Path::new("/usr/bin/timeout"), project, "")?;
    command
        .args([
            "--kill-after=1s",
            "15s",
            "/usr/local/bin/verb-recovery",
            "__project-read",
            action,
        ])
        .args(args);
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "Could not start the project read.".to_owned())?;
    let (tx, rx) = mpsc::channel();
    let readers: Vec<(bool, Box<dyn Read + Send>, usize)> = vec![
        (
            true,
            Box::new(child.stdout.take().ok_or("project read stdout missing")?),
            MAX_REPLY,
        ),
        (
            false,
            Box::new(child.stderr.take().ok_or("project read stderr missing")?),
            8192,
        ),
    ];
    for (stdout, reader, limit) in readers {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut raw = Vec::new();
            let result = reader
                .take(limit as u64 + 1)
                .read_to_end(&mut raw)
                .map_err(|_| "Could not read the project reply.".to_owned())
                .and_then(|_| {
                    if raw.len() > limit {
                        Err("Project read returned too much data.".to_owned())
                    } else {
                        Ok(raw)
                    }
                });
            let _ = tx.send((stdout, result));
        });
    }
    drop(tx);
    let until = Instant::now() + Duration::from_secs(20);
    let (mut stdout, mut stderr, mut status) = (None, None, None);
    let result = 'read: loop {
        while let Ok((is_stdout, result)) = rx.try_recv() {
            match result {
                Ok(raw) => {
                    if is_stdout {
                        stdout = Some(raw)
                    } else {
                        stderr = Some(raw)
                    }
                }
                Err(error) => break 'read Err(error),
            }
        }
        if status.is_none() {
            match child.try_wait() {
                Ok(value) => status = value,
                Err(_) => break Err("Could not check the project read.".to_owned()),
            }
        }
        if let (Some(status), Some(stdout), Some(stderr)) = (&status, &stdout, &stderr) {
            if !status.success() {
                break Err(if crate::isolation::refused(status.code(), stderr) {
                    "Verb execution unavailable: capacity".to_owned()
                } else if matches!(status.code(), Some(124 | 137)) {
                    TIMEOUT.to_owned()
                } else {
                    String::from_utf8_lossy(stderr).trim().to_owned()
                });
            }
            break serde_json::from_slice(stdout)
                .map_err(|_| "Invalid project read reply.".to_owned());
        }
        if Instant::now() >= until {
            break Err(TIMEOUT.to_owned());
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    // Killing this dedicated boundary client closes its control connection; the controller owns
    // descendant cleanup. Its receipt remains available if cleanup cannot be confirmed.
    if result.is_err() && status.is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

/// Invoked only after entering the guest, before ordinary CLI project discovery can run Git.
pub(crate) fn helper(args: &[String]) -> Result<(), String> {
    if crate::isolation::profile()?.is_some() {
        return Err("project read helper must run inside its execution environment".to_owned());
    }
    let project = std::env::current_dir().map_err(|error| error.to_string())?;
    let value = match args {
        [action] if action == "files" => serde_json::to_vec(&crate::hub::files(&project)?),
        [action, path] if action == "preview" => {
            serde_json::to_vec(&crate::hub::preview(&project, path)?)
        }
        [action] if action == "git" => serde_json::to_vec(&crate::specs::git_summary(&project)?),
        _ => return Err("invalid project read helper invocation".to_owned()),
    }
    .map_err(|error| error.to_string())?;
    if value.len() > MAX_REPLY {
        return Err("Project read returned too much data.".to_owned());
    }
    use std::io::Write;
    std::io::stdout()
        .write_all(&value)
        .map_err(|error| error.to_string())
}
