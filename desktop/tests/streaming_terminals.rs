use base64::Engine;
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tungstenite::handshake::client::generate_key;
use tungstenite::http::Request;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, Message, WebSocket};

static NEXT_SERVER_ID: AtomicU64 = AtomicU64::new(100);

struct WebServer {
    child: Child,
    port: u16,
    token: String,
    origin: String,
    root: PathBuf,
}

impl Drop for WebServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl WebServer {
    fn start() -> Self {
        let root = std::env::temp_dir().join(format!(
            "verb-stream-test-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_SERVER_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("project")).unwrap();
        fs::create_dir_all(root.join("home")).unwrap();

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_verb"));
        cmd.args(["web", "--port", "0"])
            .current_dir(root.join("project"))
            .env("VERB_STATE_DIR", root.join("state"))
            .env("HOME", root.join("home"));

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

    fn get_rss_kb(&self) -> usize {
        let pid = self.child.id();
        let output = Command::new("ps")
            .args(["-o", "rss=", "-p", &pid.to_string()])
            .output()
            .expect("Failed to run ps to check RSS");
        let s = String::from_utf8_lossy(&output.stdout);
        s.trim().parse::<usize>().unwrap_or(0)
    }

    fn post(&self, path: &str, body: Value) -> (u16, Value) {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let payload = body.to_string();
        write!(
            stream,
            "POST {path} HTTP/1.0\r\nHost: 127.0.0.1:{}\r\nOrigin: {}\r\nX-Verb-Token: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            self.port, self.origin, self.token, payload.len(), payload
        )
        .unwrap();

        let mut resp = Vec::new();
        std::io::Read::read_to_end(&mut stream, &mut resp).unwrap();
        let resp_str = String::from_utf8_lossy(&resp);
        let status = resp_str
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse::<u16>().ok())
            .unwrap_or(500);

        let body_str = resp_str.split("\r\n\r\n").nth(1).unwrap_or("");
        let val: Value = serde_json::from_str(body_str).unwrap_or(Value::Null);
        (status, val)
    }

    fn delete(&self, path: &str) -> (u16, Value) {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        write!(
            stream,
            "DELETE {path} HTTP/1.0\r\nHost: 127.0.0.1:{}\r\nOrigin: {}\r\nX-Verb-Token: {}\r\nConnection: close\r\n\r\n",
            self.port, self.origin, self.token
        )
        .unwrap();

        let mut resp = Vec::new();
        std::io::Read::read_to_end(&mut stream, &mut resp).unwrap();
        let resp_str = String::from_utf8_lossy(&resp);
        let status = resp_str
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse::<u16>().ok())
            .unwrap_or(500);

        let body_str = resp_str.split("\r\n\r\n").nth(1).unwrap_or("");
        let val: Value = serde_json::from_str(body_str).unwrap_or(Value::Null);
        (status, val)
    }

    fn connect_ws(&self) -> WebSocket<MaybeTlsStream<TcpStream>> {
        let ws_url = format!(
            "ws://127.0.0.1:{}/api/terminals/ws?token={}",
            self.port, self.token
        );
        let request = Request::builder()
            .uri(&ws_url)
            .header("Host", format!("127.0.0.1:{}", self.port))
            .header("Origin", &self.origin)
            .header("Sec-WebSocket-Key", generate_key())
            .header("Sec-WebSocket-Version", "13")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .body(())
            .unwrap();

        let (mut ws, _) = connect(request).expect("WebSocket connection handshake failed");

        // Verify v1 handshake frame
        let msg = ws.read().expect("Failed to read v1 handshake");
        if let Message::Text(text) = msg {
            let val: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(val["v"], 1, "Expected protocol version 1");
        } else {
            panic!("Expected text handshake frame");
        }

        ws
    }
}

fn encode_input_frame(terminal_id: &str, input: &[u8]) -> Message {
    let id_bytes = terminal_id.as_bytes();
    let mut frame = Vec::with_capacity(1 + id_bytes.len() + input.len());
    frame.push(id_bytes.len() as u8);
    frame.extend_from_slice(id_bytes);
    frame.extend_from_slice(input);
    Message::Binary(frame)
}

fn decode_output_frame(bytes: &[u8]) -> Option<(&str, &[u8])> {
    if bytes.len() < 2 {
        return None;
    }
    let id_len = bytes[0] as usize;
    if bytes.len() < 1 + id_len {
        return None;
    }
    let id = std::str::from_utf8(&bytes[1..1 + id_len]).ok()?;
    let payload = &bytes[1 + id_len..];
    Some((id, payload))
}

#[test]
fn websocket_rejects_unauthorized_and_foreign_origins() {
    let server = WebServer::start();

    // 1. Foreign origin is rejected with 403
    let req_foreign = Request::builder()
        .uri(format!(
            "ws://127.0.0.1:{}/api/terminals/ws?token={}",
            server.port, server.token
        ))
        .header("Host", format!("127.0.0.1:{}", server.port))
        .header("Origin", "http://evil.attacker.com")
        .header("Sec-WebSocket-Key", generate_key())
        .header("Sec-WebSocket-Version", "13")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .body(())
        .unwrap();

    let res_foreign = connect(req_foreign);
    assert!(res_foreign.is_err(), "Foreign origin must be rejected");

    // 2. Missing/invalid token is rejected with 403
    let req_unauth = Request::builder()
        .uri(format!(
            "ws://127.0.0.1:{}/api/terminals/ws?token=invalid_token",
            server.port
        ))
        .header("Host", format!("127.0.0.1:{}", server.port))
        .header("Origin", &server.origin)
        .header("Sec-WebSocket-Key", generate_key())
        .header("Sec-WebSocket-Version", "13")
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .body(())
        .unwrap();

    let res_unauth = connect(req_unauth);
    assert!(res_unauth.is_err(), "Invalid token must be rejected");
}

fn set_timeout(ws: &WebSocket<MaybeTlsStream<TcpStream>>, timeout: Duration) {
    if let MaybeTlsStream::Plain(s) = ws.get_ref() {
        s.set_read_timeout(Some(timeout)).unwrap();
    }
}

#[test]
fn keystroke_to_glyph_latency_under_10ms_median() {
    let server = WebServer::start();

    // Launch an interactive shell terminal
    let (status, res) = server.post(
        "/api/terminals",
        json!({"agent": "shell", "isolated": false, "args": []}),
    );
    assert_eq!(status, 201);
    let session_id = res["sessionId"].as_str().unwrap().to_owned();

    let mut ws = server.connect_ws();

    // Attach to the terminal
    ws.send(Message::Text(
        json!({"type": "attach", "id": session_id, "rows": 24, "cols": 80}).to_string(),
    ))
    .unwrap();

    // Wait for "attached" frame
    let msg = ws.read().unwrap();
    if let Message::Text(text) = msg {
        let val: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(val["type"], "attached");
        assert_eq!(val["id"], session_id);
    } else {
        panic!("Expected attached message");
    }

    // Set read timeout for latency testing
    set_timeout(&ws, Duration::from_secs(2));

    // Warm-up: wait for initial prompt bytes
    let warm_deadline = Instant::now() + Duration::from_millis(500);
    while Instant::now() < warm_deadline {
        if let Ok(Message::Binary(_)) = ws.read() {
            // Drain startup bytes
        }
    }

    // Measure keystroke echo round-trip latency across 100 samples
    let mut latencies: Vec<Duration> = Vec::with_capacity(100);

    for i in 0..100 {
        let test_char = (b'a' + (i % 26) as u8) as char;
        let test_str = test_char.to_string();
        let input_frame = encode_input_frame(&session_id, test_str.as_bytes());

        let start = Instant::now();
        ws.send(input_frame).unwrap();

        // Read until we receive our test char echoed back
        let mut echoed = false;
        while !echoed && start.elapsed() < Duration::from_millis(200) {
            match ws.read() {
                Ok(Message::Binary(bytes)) => {
                    if let Some((id, payload)) = decode_output_frame(&bytes) {
                        if id == session_id && payload.contains(&(test_char as u8)) {
                            echoed = true;
                            latencies.push(start.elapsed());
                        }
                    }
                }
                Ok(Message::Text(_)) => {}
                _ => break,
            }
        }
        assert!(echoed, "Keystroke must be echoed back promptly");
    }

    latencies.sort();
    let median = latencies[latencies.len() / 2];
    let p99 = latencies[(latencies.len() * 99) / 100];

    println!(
        "Keystroke-to-glyph benchmark: samples={}, median={:?}, p99={:?}",
        latencies.len(),
        median,
        p99
    );

    assert!(
        median < Duration::from_millis(10),
        "Median keystroke latency must be < 10ms (was {:?})",
        median
    );
    assert!(
        p99 < Duration::from_millis(30),
        "P99 keystroke latency must be < 30ms (was {:?})",
        p99
    );
}

#[test]
fn seq_throughput_and_second_terminal_responsiveness() {
    let server = WebServer::start();

    // 1. Launch terminal 1 running custom fast output: `seq 1 1000000`
    let (status1, res1) = server.post(
        "/api/terminals",
        json!({"agent": "custom", "command": "seq", "args": ["1", "1000000"], "isolated": false}),
    );
    assert_eq!(status1, 201);
    let id1 = res1["sessionId"].as_str().unwrap().to_owned();

    // 2. Launch terminal 2 running interactive shell
    let (status2, res2) = server.post(
        "/api/terminals",
        json!({"agent": "shell", "isolated": false, "args": []}),
    );
    assert_eq!(status2, 201);
    let id2 = res2["sessionId"].as_str().unwrap().to_owned();

    let mut ws = server.connect_ws();
    set_timeout(&ws, Duration::from_secs(5));

    // Attach both terminals
    ws.send(Message::Text(
        json!({"type": "attach", "id": id1, "rows": 24, "cols": 80}).to_string(),
    ))
    .unwrap();
    ws.send(Message::Text(
        json!({"type": "attach", "id": id2, "rows": 24, "cols": 80}).to_string(),
    ))
    .unwrap();

    let mut bytes_from_1 = 0usize;
    let mut term2_responsive = false;
    let start = Instant::now();

    // During the stream, send input to terminal 2 and verify immediate response
    let probe_char = 'Z';
    let mut probe_sent = false;

    while start.elapsed() < Duration::from_secs(10) {
        // Send probe to terminal 2 once terminal 1 is actively streaming
        if bytes_from_1 > 50_000 && !probe_sent {
            ws.send(encode_input_frame(&id2, probe_char.to_string().as_bytes()))
                .unwrap();
            probe_sent = true;
        }

        match ws.read() {
            Ok(Message::Binary(bytes)) => {
                if let Some((id, payload)) = decode_output_frame(&bytes) {
                    if id == id1 {
                        bytes_from_1 += payload.len();
                        // Send acknowledgements to exercise client ack flow
                        ws.send(Message::Text(
                            json!({"type": "ack", "id": id1, "bytes": payload.len()}).to_string(),
                        ))
                        .unwrap();
                    } else if id == id2 && payload.contains(&(probe_char as u8)) {
                        term2_responsive = true;
                    }
                }
            }
            Ok(Message::Text(text)) => {
                let val: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                if val["type"] == "exit" && val["id"] == id1 {
                    println!("Terminal 1 seq finished with code {}", val["code"]);
                    break;
                }
            }
            _ => break,
        }
    }

    assert!(
        bytes_from_1 > 100_000,
        "Terminal 1 must stream substantial output from seq (got {} bytes)",
        bytes_from_1
    );
    assert!(
        term2_responsive,
        "Terminal 2 must accept input and echo back while Terminal 1 is flooding output"
    );
}

#[test]
fn tab_disconnect_and_reopen_reattaches_with_screen_intact() {
    let server = WebServer::start();

    // Launch shell terminal
    let (status, res) = server.post(
        "/api/terminals",
        json!({"agent": "shell", "isolated": false, "args": []}),
    );
    assert_eq!(status, 201);
    let id = res["sessionId"].as_str().unwrap().to_owned();

    // 1. Connect Tab 1
    {
        let mut ws1 = server.connect_ws();
        set_timeout(&ws1, Duration::from_secs(4));

        ws1.send(Message::Text(
            json!({"type": "attach", "id": id, "rows": 24, "cols": 80}).to_string(),
        ))
        .unwrap();

        // Write a distinct marker to the shell
        let marker = "echo REATTACH_MARKER_98765\n";
        ws1.send(encode_input_frame(&id, marker.as_bytes()))
            .unwrap();

        // Wait until output contains the marker
        let mut marker_seen = false;
        let start = Instant::now();
        while !marker_seen && start.elapsed() < Duration::from_secs(3) {
            if let Ok(Message::Binary(bytes)) = ws1.read() {
                if let Some((_, payload)) = decode_output_frame(&bytes) {
                    if String::from_utf8_lossy(payload).contains("REATTACH_MARKER_98765") {
                        marker_seen = true;
                    }
                }
            }
        }
        assert!(marker_seen, "Marker must appear on terminal");

        // Close Tab 1 (drops WebSocket connection)
    }

    // Small delay to ensure clean disconnect on server
    std::thread::sleep(Duration::from_millis(100));

    // 2. Connect Tab 2 (simulating browser reload / reopen)
    let mut ws2 = server.connect_ws();
    set_timeout(&ws2, Duration::from_secs(4));

    // Reattach to the existing terminal
    ws2.send(Message::Text(
        json!({"type": "attach", "id": id, "rows": 24, "cols": 80}).to_string(),
    ))
    .unwrap();

    // Attached message must contain the formatted screen snapshot with the marker!
    let msg = ws2.read().unwrap();
    if let Message::Text(text) = msg {
        let val: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(val["type"], "attached");
        assert_eq!(val["id"], id);
        assert_eq!(val["running"], true);

        let screen_b64 = val["screen"].as_str().unwrap();
        let screen_bytes = base64::engine::general_purpose::STANDARD
            .decode(screen_b64)
            .unwrap();
        let screen_str = String::from_utf8_lossy(&screen_bytes);

        assert!(
            screen_str.contains("REATTACH_MARKER_98765"),
            "Reopened terminal screen must contain marker from previous tab session. Screen was:\n{}",
            screen_str
        );
    } else {
        panic!("Expected attached message with screen snapshot");
    }
}

#[test]
fn backpressure_pauses_pty_reader_and_resumes_on_client_ack() {
    let server = WebServer::start();

    // 1. Launch a terminal running `yes`
    let (status, resp) = server.post(
        "/api/terminals",
        json!({"agent": "custom", "command": "yes", "args": ["BACKPRESSURE_PAYLOAD_TEST_DATA"], "isolated": false}),
    );
    assert_eq!(status, 201);
    let id = resp["sessionId"].as_str().unwrap().to_owned();

    // 2. Connect WebSocket and attach
    let mut ws = server.connect_ws();
    set_timeout(&ws, Duration::from_millis(600));

    ws.send(Message::Text(
        json!({"type": "attach", "id": id, "rows": 24, "cols": 80}).to_string(),
    ))
    .unwrap();

    let mut total_bytes_received = 0usize;
    let mut frames_received = 0usize;

    // Read until the stream pauses due to backpressure (no ACK sent)
    loop {
        match ws.read() {
            Ok(Message::Binary(bin)) => {
                if bin.len() > 1 && (bin[0] as usize) < bin.len() {
                    let id_len = bin[0] as usize;
                    let payload = &bin[1 + id_len..];
                    total_bytes_received += payload.len();
                    frames_received += 1;
                }
            }
            Ok(_) => {}
            Err(_) => {
                // Read timed out because PTY reader paused when unacked >= 128KB!
                break;
            }
        }
    }

    println!(
        "Backpressure paused reader after {total_bytes_received} bytes across {frames_received} frames without ACK"
    );
    // 128 KB high water mark: reader should pause around 128 KB
    assert!(
        total_bytes_received >= 64 * 1024,
        "Expected at least 64KB before pausing, got {total_bytes_received}"
    );

    // 3. Now send ACK for the received bytes to relieve backpressure
    ws.send(Message::Text(
        json!({"type": "ack", "id": id, "bytes": total_bytes_received}).to_string(),
    ))
    .unwrap();

    // 4. Verify stream immediately resumes!
    set_timeout(&ws, Duration::from_secs(3));
    let mut resumed_bytes = 0usize;
    for _ in 0..5 {
        match ws.read() {
            Ok(Message::Binary(bin)) => {
                if bin.len() > 1 {
                    let id_len = bin[0] as usize;
                    resumed_bytes += bin[1 + id_len..].len();
                }
            }
            Ok(_) => {}
            Err(e) => panic!("Expected resumed stream after ACK, got error: {e}"),
        }
    }
    assert!(
        resumed_bytes > 0,
        "Stream should have resumed reading after ACK"
    );
    println!("Backpressure resumed successfully, received {resumed_bytes} additional bytes");

    // 5. Cleanly signal SIGINT / exit
    let _ = ws.send(Message::Text(
        json!({"type": "signal", "id": id, "signal": "SIGINT"}).to_string(),
    ));
}

#[test]
#[ignore]
fn yes_sustained_streaming_memory_bounded_60s() {
    let server = WebServer::start();

    // Launch `yes`
    let (status, resp) = server.post(
        "/api/terminals",
        json!({"agent": "custom", "command": "yes", "args": ["SUSTAINED_STREAMING_MEMORY_BOUNDED_TEST_DATA_PADDING"], "isolated": false}),
    );
    assert_eq!(status, 201);
    let id = resp["sessionId"].as_str().unwrap().to_owned();

    let mut ws = server.connect_ws();
    set_timeout(&ws, Duration::from_secs(3));

    ws.send(Message::Text(
        json!({"type": "attach", "id": id, "rows": 24, "cols": 80}).to_string(),
    ))
    .unwrap();

    let initial_rss = server.get_rss_kb();
    println!("Initial Server RSS: {initial_rss} KB");

    let start = Instant::now();
    let mut last_checkpoint = Instant::now();
    let mut total_bytes = 0usize;
    let mut pending_ack = 0usize;

    while start.elapsed() < Duration::from_secs(60) {
        match ws.read() {
            Ok(Message::Binary(bin)) => {
                if bin.len() > 1 {
                    let id_len = bin[0] as usize;
                    let payload_len = bin[1 + id_len..].len();
                    total_bytes += payload_len;
                    pending_ack += payload_len;

                    // Send ACKs in batches like xterm.js write callback
                    if pending_ack >= 32 * 1024 {
                        ws.send(Message::Text(
                            json!({"type": "ack", "id": id, "bytes": pending_ack}).to_string(),
                        ))
                        .unwrap();
                        pending_ack = 0;
                    }
                }
            }
            Ok(_) => {}
            Err(e) => panic!("Error during 60s streaming: {e}"),
        }

        if last_checkpoint.elapsed() >= Duration::from_secs(10) {
            let current_rss = server.get_rss_kb();
            let elapsed_s = start.elapsed().as_secs();
            let mb_processed = total_bytes as f64 / (1024.0 * 1024.0);
            println!(
                "[{elapsed_s}s] RSS: {current_rss} KB, Data: {mb_processed:.2} MB, Delta RSS: {} KB",
                current_rss as isize - initial_rss as isize
            );
            last_checkpoint = Instant::now();
        }
    }

    let final_rss = server.get_rss_kb();
    let delta = (final_rss as isize - initial_rss as isize).abs();
    let total_mb = total_bytes as f64 / (1024.0 * 1024.0);
    println!(
        "60s `yes` finished: Total data = {total_mb:.2} MB, Initial RSS = {initial_rss} KB, Final RSS = {final_rss} KB, Delta = {delta} KB"
    );

    // RSS growth must stay bounded (less than 15 MB) over 60s of massive output
    assert!(
        delta < 15 * 1024,
        "Server RSS grew unexpectedly by {delta} KB (Initial: {initial_rss} KB, Final: {final_rss} KB)"
    );

    // Stop terminal
    let _ = ws.send(Message::Text(
        json!({"type": "signal", "id": id, "signal": "SIGINT"}).to_string(),
    ));
}

unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

#[test]
fn closing_terminal_kills_process_group_without_orphans() {
    let server = WebServer::start();

    // 1. Launch an interactive shell terminal
    let (status, res) = server.post(
        "/api/terminals",
        json!({
            "agent": "custom",
            "command": "/bin/sh",
            "args": ["-i"],
            "isolated": false
        }),
    );
    assert_eq!(status, 201);
    let id = res["sessionId"].as_str().unwrap().to_owned();

    let mut ws = server.connect_ws();
    set_timeout(&ws, Duration::from_millis(200));

    // 2. Attach to the terminal
    ws.send(Message::Text(
        json!({
            "type": "attach",
            "id": id,
            "rows": 24,
            "cols": 80
        })
        .to_string(),
    ))
    .unwrap();

    // Verify attached frame
    let attached = ws.read().unwrap();
    if let Message::Text(txt) = attached {
        let val: Value = serde_json::from_str(&txt).unwrap();
        assert_eq!(val["type"], "attached");
    } else {
        panic!("expected attached frame");
    }

    // Warm-up: drain any prompt bytes
    let warm_deadline = Instant::now() + Duration::from_millis(300);
    while Instant::now() < warm_deadline {
        let _ = ws.read();
    }

    // 3. Send command to launch background sleep 1000 & and print its PID
    let cmd = b"sleep 1000 & echo BACKGROUND_PID=$!\n";
    ws.send(encode_input_frame(&id, cmd)).unwrap();

    let mut child_pid: Option<i32> = None;
    let mut stream_buf = String::new();
    let start = Instant::now();

    while start.elapsed() < Duration::from_secs(10) && child_pid.is_none() {
        match ws.read() {
            Ok(Message::Binary(bin)) => {
                if let Some((_, payload)) = decode_output_frame(&bin) {
                    stream_buf.push_str(&String::from_utf8_lossy(payload));
                    for line in stream_buf.lines() {
                        if let Some(val) = line.strip_prefix("BACKGROUND_PID=") {
                            let digits: String =
                                val.chars().take_while(|c| c.is_ascii_digit()).collect();
                            if let Ok(pid) = digits.parse::<i32>() {
                                child_pid = Some(pid);
                                break;
                            }
                        }
                    }
                }
            }
            Ok(_) => {}
            Err(tungstenite::Error::Io(ref e))
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => panic!("error reading from stream: {e}"),
        }
    }

    let pid = child_pid.unwrap_or_else(|| {
        panic!(
            "failed to read BACKGROUND_PID from shell output. stream_buf was: {:?}",
            stream_buf
        )
    });
    println!("Spawned background child PID: {pid}");

    // Verify child is running initially using kill(pid, 0) and ps -p <pid>
    let is_alive = unsafe { kill(pid, 0) == 0 };
    assert!(
        is_alive,
        "child process {pid} must be running before terminal close"
    );

    let ps_before = Command::new("ps")
        .args(["-p", &pid.to_string()])
        .output()
        .unwrap();
    assert!(
        ps_before.status.success(),
        "ps -p {pid} must succeed while process is alive"
    );

    // 4. Close the terminal via DELETE /api/terminals/{id}
    let (status, del_res) = server.delete(&format!("/api/terminals/{id}"));
    assert_eq!(status, 200);
    assert_eq!(del_res["ok"], true);

    // 5. Verify the entire process group including child was terminated (no orphans)
    let mut terminated = false;
    let check_start = Instant::now();
    while check_start.elapsed() < Duration::from_secs(5) {
        let alive = unsafe { kill(pid, 0) == 0 };
        if !alive {
            terminated = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    assert!(
        terminated,
        "child process {pid} survived terminal close as an orphan!"
    );

    let ps_after = Command::new("ps")
        .args(["-p", &pid.to_string()])
        .output()
        .unwrap();
    assert!(
        !ps_after.status.success(),
        "ps -p {pid} must fail after terminal close, confirming no orphan exists"
    );
}

#[test]
fn vttest_screens_1_to_3_verification() {
    // Verifies that the PTY host and WebSocket stream deliver vttest's output intact. Each
    // screen's raw bytes, exactly as a browser would receive them, are rendered by tmux (a full
    // terminal emulator) and the rendering is asserted. Rendering through Verb's own vt100 model is
    // deliberately *not* the oracle: it mis-draws screen 1 (it lacks DECALN, ESC # 8), which is a
    // separate, known limitation of the server-side snapshot used for reattach and the phone.
    let (Some(vttest), Some(tmux)) = (find_tool("vttest"), find_tool("tmux")) else {
        eprintln!("skipping vttest_screens_1_to_3_verification: needs vttest and tmux");
        return;
    };

    let server = WebServer::start();
    let (status, res) = server.post(
        "/api/terminals",
        json!({"agent": "custom", "command": vttest, "args": [], "isolated": false}),
    );
    assert_eq!(status, 201);
    let id = res["sessionId"].as_str().unwrap().to_owned();

    let mut ws = server.connect_ws();
    set_timeout(&ws, Duration::from_millis(100));
    ws.send(Message::Text(
        json!({"type": "attach", "id": id, "rows": 24, "cols": 80}).to_string(),
    ))
    .unwrap();
    let mut probe = vt100::Parser::new(24, 80, 0);
    if let Ok(Message::Text(txt)) = ws.read() {
        let val: Value = serde_json::from_str(&txt).unwrap();
        assert_eq!(val["type"], "attached");
        if let Some(screen_b64) = val["screen"].as_str() {
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(screen_b64) {
                probe.process(&bytes);
            }
        }
    }
    let mut raw = Vec::new();
    assert!(
        pump_screen(&mut ws, &mut probe, &mut raw, 5, |s| s
            .contains("Choose test type")),
        "vttest main menu not reached:\n{}",
        probe.screen().contents()
    );
    let stars = "*".repeat(80);

    // Screen 1, cursor movements: border of '*' and '+', and a frame of E's (DECALN) in the middle.
    let screen = vttest_screen(&mut ws, &mut probe, &id, b"1\n", "unbroken bor-", &tmux);
    let lines: Vec<&str> = screen.lines().collect();
    assert_eq!(
        lines.first().copied(),
        Some(stars.as_str()),
        "screen 1:\n{screen}"
    );
    assert_eq!(
        lines.get(23).copied(),
        Some(stars.as_str()),
        "screen 1:\n{screen}"
    );
    assert!(
        screen.contains(
            "*+        EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE        +*"
        ),
        "screen 1 E frame:\n{screen}"
    );

    // Screen 2, screen features: the first page is three full rows of '*' (auto-wrap).
    let screen = vttest_screen(&mut ws, &mut probe, &id, b"2\n", "WRAP AROUND", &tmux);
    let top: Vec<&str> = screen.lines().take(3).collect();
    assert_eq!(top, vec![stars.as_str(); 3], "screen 2:\n{screen}");

    // Screen 3, character sets: the designated sets render as a table.
    let screen = vttest_screen(
        &mut ws,
        &mut probe,
        &id,
        b"3\n",
        "Character set B (US ASCII)",
        &tmux,
    );
    assert!(
        screen.contains("@ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
        "screen 3:\n{screen}"
    );

    ws.send(encode_input_frame(&id, b"0\n")).unwrap();
    let (status, del_res) = server.delete(&format!("/api/terminals/{id}"));
    assert_eq!(status, 200, "DELETE failed with {status}: {del_res:?}");
    assert_eq!(del_res["ok"], true);
}

/// Opens one vttest screen, returns tmux's rendering of exactly the bytes Verb streamed for it,
/// then returns vttest to its menu.
fn vttest_screen(
    ws: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    probe: &mut vt100::Parser,
    id: &str,
    key: &[u8],
    marker: &str,
    tmux: &str,
) -> String {
    let mut raw = Vec::new();
    ws.send(encode_input_frame(id, key)).unwrap();
    assert!(
        pump_screen(ws, probe, &mut raw, 5, |s| s.contains(marker)),
        "vttest screen with {marker:?} never appeared:\n{}",
        probe.screen().contents()
    );
    let rendered = render_with_tmux(tmux, &raw);
    for _ in 0..40 {
        ws.send(encode_input_frame(id, b"\n")).unwrap();
        if pump_screen(ws, probe, &mut Vec::new(), 1, |s| {
            s.contains("Choose test type")
        }) {
            return rendered;
        }
    }
    panic!(
        "vttest menu did not come back:\n{}",
        probe.screen().contents()
    );
}

/// Renders a raw terminal byte stream in a fresh, detached 80x24 tmux session.
fn render_with_tmux(tmux: &str, raw: &[u8]) -> String {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir();
    let file = dir.join(format!("verb-vttest-{}-{n}.raw", std::process::id()));
    fs::write(&file, raw).unwrap();
    let name = format!("verb-vttest-{}-{n}", std::process::id());
    let tmux_cmd = |args: &[&str]| {
        Command::new(tmux)
            .args(["-f", "/dev/null", "-L", &name])
            .args(args)
            .output()
            .unwrap()
    };
    let shown = format!("cat {}; sleep 30", file.display());
    tmux_cmd(&[
        "new-session",
        "-d",
        "-s",
        &name,
        "-x",
        "80",
        "-y",
        "24",
        &shown,
    ]);
    std::thread::sleep(Duration::from_millis(700));
    let out = tmux_cmd(&["capture-pane", "-p", "-t", &name]);
    tmux_cmd(&["kill-server"]);
    let _ = fs::remove_file(&file);
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Feeds output frames into `parser` (and `raw`) until `done(screen)` holds and output has gone
/// quiet, or `secs` pass.
fn pump_screen(
    ws: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    parser: &mut vt100::Parser,
    raw: &mut Vec<u8>,
    secs: u64,
    done: impl Fn(&str) -> bool,
) -> bool {
    let deadline = Instant::now() + Duration::from_secs(secs);
    let mut read = |parser: &mut vt100::Parser, raw: &mut Vec<u8>| -> bool {
        if let Ok(Message::Binary(bin)) = ws.read() {
            if let Some((_, payload)) = decode_output_frame(&bin) {
                parser.process(payload);
                raw.extend_from_slice(payload);
                return true;
            }
        }
        false
    };
    loop {
        if done(&parser.screen().contents()) {
            let mut quiet_since = Instant::now();
            while quiet_since.elapsed() < Duration::from_millis(400) {
                if read(parser, raw) {
                    quiet_since = Instant::now();
                }
            }
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        read(parser, raw);
    }
}

/// A tool's path, or `None` so a test can skip rather than fail where the tool is not installed.
fn find_tool(name: &str) -> Option<String> {
    let mut dirs: Vec<PathBuf> = ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"]
        .iter()
        .map(PathBuf::from)
        .collect();
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs.into_iter()
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
        .map(|found| found.display().to_string())
}

#[test]
fn real_apps_interactive_verification() {
    let server = WebServer::start();

    // 1. Test Vim
    let (status, res) = server.post(
        "/api/terminals",
        json!({
            "agent": "custom",
            "command": "/usr/bin/vim",
            "args": ["-u", "NONE", "-N", "/tmp/verb_test_vim.txt"],
            "isolated": false
        }),
    );
    assert_eq!(status, 201);
    let vim_id = res["sessionId"].as_str().unwrap().to_owned();

    let mut ws = server.connect_ws();
    set_timeout(&ws, Duration::from_millis(300));
    ws.send(Message::Text(
        json!({"type": "attach", "id": vim_id, "rows": 24, "cols": 80}).to_string(),
    ))
    .unwrap();
    let _ = ws.read();

    // Wait for vim to initialize
    std::thread::sleep(Duration::from_millis(300));

    // Send :q!\n to cleanly exit vim
    ws.send(encode_input_frame(&vim_id, b":q!\n")).unwrap();

    // Wait for exit message
    let mut vim_exited = false;
    let deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < deadline {
        if let Ok(Message::Text(txt)) = ws.read() {
            if let Ok(val) = serde_json::from_str::<Value>(&txt) {
                if val["type"] == "exit" && val["id"] == vim_id {
                    assert_eq!(val["code"], 0);
                    vim_exited = true;
                    break;
                }
            }
        }
    }
    assert!(vim_exited, "vim should exit cleanly with code 0");

    // 2. Test Python3 REPL
    let py_bin = if std::path::Path::new("/opt/homebrew/bin/python3").exists() {
        "/opt/homebrew/bin/python3"
    } else {
        "python3"
    };
    let (status, res) = server.post(
        "/api/terminals",
        json!({
            "agent": "custom",
            "command": py_bin,
            "args": ["-i", "-q"],
            "isolated": false
        }),
    );
    assert_eq!(status, 201);
    let py_id = res["sessionId"].as_str().unwrap().to_owned();

    ws.send(Message::Text(
        json!({"type": "attach", "id": py_id, "rows": 24, "cols": 80}).to_string(),
    ))
    .unwrap();
    let _ = ws.read();

    // Send Python expression
    ws.send(encode_input_frame(
        &py_id,
        b"print('VERB_PY_' + str(40 + 2))\n",
    ))
    .unwrap();

    let mut py_output = String::new();
    let deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < deadline && !py_output.contains("VERB_PY_42") {
        if let Ok(Message::Binary(bin)) = ws.read() {
            if let Some((_, payload)) = decode_output_frame(&bin) {
                py_output.push_str(&String::from_utf8_lossy(payload));
            }
        }
    }
    assert!(
        py_output.contains("VERB_PY_42"),
        "python evaluation failed, got: {py_output}"
    );

    // Exit Python cleanly
    ws.send(encode_input_frame(&py_id, b"exit()\n")).unwrap();
    let mut py_exited = false;
    let deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < deadline {
        if let Ok(Message::Text(txt)) = ws.read() {
            if let Ok(val) = serde_json::from_str::<Value>(&txt) {
                if val["type"] == "exit" && val["id"] == py_id {
                    assert_eq!(val["code"], 0);
                    py_exited = true;
                    break;
                }
            }
        }
    }
    assert!(py_exited, "python REPL should exit cleanly with code 0");

    // 3. Test Tmux
    let Some(tmux_bin) = find_tool("tmux") else {
        eprintln!(
            "skipping the tmux part of real_apps_interactive_verification: tmux is not installed"
        );
        return;
    };
    let (status, res) = server.post(
        "/api/terminals",
        json!({
            "agent": "custom",
            "command": tmux_bin,
            "args": ["-f", "/dev/null", "new-session"],
            "isolated": false
        }),
    );
    assert_eq!(status, 201);
    let tmux_id = res["sessionId"].as_str().unwrap().to_owned();

    ws.send(Message::Text(
        json!({"type": "attach", "id": tmux_id, "rows": 24, "cols": 80}).to_string(),
    ))
    .unwrap();
    let _ = ws.read();

    std::thread::sleep(Duration::from_millis(400));
    // Exit tmux session cleanly
    ws.send(encode_input_frame(&tmux_id, b"exit\n")).unwrap();

    let mut tmux_exited = false;
    let deadline = Instant::now() + Duration::from_secs(4);
    while Instant::now() < deadline {
        if let Ok(Message::Text(txt)) = ws.read() {
            if let Ok(val) = serde_json::from_str::<Value>(&txt) {
                if val["type"] == "exit" && val["id"] == tmux_id {
                    tmux_exited = true;
                    break;
                }
            }
        }
    }
    assert!(tmux_exited, "tmux should exit cleanly");
}
