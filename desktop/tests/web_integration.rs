#![cfg(unix)]

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{
    ClientConfig, ClientConnection, DigitallySignedStruct, Error as TlsError, SignatureScheme,
    StreamOwned,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static NEXT_SERVER_ID: AtomicU64 = AtomicU64::new(0);

#[path = "support/git_paths.rs"]
mod git_paths;

struct WebServer {
    child: Child,
    port: u16,
    token: String,
    origin: String,
    root: PathBuf,
}

impl WebServer {
    fn start() -> Self {
        Self::start_with_env(&[])
    }

    fn start_with_env(envs: &[(&str, &str)]) -> Self {
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
        Self::start_in_root(root, envs)
    }

    fn start_in_root(root: PathBuf, envs: &[(&str, &str)]) -> Self {
        Self::start_in_root_with_binary(root, envs, Path::new(env!("CARGO_BIN_EXE_verb")))
    }

    fn start_in_root_with_binary(root: PathBuf, envs: &[(&str, &str)], binary: &Path) -> Self {
        let mut cmd = Command::new(binary);
        cmd.args(["web", "--port", "0"])
            .current_dir(root.join("project"))
            .env("VERB_STATE_DIR", root.join("state"))
            .env("HOME", root.join("home"));
        for (k, v) in envs {
            cmd.env(k, v);
        }
        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let url = line.strip_prefix("Verb web UI: ").unwrap().trim();
        let (origin, token) =
            if let Some((origin, _)) = url.split_once(" (configured token in use)") {
                let custom = envs
                    .iter()
                    .find(|(k, _)| *k == "VERB_TOKEN")
                    .map(|(_, v)| *v)
                    .unwrap();
                (origin.trim().to_owned(), custom.to_owned())
            } else {
                let (origin, token) = url.split_once("/#").unwrap();
                (origin.to_owned(), token.to_owned())
            };
        let port = origin.rsplit(':').next().unwrap().parse().unwrap();
        Self {
            child,
            port,
            token,
            origin,
            root,
        }
    }

    fn restart_with_env(mut self, envs: &[(&str, &str)]) -> Self {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let root = self.root.clone();
        std::mem::forget(self);
        Self::start_in_root(root, envs)
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
        let (status, _, body) = self.request_with_headers_full(method, path, body, auth, extra);
        (status, body)
    }

    fn request_with_headers_full(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        auth: bool,
        extra: &str,
    ) -> (u16, String, Vec<u8>) {
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
        let header = String::from_utf8_lossy(&response[..split]).into_owned();
        let status = header.split_whitespace().nth(1).unwrap().parse().unwrap();
        (status, header, response[split + 4..].to_vec())
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

#[derive(Debug)]
struct PinnedDesktop(String);

impl ServerCertVerifier for PinnedDesktop {
    fn verify_server_cert(
        &self,
        cert: &CertificateDer<'_>,
        _: &[CertificateDer<'_>],
        _: &ServerName<'_>,
        _: &[u8],
        _: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        let actual: String = Sha256::digest(cert.as_ref())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        if actual != self.0 {
            return Err(TlsError::General("certificate pin mismatch".into()));
        }
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }
    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &rustls::crypto::ring::default_provider().signature_verification_algorithms,
        )
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

fn phone_request(port: u16, pin: &str, request: Value) -> Result<Value, String> {
    let config = ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(PinnedDesktop(pin.to_owned())))
        .with_no_client_auth();
    let conn = ClientConnection::new(Arc::new(config), ServerName::try_from("localhost").unwrap())
        .map_err(|error| error.to_string())?;
    let tcp = TcpStream::connect(("127.0.0.1", port)).map_err(|error| error.to_string())?;
    tcp.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    tcp.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
    let mut stream = StreamOwned::new(conn, tcp);
    writeln!(stream, "{request}").map_err(|error| error.to_string())?;
    stream.flush().map_err(|error| error.to_string())?;
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|error| error.to_string())?;
    serde_json::from_str(&line).map_err(|error| error.to_string())
}

#[test]
fn phone_controls_exact_live_web_terminal_over_pinned_tls_and_revocation() {
    let server = WebServer::start();
    let started = server.json("POST", "/api/terminals", Some(json!({"agent":"custom",
        "command":"/bin/sh", "args":["-c", "printf 'phone-ready\\n'; read answer; printf 'got:%s\\n' \"$answer\"; sleep 10"],
        "isolated":false})));
    let id = started["sessionId"].as_str().unwrap();
    assert_eq!(
        server
            .request("POST", &format!("/api/terminals/{id}/phone"), None, false)
            .0,
        403
    );
    let shared = server.json("POST", &format!("/api/terminals/{id}/phone"), None);
    assert_eq!(shared["status"]["pairingReady"], true);
    assert_eq!(
        server
            .request(
                "POST",
                &format!("/api/terminals/{id}/phone/renew"),
                None,
                false
            )
            .0,
        403
    );
    let competing_share = Command::new(env!("CARGO_BIN_EXE_verb"))
        .args(["mobile", "share", id])
        .current_dir(server.root.join("project"))
        .env("VERB_STATE_DIR", server.root.join("state"))
        .output()
        .unwrap();
    assert!(!competing_share.status.success());
    assert!(String::from_utf8_lossy(&competing_share.stderr).contains("already shared"));
    let link = shared["links"][0].as_str().unwrap();
    let fields: std::collections::HashMap<_, _> = link
        .trim_start_matches("verb://pair#")
        .split('&')
        .map(|part| part.split_once('=').unwrap())
        .collect();
    let port: u16 = fields["port"].parse().unwrap();
    let pin = fields["pin"];
    let code = fields["code"];
    let denied = phone_request(port, pin, json!({"version":1,"op":"offer","secret":code})).unwrap();
    assert_eq!(denied["ok"], false);
    let paired = phone_request(port, pin, json!({"version":1,"op":"pair","secret":code})).unwrap();
    assert_eq!(paired["ok"], true);
    let token = paired["result"]["deviceToken"].as_str().unwrap();
    assert!(phone_request(
        port,
        &"0".repeat(64),
        json!({"version":1,"op":"snapshot","secret":token})
    )
    .is_err());
    assert_eq!(
        phone_request(port, pin, json!({"version":1,"op":"pair","secret":code})).unwrap()["ok"],
        false
    );
    let paired_status = server.json("GET", &format!("/api/terminals/{id}/phone"), None);
    assert_eq!(paired_status["status"]["pairingReady"], false);
    assert_eq!(paired_status["status"]["paired"], true);
    let renewed = server.json("POST", &format!("/api/terminals/{id}/phone/renew"), None);
    assert_eq!(renewed["status"]["pairingReady"], true);
    let new_link = renewed["links"][0].as_str().unwrap();
    let new_code = new_link.split("&code=").nth(1).unwrap();
    assert_ne!(new_code, code);
    assert_eq!(
        phone_request(
            port,
            pin,
            json!({"version":1,"op":"snapshot","secret":token})
        )
        .unwrap()["ok"],
        true,
        "renewing a link must not interrupt the current phone"
    );
    let replacement = phone_request(
        port,
        pin,
        json!({"version":1,"op":"pair","secret":new_code}),
    )
    .unwrap();
    assert_eq!(replacement["ok"], true);
    assert_eq!(
        phone_request(
            port,
            pin,
            json!({"version":1,"op":"snapshot","secret":token})
        )
        .unwrap()["ok"],
        false,
        "pairing another phone must revoke the earlier device"
    );
    let token = replacement["result"]["deviceToken"].as_str().unwrap();
    let mut ready = false;
    for _ in 0..30 {
        let screen = phone_request(
            port,
            pin,
            json!({"version":1,"op":"snapshot","secret":token}),
        )
        .unwrap();
        let bytes: Vec<u8> = serde_json::from_value(screen["result"]["bytes"].clone()).unwrap();
        if String::from_utf8_lossy(&bytes).contains("phone-ready") {
            ready = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(ready, "phone never saw the desktop PTY screen");
    assert_eq!(
        phone_request(port, pin, json!({"version":1,"op":"take","secret":token})).unwrap()["ok"],
        true
    );
    assert_eq!(
        server
            .request(
                "POST",
                &format!("/api/terminals/{id}/input"),
                Some(json!({"data":"desktop-should-not-type\\n"})),
                true
            )
            .0,
        400
    );
    assert_eq!(
        phone_request(
            port,
            pin,
            json!({"version":1,"op":"input","secret":token,
        "bytes":[104,101,108,108,111,10]})
        )
        .unwrap()["ok"],
        true
    );
    let mut got = false;
    for _ in 0..30 {
        let screen = phone_request(
            port,
            pin,
            json!({"version":1,"op":"snapshot","secret":token}),
        )
        .unwrap();
        let bytes: Vec<u8> = serde_json::from_value(screen["result"]["bytes"].clone()).unwrap();
        if String::from_utf8_lossy(&bytes).contains("got:hello") {
            got = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(got, "phone input did not reach the desktop PTY");
    server.json("POST", &format!("/api/terminals/{id}/control"), None);
    assert_eq!(
        phone_request(
            port,
            pin,
            json!({"version":1,"op":"input","secret":token,"bytes":[120]})
        )
        .unwrap()["ok"],
        false
    );
    server.json("DELETE", &format!("/api/terminals/{id}/phone"), None);
    assert!(phone_request(
        port,
        pin,
        json!({"version":1,"op":"snapshot","secret":token})
    )
    .is_err());
    server.json("DELETE", &format!("/api/terminals/{id}"), None);
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
    assert_eq!(
        state["sessions"][0]["terminal"],
        json!({"running": false, "replayAvailable": true})
    );
    server.json("DELETE", &format!("/api/terminals/{session_id}"), None);
    let state = server.json("GET", "/api/state", None);
    assert_eq!(state["sessions"][0]["hasTerminal"], false);
    assert_eq!(
        state["sessions"][0]["terminal"],
        json!({"running": false, "replayAvailable": false})
    );
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

fn start_shell(server: &WebServer, script: &str) -> String {
    let started = server.json(
        "POST",
        "/api/terminals",
        Some(json!({"agent":"custom","command":"/bin/sh","args":["-c", script],"isolated":false})),
    );
    started["sessionId"].as_str().unwrap().to_owned()
}

fn wait_for_output(server: &WebServer, id: &str, needle: &str) -> String {
    for _ in 0..60 {
        let output = server.json("GET", &format!("/api/terminals/{id}/output?after=0"), None);
        let text =
            String::from_utf8_lossy(&BASE64.decode(output["data"].as_str().unwrap()).unwrap())
                .into_owned();
        if text.contains(needle) {
            return text;
        }
        std::thread::sleep(Duration::from_millis(80));
    }
    panic!("never saw {needle:?}");
}

/// One session must not be able to write into another's terminal. `forkpty` returns the master
/// without close-on-exec, so before the fix every later session inherited every earlier master.
#[cfg(target_os = "linux")]
#[test]
fn a_session_does_not_inherit_another_sessions_terminal() {
    let server = WebServer::start();
    let first = start_shell(&server, "printf 'first-ready\\n'; sleep 20");
    wait_for_output(&server, &first, "first-ready");
    let second = start_shell(
        &server,
        "printf 'masters=%s\\n' \"$(ls -l /proc/$$/fd | grep -c ptmx)\"; sleep 20",
    );
    let text = wait_for_output(&server, &second, "masters=");
    assert!(text.contains("masters=0"), "{text}");
    server.json("DELETE", &format!("/api/terminals/{first}"), None);
    server.json("DELETE", &format!("/api/terminals/{second}"), None);
}

#[test]
fn session_cleanup_endpoints_delete_and_clear_ended_sessions() {
    let server = WebServer::start();
    let id = start_shell(&server, "echo done");
    wait_for_output(&server, &id, "done");
    server.json("DELETE", &format!("/api/terminals/{id}"), None);

    let state_before = server.json("GET", "/api/state", None);
    let session_exists = state_before["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["id"] == id);
    assert!(
        session_exists,
        "session should exist in ledger before clear"
    );

    let clear_result = server.json("POST", "/api/sessions/clear-ended", None);
    assert!(clear_result["cleared"].as_i64().unwrap() >= 1);

    let state_after = server.json("GET", "/api/state", None);
    let session_still_exists = state_after["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["id"] == id);
    assert!(
        !session_still_exists,
        "session should be gone after clear-ended"
    );
}

/// Closing a session stops its whole process group, including a child that ignores hangup.
#[test]
fn closing_a_session_stops_children_that_ignore_hangup() {
    let server = WebServer::start();
    let marker = server.root.join("survived");
    let id = start_shell(
        &server,
        &format!(
            "(trap '' HUP; sleep 1; touch '{}') & printf 'armed\\n'; sleep 30",
            marker.display()
        ),
    );
    wait_for_output(&server, &id, "armed");
    server.json("DELETE", &format!("/api/terminals/{id}"), None);
    std::thread::sleep(Duration::from_millis(1800));
    assert!(!marker.exists(), "a child outlived its closed session");
}

/// A background process a session left behind must not keep the finished session "hosted": the
/// hosted child inherits the session lock on purpose, and so did everything it started.
#[test]
fn a_leftover_background_process_does_not_keep_a_finished_session_live() {
    let server = WebServer::start();
    let id = start_shell(
        &server,
        "nohup sleep 6 >/dev/null 2>&1 & printf 'leaving\\n'",
    );
    for _ in 0..60 {
        let output = server.json("GET", &format!("/api/terminals/{id}/output?after=0"), None);
        if output["running"] == false {
            break;
        }
        std::thread::sleep(Duration::from_millis(80));
    }
    let listing = Command::new(env!("CARGO_BIN_EXE_verb"))
        .args(["sessions", "--json"])
        .current_dir(server.root.join("project"))
        .env("VERB_STATE_DIR", server.root.join("state"))
        .env("HOME", server.root.join("home"))
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&listing.stdout);
    assert!(text.contains(&id), "{text}");
    assert!(
        !text.contains("\"LIVE\""),
        "finished session still live: {text}"
    );
}

/// An unauthenticated request that declares a body it never sends, or one far too large to hold,
/// must neither freeze the host (which also pumps every terminal) nor abort it.
#[test]
fn an_unread_request_body_cannot_freeze_or_crash_the_host() {
    let mut server = WebServer::start();
    let host = format!("127.0.0.1:{}", server.port);
    let mut withheld = TcpStream::connect(&host).unwrap();
    write!(
        withheld,
        "POST /api/state HTTP/1.1\r\nHost: {host}\r\nContent-Length: 5000\r\n\r\n"
    )
    .unwrap();
    let mut huge = TcpStream::connect(&host).unwrap();
    write!(
        huge,
        "POST /api/state HTTP/1.1\r\nHost: {host}\r\nContent-Length: 1000000000000000\r\n\r\n{}",
        "x".repeat(2048)
    )
    .unwrap();
    std::thread::sleep(Duration::from_millis(300));

    // Both attacking sockets are still open. The host must keep answering.
    let started = std::time::Instant::now();
    let state = server.json("GET", "/api/state", None);
    assert!(state["sessions"].is_array());
    assert!(started.elapsed() < Duration::from_secs(2));
    assert!(
        server.child.try_wait().unwrap().is_none(),
        "the web host exited"
    );
    drop(withheld);
    drop(huge);
}

#[test]
fn the_desktop_can_take_input_back_and_it_needs_the_token() {
    let server = WebServer::start();
    let id = start_shell(&server, "printf 'ready\\n'; sleep 20");
    wait_for_output(&server, &id, "ready");
    let path = format!("/api/terminals/{id}/control");
    assert_eq!(server.request("POST", &path, None, false).0, 403);
    let taken = server.json("POST", &path, None);
    assert!(taken["message"]
        .as_str()
        .unwrap()
        .contains("back with this desktop"));
    server.json("DELETE", &format!("/api/terminals/{id}"), None);
}

/// A working directory reported with an encoded newline (OSC 7 `%0a`, which a `cat` of a repository
/// file can emit) must neither corrupt the session store nor move the session to another project.
#[test]
fn a_crafted_working_directory_cannot_corrupt_or_move_a_session() {
    let server = WebServer::start();
    let id = start_shell(
        &server,
        "printf '\\033]7;file:///tmp%%0aproject_id=/elsewhere\\007crafted\\n'; sleep 20",
    );
    wait_for_output(&server, &id, "crafted");
    std::thread::sleep(Duration::from_millis(200));
    let listing = Command::new(env!("CARGO_BIN_EXE_verb"))
        .args(["sessions", "--json"])
        .current_dir(server.root.join("project"))
        .env("VERB_STATE_DIR", server.root.join("state"))
        .env("HOME", server.root.join("home"))
        .output()
        .unwrap();
    assert!(listing.status.success(), "{listing:?}");
    let text = String::from_utf8_lossy(&listing.stdout);
    assert!(text.contains(&id), "{text}");
    assert!(!text.contains("/elsewhere"), "{text}");
    server.json("DELETE", &format!("/api/terminals/{id}"), None);
}

#[test]
fn configured_token_is_used_never_logged_and_persists_across_restarts() {
    let custom_token = "my-configured-persistent-stage2-token-abcdef";
    let server = WebServer::start_with_env(&[("VERB_TOKEN", custom_token)]);
    assert_eq!(server.token, custom_token);

    // Request with configured token succeeds
    let (status, _) = server.request("GET", "/api/state", None, true);
    assert_eq!(status, 200);

    // Request with missing or wrong token is rejected with 403
    let (status, _) = server.request("GET", "/api/state", None, false);
    assert_eq!(status, 403);

    let (status, _) = server.request_with_headers(
        "GET",
        "/api/state",
        None,
        false,
        "X-Verb-Token: wrong-token-xyz\r\n",
    );
    assert_eq!(status, 403);

    // Restart server with same VERB_TOKEN on the same root
    let restarted = server.restart_with_env(&[("VERB_TOKEN", custom_token)]);
    assert_eq!(restarted.token, custom_token);

    // Authenticated request still succeeds with the same token
    let (status, _) = restarted.request("GET", "/api/state", None, true);
    assert_eq!(status, 200);

    // Different token is rejected
    let (status, _) = restarted.request_with_headers(
        "GET",
        "/api/state",
        None,
        false,
        "X-Verb-Token: wrong-token-xyz\r\n",
    );
    assert_eq!(status, 403);
}

#[test]
fn allowed_origin_configuration_permits_exact_origin_and_rejects_others() {
    let allowed = "https://verb.example.com";
    let server = WebServer::start_with_env(&[("VERB_ALLOWED_ORIGIN", allowed)]);

    // Standard localhost origin is accepted (passes origin check, reaches route handler)
    let (status, body) = server.request_with_headers(
        "POST",
        "/api/good/mark",
        None,
        true,
        &format!("Origin: {}\r\n", server.origin),
    );
    assert_eq!(status, 400);
    assert!(String::from_utf8_lossy(&body).contains("needs a Git working tree"));

    // Configured allowed origin is accepted (passes origin check, reaches route handler)
    let (status, body) = server.request_with_headers(
        "POST",
        "/api/good/mark",
        None,
        true,
        &format!("Origin: {allowed}\r\n"),
    );
    assert_eq!(status, 400);
    assert!(String::from_utf8_lossy(&body).contains("needs a Git working tree"));

    // Arbitrary external origin is rejected
    let (status, body) = server.request_with_headers(
        "POST",
        "/api/good/mark",
        None,
        true,
        "Origin: https://attacker.example.com\r\n",
    );
    assert_eq!(status, 403);
    assert!(String::from_utf8_lossy(&body).contains("unexpected origin"));
}

const TEST_RSA_KEY_1: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQDLghPoUCv3kCrB
GB6tp0YbsHMXeaN9SmnccYG9qfwRNipoqWBtvFdv1ljrn4FhICvllehoSDixJjiU
adJBVsiWEW/Y8bBr1TfZEW97VS57QbzBwJ9h52Y6GqgCRfHbZvgakImEaxG91D8w
v5f7vhkL0xr8lXaHlE0kWHWTDyZHfxbg2ZN++c7yOjRBodUpQsqCDlo7LwXjyYWi
FQwWpitTShdYPRCwaUusbtBZVKof327HOgiW3fgYlGgddA7LngVBdLEzt+2m2yII
CM4jdy+tHcmiK1GICHsMX5yeKinL8v/v7u7+VFHI7eYqHx7N3k0CT71JMF7vJekz
cuP3vN1ZAgMBAAECggEAZYlkmlvp2+6DzmzU3aqgea87dUJ89kW69MBzTaiyufmv
BiJAGPBIJeYp3oHqYQXWsQlu+BzUoFpkD3SO8Ye1s95GUlUgQ2USJM0ktMHm25uM
bJVJUGVOZX4oRl4UknXZIxPrcPSk2PQ9hPqK/5E02Of+xnhiN7ogFRrHqtR7sl6L
2E2+TXVKGXp9X93jkHx3xVE2t16HI7kmD9yCcohBZXoQKcyhMA3PcaGlX07jHN+x
sUkV2zJAGutuRGx65ty95hvinGHOBnT+iPbzUe4t71GHfkyiy4R3xfDi37+vPI7r
LUOHviqOxu1UxAsFfSnWuZiuaWfNOG774h/vbgf0owKBgQDzKS6rLvK9StDK2/BM
vWdyiF3yi/KBssO7+dq+Njq5TvDvdZXu+SGjFv+ufNxL3+CTveV8YtUfmdmDBd/U
G3liGuoos8CnpD6gGu5oJP7bSDISUizWsif4ZdDCHN4q/7Qnf+eq24ruiv0075Pe
tDIDt5DqtiSUDXtv6MB01W+AXwKBgQDWQOg/Z+narQZH3cY0yLGv19QM+ND6yOmu
06O7gNwnW1h5PBkcoSalqfcZrh6utilnAV6ramspRkAoEz1g4TH74AOr1Z3ysdme
H37J7vjqe2sAzT05tzZVq1EeWt4DHBnk526vRpE22t6vEJ7ptmW6NC7CVg4/9sSF
zUWx+6qdRwKBgD0yiipPIIx/fdjwTaQirxxmMa7PhfMaeKSgl2rz3wewVHcP0vJY
BR00tpjFl/QInk7QpicOALF5WQLewZxyZbRJLdGcm8oVTiWhYYsYdIPfwapWwC4w
nFqp1UZlWYzc2gxu5nFb27V5iYx/F2ofU88XrgNEYCRa2Ewr+fPtm6hlAoGBAL1d
eOgxq9t++gIi3cBhccr9c4pTkEFXulKu9BQRfIO8lKHyoC9Rr5rUcnXcE3pPvqAv
8cCHulcspB/HgYRTBZ6dDCGgGI4c6z56j9FiydZVZum6fNa6O+fUF0pA/eC5wZkz
g/ye3lIheJg6lHn0oEzHOlzBOq8GKAQqveLlkJKZAoGBAL/PALyzuyzujjv7a7uM
zcSGX2PhZmvcuhsmTErGvaTASRxvdheKXGJ5JXgZRzOjhTTB9vaBqWorQro/Rt4o
qT50E4hkFLofUQW2/aAUbPn6BDKquz0atAv4buMTKINV3OLZdXL3QEBH+HCOWb6X
h9gaT99jEGfI2GE0v/r6Bz6s
-----END PRIVATE KEY-----"#;

const TEST_RSA_KEY_2: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQDChxA1oFRsgWwi
DGbqFJT6gkHIBwkgb4zZH8jke8uHIywFX+NH58pA9G8+AiD1T4pE2HbDJ9wUr+nX
Dvabg5uzVIk0Sxd0L9MdXQW82I1Afulc5ACS0qQh6lRRCnAXienDIamyAbGWI1iT
VOxdLUVPCEfwmStUo68aqxgp4tdxjDM83wwRHZyjY3s9qBmrDD4s62raS6757UyI
B79peoYdNlArtgp+sDc0mxh2GNaGMICYxdRwC0PkfrzdnyEuqZl7wJt88+LwMpCy
09bgGu/sBxDmbAPPBA/pqW4EOcKLP7XMjcf+eO8mFhSD8tfrNJ9WPSGnAB1ftEFl
KNPOvNDjAgMBAAECggEAEeNOo8fHC6VJFsZyLkNPQcv5lZXECpYHay3nkM8rc5VR
5nqfUUzoxdlUY2zZsAUs71DCdwayz7ovdCW9mqZbCn4TEdp34SjGrpQPw4JcVtp0
xiR3QwkYq7+7GiquDRQTCW0OiD5soKRGcGHmTFkt3ushhmfnWqSkpPynv65K5nd3
bt1W0+3NWjEXpha9PRbX5qUJ9qh9ec49poXZbe5Vg4yQ5iuRo2ynJeVqfwMhsmV4
pZM2Z9jSalP2HvPFGbLtI4vjDbHW7ae/B43OKLHNhoIed4BYnkhapwYeT7HUAgxu
rzMOt5rH0XCi7k0sAy6j8MoSW3IB3Bze59PrHAzv7QKBgQDktT5UH0AbkSeK8wfV
m6GEvSc2fLEIhm/kXhT2c1JG57SYSCKg2lOtXaFHzmvcbDOXRumvGRplq9a7MIOV
qs+YIfrBDrhV9L+zKdmNxJjw9aLoM/PFig0uCu5F/pxke/NxB/JDcCfz1xNKlyBV
sy2pFqln3MwO2MxH3foEO8fQdwKBgQDZvaZL3d+S3sXq2LO1FUEYZiXAywLIKrw8
OrTU0pHfl/hCIINtbEbZjLLQ3tRNvdf+ynw1LoidAnVPWlIQih25ST9WzkhEmR+e
rLA23NpSYMuN1avp9ti5lXbmRbWtlfWEOeHCrLXPGY2NVNPwiwvHLz5LTRZJhni4
p3QDCerp9QKBgHglLkUK1aalrlw0J51zUHpm076v6mBMH2OceO6uzj4pYpnM60QM
7YBZe2w5aDg3LzL9Ma2mRlO63ecgKT/qp3uH/i6FCRk+paX9CiiLarzKjXXmNN1F
FH9nhpyGkKnI464xOndq59IU3jGFCpt6sTXujbfeKeRyx33JgpnOvb0pAoGAYG0j
Ww+79g/f+DvVgckS1dpOt81vwvNh/w5EjMdfwHRNhgNeELRVv/wWKHe172O2ZuiH
Dwo3h8jR6L1oAFkaBrcQbMHXsUFahmuVcgZmTPr+yiYpBujBW5Z8XEfcyC3T16XG
e+7+aOO5EzDQ1wLMyX37iV9vEkqR5byKnNnkhY0CgYAYwXdR2Q71VcKRjYTu5H4l
m1NiJuOulYqm/ymoCyo/ijkCUl2HmmmoKFFa1qdmdJjtUwfENRBPdyIXUKEVWiUP
kFv+X++ntdoZuvw5kY+70G5wt+li6GT9aSKRyAVw6rfjpy59lAwDMIl0gd11Bwiy
caEvnZEU/fMoftOaF1BrOA==
-----END PRIVATE KEY-----"#;

const TEST_JWKS_MULTI: &str = r#"{"keys":[
    {"kty":"RSA","alg":"RS256","use":"sig","kid":"test-pilot-key-1","n":"y4IT6FAr95AqwRgeradGG7BzF3mjfUpp3HGBvan8ETYqaKlgbbxXb9ZY65-BYSAr5ZXoaEg4sSY4lGnSQVbIlhFv2PGwa9U32RFve1Uue0G8wcCfYedmOhqoAkXx22b4GpCJhGsRvdQ_ML-X-74ZC9Ma_JV2h5RNJFh1kw8mR38W4NmTfvnO8jo0QaHVKULKgg5aOy8F48mFohUMFqYrU0oXWD0QsGlLrG7QWVSqH99uxzoIlt34GJRoHXQOy54FQXSxM7ftptsiCAjOI3cvrR3JoitRiAh7DF-cniopy_L_7-7u_lRRyO3mKh8ezd5NAk-9STBe7yXpM3Lj97zdWQ","e":"AQAB"},
    {"kty":"RSA","alg":"RS256","use":"sig","kid":"test-pilot-key-2","n":"wocQNaBUbIFsIgxm6hSU-oJByAcJIG-M2R_I5HvLhyMsBV_jR-fKQPRvPgIg9U-KRNh2wyfcFK_p1w72m4Obs1SJNEsXdC_THV0FvNiNQH7pXOQAktKkIepUUQpwF4npwyGpsgGxliNYk1TsXS1FTwhH8JkrVKOvGqsYKeLXcYwzPN8MER2co2N7PagZqww-LOtq2kuu-e1MiAe_aXqGHTZQK7YKfrA3NJsYdhjWhjCAmMXUcAtD5H683Z8hLqmZe8CbfPPi8DKQstPW4Brv7AcQ5mwDzwQP6aluBDnCiz-1zI3H_njvJhYUg_LX6zSfVj0hpwAdX7RBZSjTzrzQ4w","e":"AQAB"}
]}"#;

fn sign_test_jwt(key_pem: &str, kid: &str, iss: &str, aud: &str, email: &str, exp: u64) -> String {
    let encoding_key = jsonwebtoken::EncodingKey::from_rsa_pem(key_pem.as_bytes()).unwrap();
    let header = jsonwebtoken::Header {
        alg: jsonwebtoken::Algorithm::RS256,
        kid: Some(kid.to_owned()),
        ..Default::default()
    };
    let claims = serde_json::json!({
        "aud": [aud],
        "email": email,
        "iss": iss,
        "exp": exp,
        "sub": "user-123",
        "type": "app"
    });
    jsonwebtoken::encode(&header, &claims, &encoding_key).unwrap()
}

#[test]
fn cf_access_assertion_cryptographic_verification_and_negative_matrix() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let allowed_email = "shailesh.rawat1403@gmail.com";
    let cf_iss = "https://shailesh1403.cloudflareaccess.com";
    let cf_aud = "da8bed97949a85e5d95856a5178b5afa1fd0cb7f76c94a295f90fdb624e56420";
    let server = WebServer::start_with_env(&[
        ("VERB_ALLOWED_EMAIL", allowed_email),
        ("VERB_CF_ACCESS_ISS", cf_iss),
        ("VERB_CF_ACCESS_AUD", cf_aud),
        ("VERB_CF_JWKS_JSON", TEST_JWKS_MULTI),
    ]);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let valid_jwt = sign_test_jwt(
        TEST_RSA_KEY_1,
        "test-pilot-key-1",
        cf_iss,
        cf_aud,
        allowed_email,
        now + 3600,
    );

    // 1. Genuinely signed RS256 token succeeds and sets HttpOnly, SameSite=Strict session cookie
    let extra_headers = format!(
        "Cf-Access-Authenticated-User-Email: {allowed_email}\r\nCf-Access-Jwt-Assertion: {valid_jwt}\r\n"
    );
    let (status, headers, body) =
        server.request_with_headers_full("GET", "/", None, false, &extra_headers);
    assert_eq!(status, 200);
    assert!(String::from_utf8_lossy(&body).contains("<!doctype html>"));
    assert!(
        headers.contains("Set-Cookie: verb_session="),
        "Headers: {headers}"
    );
    assert!(headers.contains("HttpOnly"));
    assert!(headers.contains("SameSite=Strict"));

    let cookie_val = headers
        .lines()
        .find(|l| l.to_lowercase().starts_with("set-cookie:"))
        .and_then(|l| l.split("verb_session=").nth(1))
        .and_then(|l| l.split(';').next())
        .expect("verb_session cookie found");

    // 2. Subsequent request using ONLY the issued session cookie succeeds without tokens or CF headers
    let cookie_header = format!("Cookie: verb_session={cookie_val}\r\n");
    let (status, _) = server.request_with_headers("GET", "/api/state", None, false, &cookie_header);
    assert_eq!(status, 200);

    // NEGATIVE TEST 1: Valid-looking claims + fake signature MUST return 403
    let parts: Vec<&str> = valid_jwt.split('.').collect();
    let fake_sig_jwt = format!("{}.{}.fake_unverified_signature_bytes", parts[0], parts[1]);
    let fake_sig_headers = format!(
        "Cf-Access-Authenticated-User-Email: {allowed_email}\r\nCf-Access-Jwt-Assertion: {fake_sig_jwt}\r\n"
    );
    let (status, _) =
        server.request_with_headers("GET", "/api/state", None, false, &fake_sig_headers);
    assert_eq!(status, 403, "Fake signature must be rejected");

    // NEGATIVE TEST 2: Modified payload with original signature MUST return 403
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    let tampered_payload = URL_SAFE_NO_PAD.encode(
        serde_json::json!({
            "aud": [cf_aud],
            "email": "tampered@example.com",
            "iss": cf_iss,
            "exp": now + 3600,
        })
        .to_string(),
    );
    let tampered_jwt = format!("{}.{}.{}", parts[0], tampered_payload, parts[2]);
    let tampered_headers = format!(
        "Cf-Access-Authenticated-User-Email: {allowed_email}\r\nCf-Access-Jwt-Assertion: {tampered_jwt}\r\n"
    );
    let (status, _) =
        server.request_with_headers("GET", "/api/state", None, false, &tampered_headers);
    assert_eq!(
        status, 403,
        "Tampered payload with original signature must be rejected"
    );

    // NEGATIVE TEST 3: Wrong kid (not in JWKS) MUST return 403
    let wrong_kid_jwt = sign_test_jwt(
        TEST_RSA_KEY_1,
        "unknown-kid-xyz",
        cf_iss,
        cf_aud,
        allowed_email,
        now + 3600,
    );
    let wrong_kid_headers = format!(
        "Cf-Access-Authenticated-User-Email: {allowed_email}\r\nCf-Access-Jwt-Assertion: {wrong_kid_jwt}\r\n"
    );
    let (status, _) =
        server.request_with_headers("GET", "/api/state", None, false, &wrong_kid_headers);
    assert_eq!(status, 403, "Unknown key id must be rejected");

    // NEGATIVE TEST 4: Wrong issuer in signed token MUST return 403
    let wrong_iss_jwt = sign_test_jwt(
        TEST_RSA_KEY_1,
        "test-pilot-key-1",
        "https://attacker.example.com",
        cf_aud,
        allowed_email,
        now + 3600,
    );
    let wrong_iss_headers = format!(
        "Cf-Access-Authenticated-User-Email: {allowed_email}\r\nCf-Access-Jwt-Assertion: {wrong_iss_jwt}\r\n"
    );
    let (status, _) =
        server.request_with_headers("GET", "/api/state", None, false, &wrong_iss_headers);
    assert_eq!(status, 403, "Wrong issuer must be rejected");

    // NEGATIVE TEST 5: Wrong audience in signed token MUST return 403
    let wrong_aud_jwt = sign_test_jwt(
        TEST_RSA_KEY_1,
        "test-pilot-key-1",
        cf_iss,
        "wrong-audience-uuid",
        allowed_email,
        now + 3600,
    );
    let wrong_aud_headers = format!(
        "Cf-Access-Authenticated-User-Email: {allowed_email}\r\nCf-Access-Jwt-Assertion: {wrong_aud_jwt}\r\n"
    );
    let (status, _) =
        server.request_with_headers("GET", "/api/state", None, false, &wrong_aud_headers);
    assert_eq!(status, 403, "Wrong audience must be rejected");

    // NEGATIVE TEST 6: Wrong email in signed token payload MUST return 403
    let wrong_email_jwt = sign_test_jwt(
        TEST_RSA_KEY_1,
        "test-pilot-key-1",
        cf_iss,
        cf_aud,
        "intruder@example.com",
        now + 3600,
    );
    let wrong_email_headers = format!(
        "Cf-Access-Authenticated-User-Email: {allowed_email}\r\nCf-Access-Jwt-Assertion: {wrong_email_jwt}\r\n"
    );
    let (status, _) =
        server.request_with_headers("GET", "/api/state", None, false, &wrong_email_headers);
    assert_eq!(status, 403, "Wrong email in claims must be rejected");

    // NEGATIVE TEST 7: Mismatched email header MUST return 403
    let bad_email_hdr = format!(
        "Cf-Access-Authenticated-User-Email: intruder@example.com\r\nCf-Access-Jwt-Assertion: {valid_jwt}\r\n"
    );
    let (status, _) = server.request_with_headers("GET", "/api/state", None, false, &bad_email_hdr);
    assert_eq!(status, 403, "Mismatched email header must be rejected");

    // NEGATIVE TEST 8: Expired token MUST return 403
    let expired_jwt = sign_test_jwt(
        TEST_RSA_KEY_1,
        "test-pilot-key-1",
        cf_iss,
        cf_aud,
        allowed_email,
        now - 3600,
    );
    let expired_headers = format!(
        "Cf-Access-Authenticated-User-Email: {allowed_email}\r\nCf-Access-Jwt-Assertion: {expired_jwt}\r\n"
    );
    let (status, _) =
        server.request_with_headers("GET", "/api/state", None, false, &expired_headers);
    assert_eq!(status, 403, "Expired token must be rejected");

    // NEGATIVE TEST 9: Missing assertion headers MUST return 403
    let (status, _) = server.request("GET", "/api/state", None, false);
    assert_eq!(status, 403, "Missing assertion must be rejected");
}

#[test]
fn cf_access_signing_key_rotation_supports_multiple_keys() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let allowed_email = "shailesh.rawat1403@gmail.com";
    let cf_iss = "https://shailesh1403.cloudflareaccess.com";
    let cf_aud = "da8bed97949a85e5d95856a5178b5afa1fd0cb7f76c94a295f90fdb624e56420";
    let server = WebServer::start_with_env(&[
        ("VERB_ALLOWED_EMAIL", allowed_email),
        ("VERB_CF_ACCESS_ISS", cf_iss),
        ("VERB_CF_ACCESS_AUD", cf_aud),
        ("VERB_CF_JWKS_JSON", TEST_JWKS_MULTI),
    ]);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Key 1 (Current Key) signs valid JWT -> Accepted (200)
    let jwt_key_1 = sign_test_jwt(
        TEST_RSA_KEY_1,
        "test-pilot-key-1",
        cf_iss,
        cf_aud,
        allowed_email,
        now + 3600,
    );
    let headers_1 = format!(
        "Cf-Access-Authenticated-User-Email: {allowed_email}\r\nCf-Access-Jwt-Assertion: {jwt_key_1}\r\n"
    );
    let (status, _) = server.request_with_headers("GET", "/api/state", None, false, &headers_1);
    assert_eq!(status, 200, "Key 1 must be accepted");

    // Key 2 (Rotated Key) signs valid JWT -> Accepted (200)
    let jwt_key_2 = sign_test_jwt(
        TEST_RSA_KEY_2,
        "test-pilot-key-2",
        cf_iss,
        cf_aud,
        allowed_email,
        now + 3600,
    );
    let headers_2 = format!(
        "Cf-Access-Authenticated-User-Email: {allowed_email}\r\nCf-Access-Jwt-Assertion: {jwt_key_2}\r\n"
    );
    let (status, _) = server.request_with_headers("GET", "/api/state", None, false, &headers_2);
    assert_eq!(status, 200, "Rotated Key 2 must be accepted");

    // Retired / Non-existent Key 3 signs JWT -> Rejected (403)
    let jwt_key_3 = sign_test_jwt(
        TEST_RSA_KEY_1,
        "test-pilot-key-retired-3",
        cf_iss,
        cf_aud,
        allowed_email,
        now + 3600,
    );
    let headers_3 = format!(
        "Cf-Access-Authenticated-User-Email: {allowed_email}\r\nCf-Access-Jwt-Assertion: {jwt_key_3}\r\n"
    );
    let (status, _) = server.request_with_headers("GET", "/api/state", None, false, &headers_3);
    assert_eq!(status, 403, "Retired key must be rejected");
}

#[test]
fn cf_access_live_published_jwks_parsing_verification() {
    // Tests that Cloudflare Access's published certs endpoint is accessible and
    // parses into valid JWKS keys that construct DecodingKeys cleanly.
    let url = "https://shailesh1403.cloudflareaccess.com/cdn-cgi/access/certs";
    if let Ok(resp) = ureq::get(url)
        .timeout(std::time::Duration::from_secs(5))
        .call()
    {
        if let Ok(body) = resp.into_string() {
            let jwks: jsonwebtoken::jwk::JwkSet =
                serde_json::from_str(&body).expect("parse Cloudflare JWKS");
            assert!(
                !jwks.keys.is_empty(),
                "Cloudflare published keys must not be empty"
            );
            for key in &jwks.keys {
                assert_eq!(
                    key.common.key_algorithm,
                    Some(jsonwebtoken::jwk::KeyAlgorithm::RS256)
                );
                let decoding_key = jsonwebtoken::DecodingKey::from_jwk(key);
                assert!(
                    decoding_key.is_ok(),
                    "Must construct decoding key from published JWK"
                );
            }
        }
    }
}

#[test]
fn terminal_mode_launches_receives_pty_commands_and_persists() {
    let server = WebServer::start();

    // 1. Launch a first-class Terminal session (agent: "shell")
    let started = server.json(
        "POST",
        "/api/terminals",
        Some(json!({
            "agent": "shell",
            "isolated": false
        })),
    );
    let session_id = started["sessionId"]
        .as_str()
        .expect("must return sessionId");

    // 2. Verify state includes this session as live shell
    let state = server.json("GET", "/api/state", None);
    let session_entry = state["sessions"]
        .as_array()
        .expect("sessions array")
        .iter()
        .find(|s| s["id"] == session_id)
        .expect("session must be listed in state");
    assert_eq!(session_entry["agent"], "shell");
    assert_eq!(session_entry["state"], "live");
    assert_eq!(session_entry["hostedHere"], true);
    assert_eq!(session_entry["hasTerminal"], true);
    assert_eq!(
        session_entry["terminal"],
        json!({"running": true, "replayAvailable": true})
    );

    // 3. Send a shell command
    let input_res = server.request(
        "POST",
        &format!("/api/terminals/{session_id}/input"),
        Some(json!({"data": "echo 'VERB_TERMINAL_NODE1_TEST'\n"})),
        true,
    );
    assert_eq!(input_res.0, 200);

    // 4. Verify PTY produces the command output
    let mut seen_first = false;
    let mut next_cursor = 0;
    for _ in 0..40 {
        let output = server.json(
            "GET",
            &format!("/api/terminals/{session_id}/output?after={next_cursor}"),
            None,
        );
        let raw = output["data"].as_str().unwrap_or_default();
        next_cursor = output["next"].as_u64().unwrap_or(next_cursor);
        let decoded = BASE64.decode(raw).unwrap_or_default();
        let text = String::from_utf8_lossy(&decoded);
        if text.contains("VERB_TERMINAL_NODE1_TEST") {
            seen_first = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(80));
    }
    assert!(
        seen_first,
        "Terminal session did not receive or echo command"
    );

    // 5. Test persistence: simulate browser disconnect (idle period with no active polling)
    std::thread::sleep(Duration::from_millis(200));

    // Reconnect: verify session is still live in state and accepts new commands
    let state_after = server.json("GET", "/api/state", None);
    let session_after = state_after["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == session_id)
        .expect("session must still be in state after disconnect");
    assert_eq!(session_after["state"], "live");

    // Send second command
    server.request(
        "POST",
        &format!("/api/terminals/{session_id}/input"),
        Some(json!({"data": "printf 'PERSISTENCE_CONFIRMED\\n'\n"})),
        true,
    );

    let mut seen_second = false;
    for _ in 0..40 {
        let output = server.json(
            "GET",
            &format!("/api/terminals/{session_id}/output?after={next_cursor}"),
            None,
        );
        let raw = output["data"].as_str().unwrap_or_default();
        next_cursor = output["next"].as_u64().unwrap_or(next_cursor);
        let decoded = BASE64.decode(raw).unwrap_or_default();
        let text = String::from_utf8_lossy(&decoded);
        if text.contains("PERSISTENCE_CONFIRMED") {
            seen_second = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(80));
    }
    assert!(
        seen_second,
        "Terminal session did not persist or process input after reconnect"
    );

    // Clean up
    server.json("DELETE", &format!("/api/terminals/{session_id}"), None);
}

#[test]
fn agy_agent_launch_accepted_by_web_api() {
    // Verb checks that an agent is installed before starting it; CI runners have no agy.
    let path = path_with_fake_agents(&["agy"]);
    let server = WebServer::start_with_env(&[("PATH", &path)]);
    // Launching agy should be accepted by validation and recorded with agent label agy
    let started = server.json(
        "POST",
        "/api/terminals",
        Some(json!({
            "agent": "agy",
            "isolated": false
        })),
    );
    let session_id = started["sessionId"]
        .as_str()
        .expect("must return sessionId");
    let state = server.json("GET", "/api/state", None);
    let session_entry = state["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == session_id)
        .expect("agy session must be in state");
    assert_eq!(session_entry["agent"], "agy");
    server.json("DELETE", &format!("/api/terminals/{session_id}"), None);
}

#[test]
fn gemini_agent_launch_accepted_by_web_api() {
    let path = path_with_fake_agents(&["gemini"]);
    let server = WebServer::start_with_env(&[("PATH", &path)]);
    // Launching gemini (or mock agent) should be accepted by validation
    let started = server.json(
        "POST",
        "/api/terminals",
        Some(json!({
            "agent": "gemini",
            "isolated": false
        })),
    );
    let session_id = started["sessionId"]
        .as_str()
        .expect("must return sessionId");
    let state = server.json("GET", "/api/state", None);
    let session_entry = state["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == session_id)
        .expect("gemini session must be in state");
    assert_eq!(session_entry["agent"], "gemini");
    server.json("DELETE", &format!("/api/terminals/{session_id}"), None);
}

#[test]
fn cf_access_non_blocking_jwks_reliability_and_offline_responsiveness() {
    let allowed_email = "shailesh.rawat1403@gmail.com";
    let cf_iss = "https://shailesh1403.cloudflareaccess.com";
    let cf_aud = "da8bed97949a85e5d95856a5178b5afa1fd0cb7f76c94a295f90fdb624e56420";
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Start server with:
    // 1. Cached in-memory keys from VERB_CF_JWKS_JSON
    // 2. An UNAVAILABLE / dead external JWKS endpoint (port 1 drops or refuses)
    let server = WebServer::start_with_env(&[
        ("VERB_ALLOWED_EMAIL", allowed_email),
        ("VERB_CF_ACCESS_ISS", cf_iss),
        ("VERB_CF_ACCESS_AUD", cf_aud),
        ("VERB_CF_JWKS_JSON", TEST_JWKS_MULTI),
        ("VERB_CF_CERTS_URL", "http://127.0.0.1:1/unreachable-certs"),
    ]);

    // 1. Unknown kid must fail quickly with 403 without waiting for network timeout
    let unknown_kid_jwt = sign_test_jwt(
        TEST_RSA_KEY_1,
        "non-existent-kid-999",
        cf_iss,
        cf_aud,
        allowed_email,
        now + 3600,
    );
    let unknown_headers = format!(
        "Cf-Access-Authenticated-User-Email: {allowed_email}\r\nCf-Access-Jwt-Assertion: {unknown_kid_jwt}\r\n"
    );

    // Blocking on the dead endpoint would take its full 5 s network timeout. The bounds below sit
    // well under that but leave room for a loaded machine: the parallel suite once pushed an RSA
    // check past a 500 ms bound with nothing blocked at all.
    let fast = Duration::from_millis(2500);
    let start_unknown = std::time::Instant::now();
    let (status, _) =
        server.request_with_headers("GET", "/api/state", None, false, &unknown_headers);
    let duration_unknown = start_unknown.elapsed();

    assert_eq!(status, 403, "Unknown kid must return 403");
    assert!(
        duration_unknown < fast,
        "Unknown kid took {:?} - must fail fast without waiting for network timeout",
        duration_unknown
    );

    // 2. Cached valid authentication still works while endpoint is unavailable
    let valid_jwt = sign_test_jwt(
        TEST_RSA_KEY_1,
        "test-pilot-key-1",
        cf_iss,
        cf_aud,
        allowed_email,
        now + 3600,
    );
    let valid_headers = format!(
        "Cf-Access-Authenticated-User-Email: {allowed_email}\r\nCf-Access-Jwt-Assertion: {valid_jwt}\r\n"
    );

    let start_valid = std::time::Instant::now();
    let (status, _) = server.request_with_headers("GET", "/api/state", None, false, &valid_headers);
    let duration_valid = start_valid.elapsed();

    assert_eq!(status, 200, "Cached valid authentication must succeed");
    assert!(
        duration_valid < fast,
        "Valid request took {:?} - must be fast from in-memory cache",
        duration_valid
    );

    // 3. Root index (/) and terminal polling remain responsive
    let (status, body) = server.request("GET", "/", None, false);
    assert_eq!(status, 200);
    assert!(String::from_utf8_lossy(&body).contains("Verb"));

    // 4. Launch terminal and verify prompt/input/output polling is responsive
    let start_term = std::time::Instant::now();
    let (status, _, body) = server.request_with_headers_full(
        "POST",
        "/api/terminals",
        Some(json!({"agent": "shell", "isolated": false})),
        false,
        &valid_headers,
    );
    assert_eq!(status, 201);
    let launch_json: Value = serde_json::from_slice(&body).unwrap();
    let session_id = launch_json["sessionId"].as_str().unwrap();

    let (status, _) = server.request_with_headers(
        "GET",
        &format!("/api/terminals/{session_id}/output?after=0"),
        None,
        false,
        &valid_headers,
    );
    assert_eq!(status, 200);
    assert!(
        start_term.elapsed() < Duration::from_secs(4),
        "Terminal creation and polling took {:?} - must remain fully responsive",
        start_term.elapsed()
    );

    server.request_with_headers(
        "DELETE",
        &format!("/api/terminals/{session_id}"),
        None,
        false,
        &valid_headers,
    );
}

#[test]
fn secondary_state_work_does_not_block_workspace_or_terminal_io() {
    use std::os::unix::fs::PermissionsExt;
    let tools = std::env::temp_dir().join(format!("verb-slow-git-{}", std::process::id()));
    fs::create_dir(&tools).unwrap();
    let blocked = tools.join("slow");
    let wrapper = tools.join("git");
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\ncase \" $* \" in *' status '*) if test -e '{}'; then sleep 2; fi;; esac\nexec /usr/bin/git \"$@\"\n",
            blocked.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:{}", tools.display(), std::env::var("PATH").unwrap());
    let server = Arc::new(WebServer::start_with_env(&[("PATH", &path)]));
    let id = start_shell(&server, "cat");
    // Delay the actual repository observer. State no longer reacquires the
    // identity registry lock when reading tasks and memory.
    fs::write(&blocked, "fixture").unwrap();
    let reader = Arc::clone(&server);
    let state = std::thread::spawn(move || reader.request("GET", "/api/state", None, true));
    std::thread::sleep(Duration::from_millis(150));
    let started = std::time::Instant::now();
    let workspace = server.json("GET", "/api/workspace", None);
    assert!(workspace["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|session| session["id"] == id));
    server.json(
        "POST",
        &format!("/api/terminals/{id}/input"),
        Some(json!({"data": "responsive\n"})),
    );
    let output = server.json("GET", &format!("/api/terminals/{id}/output?after=0"), None);
    assert!(output["running"].as_bool().unwrap());
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "secondary state blocked PTY I/O"
    );
    assert!(
        !state.is_finished(),
        "fixture must keep secondary state blocked"
    );
    assert_eq!(state.join().unwrap().0, 200);
    fs::remove_dir_all(tools).unwrap();
}

#[test]
fn workspace_is_available_without_reading_history_and_has_configured_deployment() {
    let server = WebServer::start_with_env(&[
        ("VERB_DEPLOYMENT", "remote"),
        ("VERB_NODE_NAME", "Test Node"),
    ]);
    let id = start_shell(&server, "cat");
    // Corrupt unrelated history: the usable workspace must not depend on ledger parsing.
    fs::write(
        server.root.join("state/sessions/s-corrupt.session"),
        "invalid",
    )
    .unwrap();
    let workspace = server.json("GET", "/api/workspace", None);
    assert_eq!(
        workspace["deployment"],
        json!({"name": "Test Node", "mode": "REMOTE"})
    );
    assert_eq!(workspace["sessions"][0]["id"], id);
    assert!(workspace["project"]["branch"].is_null());
    assert_eq!(server.request("GET", "/api/workspace", None, false).0, 403);
}

#[test]
fn project_state_reconciles_only_its_own_history() {
    let server = WebServer::start();
    let ended = start_shell(&server, "echo done");
    wait_for_output(&server, &ended, "done");
    server.json("DELETE", &format!("/api/terminals/{ended}"), None);
    let path_for = |id: &str| {
        let encoded: String = id.bytes().map(|byte| format!("{byte:02x}")).collect();
        server
            .root
            .join(format!("state/sessions/s-{encoded}.session"))
    };
    let original = fs::read_to_string(path_for(&ended)).unwrap();
    let local_id = "11111111111111111111111111111111";
    let foreign_id = "22222222222222222222222222222222";
    let history = |id: &str, foreign: bool| {
        original
            .replace(&ended, id)
            .lines()
            .map(|line| {
                if line.starts_with("state=") {
                    "state=live".to_owned()
                } else if foreign && line.starts_with("verb_project_id=") {
                    format!("verb_project_id={foreign_id}")
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    };
    let foreign_record = history(foreign_id, true);
    fs::write(path_for(foreign_id), &foreign_record).unwrap();
    fs::write(path_for(local_id), history(local_id, false)).unwrap();

    let state = server.json("GET", "/api/state", None);
    let rows = state["sessions"].as_array().unwrap();
    assert!(rows
        .iter()
        .any(|row| row["id"] == local_id && row["state"] == "ended"));
    assert!(!rows.iter().any(|row| row["id"] == foreign_id));
    assert_eq!(
        fs::read_to_string(path_for(foreign_id)).unwrap(),
        foreign_record,
        "opening one project must not reconcile or rewrite another project's history"
    );
    assert!(fs::read_to_string(path_for(local_id))
        .unwrap()
        .contains("state=ended"));
}

#[test]
fn history_cleanup_preserves_active_sessions_and_other_projects() {
    let server = WebServer::start();
    let active = start_shell(&server, "cat");
    assert_eq!(
        server
            .request("DELETE", &format!("/api/sessions/{active}"), None, true)
            .0,
        400
    );
    let ended = start_shell(&server, "echo done");
    wait_for_output(&server, &ended, "done");
    server.json("DELETE", &format!("/api/terminals/{ended}"), None);
    let encoded = |id: &str| {
        id.bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let path = server
        .root
        .join(format!("state/sessions/s-{}.session", encoded(&ended)));
    let original = fs::read_to_string(&path).unwrap();
    let other = "1234567890abcdef1234567890abcdef";
    let record = original
        .replace(&ended, other)
        .lines()
        .map(|line| {
            if line.starts_with("verb_project_id=") {
                format!("verb_project_id={other}")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let other_path = server
        .root
        .join(format!("state/sessions/s-{}.session", encoded(other)));
    fs::write(&other_path, record).unwrap();
    assert_eq!(
        server
            .request("DELETE", &format!("/api/sessions/{other}"), None, true)
            .0,
        400
    );
    let result = server.json("POST", "/api/sessions/clear-ended", None);
    assert_eq!(result["cleared"], 1);
    assert!(other_path.exists());
    assert!(server
        .root
        .join(format!("state/sessions/s-{}.session", encoded(&active)))
        .exists());
}

#[test]
fn the_agent_stream_is_off_until_turned_on_and_shells_have_none() {
    let server = WebServer::start();
    let (status, body) = server.request("GET", "/api/stream", None, true);
    assert_eq!(status, 200);
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["enabled"],
        false
    );

    let (status, body) = server.request(
        "POST",
        "/api/terminals",
        Some(json!({"agent": "shell", "isolated": false})),
        true,
    );
    assert_eq!(status, 201);
    let id = serde_json::from_slice::<Value>(&body).unwrap()["sessionId"]
        .as_str()
        .unwrap()
        .to_owned();

    // The superseded content route is gone, including when the stream is off.
    let (status, _) = server.request("GET", &format!("/api/terminals/{id}/stream"), None, true);
    assert_ne!(status, 200);
    let (status, _) = server.request(
        "GET",
        &format!("/api/terminals/{id}/conversation"),
        None,
        false,
    );
    assert_ne!(status, 200);

    let (_, body) = server.request("POST", "/api/stream", Some(json!({"enabled": true})), true);
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["enabled"],
        true
    );
    // A shell cannot acquire an agent conversation by opting in.
    let (status, _) = server.request(
        "GET",
        &format!("/api/terminals/{id}/conversation"),
        None,
        true,
    );
    assert_ne!(status, 200);

    let (_, body) = server.request("POST", "/api/stream", Some(json!({"enabled": false})), true);
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["enabled"],
        false
    );
    server.request("DELETE", &format!("/api/terminals/{id}"), None, true);
}

/// Access tokens: a read token reads and cannot act, a drive token works under its own name, neither
/// reaches owner-only settings, and a revoked token stops at once.
#[test]
fn access_tokens_are_scoped_named_and_revocable() {
    let server = WebServer::start();
    let project = server.root.join("project");
    for args in [
        vec!["init", "-q", "-b", "main"],
        vec!["config", "user.name", "Owner Person"],
        vec!["config", "user.email", "owner@example.com"],
        vec!["commit", "-q", "--allow-empty", "-m", "init"],
    ] {
        assert!(Command::new("git")
            .arg("-C")
            .arg(&project)
            .args(&args)
            .status()
            .unwrap()
            .success());
    }
    let issue = |name: &str, scope: &str| -> String {
        let out = Command::new(env!("CARGO_BIN_EXE_verb"))
            .args([
                "token",
                "create",
                "--name",
                name,
                "--scope",
                scope,
                "--expires",
                "1h",
            ])
            .current_dir(&project)
            .env("VERB_STATE_DIR", server.root.join("state"))
            .env("HOME", server.root.join("home"))
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().to_owned()
    };
    let call = |token: &str, method: &str, path: &str, body: Option<Value>| {
        server.request_with_headers(
            method,
            path,
            body,
            false,
            &format!("X-Verb-Token: {token}\r\n"),
        )
    };

    let reader = issue("dashboard", "read");
    assert_eq!(call(&reader, "GET", "/api/specs", None).0, 200);
    let (status, body) = call(&reader, "GET", "/api/whoami", None);
    let who: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        (status, who["token"].as_str(), who["scope"].as_str()),
        (200, Some("dashboard"), Some("read"))
    );
    let (status, body) = call(
        &reader,
        "POST",
        "/api/terminals",
        Some(json!({"agent": "shell"})),
    );
    assert_eq!(status, 403, "read cannot open a terminal");
    assert!(String::from_utf8_lossy(&body).contains("has read scope; this needs drive"));

    let agent = issue("claude", "drive");
    let (status, body) = call(
        &agent,
        "POST",
        "/api/specs",
        Some(json!({"title": "Tokens", "problem": "p", "criteria": ["works"]})),
    );
    assert_eq!(status, 201, "{}", String::from_utf8_lossy(&body));
    let spec: Value = serde_json::from_slice(&body).unwrap();
    let actor = spec["audit"][0]["actor"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    assert!(
        actor.starts_with("claude (access token)"),
        "audit names the token: {actor}"
    );
    assert_eq!(
        call(
            &agent,
            "POST",
            "/api/stream",
            Some(json!({"enabled": true}))
        )
        .0,
        403,
        "privacy setting"
    );
    assert_eq!(
        call(&agent, "POST", "/api/tokens", Some(json!({}))).0,
        403,
        "no token mints a token"
    );

    let revoked = Command::new(env!("CARGO_BIN_EXE_verb"))
        .args(["token", "revoke", "claude"])
        .current_dir(&project)
        .env("VERB_STATE_DIR", server.root.join("state"))
        .env("HOME", server.root.join("home"))
        .status()
        .unwrap();
    assert!(revoked.success());
    assert_eq!(
        call(&agent, "GET", "/api/specs", None).0,
        403,
        "revoked at once"
    );
    assert_eq!(
        server.request("GET", "/api/whoami", None, true).0,
        200,
        "the owner is unaffected"
    );
}

/// A `PATH` with stand-in executables for `agents` first, so launches pass Verb's installed check.
fn path_with_fake_agents(agents: &[&str]) -> String {
    let dir = std::env::temp_dir().join(format!(
        "verb-fake-agents-{}-{}",
        std::process::id(),
        NEXT_SERVER_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).unwrap();
    for agent in agents {
        let file = dir.join(agent);
        fs::write(&file, "#!/bin/sh\nsleep 5\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    format!(
        "{}:{}",
        dir.display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

/// An agent that is not installed is refused with a reason, before any terminal starts.
#[test]
fn a_missing_agent_is_refused_before_a_terminal_starts() {
    let server = WebServer::start_with_env(&[("PATH", "/usr/bin:/bin")]);
    let (status, body) = server.request(
        "POST",
        "/api/terminals",
        Some(json!({"agent": "agy", "isolated": false})),
        true,
    );
    assert_eq!(status, 400);
    assert!(
        String::from_utf8_lossy(&body).contains("agy is unavailable in this execution environment")
    );
    let agents: Value =
        serde_json::from_slice(&server.request("GET", "/api/agents", None, true).1).unwrap();
    assert_eq!(agents["installed"]["agy"], json!(false));
}

/// Hosted terminals can get a lower open-file limit (`VERB_CHILD_NOFILE`, automatic under proot,
/// where a high limit made Claude Code hang before drawing anything).
#[cfg(unix)]
#[test]
fn hosted_terminals_get_the_configured_open_file_limit() {
    let server = WebServer::start_with_env(&[("VERB_CHILD_NOFILE", "200")]);
    let out = server.root.join("nofile.txt");
    let started = server.json(
        "POST",
        "/api/terminals",
        Some(json!({"agent": "shell", "isolated": false})),
    );
    let id = started["sessionId"].as_str().unwrap().to_owned();
    std::thread::sleep(Duration::from_millis(800));
    server.json(
        "POST",
        &format!("/api/terminals/{id}/input"),
        Some(json!({"data": format!("echo $(ulimit -Sn) $(ulimit -Hn) > '{}'\n", out.display())})),
    );
    let mut seen = String::new();
    for _ in 0..50 {
        seen = fs::read_to_string(&out).unwrap_or_default();
        if !seen.trim().is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(
        seen.trim(),
        "200 200",
        "soft and hard: Bun raises soft to hard"
    );
}

#[test]
fn talk_agents_do_not_inherit_the_owner_token_or_launcher_identity() {
    use std::os::unix::fs::PermissionsExt;

    let server = WebServer::start();
    let script = server.root.join("agy-fixture");
    fs::write(
        &script,
        "#!/bin/sh\n\
         if [ -n \"${VERB_TOKEN+x}${CLAUDECODE+x}${CLAUDE_CODE_MESSAGING_TOKEN+x}\" ]; then\n\
           echo leaked > environment-check\n\
         elif [ \"$CLAUDE_CONFIG_DIR\" = fixture-config ]; then\n\
           echo clean > environment-check\n\
         fi\n\
         echo '{\"event\":\"result\",\"result\":{\"status\":\"SUCCESS\",\"response\":\"done\"}}'\n",
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let server = server.restart_with_env(&[
        ("VERB_AGY_BIN", script.to_str().unwrap()),
        ("VERB_TOKEN", "synthetic-owner-token"),
        ("CLAUDECODE", "1"),
        ("CLAUDE_CODE_MESSAGING_TOKEN", "synthetic-launcher-token"),
        ("CLAUDE_CONFIG_DIR", "fixture-config"),
    ]);
    let spec = server.json(
        "POST",
        "/api/specs",
        Some(json!({"title": "Talk environment", "problem": "p", "criteria": ["works"]})),
    );
    let (status, body) = server.request(
        "POST",
        &format!("/api/specs/{}/talk", spec["id"].as_str().unwrap()),
        None,
        true,
    );
    assert_eq!(status, 201, "{}", String::from_utf8_lossy(&body));
    let check = server.root.join("project/environment-check");
    for _ in 0..50 {
        if check.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(fs::read_to_string(check).unwrap().trim(), "clean");
}

#[test]
fn existing_terminal_streams_close_on_access_token_revocation_and_expiry() {
    use tungstenite::{handshake::client::generate_key, http::Request, Message};
    for revoked in [true, false] {
        let server = WebServer::start();
        let output = Command::new(env!("CARGO_BIN_EXE_verb"))
            .args([
                "token",
                "create",
                "--name",
                "stream",
                "--scope",
                "drive",
                "--expires",
                "1h",
            ])
            .env("VERB_STATE_DIR", server.root.join("state"))
            .env("HOME", server.root.join("home"))
            .output()
            .unwrap();
        assert!(output.status.success());
        let token = String::from_utf8(output.stdout).unwrap();
        let request = Request::builder()
            .uri(format!(
                "ws://127.0.0.1:{}/api/terminals/ws?token={}",
                server.port,
                token.trim()
            ))
            .header("Host", format!("127.0.0.1:{}", server.port))
            .header("Origin", &server.origin)
            .header("Sec-WebSocket-Key", generate_key())
            .header("Sec-WebSocket-Version", "13")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .body(())
            .unwrap();
        let tcp = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
        tcp.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let (mut ws, _) = tungstenite::client(request, tcp).unwrap();
        assert!(matches!(ws.read().unwrap(), Message::Text(_)));
        if revoked {
            let output = Command::new(env!("CARGO_BIN_EXE_verb"))
                .args(["token", "revoke", "stream"])
                .env("VERB_STATE_DIR", server.root.join("state"))
                .env("HOME", server.root.join("home"))
                .output()
                .unwrap();
            assert!(output.status.success());
        } else {
            let path = server.root.join("state/access-tokens.json");
            let mut tokens: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            tokens[0]["expires"] = json!(0);
            fs::write(path, serde_json::to_vec(&tokens).unwrap()).unwrap();
        }
        match ws.read() {
            Ok(Message::Close(_)) | Err(tungstenite::Error::ConnectionClosed) => {}
            other => panic!("invalidated idle stream did not close: {other:?}"),
        }
        assert_eq!(server.request("GET", "/api/whoami", None, true).0, 200);
    }
}

#[test]
fn host_identifies_running_executable_after_atomic_file_replacement() {
    let root = std::env::temp_dir().join(format!("verb-running-build-{}", std::process::id()));
    fs::create_dir_all(root.join("project")).unwrap();
    fs::create_dir_all(root.join("home")).unwrap();
    let binary = root.join("verb-original");
    fs::copy(env!("CARGO_BIN_EXE_verb"), &binary).unwrap();
    let expected = format!("{:x}", Sha256::digest(fs::read(&binary).unwrap()));
    let server = WebServer::start_in_root_with_binary(root.clone(), &[], &binary);
    let replacement = root.join("replacement");
    fs::write(
        &replacement,
        b"replacement is deliberately a different artifact",
    )
    .unwrap();
    fs::rename(replacement, binary).unwrap();
    let report = server.json("GET", "/api/host", None);
    assert_eq!(report["verb"]["binary_sha256"], expected);
    assert!(report["verb"]["source_revision"]
        .as_str()
        .is_some_and(|s| s.len() == 40 || s == "unknown"));
    assert!(report["verb"].get("source_dirty").is_some());
}

#[test]
fn readiness_is_minimal_without_granting_api_access() {
    let server = WebServer::start();
    let (status, body) = server.request("GET", "/healthz", None, false);
    assert_eq!(status, 200);
    let report: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(report["status"], "serving");
    assert_eq!(report.as_object().unwrap().len(), 4);
    assert_eq!(server.request("GET", "/api/state", None, false).0, 403);
}

#[test]
fn stalled_authenticated_request_body_has_a_deadline() {
    let server = WebServer::start();
    let mut slow = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    write!(slow,"POST /api/tasks HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nOrigin: {}\r\nX-Verb-Token: {}\r\nContent-Type: application/json\r\nContent-Length: 60\r\n\r\n{{",server.port,server.origin,server.token).unwrap();
    slow.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let started = std::time::Instant::now();
    let mut response = String::new();
    BufReader::new(&mut slow).read_line(&mut response).unwrap();
    assert!(
        response.contains("400"),
        "partial body was not rejected: {response}"
    );
    assert_eq!(server.request("GET", "/healthz", None, false).0, 200);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "stalled body held the controller indefinitely"
    );
}

#[test]
fn oversized_http_header_is_closed_before_api_dispatch() {
    let server = WebServer::start();
    let mut stream = TcpStream::connect(("127.0.0.1", server.port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let request = format!(
        "GET /api/state HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nX-Large: {}\r\n\r\n",
        server.port,
        "x".repeat(10_000)
    );
    let _ = stream.write_all(request.as_bytes());
    let mut response = [0u8; 64];
    match stream.read(&mut response) {
        Ok(0) => {}
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
            ) => {}
        other => panic!("oversized header connection was not closed: {other:?}"),
    }
    assert_eq!(server.request("GET", "/healthz", None, false).0, 200);
}

/// Talk to terminal, end to end with a stand-in agy and no provider: the talk's first reply gives a
/// conversation id, and continuing it in a terminal starts `agy --conversation <id>` and records the
/// start on the spec exactly once.
#[test]
fn talk_continues_in_a_terminal_and_records_the_start_once() {
    use std::os::unix::fs::PermissionsExt;

    let server = WebServer::start();
    let bin = server.root.join("fake-bin");
    fs::create_dir_all(&bin).unwrap();
    let agy = bin.join("agy");
    let launched = server.root.join("terminal-args");
    fs::write(
        &agy,
        format!(
            "#!/bin/sh\n\
             if [ \"$1\" = --conversation ]; then echo \"$@\" > '{}'; exec sleep 30; fi\n\
             echo '{{\"event\":\"init\",\"conversation_id\":\"synthetic-conversation\"}}'\n\
             echo '{{\"event\":\"result\",\"result\":{{\"status\":\"SUCCESS\",\"response\":\"planned\"}}}}'\n",
            launched.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&agy, fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let server = server.restart_with_env(&[
        ("VERB_AGY_BIN", agy.to_str().unwrap()),
        ("PATH", path.as_str()),
    ]);
    let spec = server.json(
        "POST",
        "/api/specs",
        Some(json!({"title": "Talk to terminal", "problem": "p", "criteria": ["works"]})),
    );
    let spec = spec["id"].as_str().unwrap().to_owned();
    let (status, body) = server.request("POST", &format!("/api/specs/{spec}/talk"), None, true);
    assert_eq!(status, 201, "{}", String::from_utf8_lossy(&body));
    let talk: Value = serde_json::from_slice(&body).unwrap();
    let talk = talk["talkId"].as_str().unwrap().to_owned();
    // Before the first reply there is no conversation to continue.
    let mut conversation = Value::Null;
    for _ in 0..50 {
        conversation =
            server.json("GET", &format!("/api/talk/{talk}"), None)["conversation_id"].clone();
        if conversation.is_string() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(conversation, "synthetic-conversation");
    let (status, body) = server.request("POST", &format!("/api/talk/{talk}/terminal"), None, true);
    assert_eq!(status, 201, "{}", String::from_utf8_lossy(&body));
    let reply: Value = serde_json::from_slice(&body).unwrap();
    assert!(reply.get("warning").is_none(), "{reply}");
    let session = reply["sessionId"].as_str().unwrap().to_owned();
    for _ in 0..50 {
        // The file exists as soon as the shell opens it; wait for the arguments themselves.
        if fs::read_to_string(&launched).is_ok_and(|args| !args.trim().is_empty()) {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(
        fs::read_to_string(&launched).unwrap().trim(),
        "--conversation synthetic-conversation"
    );
    let audit = fs::read_dir(server.root.join("project/specs"))
        .unwrap()
        .map(|entry| fs::read_to_string(entry.unwrap().path()).unwrap())
        .collect::<String>();
    assert_eq!(
        audit
            .matches(&format!(
                "started agy on this spec (session {}) to continue the talk",
                &session[..8]
            ))
            .count(),
        1,
        "{audit}"
    );
    let (status, _) = server.request("DELETE", &format!("/api/terminals/{session}"), None, true);
    assert_eq!(status, 200);
}

#[test]
fn a_rejected_body_retires_keep_alive_without_draining_or_poisoning_another_request() {
    let server = WebServer::start();
    let host = format!("127.0.0.1:{}", server.port);
    let stream = TcpStream::connect(&host).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut reader = BufReader::new(stream);
    let body = r#"{"op":"start"}"#;
    write!(reader.get_mut(), "POST /api/browser HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
    let mut status = String::new();
    reader.read_line(&mut status).unwrap();
    assert!(status.contains("403"), "{status}");
    let mut closing = false;
    let mut length = None;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" {
            break;
        }
        if line.to_ascii_lowercase().starts_with("connection:") {
            closing = line.trim().eq_ignore_ascii_case("connection: close");
        }
        if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
            length = value.trim().parse::<usize>().ok();
        }
    }
    assert!(
        closing,
        "a body-bearing refusal must advertise connection closure"
    );
    let mut data = vec![0; length.unwrap()];
    reader.read_exact(&mut data).unwrap();
    let mut next = [0; 1];
    match reader.read(&mut next) {
        Ok(0) => {}
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
            ) => {}
        result => panic!("rejected connection was not closed: {result:?}"),
    }
    assert_eq!(server.request("GET", "/api/state", None, true).0, 200);
}

#[test]
fn approved_agent_handoff_survives_host_interruption() {
    use std::os::unix::fs::PermissionsExt;

    let server = WebServer::start();
    let bin = server.root.join("handoff-bin");
    fs::create_dir(&bin).unwrap();
    let fixture = server.root.join("project");
    fs::write(fixture.join("unapproved-private-file"), "unapproved").unwrap();
    for (agent, script) in [
        ("claude", "printf 'reported-defect\\n' > issue.txt"),
        (
            "codex",
            "grep -qx reported-defect issue.txt && printf 'verified\\n' > review.txt",
        ),
        (
            "agy",
            "grep -qx verified review.txt && printf 'continued\\n' > successor.txt",
        ),
    ] {
        let path = bin.join(agent);
        let capture = server.root.join(format!("{agent}-args"));
        fs::write(
            &path,
            format!(
                "#!/bin/sh\nset -e\nprintf '%s\\n' \"$@\" > '{}'\n{script}\nexec sleep 30\n",
                capture.display(),
            ),
        )
        .unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let env = [("PATH", path.as_str())];
    let server = server.restart_with_env(&env);
    let spec = server.json(
        "POST",
        "/api/specs",
        Some(json!({
            "title": "Handoff fixture", "problem": "unapproved-problem-marker",
            "criteria": ["The successor can continue from checked evidence"]
        })),
    );
    let endpoint = format!("/api/specs/{}", spec["id"].as_str().unwrap());
    let start = server.json(
        "POST",
        &format!("{endpoint}/agent"),
        Some(json!({
            "agent": "claude", "role": "implement"
        })),
    );
    let wait_for = |name: &str, expected: &str| {
        let path = fixture.join(name);
        for _ in 0..100 {
            if fs::read_to_string(&path).ok().as_deref() == Some(expected) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("{name} did not contain the expected evidence");
    };
    wait_for("issue.txt", "reported-defect\n");
    server.json(
        "DELETE",
        &format!("/api/terminals/{}", start["sessionId"].as_str().unwrap()),
        None,
    );
    let current = server.json("GET", &endpoint, None);
    let handoff = |server: &WebServer, to: &str, revision: &Value, role: &str, note: &str| {
        server.request("POST", &format!("{endpoint}/handoff"), Some(json!({
            "to": to, "note": note,
            "approved": {"revision": revision, "role": role, "scope": "Inspect issue.txt", "checks": ""}
        })), true)
    };
    let (status, body) = handoff(
        &server,
        "codex",
        &current["revision"],
        "review",
        "approved-review-marker",
    );
    assert_eq!(status, 201, "{}", String::from_utf8_lossy(&body));
    wait_for("review.txt", "verified\n");
    let args = fs::read_to_string(server.root.join("codex-args")).unwrap();
    assert!(args.contains("Review independently") && args.contains("approved-review-marker"));
    assert!(args.contains("Work only within the approved scope and checks below"));
    assert!(!args.contains("Inspect the assigned changes"));
    assert!(
        !args.contains("unapproved-problem-marker") && !args.contains("unapproved-private-file")
    );

    let reviewed = server.json("GET", &endpoint, None);
    let server = server.restart_with_env(&env);
    let (status, body) = handoff(&server, "agy", &current["revision"], "implement", "stale");
    assert_eq!(status, 409, "{}", String::from_utf8_lossy(&body));
    assert!(!server.root.join("agy-args").exists());
    let (status, body) = handoff(
        &server,
        "agy",
        &reviewed["revision"],
        "implement",
        "approved-successor-marker",
    );
    assert_eq!(status, 201, "{}", String::from_utf8_lossy(&body));
    wait_for("successor.txt", "continued\n");
    let args = fs::read_to_string(server.root.join("agy-args")).unwrap();
    assert!(args.contains(reviewed["revision"].as_str().unwrap()));
    assert!(args.contains("approved-successor-marker"));
    assert!(
        !args.contains("approved-review-marker") && !args.contains("unapproved-problem-marker")
    );
    let result: Value = serde_json::from_slice(&body).unwrap();
    server.json(
        "DELETE",
        &format!("/api/terminals/{}", result["sessionId"].as_str().unwrap()),
        None,
    );
}

fn assert_git_paths(server: &WebServer) {
    let request = |path: &str| {
        let (status, body) = server.request("GET", path, None, true);
        (status, serde_json::from_slice(&body).unwrap())
    };
    let (status, summary) = request("/api/git");
    assert_eq!(status, 200, "{summary}");
    git_paths::assert_summary(&summary);
    git_paths::assert_previews(request);
}

#[test]
fn local_git_paths_preserve_rename_copy_and_literal_arrow() {
    let server = WebServer::start();
    git_paths::fixture(&server.root.join("project"));
    assert_git_paths(&server);
}

#[cfg(all(unix, feature = "synthetic-boundary"))]
#[test]
fn external_git_paths_preserve_rename_copy_and_literal_arrow() {
    use std::os::unix::fs::{symlink, PermissionsExt};

    let root = std::env::temp_dir().join(format!("verb-external-git-paths-{}", std::process::id()));
    let checkout = root.join("0f1e2d3c4b5a69788796a5b4c3d2e1f0");
    fs::create_dir_all(&checkout).unwrap();
    fs::create_dir(root.join("home")).unwrap();
    let boundary = root.join("boundary");
    fs::create_dir_all(boundary.join("receipts")).unwrap();
    symlink(&checkout, root.join("project")).unwrap();
    git_paths::fixture(&checkout);
    fs::write(boundary.join("fixture-binary"), env!("CARGO_BIN_EXE_verb")).unwrap();
    let client = boundary.join("client");
    fs::write(
        &client,
        r#"#!/bin/sh
[ "$1" = --capabilities ] && exit 2
[ "$5" = --mode ] && [ "$6" = pipes ] && [ "$7" = --program ] || exit 64
program=$8
shift 9
if [ "$program" = git ]; then
    exec /usr/bin/git "$@"
fi
case "$*" in
    *"__project-read git")
        exec env -u VERB_SYNTHETIC_BOUNDARY "$(cat "${0%/*}/fixture-binary")" __project-read git
        ;;
esac
exit 64
"#,
    )
    .unwrap();
    fs::set_permissions(&client, fs::Permissions::from_mode(0o755)).unwrap();
    let server = WebServer::start_in_root(
        root,
        &[
            ("PATH", "/usr/bin:/bin"),
            ("VERB_SYNTHETIC_BOUNDARY", boundary.to_str().unwrap()),
        ],
    );
    assert_git_paths(&server);
}

#[test]
fn work_item_reader_validates_links_and_preserves_legacy_spec_reads() {
    use sha2::{Digest, Sha256};

    let server = WebServer::start();
    let project = server.root.join("project");
    let work = project.join("work");
    let standard = work.join("001-standard");
    let small = work.join("002-small");
    let draft = work.join("003-draft");
    let malformed = work.join("004-malformed");
    for dir in [
        &standard,
        &small,
        &draft,
        &malformed,
        &work.join("001-duplicate"),
    ] {
        fs::create_dir_all(dir).unwrap();
    }
    fs::write(
        standard.join("brd.md"),
        "# Standard\nSize: standard\n## Problem\nProblem\n## Audience\nOwner\n## Goals\n| G1 | Trace work |\n| G1 | Duplicate |\n## Success measures\nObserve links\n## Scope\nProject work\n",
    )
    .unwrap();
    fs::write(
        standard.join("prd.md"),
        "| R1 | G1 | Read items | Open the item |\n| R2 (retired) | G1 | Old rule | |\n| R3 | G9 | Missing goal | |\n| R3 | G9 | Duplicate | |\n",
    )
    .unwrap();
    let small_brd = "Size: small\nProblem and audience: Owner needs a reader\nGoal: G1 · Read a work item\nSuccess and boundary: Read without publishing\n";
    fs::write(small.join("brd.md"), small_brd).unwrap();
    fs::write(draft.join("brd.md"), small_brd).unwrap();
    fs::write(
        small.join("prd.md"),
        (1..=6)
            .map(|id| format!("- R{id} · G1 · Read · Verify"))
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
    fs::write(
        malformed.join("brd.md"),
        "Size: standard\n## Goals\n- G1 · Goal\n",
    )
    .unwrap();
    fs::write(malformed.join("prd.md"), [0xff]).unwrap();
    let specs = project.join("specs");
    fs::create_dir_all(&specs).unwrap();
    let legacy = "---\nid: 010\ntitle: Legacy\nstage: plan\n---\n## Acceptance criteria\n- [ ] unchanged legacy text\n";
    fs::write(specs.join("010-legacy.md"), legacy).unwrap();
    let identity = format!("{:x}", Sha256::digest(b"1\0Stable legacy criterion"));
    let linked = format!(
        "---\nid: 011\ntitle: Linked\nstage: plan\nwork-item: 001-standard\n---\n## Acceptance criteria\n- [ ] New (R1, R2, R9)\n- [ ] Stable legacy criterion\n- [ ] Edited legacy criterion\n- [ ] Cache hits (Redis)\n- [ ] Mixed prose (R1, Redis)\n## Traceability\n- 2 · {identity} · R1\n- 3 · old-identity · R2\n"
    );
    fs::write(specs.join("011-linked.md"), &linked).unwrap();

    let (status, body) = server.request("GET", "/api/work-items", None, false);
    assert_eq!(status, 403, "{}", String::from_utf8_lossy(&body));
    let (status, body) = server.request("GET", "/api/work-items", None, true);
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&body));
    let data: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(data["unlinkedSpecs"], json!(["010"]));
    assert!(data["warnings"]
        .to_string()
        .contains("Duplicate work item number"));
    let items = data["items"].as_array().unwrap();
    let item = items
        .iter()
        .find(|item| item["id"] == "001-standard")
        .unwrap();
    assert_eq!(item["size"], "standard");
    let warnings = item["warnings"].to_string();
    for message in [
        "Duplicate BRD goal G1",
        "Duplicate PRD requirement R3",
        "references missing goal",
        "missing verification",
        "references retired requirement R2",
        "references missing requirement",
        "stale Traceability link",
    ] {
        assert!(warnings.contains(message), "{message}: {warnings}");
    }
    let criteria = &item["specs"][0]["criteria"];
    assert_eq!(criteria[0]["requirements"], json!(["R1", "R2", "R9"]));
    assert_eq!(criteria[1]["stale"], false);
    assert_eq!(criteria[2]["stale"], true);
    assert_eq!(criteria[3]["requirements"], json!([]));
    assert_eq!(criteria[4]["requirements"], json!([]));
    assert!(!warnings.contains("Redis"));
    let small = items.iter().find(|item| item["id"] == "002-small").unwrap();
    assert_eq!(small["goals"]["G1"], false);
    assert_eq!(
        small["warnings"],
        json!(["Small PRD exceeds five requirements or 60 lines"])
    );
    let draft = items.iter().find(|item| item["id"] == "003-draft").unwrap();
    assert!(draft["prd"].is_null());
    assert_eq!(draft["warnings"], json!([]));
    let malformed = items
        .iter()
        .find(|item| item["id"] == "004-malformed")
        .unwrap();
    assert!(malformed["prd"].is_null());
    let warnings = malformed["warnings"].to_string();
    assert!(warnings.contains("prd.md"));
    assert!(warnings.contains("missing content in Problem"));

    let (status, body) = server.request("GET", "/api/specs", None, true);
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&body));
    let data: Value = serde_json::from_slice(&body).unwrap();
    let legacy_spec = data["specs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|spec| spec["id"] == "010")
        .unwrap();
    assert_eq!(legacy_spec["criteria"][0]["text"], "unchanged legacy text");
    assert!(legacy_spec.get("work_item").is_none());
    assert_eq!(
        fs::read_to_string(specs.join("010-legacy.md")).unwrap(),
        legacy
    );
    assert_eq!(
        fs::read_to_string(specs.join("011-linked.md")).unwrap(),
        linked
    );
}

#[test]
fn publishes_and_links_work_items_without_reusing_ids_or_changing_evidence_keys() {
    let server = WebServer::start();
    let project = server.root.join("project");
    let request = |method: &str, path: &str, payload| {
        let (status, body) = server.request(method, path, payload, true);
        (status, serde_json::from_slice::<Value>(&body).unwrap())
    };
    let post = |path, payload| request("POST", path, Some(payload));
    let brd = "Size: small\nProblem and audience: Owner needs a reader\nGoal: G1 · Read work\nSuccess and boundary: Read without publishing";
    let create = || {
        post(
            "/api/work-items",
            json!({
                "title": "Sample work", "document": "brd", "content": brd
            }),
        )
    };
    let (status, first) = create();
    assert_eq!(status, 200, "{first}");
    assert_eq!(first["id"], "001-sample-work");
    fs::remove_dir_all(project.join("work/001-sample-work")).unwrap();
    let (status, second) = create();
    assert_eq!(status, 200, "{second}");
    assert_eq!(second["id"], "002-sample-work");
    let id = second["id"].as_str().unwrap();
    assert_eq!(
        fs::read_dir(server.root.join("state/work"))
            .unwrap()
            .flatten()
            .find_map(|entry| fs::read_to_string(entry.path().join("work-item-next-id")).ok())
            .unwrap(),
        "3\n"
    );

    let (status, prd) = post(
        "/api/work-items",
        json!({
            "id": id, "document": "prd", "content": "- R1 · G1 · Read · Verify\n- R2 (retired) · G1 · Old rule · Old check"
        }),
    );
    assert_eq!(status, 200, "{prd}");
    let (status, updated) = post(
        "/api/work-items",
        json!({
            "id": id, "document": "prd", "expectedRevision": prd["revision"],
            "content": "- R1 · G1 · Read · Verify\n- R2 (retired) · G1 · Old rule · Old check\n- R3 · G9 · Missing goal · Verify"
        }),
    );
    assert_eq!(status, 200, "{updated}");
    assert!(updated["warnings"].to_string().contains("missing goal"));
    let (_, stale) = post(
        "/api/work-items",
        json!({
            "id": id, "document": "prd", "expectedRevision": prd["revision"], "content": "stale"
        }),
    );
    assert!(stale["error"].as_str().unwrap().contains("changed"));
    let raw = fs::read_to_string(project.join("work").join(id).join("prd.md")).unwrap();
    assert_eq!(raw.matches("published prd.md").count(), 2);

    fs::create_dir(project.join("specs")).unwrap();
    let path = project.join("specs/004-legacy.md");
    let criteria = "## Acceptance criteria\n\n- [x] Keep my text\n- [ ] Second criterion\n";
    fs::write(&path, format!("---\nid: 004\ntitle: Legacy\nstage: plan\n---\n{criteria}\n## Audit trail\n\n- date · Owner · existing audit\n")).unwrap();
    let (_, specs) = request("GET", "/api/specs", None);
    let expected = &specs["specs"][0]["revision"];
    let (status, linked) = post(
        "/api/work-items/link",
        json!({
            "workItem": id, "specId": "004", "expectedRevision": expected, "criteria": {"1": ["R1", "R2"]}
        }),
    );
    assert_eq!(status, 200, "{linked}");
    let raw = fs::read_to_string(&path).unwrap();
    assert!(raw.contains(criteria));
    assert!(raw.contains("- date · Owner · existing audit"));
    assert!(raw.contains("links to retired R2"));
    let (_, items) = request("GET", "/api/work-items", None);
    assert_eq!(items["items"][0]["specs"][0]["criteria"][0]["stale"], false);
    assert_eq!(
        items["items"][0]["specs"][0]["criteria"][0]["requirements"],
        json!(["R1", "R2"])
    );
    let current_revision = || {
        let (_, specs) = request("GET", "/api/specs", None);
        specs["specs"][0]["revision"].clone()
    };
    let (status, linked) = post(
        "/api/work-items/link",
        json!({
            "workItem": id, "specId": "004", "expectedRevision": current_revision(),
            "criteria": {"2": ["R1"]}
        }),
    );
    assert_eq!(status, 200, "{linked}");
    let (_, items) = request("GET", "/api/work-items", None);
    let links = &items["items"][0]["specs"][0]["criteria"];
    assert_eq!(links[0]["requirements"], json!(["R1", "R2"]));
    assert_eq!(links[1]["requirements"], json!(["R1"]));

    let (status, removed) = post(
        "/api/work-items/link",
        json!({
            "workItem": id, "specId": "004", "expectedRevision": current_revision(),
            "removeCriteria": [1]
        }),
    );
    assert_eq!(status, 200, "{removed}");
    let (_, items) = request("GET", "/api/work-items", None);
    let links = &items["items"][0]["specs"][0]["criteria"];
    assert_eq!(links[0]["requirements"], json!([]));
    assert_eq!(links[1]["requirements"], json!(["R1"]));
    std::thread::scope(|scope| {
        let one = scope.spawn(create);
        let two = scope.spawn(create);
        let mut ids = [one.join().unwrap(), two.join().unwrap()].map(|(status, body)| {
            assert_eq!(status, 200, "{body}");
            body["id"].as_str().unwrap().to_owned()
        });
        ids.sort();
        assert_eq!(ids, ["003-sample-work", "004-sample-work"]);
    });
}

#[test]
fn shared_cli_projects_only_visible_context_and_checkout_read_uses_no_ledger() {
    let server = WebServer::start();
    let project = server.root.join("project");
    assert!(Command::new("git")
        .current_dir(&project)
        .arg("init")
        .output()
        .unwrap()
        .status
        .success());
    fs::create_dir(project.join("specs")).unwrap();
    fs::write(project.join(".gitignore"), "specs/002-private.md\n").unwrap();
    fs::write(
        project.join("specs/001-visible.md"),
        "---\nid: 001\ntitle: Visible\nstage: plan\nwork-item: 001-visible\n---\n# Visible\n## Acceptance criteria\n- [ ] Works (R1)\n",
    ).unwrap();
    fs::write(
        project.join("specs/002-private.md"),
        "---\nid: 002\ntitle: Private marker\nstage: plan\n---\n# Private marker\n",
    )
    .unwrap();
    let work = project.join("work/001-visible");
    fs::create_dir_all(&work).unwrap();
    fs::write(work.join("brd.md"), "Size: small\nProblem and audience: owner\nGoal: G1 · Target goal\nSuccess and boundary: useful\n").unwrap();
    fs::write(
        work.join("prd.md"),
        "- R1 · G1 · Target requirement · Verify\n- R2 · G1 · Unrelated requirement · Verify\n",
    )
    .unwrap();

    for note in ["First note", "Latest note"] {
        let (status, body) =
            server.request("POST", "/api/memory", Some(json!({"note": note})), true);
        assert_eq!(status, 200, "{}", String::from_utf8_lossy(&body));
    }
    let (status, body) = server.request("POST", "/api/hub/sync", None, true);
    assert_eq!(status, 200, "{}", String::from_utf8_lossy(&body));
    let cli = |args: &[&str], checkout_only: bool| {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_verb"));
        cmd.current_dir(&project)
            .env("VERB_STATE_DIR", server.root.join("state"))
            .arg("shared")
            .args(args);
        if checkout_only {
            cmd.env("VERB_SESSION_ID", "nonexistent-session")
                .env("VERB_PROJECT_ROOT", &project);
        }
        let result = cmd.output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        String::from_utf8(result.stdout).unwrap()
    };
    let shared = cli(&["full"], false);
    assert!(shared.contains("001-visible"));
    assert!(!shared.contains("Private marker"));
    assert!(shared.find("Latest note").unwrap() < shared.find("First note").unwrap());
    let spec = cli(&["spec", "001"], false);
    assert!(spec.contains("Target goal"));
    assert!(spec.contains("Target requirement"));
    assert!(!spec.contains("Unrelated requirement"));
    let checkout = cli(&["read", "--checkout"], true);
    assert!(checkout.contains("Latest note"));
    assert!(!checkout.contains("Private marker"));
    assert!(checkout.len() <= 8192);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        use std::time::Instant;
        let bin = server.root.join("context-bin");
        fs::create_dir(&bin).unwrap();
        let capture = server.root.join("context-prompt");
        let launcher = bin.join("claude");
        fs::write(
            &launcher,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\nexec sleep 30\n",
                capture.display()
            ),
        )
        .unwrap();
        fs::set_permissions(launcher, fs::Permissions::from_mode(0o755)).unwrap();
        let path = format!(
            "{}:{}",
            bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let server = server.restart_with_env(&[("PATH", &path)]);
        let (status, body) = server.request(
            "POST",
            "/api/specs/001/agent",
            Some(json!({"agent": "claude"})),
            true,
        );
        assert_eq!(status, 201, "{}", String::from_utf8_lossy(&body));
        let deadline = Instant::now() + Duration::from_secs(5);
        let prompt = loop {
            if let Ok(text) = fs::read_to_string(&capture) {
                if !text.is_empty() {
                    break text;
                }
            }
            assert!(Instant::now() < deadline, "no captured prompt");
            std::thread::sleep(Duration::from_millis(20));
        };
        assert!(prompt.contains("Target goal"));
        assert!(prompt.contains("Target requirement"));
        assert!(prompt.contains("shared read"));
        assert!(!prompt.contains("Private marker"));
        let launch: Value = serde_json::from_slice(&body).unwrap();
        let session = launch["sessionId"].as_str().unwrap();
        let (status, body) = server.request(
            "POST",
            &format!("/api/hub/sync?session={session}"),
            None,
            true,
        );
        assert_eq!(status, 200, "{}", String::from_utf8_lossy(&body));

        for args in [
            &["add", ".gitignore", "specs", "work"][..],
            &[
                "-c",
                "user.name=Context Fixture",
                "-c",
                "user.email=fixture@example.invalid",
                "commit",
                "-qm",
                "Fixture",
            ],
        ] {
            let result = Command::new("git")
                .current_dir(&project)
                .args(args)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
        let (status, body) = server.request(
            "POST",
            "/api/terminals",
            Some(json!({"agent": "shell", "isolated": true})),
            true,
        );
        assert_eq!(status, 201, "{}", String::from_utf8_lossy(&body));
        let launch: Value = serde_json::from_slice(&body).unwrap();
        let id = launch["sessionId"].as_str().unwrap();
        let (_, body) = server.request("GET", "/api/state", None, true);
        let state: Value = serde_json::from_slice(&body).unwrap();
        let session = state["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|session| session["id"] == id)
            .unwrap();
        let workspace = Path::new(session["workspace"].as_str().unwrap());
        assert!(session["isolated"].as_bool().unwrap());
        assert!(!workspace.join("AGENTS.md").exists());
        let original = fs::read(project.join("AGENTS.md")).unwrap();
        let (status, body) =
            server.request("POST", &format!("/api/hub/sync?session={id}"), None, true);
        assert_eq!(status, 200, "{}", String::from_utf8_lossy(&body));
        assert_eq!(fs::read(project.join("AGENTS.md")).unwrap(), original);
        let published = fs::read_to_string(workspace.join("AGENTS.md")).unwrap();
        assert!(published.contains("Latest note"));
        assert!(!published.contains("Private marker"));
    }
}

#[test]
fn work_item_numbers_are_shared_by_git_checkouts_and_require_the_main_store() {
    let main = WebServer::start();
    let project = main.root.join("project");
    let git = |args: &[&str]| {
        let result = Command::new("git")
            .current_dir(&project)
            .args(args)
            .output()
            .unwrap();
        assert!(result.status.success(), "{:?}", result);
    };
    git(&["init", "-q"]);
    git(&[
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.invalid",
        "commit",
        "--allow-empty",
        "-qm",
        "Start",
    ]);
    fs::create_dir(project.join("work")).unwrap();
    fs::write(project.join("work/.next-id"), "7\n").unwrap();
    let input = json!({"title":"Shared", "document":"brd", "content":"Size: standard"});
    let publish = |server: &WebServer| {
        let (status, body) = server.request("POST", "/api/work-items", Some(input.clone()), true);
        (status, serde_json::from_slice::<Value>(&body).unwrap())
    };
    let (status, first) = publish(&main);
    assert_eq!(status, 200, "{first}");
    assert_eq!(first["id"], "007-shared");

    let root = std::env::temp_dir().join(format!(
        "verb-shared-allocation-{}",
        NEXT_SERVER_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(root.join("home")).unwrap();
    let checkout = root.join("project");
    git(&["worktree", "add", "--detach", checkout.to_str().unwrap()]);
    let isolated = WebServer::start_in_root(root, &[]);
    let (status, refused) = publish(&isolated);
    assert_eq!(status, 400, "{refused}");
    assert!(refused["error"]
        .as_str()
        .unwrap()
        .contains("allocation store"));
    let state = main.root.join("state");
    let isolated = isolated.restart_with_env(&[("VERB_STATE_DIR", state.to_str().unwrap())]);
    let (status, second) = publish(&isolated);
    assert_eq!(status, 200, "{second}");
    assert_eq!(second["id"], "008-shared");
    let (status, third) = publish(&main);
    assert_eq!(status, 200, "{third}");
    assert_eq!(third["id"], "009-shared");
    assert_eq!(
        fs::read_to_string(project.join("work/.next-id")).unwrap(),
        "7\n"
    );
}

#[test]
fn spec_terminal_deliveries_and_notes_keep_the_reviewed_revision() {
    use std::os::unix::fs::PermissionsExt;

    let server = WebServer::start();
    let project = server.root.join("project");
    let git = |args: &[&str]| {
        let result = Command::new("git")
            .current_dir(&project)
            .args(args)
            .output()
            .unwrap();
        assert!(result.status.success(), "{:?}", result);
    };
    git(&["init", "-q"]);
    git(&[
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.invalid",
        "commit",
        "--allow-empty",
        "-qm",
        "Start",
    ]);
    let bin = server.root.join("bin");
    fs::create_dir(&bin).unwrap();
    let capture = server.root.join("prompt");
    let script = format!(
        "#!/bin/sh\nprintf '%s' \"$1\" > '{}'\nprintf '\\033[?2004h'\nexec sleep 30\n",
        capture.display()
    );
    fs::write(bin.join("claude"), &script).unwrap();
    fs::set_permissions(bin.join("claude"), fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(bin.join("codex"), &script).unwrap();
    fs::set_permissions(bin.join("codex"), fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let server = server.restart_with_env(&[("PATH", &path)]);
    let item = project.join("work/001-delivery");
    fs::create_dir_all(&item).unwrap();
    fs::write(item.join("brd.md"), "Size: small\nProblem: Test the route\nGoal: G1 · Verify delivery\nSuccess: Observed context").unwrap();
    fs::write(
        item.join("prd.md"),
        "| R1 | G1 | Include the note | Test the prompt |\n",
    )
    .unwrap();
    let spec = server.json(
        "POST",
        "/api/specs",
        Some(json!({"title":"Delivery fixture","criteria":["Observe the note (R1)"]})),
    );
    let spec_id = spec["id"].as_str().unwrap();
    let route = format!("/api/specs/{spec_id}");
    server.json("POST", "/api/work-items/link", Some(json!({
        "workItem":"001-delivery", "specId":spec_id, "expectedRevision":spec["revision"], "criteria":{"1":["R1"]}
    })));
    let started = server.json(
        "POST",
        &format!("{route}/agent"),
        Some(json!({"agent":"claude","role":"implement"})),
    );
    let id = started["sessionId"].as_str().unwrap();
    let terminal = format!("/api/terminals/{id}");
    let snapshot = server.json("GET", "/api/workspace", None);
    let row = snapshot["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .unwrap();
    assert_eq!(row["specId"], spec_id);
    assert_eq!(row["role"], "implement");
    assert_eq!(row["workItemId"], "001-delivery");
    let delivery = server.json("GET", &format!("{terminal}/delivered-context"), None);
    assert_eq!(delivery["delivery"]["trigger"], "launch");
    assert_eq!(delivery["delivery"]["requirements"], json!(["R1"]));
    assert_eq!(delivery["delivery"]["goals"], json!(["G1"]));
    assert_eq!(
        delivery["delivery"]["bytes"].as_u64().unwrap() as usize,
        delivery["text"].as_str().unwrap().len()
    );
    for _ in 0..100 {
        if fs::read_to_string(&capture)
            .is_ok_and(|text| text.contains(delivery["text"].as_str().unwrap()))
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(fs::read_to_string(&capture)
        .unwrap()
        .contains(delivery["text"].as_str().unwrap()));
    assert!(!project
        .join(format!(
            "delivered-context-{}.txt",
            delivery["delivery"]["hash"].as_str().unwrap()
        ))
        .exists());

    let preview = server.json("GET", &format!("{terminal}/context-preview"), None);
    let resend = |hash: &Value| {
        server.request("POST", &format!("{terminal}/resend-context"), Some(json!({
        "hash": hash, "specRevision": preview["delivery"]["specRevision"], "deliveryCount": preview["deliveryCount"]
    })), true)
    };
    assert_eq!(resend(&json!("different text")).0, 400);
    let mut sent = (400, Vec::new());
    for _ in 0..100 {
        sent = resend(&preview["delivery"]["hash"]);
        if sent.0 == 200 {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(sent.0, 200, "{}", String::from_utf8_lossy(&sent.1));
    assert_eq!(resend(&preview["delivery"]["hash"]).0, 400);
    let resent = server.json("GET", &format!("{terminal}/delivered-context"), None);
    assert_eq!(resent["text"], preview["text"]);
    assert_eq!(resent["delivery"]["trigger"], "resend");
    server.json(
        "POST",
        &format!("{terminal}/binding"),
        Some(json!({"specId":spec_id,"role":"review"})),
    );
    let current = server.json("GET", "/api/workspace", None);
    let bound = current["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .unwrap();
    assert_eq!(bound["role"], "review");
    let encoded: String = id.bytes().map(|byte| format!("{byte:02x}")).collect();
    let record = fs::read_to_string(
        server
            .root
            .join(format!("state/sessions/s-{encoded}.session")),
    )
    .unwrap();
    assert!(record.contains("changed session binding from"));
    assert!(record.contains("resend"));
    assert!(!record.contains("Project context (maintained by Verb)"));

    fs::write(project.join("changed.txt"), "after launch").unwrap();
    server.json("DELETE", &terminal, None);
    let revision = server.json("GET", &route, None)["revision"].clone();
    let note = server.json("GET", &format!("{terminal}/session-note"), None);
    assert!(note["files"]
        .as_str()
        .unwrap()
        .lines()
        .any(|path| path == "changed.txt"));
    let secret = "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ123456";
    let note_body = json!({"outcome":format!("Second agent should verify marker {secret}"),"files":note["files"],"checks":"Manual review"});
    server.json(
        "POST",
        &format!("{terminal}/session-note"),
        Some(note_body.clone()),
    );
    assert_eq!(
        server
            .request(
                "POST",
                &format!("{terminal}/session-note"),
                Some(note_body),
                true
            )
            .0,
        400
    );
    let notes = fs::read_to_string(project.join(format!("specs/sessions/{spec_id}.md"))).unwrap();
    assert!(
        notes.contains("reported, unverified")
            && notes.contains("observed by Verb")
            && notes.contains("[redacted]")
    );
    assert!(!notes.contains(secret));
    assert_eq!(server.json("GET", &route, None)["revision"], revision);
    let listing = server.json("GET", "/api/specs", None);
    assert_eq!(listing["specs"].as_array().unwrap().len(), 1);
    let (status, response) = server.request("POST", &format!("{route}/handoff"), Some(json!({
        "to":"codex", "approved":{"revision":revision,"role":"review","scope":"Review changed.txt","checks":""}
    })), true);
    assert_eq!(status, 201, "{}", String::from_utf8_lossy(&response));
    let next = server.json(
        "POST",
        &format!("{route}/agent"),
        Some(json!({"agent":"claude","role":"investigate"})),
    );
    let next_context = server.json(
        "GET",
        &format!(
            "/api/terminals/{}/delivered-context",
            next["sessionId"].as_str().unwrap()
        ),
        None,
    );
    let text = next_context["text"].as_str().unwrap();
    assert!(
        text.contains("Session notes (newest first)")
            && text.contains("Second agent should verify marker [redacted]")
    );
    assert_eq!(next_context["delivery"]["noteCount"], 1);
    assert!(!text.contains(secret));
    let mut received = false;
    for _ in 0..100 {
        if fs::read_to_string(&capture).is_ok_and(|prompt| prompt.contains(text)) {
            received = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(
        received,
        "The next agent must receive the exact recorded context"
    );
}
