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

use crate::pty::{self, PollFd, POLLERR, POLLHUP, POLLIN, POLLOUT};

pub const HIGH_WATER_MARK: usize = 128 * 1024;
pub const LOW_WATER_MARK: usize = 32 * 1024;
/// The most one terminal sends before every other terminal (and the reader) gets a turn.
const SLICE: usize = 32 * 1024;

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

/// Outgoing queue for one WebSocket connection, scheduled fairly.
///
/// Output is kept per terminal and coalesced, then sent round-robin in slices of at most `SLICE`, so
/// a few bytes of echo in one terminal never wait behind another terminal's backlog. (One FIFO of
/// frames did exactly that: with a second terminal flooding output, a keystroke's echo took ~90 ms
/// even on a fast machine with no network.) A terminal's control messages, such as `exit`, wait
/// until its earlier output has gone, so their order relative to that output is kept.
pub struct WsQueue {
    inner: Mutex<QueueInner>,
    pub wake: Arc<WakePipe>,
}

#[derive(Default)]
struct QueueInner {
    control: VecDeque<(Option<String>, Message)>,
    outputs: HashMap<String, Vec<u8>>,
    order: VecDeque<String>,
}

impl WsQueue {
    pub fn new(wake: Arc<WakePipe>) -> Self {
        Self {
            inner: Mutex::new(QueueInner::default()),
            wake,
        }
    }

    /// A control message, optionally belonging to one terminal.
    pub fn push_control(&self, id: Option<&str>, msg: Message) {
        self.inner
            .lock()
            .unwrap()
            .control
            .push_back((id.map(str::to_owned), msg));
        self.wake.wake();
    }

    pub fn push(&self, msg: Message) {
        self.push_control(None, msg);
    }

    /// PTY bytes for one terminal, appended to whatever of its output is still waiting.
    pub fn push_output(&self, id: &str, data: &[u8]) {
        let mut inner = self.inner.lock().unwrap();
        let buffer = inner.outputs.entry(id.to_owned()).or_default();
        let was_empty = buffer.is_empty();
        buffer.extend_from_slice(data);
        if was_empty {
            inner.order.push_back(id.to_owned());
        }
        drop(inner);
        self.wake.wake();
    }

    /// One fair round: a slice of output from each terminal that has some, then the control
    /// messages that no longer wait on output.
    pub fn next_round(&self) -> Vec<Message> {
        let mut inner = self.inner.lock().unwrap();
        let mut out = Vec::new();
        for _ in 0..inner.order.len() {
            let Some(id) = inner.order.pop_front() else {
                break;
            };
            let Some(buffer) = inner.outputs.get_mut(&id) else {
                continue;
            };
            let take = buffer.len().min(SLICE);
            let chunk: Vec<u8> = buffer.drain(..take).collect();
            let more = !buffer.is_empty();
            let id_bytes = id.as_bytes();
            let mut frame = Vec::with_capacity(1 + id_bytes.len() + chunk.len());
            frame.push(id_bytes.len() as u8);
            frame.extend_from_slice(id_bytes);
            frame.extend_from_slice(&chunk);
            out.push(Message::Binary(frame));
            if more {
                inner.order.push_back(id);
            } else {
                inner.outputs.remove(&id);
            }
        }
        let control = std::mem::take(&mut inner.control);
        for (id, msg) in control {
            let waits = id
                .as_deref()
                .is_some_and(|id| inner.outputs.contains_key(id));
            if waits {
                inner.control.push_back((id, msg));
            } else {
                out.push(msg);
            }
        }
        out
    }

    pub fn has_pending(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        !inner.order.is_empty() || !inner.control.is_empty()
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

        for queue in listeners.values() {
            queue.push_output(&self.id, data);
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
        let id = val.get("id").and_then(|v| v.as_str()).map(str::to_owned);
        let msg = Message::Text(val.to_string());
        for queue in listeners.values() {
            queue.push_control(id.as_deref(), msg.clone());
        }
    }

    /// Acknowledges processed bytes to relieve backpressure.
    pub fn ack(&self, bytes: usize) {
        // `fetch_update` is renamed `try_update` in Rust 1.99, which deprecates the old name. The new
        // one does not exist in older toolchains (Homebrew ships 1.93), so keep the old call.
        #[allow(deprecated)]
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

/// What the writer should do next.
#[derive(PartialEq, Debug)]
enum Drain {
    /// Nothing left to send.
    Idle,
    /// More is queued: give the reader a turn, then come straight back.
    More,
    /// The socket is full: wait until it is writable. Nothing is lost; tungstenite keeps the frames.
    Blocked,
    /// The connection is gone.
    Closed,
}

fn would_block(error: &tungstenite::Error) -> bool {
    matches!(error, tungstenite::Error::Io(e) if e.kind() == io::ErrorKind::WouldBlock)
}

/// Sends one fair round of queued output. A full socket is never treated as a disconnect: the
/// previous code did, so a burst of output on a slow link dropped the connection and the browser
/// had to reconnect and repaint.
fn drain_outgoing(ws: &mut tungstenite::WebSocket<std::net::TcpStream>, queue: &WsQueue) -> Drain {
    match ws.flush() {
        Ok(()) => {}
        Err(e) if would_block(&e) => return Drain::Blocked,
        Err(_) => return Drain::Closed,
    }
    let round = queue.next_round();
    if round.is_empty() {
        return Drain::Idle;
    }
    for msg in round {
        match ws.write(msg) {
            Ok(()) => {}
            // The frame is kept in tungstenite's write buffer and goes out with the next flush.
            Err(e) if would_block(&e) => {}
            Err(_) => return Drain::Closed,
        }
    }
    match ws.flush() {
        Ok(()) if queue.has_pending() => Drain::More,
        Ok(()) => Drain::Idle,
        Err(e) if would_block(&e) => Drain::Blocked,
        Err(_) => Drain::Closed,
    }
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
    let mut state = Drain::Idle;

    loop {
        let mut poll_fds = [
            PollFd {
                fd,
                events: if state == Drain::Blocked {
                    POLLIN | POLLOUT
                } else {
                    POLLIN
                },
                revents: 0,
            },
            PollFd {
                fd: wake.read_fd(),
                events: POLLIN,
                revents: 0,
            },
        ];

        let timeout = if state == Drain::More { 0 } else { 5000 };
        if pty::poll_fds(&mut poll_fds, timeout).is_err() {
            break;
        }

        // 1. One fair round of output, when there is something to send and room to send it.
        let woken = poll_fds[1].revents & POLLIN != 0;
        if woken {
            wake.drain();
        }
        let writable = poll_fds[0].revents & POLLOUT != 0;
        if woken || writable || state == Drain::More {
            state = drain_outgoing(&mut ws, &queue);
            if state == Drain::Closed {
                break;
            }
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
                                        match ws.write(Message::Text(ack.to_string())) {
                                            Ok(()) => {}
                                            Err(e) if would_block(&e) => {}
                                            Err(_) => {
                                                disconnected = true;
                                                break;
                                            }
                                        }
                                        // Sent with the next round's flush.
                                        state = Drain::More;
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
                    // tungstenite queues the pong itself; the next flush sends it.
                    Ok(Message::Ping(_)) => state = Drain::More,
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
