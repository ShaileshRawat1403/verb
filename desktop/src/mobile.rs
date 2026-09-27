//! The desktop half of a live mobile continuation.
//!
//! A transport may hold a clone of `LiveBridge`, but only the PTY host can publish its current
//! screen or deliver queued input. Pairing is a short-lived, one-use capability. One phone can be
//! paired and connected; only one side owns input at a time. The local Unix socket is accessible
//! only to the desktop account. A future phone receiver must reach it through an authenticated,
//! encrypted transport; Verb deliberately exposes no LAN listener here.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const OFFER_LIFETIME: Duration = Duration::from_secs(120);
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_PAIR_ATTEMPTS: u8 = 5;
const MAX_SCREEN_BYTES: usize = 256 * 1024;
const MAX_INPUT_CHUNK: usize = 4 * 1024;
const MAX_QUEUED_INPUT: usize = 64 * 1024;
const MAX_REQUEST_BYTES: usize = 32 * 1024;

unsafe extern "C" {
    fn geteuid() -> u32;
    fn getsid(pid: i32) -> i32;
    fn getsockopt(
        fd: std::os::raw::c_int,
        level: std::os::raw::c_int,
        name: std::os::raw::c_int,
        value: *mut std::os::raw::c_void,
        len: *mut u32,
    ) -> std::os::raw::c_int;
}

/// Who is on the other end of a bridge connection, relative to the hosted session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Caller {
    /// A process outside the hosted program's session: the user's own terminal, the host UI.
    Outside,
    /// The hosted program or something it started. It shares the session's socket path (it knows
    /// its own `VERB_SESSION_ID`) and runs as the same account, so only this check keeps it from
    /// pairing itself as "the phone" and pushing keystrokes into its own terminal.
    InsideSession,
    /// The peer could not be identified. Treated as inside: unknown is not permission.
    Unknown,
}

fn peer_pid(stream: &UnixStream) -> Option<i32> {
    use std::os::unix::io::AsRawFd;
    #[cfg(target_os = "linux")]
    {
        #[repr(C)]
        struct Credentials {
            pid: i32,
            uid: u32,
            gid: u32,
        }
        let mut credentials = Credentials {
            pid: 0,
            uid: 0,
            gid: 0,
        };
        let mut len = std::mem::size_of::<Credentials>() as u32;
        // SOL_SOCKET = 1, SO_PEERCRED = 17.
        let result = unsafe {
            getsockopt(
                stream.as_raw_fd(),
                1,
                17,
                std::ptr::from_mut(&mut credentials).cast(),
                &mut len,
            )
        };
        (result == 0 && credentials.pid > 0).then_some(credentials.pid)
    }
    #[cfg(target_os = "macos")]
    {
        let mut pid: i32 = 0;
        let mut len = std::mem::size_of::<i32>() as u32;
        // SOL_LOCAL = 0, LOCAL_PEERPID = 2.
        let result = unsafe {
            getsockopt(
                stream.as_raw_fd(),
                0,
                2,
                std::ptr::from_mut(&mut pid).cast(),
                &mut len,
            )
        };
        (result == 0 && pid > 0).then_some(pid)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = stream;
        None
    }
}

/// `forkpty` makes the hosted child a session leader, so its pid is the session id every process
/// it starts inherits (unless one deliberately calls `setsid`, which this cannot see).
fn classify(stream: &UnixStream, hosted: Option<i32>) -> Caller {
    let Some(hosted) = hosted else {
        return Caller::Outside;
    };
    match peer_pid(stream) {
        Some(pid) => {
            let session = unsafe { getsid(pid) };
            if session < 0 {
                Caller::Unknown
            } else if session == hosted {
                Caller::InsideSession
            } else {
                Caller::Outside
            }
        }
        None => Caller::Unknown,
    }
}

#[derive(Clone)]
pub(crate) struct LiveBridge(Arc<Mutex<BridgeState>>);

struct BridgeState {
    session_id: String,
    ended: bool,
    offer: Option<Offer>,
    paired_digest: Option<[u8; 32]>,
    connected: bool,
    last_contact: Option<Instant>,
    controller: Controller,
    generation: u64,
    queued_input: VecDeque<QueuedInput>,
    queued_bytes: usize,
    screen: Vec<u8>,
    screen_revision: u64,
    screen_available: bool,
}

struct Offer {
    digest: [u8; 32],
    expires_at: Instant,
    failed_attempts: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Controller {
    Desktop,
    Phone,
}

struct QueuedInput {
    generation: u64,
    bytes: Vec<u8>,
}

/// A complete current-screen view, rather than a transcript or an unbounded output stream.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScreenSnapshot {
    pub session_id: String,
    pub revision: u64,
    pub bytes: Option<Vec<u8>>,
    pub controller: Controller,
}

impl LiveBridge {
    pub(crate) fn new(session_id: String) -> Self {
        Self(Arc::new(Mutex::new(BridgeState {
            session_id,
            ended: false,
            offer: None,
            paired_digest: None,
            connected: false,
            last_contact: None,
            controller: Controller::Desktop,
            generation: 0,
            queued_input: VecDeque::new(),
            queued_bytes: 0,
            screen: Vec::new(),
            screen_revision: 0,
            screen_available: true,
        })))
    }

    /// Generate a cryptographically random, one-use pairing secret. A UI can encode it as a QR
    /// code; a transport must never put it in a URL query, event log, or unencrypted channel.
    pub(crate) fn offer(&self, now: Instant) -> Result<String, String> {
        let secret = random_secret()?;
        let mut state = self.lock()?;
        state.ensure_live()?;
        state.offer = Some(Offer {
            digest: digest(&secret),
            expires_at: now + OFFER_LIFETIME,
            failed_attempts: 0,
        });
        if state.paired_digest.is_none() {
            state.screen_revision = 0;
            state.screen.clear();
        }
        Ok(secret)
    }

    /// Exchange the pairing secret for a device capability, valid until the desktop revokes it or
    /// this exact hosted process ends. Pairing another phone replaces the earlier device.
    pub(crate) fn pair(&self, secret: &str, now: Instant) -> Result<String, String> {
        let mut state = self.lock()?;
        state.ensure_live()?;
        let Some(offer) = state.offer.as_mut() else {
            return Err("no phone pairing offer is open".to_owned());
        };
        if now >= offer.expires_at {
            state.offer = None;
            return Err("phone pairing offer expired".to_owned());
        }
        if !constant_time_eq(&offer.digest, &digest(secret)) {
            offer.failed_attempts += 1;
            if offer.failed_attempts >= MAX_PAIR_ATTEMPTS {
                state.offer = None;
            }
            return Err("phone pairing code was not accepted".to_owned());
        }
        let device_secret = random_secret()?;
        state.offer = None;
        state.paired_digest = Some(digest(&device_secret));
        state.connected = true;
        state.last_contact = Some(now);
        state.return_control_to_desktop();
        Ok(device_secret)
    }

    pub(crate) fn reconnect(&self, device_secret: &str) -> Result<(), String> {
        let mut state = self.lock()?;
        state.ensure_device(device_secret)?;
        state.connected = true;
        state.last_contact = Some(Instant::now());
        Ok(())
    }

    pub(crate) fn disconnect(&self, device_secret: &str) -> Result<(), String> {
        let mut state = self.lock()?;
        state.ensure_device(device_secret)?;
        state.connected = false;
        state.last_contact = None;
        state.return_control_to_desktop();
        Ok(())
    }

    pub(crate) fn take_control(&self, device_secret: &str) -> Result<(), String> {
        let mut state = self.lock()?;
        state.ensure_device(device_secret)?;
        if !state.connected {
            return Err("phone is disconnected".to_owned());
        }
        state.last_contact = Some(Instant::now());
        state.controller = Controller::Phone;
        state.generation = state.generation.wrapping_add(1);
        state.clear_input();
        Ok(())
    }

    /// Desktop input always has a way back. Input queued under the previous lease is discarded.
    pub(crate) fn desktop_take_control(&self) -> Result<(), String> {
        let mut state = self.lock()?;
        state.ensure_live()?;
        state.return_control_to_desktop();
        Ok(())
    }

    pub(crate) fn revoke(&self) -> Result<(), String> {
        let mut state = self.lock()?;
        state.ensure_live()?;
        state.offer = None;
        state.paired_digest = None;
        state.connected = false;
        state.last_contact = None;
        state.return_control_to_desktop();
        Ok(())
    }

    pub(crate) fn submit_input(&self, device_secret: &str, bytes: &[u8]) -> Result<(), String> {
        let mut state = self.lock()?;
        state.ensure_device(device_secret)?;
        if !state.connected || state.controller != Controller::Phone {
            return Err("phone does not control this session".to_owned());
        }
        if bytes.is_empty() || bytes.len() > MAX_INPUT_CHUNK {
            return Err(format!("phone input must be 1–{MAX_INPUT_CHUNK} bytes"));
        }
        if state.queued_bytes + bytes.len() > MAX_QUEUED_INPUT {
            return Err("phone input queue is full; wait for the session".to_owned());
        }
        state.last_contact = Some(Instant::now());
        state.queued_bytes += bytes.len();
        let generation = state.generation;
        state.queued_input.push_back(QueuedInput {
            generation,
            bytes: bytes.to_vec(),
        });
        Ok(())
    }

    /// Called only by the PTY host. Holding the lease lock through the write makes a take-control
    /// action order after any in-flight bytes; stale phone bytes cannot arrive after takeback.
    pub(crate) fn deliver_phone_input(&self, writer: &mut impl Write) -> Result<(), String> {
        let mut state = self.lock()?;
        if state.ended || !state.connected || state.controller != Controller::Phone {
            state.clear_input();
            return Ok(());
        }
        let generation = state.generation;
        while let Some(chunk) = state.queued_input.pop_front() {
            if chunk.generation == generation {
                writer
                    .write_all(&chunk.bytes)
                    .map_err(|error| format!("could not deliver phone input: {error}"))?;
            }
        }
        state.queued_bytes = 0;
        Ok(())
    }

    /// The desktop input path uses the same lock as phone input and control transfers.
    pub(crate) fn write_desktop(
        &self,
        writer: &mut impl Write,
        bytes: &[u8],
    ) -> Result<(), String> {
        let state = self.lock()?;
        if state.ended || state.controller != Controller::Desktop {
            return Err(
                "phone controls input for this session; take control back first".to_owned(),
            );
        }
        writer
            .write_all(bytes)
            .map_err(|error| format!("could not write to the session: {error}"))
    }

    pub(crate) fn wants_screen(&self) -> Result<bool, String> {
        let state = self.lock()?;
        Ok(!state.ended && (state.offer.is_some() || state.paired_digest.is_some()))
    }

    pub(crate) fn needs_initial_screen(&self) -> Result<bool, String> {
        let state = self.lock()?;
        Ok(!state.ended
            && state.screen_revision == 0
            && (state.offer.is_some() || state.paired_digest.is_some()))
    }

    /// Publish the current parsed terminal screen. Oversized views are marked unavailable rather
    /// than truncated into a misleading partial terminal. No raw output history is retained.
    pub(crate) fn publish_screen(&self, formatted: Vec<u8>) -> Result<(), String> {
        let mut state = self.lock()?;
        if state.ended || (state.offer.is_none() && state.paired_digest.is_none()) {
            return Ok(());
        }
        state.screen_revision = state.screen_revision.wrapping_add(1);
        state.screen_available = formatted.len() <= MAX_SCREEN_BYTES;
        state.screen = if state.screen_available {
            formatted
        } else {
            Vec::new()
        };
        Ok(())
    }

    pub(crate) fn snapshot(&self, device_secret: &str) -> Result<ScreenSnapshot, String> {
        let mut state = self.lock()?;
        state.ensure_device(device_secret)?;
        if !state.connected {
            return Err("phone is disconnected".to_owned());
        }
        state.last_contact = Some(Instant::now());
        Ok(ScreenSnapshot {
            session_id: state.session_id.clone(),
            revision: state.screen_revision,
            bytes: state.screen_available.then(|| state.screen.clone()),
            controller: state.controller,
        })
    }

    /// The bridge has exactly the lifetime of the hosted process. No token can revive a session.
    pub(crate) fn end(&self) -> Result<(), String> {
        let mut state = self.lock()?;
        state.ended = true;
        state.offer = None;
        state.paired_digest = None;
        state.connected = false;
        state.last_contact = None;
        state.clear_input();
        state.screen.clear();
        state.screen_available = false;
        state.controller = Controller::Desktop;
        Ok(())
    }

    /// A missing heartbeat is a disconnect, never evidence that a desktop process stopped.
    pub(crate) fn expire_idle(&self, now: Instant) -> Result<(), String> {
        let mut state = self.lock()?;
        if state
            .offer
            .as_ref()
            .is_some_and(|offer| now >= offer.expires_at)
        {
            state.offer = None;
            if state.paired_digest.is_none() {
                state.screen.clear();
            }
        }
        if state.connected
            && state
                .last_contact
                .is_some_and(|last| now.saturating_duration_since(last) >= IDLE_TIMEOUT)
        {
            state.connected = false;
            state.last_contact = None;
            state.return_control_to_desktop();
        }
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, BridgeState>, String> {
        self.0
            .lock()
            .map_err(|_| "phone bridge lock failed".to_owned())
    }
}

impl BridgeState {
    fn ensure_live(&self) -> Result<(), String> {
        if self.ended {
            Err("this session has ended".to_owned())
        } else {
            Ok(())
        }
    }

    fn ensure_device(&self, secret: &str) -> Result<(), String> {
        self.ensure_live()?;
        if self
            .paired_digest
            .as_ref()
            .is_some_and(|known| constant_time_eq(known, &digest(secret)))
        {
            Ok(())
        } else {
            Err("phone is not paired with this session".to_owned())
        }
    }

    fn clear_input(&mut self) {
        self.queued_input.clear();
        self.queued_bytes = 0;
    }

    fn return_control_to_desktop(&mut self) {
        self.controller = Controller::Desktop;
        self.generation = self.generation.wrapping_add(1);
        self.clear_input();
    }
}

/// One owner-only local IPC endpoint per live PTY. It is an adapter for the bridge, not a
/// network listener. A future mobile transport can invoke this protocol through an authenticated
/// channel without moving the agent process or giving a remote record execution authority.
pub(crate) struct LocalServer {
    path: PathBuf,
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: u8,
    op: String,
    #[serde(default)]
    secret: Option<String>,
    #[serde(default)]
    bytes: Option<Vec<u8>>,
}

impl LocalServer {
    /// `hosted_pid` is the hosted program's pid, which is also its session id; connections from
    /// inside that session may not create offers or move input control.
    pub(crate) fn bind(
        session_id: &str,
        bridge: LiveBridge,
        hosted_pid: i32,
    ) -> Result<Self, String> {
        Self::bind_at(socket_path(session_id)?, bridge, Some(hosted_pid))
    }

    fn bind_at(path: PathBuf, bridge: LiveBridge, hosted: Option<i32>) -> Result<Self, String> {
        let parent = path.parent().ok_or("invalid phone socket directory")?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create phone socket directory: {error}"))?;
        let metadata = fs::symlink_metadata(parent)
            .map_err(|error| format!("could not inspect phone socket directory: {error}"))?;
        if !metadata.file_type().is_dir() {
            return Err("phone socket directory is not a real directory".to_owned());
        }
        if metadata.uid() != unsafe { geteuid() } {
            return Err("phone socket directory belongs to another user".to_owned());
        }
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("could not protect phone socket directory: {error}"))?;
        if path.exists() {
            if UnixStream::connect(&path).is_ok() {
                return Err("this session already has a phone bridge".to_owned());
            }
            fs::remove_file(&path)
                .map_err(|error| format!("could not replace a stale phone socket: {error}"))?;
        }
        let listener = UnixListener::bind(&path)
            .map_err(|error| format!("could not open the local phone bridge: {error}"))?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("could not protect the local phone bridge: {error}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("could not start the local phone bridge: {error}"))?;
        let running = Arc::new(AtomicBool::new(true));
        let alive = Arc::clone(&running);
        let thread = thread::spawn(move || {
            while alive.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                        let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                        let caller = classify(&stream, hosted);
                        let response = match read_request(&mut stream)
                            .and_then(|request| process_request(&bridge, request, caller))
                        {
                            Ok(result) => {
                                serde_json::json!({"version":1,"ok":true,"result":result})
                            }
                            Err(error) => serde_json::json!({"version":1,"ok":false,"error":error}),
                        };
                        if let Ok(mut bytes) = serde_json::to_vec(&response) {
                            bytes.push(b'\n');
                            let _ = stream.write_all(&bytes);
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(25));
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            path,
            running,
            thread: Some(thread),
        })
    }
}

impl Drop for LocalServer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        // Only the socket is ours. The per-user directory is shared by every Verb session on this
        // account, so removing it here raced another session between its `create_dir_all` and its
        // `bind`, and that session failed to start its bridge.
        let _ = fs::remove_file(&self.path);
    }
}

fn socket_path(session_id: &str) -> Result<PathBuf, String> {
    socket_path_in(&crate::state_root()?, session_id)
}

fn socket_path_in(root: &Path, session_id: &str) -> Result<PathBuf, String> {
    if session_id.is_empty()
        || session_id.len() > 128
        || !session_id.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err("invalid session id for the phone bridge".to_owned());
    }
    let mut hasher = Sha256::new();
    hasher.update(root.as_os_str().as_bytes());
    hasher.update([0]);
    hasher.update(session_id.as_bytes());
    let digest = hasher.finalize();
    let short: String = digest[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok(
        PathBuf::from(format!("/tmp/verb-mobile-{}", unsafe { geteuid() }))
            .join(format!("{short}.sock")),
    )
}

fn read_request(stream: &mut UnixStream) -> Result<Request, String> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let count = stream
            .read(&mut buffer)
            .map_err(|error| format!("could not read phone request: {error}"))?;
        if count == 0 {
            return Err("phone request ended before its newline".to_owned());
        }
        bytes.extend_from_slice(&buffer[..count]);
        if bytes.len() > MAX_REQUEST_BYTES {
            return Err("phone request is too large".to_owned());
        }
        if let Some(end) = bytes.iter().position(|byte| *byte == b'\n') {
            if end + 1 != bytes.len() {
                return Err("phone bridge accepts one request per connection".to_owned());
            }
            return serde_json::from_slice(&bytes[..end])
                .map_err(|error| format!("invalid phone request: {error}"));
        }
    }
}

fn process_request(
    bridge: &LiveBridge,
    request: Request,
    caller: Caller,
) -> Result<serde_json::Value, String> {
    if request.version != 1 {
        return Err("unsupported phone bridge protocol version".to_owned());
    }
    // Offers and control moves are the desktop user's decisions. The hosted program may not make
    // them for itself: `docs/DESKTOP_MOBILE_BRIDGE_PROTOCOL.md`, "the session ID is never enough to
    // authorize phone input".
    if matches!(request.op.as_str(), "offer" | "desktop_take" | "revoke")
        && caller != Caller::Outside
    {
        return Err("only the desktop user can do that, not the hosted session itself".to_owned());
    }
    let secret = || {
        request
            .secret
            .as_deref()
            .ok_or("phone token is required".to_owned())
    };
    match request.op.as_str() {
        "offer" => Ok(serde_json::json!({"pairingToken":bridge.offer(Instant::now())?})),
        "pair" => Ok(serde_json::json!({"deviceToken":bridge.pair(secret()?, Instant::now())?})),
        "reconnect" => {
            bridge.reconnect(secret()?)?;
            Ok(serde_json::json!({"status":"connected"}))
        }
        "disconnect" => {
            bridge.disconnect(secret()?)?;
            Ok(serde_json::json!({"status":"disconnected"}))
        }
        "take" => {
            bridge.take_control(secret()?)?;
            Ok(serde_json::json!({"controller":"phone"}))
        }
        "desktop_take" => {
            bridge.desktop_take_control()?;
            Ok(serde_json::json!({"controller":"desktop"}))
        }
        "revoke" => {
            bridge.revoke()?;
            Ok(serde_json::json!({"status":"revoked"}))
        }
        "input" => {
            bridge.submit_input(secret()?, request.bytes.as_deref().unwrap_or(&[]))?;
            Ok(serde_json::json!({"status":"queued"}))
        }
        "snapshot" => Ok(serde_json::to_value(bridge.snapshot(secret()?)?)
            .map_err(|error| format!("could not encode phone screen: {error}"))?),
        _ => Err("unknown phone bridge operation".to_owned()),
    }
}

fn request_local(session_id: &str, request: &[u8]) -> Result<serde_json::Value, String> {
    request_path(&socket_path(session_id)?, request)
}

fn request_path(path: &Path, request: &[u8]) -> Result<serde_json::Value, String> {
    if request.len() > MAX_REQUEST_BYTES || !request.ends_with(b"\n") {
        return Err("phone request must be one bounded JSON line".to_owned());
    }
    let mut stream = UnixStream::connect(path)
        .map_err(|error| format!("session has no live local phone bridge: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|error| format!("could not set phone bridge timeout: {error}"))?;
    stream
        .write_all(request)
        .map_err(|error| format!("could not send phone request: {error}"))?;
    let mut response = Vec::new();
    BufReader::new(stream)
        .take((MAX_SCREEN_BYTES * 5) as u64)
        .read_to_end(&mut response)
        .map_err(|error| format!("could not read phone response: {error}"))?;
    if !response.ends_with(b"\n") {
        return Err("phone response was incomplete".to_owned());
    }
    let response: serde_json::Value = serde_json::from_slice(&response)
        .map_err(|error| format!("invalid phone response: {error}"))?;
    if response["version"] != 1 {
        return Err("unsupported phone bridge response version".to_owned());
    }
    if response["ok"] != true {
        return Err(response["error"]
            .as_str()
            .unwrap_or("phone bridge rejected the request")
            .to_owned());
    }
    Ok(response["result"].clone())
}

pub(crate) fn command(args: &[String]) -> Result<(), String> {
    match args {
        [action, session_id] if action == "offer" => {
            let result = request_local(session_id, b"{\"version\":1,\"op\":\"offer\"}\n")?;
            println!("{}", result["pairingToken"].as_str().unwrap_or_default());
            Ok(())
        }
        [action, session_id] if action == "request" => {
            let mut request = Vec::new();
            std::io::stdin()
                .take((MAX_REQUEST_BYTES + 1) as u64)
                .read_to_end(&mut request)
                .map_err(|error| format!("could not read phone request: {error}"))?;
            let result = request_local(session_id, &request)?;
            println!("{result}");
            Ok(())
        }
        _ => Err(
            "usage: verb mobile offer SESSION_ID | request SESSION_ID < request.jsonl".to_owned(),
        ),
    }
}

fn random_secret() -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    File::open("/dev/urandom")
        .and_then(|mut source| source.read_exact(&mut bytes))
        .map_err(|error| format!("could not create a secure pairing secret: {error}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn digest(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}

fn constant_time_eq(left: &[u8; 32], right: &[u8; 32]) -> bool {
    let mut difference = 0_u8;
    for (left, right) in left.iter().zip(right.iter()) {
        difference |= left ^ right;
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(path: &Path, mut request: serde_json::Value) -> Result<serde_json::Value, String> {
        request["version"] = serde_json::json!(1);
        let mut bytes = serde_json::to_vec(&request).unwrap();
        bytes.push(b'\n');
        request_path(path, &bytes)
    }

    #[test]
    fn a_paired_phone_can_reconnect_without_creating_or_resuming_a_session() {
        let now = Instant::now();
        let bridge = LiveBridge::new("session-one".to_owned());
        let offer = bridge.offer(now).unwrap();
        assert_eq!(offer.len(), 32);
        let phone = bridge.pair(&offer, now).unwrap();
        bridge.publish_screen(b"screen one".to_vec()).unwrap();
        let first = bridge.snapshot(&phone).unwrap();
        assert_eq!(first.session_id, "session-one");
        assert_eq!(first.bytes.unwrap(), b"screen one");
        assert_eq!(first.revision, 1);
        assert_eq!(first.controller, Controller::Desktop);
        bridge.disconnect(&phone).unwrap();
        assert!(bridge.snapshot(&phone).is_err());
        bridge.reconnect(&phone).unwrap();
        assert_eq!(bridge.snapshot(&phone).unwrap().session_id, "session-one");
        bridge.end().unwrap();
        assert!(bridge.reconnect(&phone).is_err());
    }

    #[test]
    fn the_control_lease_prevents_two_devices_from_typing() {
        let now = Instant::now();
        let bridge = LiveBridge::new("session-two".to_owned());
        let offer = bridge.offer(now).unwrap();
        let phone = bridge.pair(&offer, now).unwrap();
        assert!(bridge.submit_input(&phone, b"before control").is_err());
        bridge.take_control(&phone).unwrap();
        assert!(bridge.write_desktop(&mut Vec::new(), b"blocked").is_err());
        bridge.submit_input(&phone, b"kept").unwrap();
        let mut written = Vec::new();
        bridge.deliver_phone_input(&mut written).unwrap();
        assert_eq!(written, b"kept");
        bridge.submit_input(&phone, b"stale").unwrap();
        bridge.desktop_take_control().unwrap();
        bridge.write_desktop(&mut written, b"desktop").unwrap();
        bridge.deliver_phone_input(&mut written).unwrap();
        assert_eq!(written, b"keptdesktop");
        assert!(bridge.submit_input(&phone, b"rejected").is_err());
        bridge.take_control(&phone).unwrap();
        bridge.disconnect(&phone).unwrap();
        bridge.write_desktop(&mut written, b"again").unwrap();
        bridge.deliver_phone_input(&mut written).unwrap();
        assert_eq!(written, b"keptdesktopagain");
    }

    #[test]
    fn expired_or_guessed_offers_fail_and_revocation_ends_access() {
        let now = Instant::now();
        let bridge = LiveBridge::new("session-three".to_owned());
        let offer = bridge.offer(now).unwrap();
        assert!(bridge.pair("wrong", now).is_err());
        assert!(bridge.pair(&offer, now + OFFER_LIFETIME).is_err());
        assert!(bridge.pair(&offer, now).is_err());
        let offer = bridge.offer(now).unwrap();
        let phone = bridge.pair(&offer, now).unwrap();
        bridge.revoke().unwrap();
        assert!(bridge.snapshot(&phone).is_err());
        assert!(bridge.reconnect(&phone).is_err());
    }

    #[test]
    fn an_idle_phone_loses_input_control_without_ending_the_agent() {
        let now = Instant::now();
        let bridge = LiveBridge::new("still-running".to_owned());
        let offer = bridge.offer(now).unwrap();
        let phone = bridge.pair(&offer, now).unwrap();
        bridge.take_control(&phone).unwrap();
        bridge.submit_input(&phone, b"do not send later").unwrap();
        bridge
            .expire_idle(now + IDLE_TIMEOUT + Duration::from_secs(1))
            .unwrap();
        let mut pty_input = Vec::new();
        bridge.deliver_phone_input(&mut pty_input).unwrap();
        assert!(pty_input.is_empty());
        bridge
            .write_desktop(&mut pty_input, b"desktop works")
            .unwrap();
        assert_eq!(pty_input, b"desktop works");
        assert!(bridge.snapshot(&phone).is_err());
        bridge.reconnect(&phone).unwrap();
        assert_eq!(bridge.snapshot(&phone).unwrap().session_id, "still-running");
    }

    #[test]
    fn snapshots_are_bounded_and_input_cannot_grow_without_limit() {
        let now = Instant::now();
        let bridge = LiveBridge::new("session-four".to_owned());
        let offer = bridge.offer(now).unwrap();
        let phone = bridge.pair(&offer, now).unwrap();
        bridge
            .publish_screen(vec![b'x'; MAX_SCREEN_BYTES + 1])
            .unwrap();
        assert!(bridge.snapshot(&phone).unwrap().bytes.is_none());
        bridge.take_control(&phone).unwrap();
        assert!(bridge
            .submit_input(&phone, &vec![b'x'; MAX_INPUT_CHUNK + 1])
            .is_err());
        for _ in 0..MAX_QUEUED_INPUT / MAX_INPUT_CHUNK {
            bridge
                .submit_input(&phone, &vec![b'x'; MAX_INPUT_CHUNK])
                .unwrap();
        }
        assert!(bridge.submit_input(&phone, b"overflow").is_err());
    }

    #[test]
    fn the_hosted_session_cannot_offer_pairing_or_move_control_itself() {
        let root = std::env::temp_dir().join(format!("verb-mobile-{}", random_secret().unwrap()));
        let path = socket_path_in(&root, "session-inside").unwrap();
        let bridge = LiveBridge::new("session-inside".to_owned());
        // Pretend this test process *is* the hosted session: bind with our own session id.
        let own_session = unsafe { getsid(0) };
        let server = LocalServer::bind_at(path.clone(), bridge.clone(), Some(own_session)).unwrap();
        for op in ["offer", "desktop_take", "revoke"] {
            let error = ask(&path, serde_json::json!({ "op": op })).unwrap_err();
            assert!(error.contains("only the desktop user"), "{op}: {error}");
        }
        drop(server);
    }

    #[test]
    fn local_process_protocol_pairs_and_controls_only_the_chosen_live_session() {
        let root = std::env::temp_dir().join(format!("verb-mobile-{}", random_secret().unwrap()));
        let path = socket_path_in(&root, "session-five").unwrap();
        let bridge = LiveBridge::new("session-five".to_owned());
        // A hosted pid that is not this test's session: the test process is "outside" it.
        let server = LocalServer::bind_at(path.clone(), bridge.clone(), Some(i32::MAX)).unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(request_path(&path, b"{\"version\":2,\"op\":\"offer\"}\n").is_err());
        let offer = ask(&path, serde_json::json!({"op":"offer"})).unwrap();
        let token = offer["pairingToken"].as_str().unwrap();
        assert!(ask(&path, serde_json::json!({"op":"pair","secret":"wrong"})).is_err());
        let paired = ask(&path, serde_json::json!({"op":"pair","secret":token})).unwrap();
        let device = paired["deviceToken"].as_str().unwrap();
        bridge.publish_screen(b"hello phone".to_vec()).unwrap();
        let snapshot = ask(&path, serde_json::json!({"op":"snapshot","secret":device})).unwrap();
        assert_eq!(snapshot["sessionId"], "session-five");
        assert_eq!(
            serde_json::from_value::<Vec<u8>>(snapshot["bytes"].clone()).unwrap(),
            b"hello phone"
        );
        ask(&path, serde_json::json!({"op":"take","secret":device})).unwrap();
        ask(
            &path,
            serde_json::json!({"op":"input","secret":device,"bytes":[104,105]}),
        )
        .unwrap();
        let mut pty_input = Vec::new();
        bridge.deliver_phone_input(&mut pty_input).unwrap();
        assert_eq!(pty_input, b"hi");
        ask(&path, serde_json::json!({"op":"desktop_take"})).unwrap();
        assert!(ask(
            &path,
            serde_json::json!({"op":"input","secret":device,"bytes":[33]})
        )
        .is_err());
        ask(&path, serde_json::json!({"op":"revoke"})).unwrap();
        assert!(ask(&path, serde_json::json!({"op":"snapshot","secret":device})).is_err());
        bridge.end().unwrap();
        drop(server);
        assert!(!path.exists());
        let _ = fs::remove_dir_all(root);
    }
}
