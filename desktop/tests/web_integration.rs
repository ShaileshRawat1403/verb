#![cfg(unix)]

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static NEXT_SERVER_ID: AtomicU64 = AtomicU64::new(0);

struct WebServer {
    child: Child,
    port: u16,
    token: String,
    origin: String,
    root: PathBuf,
}

impl WebServer {
    fn start() -> Self {
        let root = std::env::temp_dir().join(format!(
            "verb-web-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_SERVER_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("project")).unwrap();
        fs::create_dir_all(root.join("home")).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_verb"))
            .args(["web", "--port", "0"])
            .current_dir(root.join("project"))
            .env("VERB_STATE_DIR", root.join("state"))
            .env("HOME", root.join("home"))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let url = line.strip_prefix("Verb web UI: ").unwrap().trim();
        let (origin, token) = url.split_once("/#").unwrap();
        let port = origin.rsplit(':').next().unwrap().parse().unwrap();
        Self {
            child,
            port,
            token: token.to_owned(),
            origin: origin.to_owned(),
            root,
        }
    }

    fn request(&self, method: &str, path: &str, body: Option<Value>, auth: bool) -> (u16, Vec<u8>) {
        self.request_with_headers(method, path, body, auth, "")
    }

    fn request_with_headers(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        auth: bool,
        extra: &str,
    ) -> (u16, Vec<u8>) {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(4)))
            .unwrap();
        let payload = body.map(|value| value.to_string()).unwrap_or_default();
        write!(
            stream,
            "{method} {path} HTTP/1.0\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n{}{}{}Content-Length: {}\r\n\r\n{}",
            self.port,
            if auth {
                format!("X-Verb-Token: {}\r\n", self.token)
            } else {
                String::new()
            },
            if payload.is_empty() {
                String::new()
            } else {
                "Content-Type: application/json\r\n".to_owned()
            },
            extra,
            payload.len(),
            payload
        )
        .unwrap();
        stream.flush().unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        let split = response
            .windows(4)
            .position(|bytes| bytes == b"\r\n\r\n")
            .unwrap();
        let header = String::from_utf8_lossy(&response[..split]);
        let status = header.split_whitespace().nth(1).unwrap().parse().unwrap();
        (status, response[split + 4..].to_vec())
    }

    fn json(&self, method: &str, path: &str, body: Option<Value>) -> Value {
        let (status, bytes) = self.request(method, path, body, true);
        assert!(
            (200..300).contains(&status),
            "{status}: {}",
            String::from_utf8_lossy(&bytes)
        );
        serde_json::from_slice(&bytes).unwrap()
    }
}

impl Drop for WebServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn local_web_ui_shares_the_ledger_and_hosts_a_real_terminal() {
    let server = WebServer::start();
    assert!(server.origin.starts_with("http://127.0.0.1:"));
    assert_eq!(server.token.len(), 64);
    assert_eq!(server.request("GET", "/", None, false).0, 200);
    assert_eq!(server.request("GET", "/app.js", None, false).0, 200);
    assert_eq!(server.request("GET", "/api/state", None, false).0, 403);
    assert_eq!(
        server
            .request_with_headers(
                "GET",
                "/api/state",
                None,
                true,
                "Origin: http://evil.invalid\r\n"
            )
            .0,
        403
    );
    let (status, body) = server.request(
        "POST",
        "/api/terminals",
        Some(json!({"agent":"unknown","isolated":true})),
        true,
    );
    assert_eq!(status, 400);
    assert!(String::from_utf8_lossy(&body).contains("choose a supported agent"));
    let (status, body) = server.request(
        "POST",
        "/api/terminals",
        Some(json!({"agent":"custom","command":"  ","isolated":true})),
        true,
    );
    assert_eq!(status, 400);
    assert!(String::from_utf8_lossy(&body).contains("enter a CLI executable"));

    let created = server.json(
        "POST",
        "/api/tasks",
        Some(json!({"title":"Review browser terminal","brief":"Keep the handoff visible"})),
    );
    server.json(
        "POST",
        "/api/memory",
        Some(json!({"note":"Use one shared project ledger."})),
    );
    let state = server.json("GET", "/api/state", None);
    assert_eq!(created["id"], state["tasks"][0]["id"]);
    assert_eq!(state["tasks"][0]["title"], "Review browser terminal");
    assert!(state["memory"]
        .as_str()
        .unwrap()
        .contains("shared project ledger"));
    let task_id = state["tasks"][0]["id"].as_str().unwrap();

    let started = server.json(
        "POST",
        "/api/terminals",
        Some(json!({"agent":"custom","command":"/bin/sh",
            "args":["-c","printf 'web-pty-ok\\n'; sleep 3"],"isolated":false})),
    );
    let session_id = started["sessionId"].as_str().unwrap();
    server.json(
        "POST",
        &format!("/api/tasks/{task_id}/actions"),
        Some(json!({"action":"claim","session_id":session_id})),
    );
    let claimed = server.json("GET", "/api/state", None);
    assert_eq!(claimed["tasks"][0]["status"], "active");
    assert_eq!(claimed["tasks"][0]["owner"], session_id);
    assert_eq!(claimed["sessions"][0]["hostedHere"], true);
    let mut seen_output = false;
    for _ in 0..30 {
        let output = server.json(
            "GET",
            &format!("/api/terminals/{session_id}/output?after=0"),
            None,
        );
        let bytes = BASE64.decode(output["data"].as_str().unwrap()).unwrap();
        if String::from_utf8_lossy(&bytes).contains("web-pty-ok") {
            seen_output = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(80));
    }
    assert!(
        seen_output,
        "the browser terminal did not receive PTY output"
    );
    server.json("DELETE", &format!("/api/terminals/{session_id}"), None);
}

#[test]
fn short_lived_cli_keeps_its_final_output_until_tile_is_closed() {
    let server = WebServer::start();
    let started = server.json(
        "POST",
        "/api/terminals",
        Some(json!({"agent":"custom","command":"/bin/sh",
            "args":["-c","printf 'final-pty-tail\\n'"],"isolated":false})),
    );
    let session_id = started["sessionId"].as_str().unwrap();
    let mut final_output = None;
    for _ in 0..60 {
        let output = server.json(
            "GET",
            &format!("/api/terminals/{session_id}/output?after=0"),
            None,
        );
        if output["running"] == false {
            final_output = Some(BASE64.decode(output["data"].as_str().unwrap()).unwrap());
            break;
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    let final_output = final_output.expect("short CLI must end");
    assert!(String::from_utf8_lossy(&final_output).contains("final-pty-tail"));
    let state = server.json("GET", "/api/state", None);
    assert_eq!(state["sessions"][0]["hasTerminal"], true);
    assert_eq!(state["sessions"][0]["hostedHere"], false);
    server.json("DELETE", &format!("/api/terminals/{session_id}"), None);
    let state = server.json("GET", "/api/state", None);
    assert_eq!(state["sessions"][0]["hasTerminal"], false);
}

#[test]
fn interrupting_web_host_closes_its_live_session_record() {
    let mut server = WebServer::start();
    let started = server.json(
        "POST",
        "/api/terminals",
        Some(json!({"agent":"custom","command":"/bin/sh",
            "args":["-c","sleep 30"],"isolated":false})),
    );
    let session_id = started["sessionId"].as_str().unwrap();
    assert_eq!(
        server.json("GET", "/api/state", None)["sessions"][0]["state"],
        "live"
    );
    assert!(Command::new("kill")
        .args(["-INT", &server.child.id().to_string()])
        .status()
        .unwrap()
        .success());
    let mut exited = false;
    for _ in 0..40 {
        if server.child.try_wait().unwrap().is_some() {
            exited = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(exited, "web host did not handle SIGINT");
    let output = Command::new(env!("CARGO_BIN_EXE_verb"))
        .args(["sessions", "--json"])
        .current_dir(server.root.join("project"))
        .env("VERB_STATE_DIR", server.root.join("state"))
        .output()
        .unwrap();
    assert!(output.status.success());
    let sessions: Value = serde_json::from_slice(&output.stdout).unwrap();
    let record = sessions
        .as_array()
        .unwrap()
        .iter()
        .find(|session| session["sessionId"] == session_id)
        .unwrap();
    assert_eq!(record["state"], "ENDED");
}

#[test]
fn checks_are_served_on_request_and_mark_good_needs_the_token() {
    let server = WebServer::start();
    assert_eq!(server.request("GET", "/api/checks", None, false).0, 403);
    assert_eq!(server.request("POST", "/api/good/mark", None, false).0, 403);
    // Checks are read off the request loop; the first answer may be a placeholder.
    let mut report = server.json("GET", "/api/checks", None);
    for _ in 0..100 {
        if report["pending"] != true {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
        report = server.json("GET", "/api/checks", None);
    }
    assert_eq!(report["schemaVersion"], 1);
    assert_eq!(report["repositoryStatus"], "notRepository");
    // The fixture project is a plain directory: no repository, no declared runtime.
    assert_eq!(report["repository"], Value::Null);
    assert_eq!(report["runtimes"], json!([]));
    assert_eq!(report["clear"], true);
    // Marking needs a Git working tree, and says so rather than recording nothing.
    let (status, body) = server.request("POST", "/api/good/mark", None, true);
    assert_eq!(status, 400);
    assert!(String::from_utf8_lossy(&body).contains("needs a Git working tree"));
}
