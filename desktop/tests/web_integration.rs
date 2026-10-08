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
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
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
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_verb"));
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
    let allowed = "https://verb.pruningmypothos.com";
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
    let server = WebServer::start();
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
    let server = WebServer::start();
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
    let server = Arc::new(WebServer::start());
    let id = start_shell(&server, "cat");
    // Full state must acquire this registry lock for secondary project work. Hold it to
    // deterministically model a slow repository without wall-clock guesses about Git.
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(server.root.join("state/projects/.lock"))
        .unwrap();
    lock.lock().unwrap();
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
    drop(lock);
    assert_eq!(state.join().unwrap().0, 200);
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
