//! WebSocket streaming protocol and connection handling for real terminals.
//!
//! Protocol:
//! - One WebSocket per browser tab, multiplexed by terminal ID.
//! - Binary frames for raw PTY bytes:
//!   - Byte 0: terminal id string length (u8)
//!   - Bytes 1..1+id_len: terminal id UTF-8 string
//!   - Bytes 1+id_len..: PTY payload bytes
//! - JSON text frames for control (`attach`, `detach`, `resize`, `ack`, `signal`, `exit`, `title`, `cwd`, `bell`).
//! - Versioned handshake: `{"v": 1}` sent immediately upon connection.
//! - Backpressure: high-water mark pauses PTY reader; client acknowledgements resume reading.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::json;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io;
use std::os::fd::AsRawFd;
use std::os::raw::c_int;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use tungstenite::Message;

use crate::pty::{self, PollFd, POLLERR, POLLHUP, POLLIN};

pub const HIGH_WATER_MARK: usize = 128 * 1024;
pub const LOW_WATER_MARK: usize = 32 * 1024;

/// Safe RAII wrapper around a nonblocking Unix pipe for waking poll loops.
pub struct WakePipe {
    read_fd: c_int,
    write_fd: c_int,
}

impl WakePipe {
    pub fn new() -> io::Result<Self> {
        let (read_fd, write_fd) = pty::make_pipe()?;
        Ok(Self { read_fd, write_fd })
    }

    pub fn read_fd(&self) -> c_int {
        self.read_fd
    }

    pub fn wake(&self) {
        let _ = pty::write_fd(self.write_fd, &[1u8]);
    }

    pub fn drain(&self) {
        let mut buf = [0u8; 128];
        while pty::read_fd(self.read_fd, &mut buf) > 0 {}
    }
}

impl Drop for WakePipe {
    fn drop(&mut self) {
        pty::close_fd(self.read_fd);
        pty::close_fd(self.write_fd);
    }
}

/// Outgoing message queue for a single WebSocket connection.
pub struct WsQueue {
    pub messages: Mutex<VecDeque<Message>>,
    pub wake: Arc<WakePipe>,
}

impl WsQueue {
    pub fn new(wake: Arc<WakePipe>) -> Self {
        Self {
            messages: Mutex::new(VecDeque::new()),
            wake,
        }
    }

    pub fn push(&self, msg: Message) {
        self.messages.lock().unwrap().push_back(msg);
        self.wake.wake();
    }
}

/// Manages streaming output, active listeners, and backpressure for a single terminal.
pub struct TerminalStreamSink {
    id: String,
    unacked_bytes: AtomicUsize,
    paused: AtomicBool,
    cond: Condvar,
    lock: Mutex<()>,
    listeners: Mutex<HashMap<u64, Arc<WsQueue>>>,
}

impl TerminalStreamSink {
    pub fn new(id: String) -> Arc<Self> {
        Arc::new(Self {
            id,
            unacked_bytes: AtomicUsize::new(0),
            paused: AtomicBool::new(false),
            cond: Condvar::new(),
            lock: Mutex::new(()),
            listeners: Mutex::new(HashMap::new()),
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn add_listener(&self, conn_id: u64, queue: Arc<WsQueue>) {
        self.listeners.lock().unwrap().insert(conn_id, queue);
    }

    pub fn remove_listener(&self, conn_id: u64) {
        let mut listeners = self.listeners.lock().unwrap();
        listeners.remove(&conn_id);
        if listeners.is_empty() {
            self.unacked_bytes.store(0, Ordering::SeqCst);
            self.paused.store(false, Ordering::SeqCst);
            self.cond.notify_all();
        }
    }

    pub fn has_listeners(&self) -> bool {
        !self.listeners.lock().unwrap().is_empty()
    }

    /// Broadcasts raw PTY output as a multiplexed binary frame.
    pub fn broadcast_output(&self, data: &[u8]) {
        let listeners = self.listeners.lock().unwrap();
        if listeners.is_empty() {
            return;
        }

        let id_bytes = self.id.as_bytes();
        let mut frame = Vec::with_capacity(1 + id_bytes.len() + data.len());
        frame.push(id_bytes.len() as u8);
        frame.extend_from_slice(id_bytes);
        frame.extend_from_slice(data);
        let msg = Message::Binary(frame);

        for queue in listeners.values() {
            queue.push(msg.clone());
        }

        let unacked = self.unacked_bytes.fetch_add(data.len(), Ordering::SeqCst) + data.len();
        if unacked >= HIGH_WATER_MARK {
            self.paused.store(true, Ordering::SeqCst);
        }
    }

    /// Broadcasts a JSON control message to all attached listeners.
    pub fn broadcast_control(&self, val: serde_json::Value) {
        let listeners = self.listeners.lock().unwrap();
        if listeners.is_empty() {
            return;
        }
        let msg = Message::Text(val.to_string());
        for queue in listeners.values() {
            queue.push(msg.clone());
        }
    }

    /// Acknowledges processed bytes to relieve backpressure.
    pub fn ack(&self, bytes: usize) {
        let remaining = self
            .unacked_bytes
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |cur| {
                Some(cur.saturating_sub(bytes))
            })
            .map(|prev| prev.saturating_sub(bytes))
            .unwrap_or(0);
        if remaining <= LOW_WATER_MARK {
            self.paused.store(false, Ordering::SeqCst);
            let _guard = self.lock.lock().unwrap();
            self.cond.notify_all();
        }
    }

    /// Pauses PTY reading when send queue passes high-water mark until client acknowledgements arrive.
    pub fn wait_for_drain(&self) {
        if self.paused.load(Ordering::SeqCst) {
            let mut guard = self.lock.lock().unwrap();
            while self.paused.load(Ordering::SeqCst) {
                let (g, timeout) = self
                    .cond
                    .wait_timeout(guard, Duration::from_millis(50))
                    .unwrap();
                guard = g;
                if timeout.timed_out() {
                    let listeners = self.listeners.lock().unwrap();
                    if listeners.is_empty() {
                        self.unacked_bytes.store(0, Ordering::SeqCst);
                        self.paused.store(false, Ordering::SeqCst);
                        break;
                    }
                }
            }
        }
    }
}

pub struct TerminalAttachInfo {
    pub screen: Vec<u8>,
    pub running: bool,
    pub controller: String,
    pub phone_connected: bool,
}

pub trait StreamTerminalHandler: Send + Sync {
    fn write_terminal_input(&self, id: &str, data: &[u8]) -> Result<(), String>;
    fn attach_terminal(
        &self,
        id: &str,
        conn_id: u64,
        queue: Arc<WsQueue>,
        rows: u16,
        cols: u16,
    ) -> Option<TerminalAttachInfo>;
    fn detach_terminal(&self, id: &str, conn_id: u64);
    fn resize_terminal(&self, id: &str, rows: u16, cols: u16);
    fn ack_terminal(&self, id: &str, bytes: usize);
    fn signal_terminal(&self, id: &str, signal: &str);
}

fn drain_outgoing<T: StreamTerminalHandler>(
    ws: &mut tungstenite::WebSocket<std::net::TcpStream>,
    queue: &WsQueue,
    wake: &WakePipe,
    attached: &HashSet<String>,
    handler: &Arc<T>,
    conn_id: u64,
) -> bool {
    wake.drain();
    loop {
        let next_msg = {
            let mut msgs = queue.messages.lock().unwrap();
            msgs.pop_front()
        };
        let Some(msg) = next_msg else { break };
        if ws.send(msg).is_err() {
            cleanup_attached(attached, handler, conn_id);
            return false;
        }
    }
    true
}

/// Handles an upgraded WebSocket connection on its own background thread.
pub fn handle_ws_connection<T: StreamTerminalHandler + 'static>(
    tcp_stream: std::net::TcpStream,
    handler: Arc<T>,
    conn_id: u64,
) {
    tcp_stream.set_nodelay(true).ok();
    tcp_stream.set_nonblocking(true).ok();
    let fd = tcp_stream.as_raw_fd();

    let mut ws = tungstenite::WebSocket::from_raw_socket(
        tcp_stream,
        tungstenite::protocol::Role::Server,
        None,
    );

    // Protocol v1 handshake frame
    if ws.send(Message::Text(json!({"v": 1}).to_string())).is_err() {
        return;
    }

    let wake = match WakePipe::new() {
        Ok(w) => Arc::new(w),
        Err(_) => return,
    };
    let queue = Arc::new(WsQueue::new(Arc::clone(&wake)));

    let mut attached: HashSet<String> = HashSet::new();

    loop {
        let mut poll_fds = [
            PollFd {
                fd,
                events: POLLIN,
                revents: 0,
            },
            PollFd {
                fd: wake.read_fd(),
                events: POLLIN,
                revents: 0,
            },
        ];

        let poll_res = pty::poll_fds(&mut poll_fds, 5000);
        if poll_res.is_err() {
            break;
        }

        // 1. Drain outgoing messages and write to WebSocket
        if poll_fds[1].revents & POLLIN != 0
            && !drain_outgoing(&mut ws, &queue, &wake, &attached, &handler, conn_id)
        {
            return;
        }

        // 2. Read incoming messages
        if poll_fds[0].revents & (POLLERR | POLLHUP) != 0 {
            break;
        }
        if poll_fds[0].revents & POLLIN != 0 {
            let mut disconnected = false;
            loop {
                match ws.read() {
                    Ok(Message::Binary(bytes)) => {
                        if bytes.len() > 1 {
                            let id_len = bytes[0] as usize;
                            if bytes.len() > 1 + id_len {
                                if let Ok(id) = std::str::from_utf8(&bytes[1..1 + id_len]) {
                                    let input = &bytes[1 + id_len..];
                                    let _ = handler.write_terminal_input(id, input);
                                }
                            }
                        }
                    }
                    Ok(Message::Text(text)) => {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                            let msg_type = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
                            let id = val.get("id").and_then(|v| v.as_str()).unwrap_or("");
                            match msg_type {
                                "attach" => {
                                    let cols =
                                        val.get("cols").and_then(|v| v.as_u64()).unwrap_or(80)
                                            as u16;
                                    let rows =
                                        val.get("rows").and_then(|v| v.as_u64()).unwrap_or(24)
                                            as u16;
                                    if let Some(info) = handler.attach_terminal(
                                        id,
                                        conn_id,
                                        Arc::clone(&queue),
                                        rows,
                                        cols,
                                    ) {
                                        attached.insert(id.to_string());
                                        let ack = json!({
                                            "type": "attached",
                                            "id": id,
                                            "screen": BASE64.encode(&info.screen),
                                            "running": info.running,
                                            "controller": info.controller,
                                            "phoneConnected": info.phone_connected,
                                        });
                                        if ws.send(Message::Text(ack.to_string())).is_err() {
                                            disconnected = true;
                                            break;
                                        }
                                    }
                                }
                                "detach" => {
                                    handler.detach_terminal(id, conn_id);
                                    attached.remove(id);
                                }
                                "resize" => {
                                    let cols =
                                        val.get("cols").and_then(|v| v.as_u64()).unwrap_or(80)
                                            as u16;
                                    let rows =
                                        val.get("rows").and_then(|v| v.as_u64()).unwrap_or(24)
                                            as u16;
                                    handler.resize_terminal(id, rows, cols);
                                }
                                "ack" => {
                                    let count =
                                        val.get("bytes").and_then(|v| v.as_u64()).unwrap_or(0)
                                            as usize;
                                    handler.ack_terminal(id, count);
                                }
                                "signal" => {
                                    if let Some(sig) = val.get("signal").and_then(|v| v.as_str()) {
                                        handler.signal_terminal(id, sig);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Ok(Message::Ping(data)) => {
                        let _ = ws.send(Message::Pong(data));
                    }
                    Ok(Message::Close(_)) => {
                        disconnected = true;
                        break;
                    }
                    Err(tungstenite::Error::Io(ref e)) if e.kind() == io::ErrorKind::WouldBlock => {
                        break;
                    }
                    Err(_) => {
                        disconnected = true;
                        break;
                    }
                    _ => {}
                }
            }
            if disconnected {
                break;
            }
            if !drain_outgoing(&mut ws, &queue, &wake, &attached, &handler, conn_id) {
                return;
            }
        }
    }

    cleanup_attached(&attached, &handler, conn_id);
}

fn cleanup_attached<T: StreamTerminalHandler>(
    attached: &HashSet<String>,
    handler: &Arc<T>,
    conn_id: u64,
) {
    for id in attached {
        handler.detach_terminal(id, conn_id);
    }
}
