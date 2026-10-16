//! Encrypted, session-scoped same-network transport for Verb Mobile.
//!
//! The PTY host remains the only process that reads the terminal or writes its master. This relay
//! accepts a bounded request over TLS, allows only phone operations, and forwards it to the
//! owner-only Unix bridge. A one-use offer and the exact certificate fingerprint are shown to the
//! desktop user as one pairing link. Neither native agent state nor credentials cross the wire.

use crate::mobile;
use if_addrs::{get_if_addrs, IfAddr};
use rcgen::generate_simple_self_signed;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAX_REQUEST: usize = 32 * 1024;
const MAX_REPLY: usize = 2 * 1024 * 1024;

pub(crate) struct PhoneShare {
    pub links: Vec<String>,
    pub offer_expires_at: u64,
    link_bases: Vec<String>,
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    session_id: String,
    _share_lock: File,
}

impl PhoneShare {
    pub(crate) fn start(session_id: &str) -> Result<Self, String> {
        let share_lock = claim_share(session_id)?;
        let addresses = lan_addresses()?;
        let names: Vec<String> = addresses.iter().map(ToString::to_string).collect();
        let certified = generate_simple_self_signed(names)
            .map_err(|error| format!("could not create phone TLS identity: {error}"))?;
        let certificate: CertificateDer<'static> = certified.cert.der().clone();
        let fingerprint: String = Sha256::digest(certificate.as_ref())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
            certified.signing_key.serialize_der(),
        ));
        let config = Arc::new(
            ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(vec![certificate], key)
                .map_err(|error| format!("could not configure phone TLS: {error}"))?,
        );
        let listener = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))
            .map_err(|error| format!("could not listen for the phone: {error}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| format!("could not start phone listener: {error}"))?;
        let port = listener
            .local_addr()
            .map_err(|error| format!("could not read phone listener address: {error}"))?
            .port();
        let link_bases: Vec<String> = addresses
            .iter()
            .map(|address| format!("verb://pair#host={address}&port={port}&pin={fingerprint}"))
            .collect();
        let (links, offer_expires_at) = make_offer(session_id, &link_bases)?;
        let running = Arc::new(AtomicBool::new(true));
        let active = Arc::clone(&running);
        let owned_session = session_id.to_owned();
        let thread = thread::spawn(move || {
            while active.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let _ = handle_connection(stream, &config, &owned_session);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(30));
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            links,
            offer_expires_at,
            link_bases,
            running,
            thread: Some(thread),
            session_id: session_id.to_owned(),
            _share_lock: share_lock,
        })
    }

    /// A fresh one-use code leaves the current paired phone connected until a new phone pairs.
    pub(crate) fn renew_offer(&mut self) -> Result<(), String> {
        let (links, offer_expires_at) = make_offer(&self.session_id, &self.link_bases)?;
        self.links = links;
        self.offer_expires_at = offer_expires_at;
        Ok(())
    }

    pub(crate) fn status(&self) -> Result<serde_json::Value, String> {
        mobile::request_local(&self.session_id, b"{\"version\":1,\"op\":\"status\"}\n")
    }
}

fn make_offer(session_id: &str, link_bases: &[String]) -> Result<(Vec<String>, u64), String> {
    let offer = mobile::request_local(session_id, b"{\"version\":1,\"op\":\"offer\"}\n")?;
    let code = offer["pairingToken"]
        .as_str()
        .filter(|value| value.len() == 32)
        .ok_or("local phone bridge returned no pairing code")?;
    let expires_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("could not read pairing expiry: {error}"))?
        .as_secs()
        + 120;
    Ok((
        link_bases
            .iter()
            .map(|base| format!("{base}&code={code}"))
            .collect(),
        expires_at,
    ))
}

fn claim_share(session_id: &str) -> Result<File, String> {
    let directory = crate::state_root()?.join("phone-share-locks");
    fs::create_dir_all(&directory)
        .map_err(|error| format!("could not create phone share lock directory: {error}"))?;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("could not protect phone share lock directory: {error}"))?;
    let name: String = Sha256::digest(session_id.as_bytes())[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(directory.join(format!("{name}.lock")))
        .map_err(|error| format!("could not open phone share lock: {error}"))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(std::fs::TryLockError::WouldBlock) => {
            Err("this terminal is already shared with a phone".to_owned())
        }
        Err(std::fs::TryLockError::Error(error)) => {
            Err(format!("could not claim phone share: {error}"))
        }
    }
}

impl Drop for PhoneShare {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        let _ = mobile::request_local(&self.session_id, b"{\"version\":1,\"op\":\"revoke\"}\n");
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn lan_addresses() -> Result<Vec<Ipv4Addr>, String> {
    let mut addresses: Vec<(u8, Ipv4Addr)> = get_if_addrs()
        .map_err(|error| format!("could not find this desktop's network address: {error}"))?
        .into_iter()
        .filter_map(|interface| match interface.addr {
            IfAddr::V4(v4) if !v4.ip.is_loopback() && !v4.ip.is_link_local() => {
                let name = interface.name.to_ascii_lowercase();
                let rank = if name == "en0" || name.starts_with("wlan") || name.starts_with("eth") {
                    0
                } else if name.starts_with("en") {
                    1
                } else {
                    2
                };
                Some((rank, v4.ip))
            }
            _ => None,
        })
        .collect();
    addresses.sort_by_key(|(rank, ip)| (*rank, !ip.is_private(), *ip));
    let mut seen = std::collections::HashSet::new();
    let addresses: Vec<Ipv4Addr> = addresses
        .into_iter()
        .map(|(_, ip)| ip)
        .filter(|ip| seen.insert(*ip))
        .collect();
    if addresses.is_empty() {
        return Err(
            "connect this desktop and phone to the same network, then try again".to_owned(),
        );
    }
    Ok(addresses)
}

fn handle_connection(
    stream: TcpStream,
    config: &Arc<ServerConfig>,
    session_id: &str,
) -> Result<(), String> {
    stream
        .set_nonblocking(false)
        .map_err(|error| error.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|error| error.to_string())?;
    let connection = ServerConnection::new(Arc::clone(config))
        .map_err(|error| format!("phone TLS handshake failed: {error}"))?;
    let mut tls = StreamOwned::new(connection, stream);
    let reply = match read_line(&mut tls).and_then(|request| forward(session_id, &request)) {
        Ok(result) => json!({"version":1,"ok":true,"result":result}),
        Err(error) => json!({"version":1,"ok":false,"error":error}),
    };
    let bytes = serde_json::to_vec(&reply).map_err(|error| error.to_string())?;
    if bytes.len() >= MAX_REPLY {
        return Err("phone reply exceeded its limit".to_owned());
    }
    tls.write_all(&bytes).map_err(|error| error.to_string())?;
    tls.write_all(b"\n").map_err(|error| error.to_string())?;
    tls.flush().map_err(|error| error.to_string())?;
    Ok(())
}

fn read_line(stream: &mut impl Read) -> Result<Value, String> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let count = stream
            .read(&mut buffer)
            .map_err(|error| format!("could not read phone request: {error}"))?;
        if count == 0 {
            return Err("phone request ended early".to_owned());
        }
        bytes.extend_from_slice(&buffer[..count]);
        if bytes.len() > MAX_REQUEST {
            return Err("phone request is too large".to_owned());
        }
        if let Some(end) = bytes.iter().position(|byte| *byte == b'\n') {
            if end + 1 != bytes.len() {
                return Err("send one phone request per connection".to_owned());
            }
            return serde_json::from_slice(&bytes[..end])
                .map_err(|error| format!("invalid phone request: {error}"));
        }
    }
}

fn forward(session_id: &str, request: &Value) -> Result<Value, String> {
    let fields = request
        .as_object()
        .ok_or("phone request must be an object")?;
    if fields
        .keys()
        .any(|key| !matches!(key.as_str(), "version" | "op" | "secret" | "bytes"))
    {
        return Err("unknown phone request field".to_owned());
    }
    if request["version"].as_u64() != Some(1) {
        return Err("unsupported phone protocol version".to_owned());
    }
    let op = request["op"]
        .as_str()
        .ok_or("phone operation is required")?;
    if !matches!(
        op,
        "pair" | "reconnect" | "disconnect" | "take" | "input" | "snapshot"
    ) {
        return Err("this action belongs to the desktop".to_owned());
    }
    let secret = request["secret"]
        .as_str()
        .filter(|value| value.len() <= 128)
        .ok_or("phone token is required")?;
    if op != "input" && fields.contains_key("bytes") {
        return Err("this phone operation has no input bytes".to_owned());
    }
    let bytes: Vec<u8> = if op == "input" {
        serde_json::from_value(request["bytes"].clone()).map_err(|_| "phone input must be bytes")?
    } else {
        Vec::new()
    };
    if bytes.len() > 4 * 1024 {
        return Err("phone input is too long".to_owned());
    }
    let mut local = serde_json::to_vec(&json!({
        "version":1,"op":op,"secret":secret,"bytes":bytes
    }))
    .map_err(|error| error.to_string())?;
    local.push(b'\n');
    mobile::request_local(session_id, &local)
}

pub(crate) fn share_command(session_id: &str) -> Result<(), String> {
    let share = PhoneShare::start(session_id)?;
    println!("Open or scan a pairing link on your phone. This desktop keeps the agent running:");
    for link in &share.links {
        println!("{link}");
    }
    println!("Press Ctrl-C to stop sharing and revoke the phone.");
    let running = Arc::new(AtomicBool::new(true));
    let stop = Arc::clone(&running);
    ctrlc::set_handler(move || stop.store(false, Ordering::SeqCst))
        .map_err(|error| format!("could not watch for share shutdown: {error}"))?;
    while running.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(100));
    }
    drop(share);
    Ok(())
}
