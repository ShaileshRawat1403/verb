//! Ephemeral, browser-only outbound companion. No project, terminal or credential authority.
use serde_json::{json, Value};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Default, Clone)]
pub(crate) struct Relay(Arc<Mutex<State>>);
#[derive(Default)]
struct State {
    pairing: Option<(String, Instant)>,
    connection: Option<Connection>,
    job: Option<Job>,
}
struct Connection {
    key: String,
    seen: Instant,
    expires: Instant,
    session: Option<String>,
}
struct Job {
    id: String,
    input: Value,
    delivered: bool,
    reply: mpsc::SyncSender<Value>,
}
impl State {
    fn prune(&mut self) {
        let now = Instant::now();
        if self
            .pairing
            .as_ref()
            .is_some_and(|(_, expiry)| *expiry <= now)
        {
            self.pairing = None;
        }
        if self.connection.as_ref().is_some_and(|c| {
            c.expires <= now || now.duration_since(c.seen) > Duration::from_secs(35)
        }) {
            self.connection = None;
            self.job = None;
        }
    }
}
impl Relay {
    pub(crate) fn status(&self) -> Value {
        let mut s = self.0.lock().unwrap_or_else(|p| p.into_inner());
        s.prune();
        json!({"connected":s.connection.is_some(),"pairing":s.pairing.is_some(),"location":"Paired Mac companion"})
    }
    pub(crate) fn pairing(&self, op: &str) -> Result<Value, String> {
        let mut s = self
            .0
            .lock()
            .map_err(|_| "Browser companion lock unavailable")?;
        s.prune();
        match op {
            "pair" => {
                if s.connection.is_some() {
                    return Err("Disconnect the existing companion before pairing another.".into());
                }
                let code = crate::web::secure_token()?;
                s.pairing = Some((code.clone(), Instant::now() + Duration::from_secs(120)));
                Ok(json!({"code":code,"expiresIn":120}))
            }
            "revoke" => {
                *s = State::default();
                Ok(json!({"connected":false,"revoked":true}))
            }
            _ => Err("Unknown pairing operation.".into()),
        }
    }
    pub(crate) fn accepts_key(&self, key: Option<&str>) -> bool {
        let mut s = self.0.lock().unwrap_or_else(|p| p.into_inner());
        s.prune();
        s.connection.as_ref().is_some_and(|c| {
            crate::tokens::constant_time_eq(c.key.as_bytes(), key.unwrap_or("").as_bytes())
        })
    }
    pub(crate) fn peer(&self, key: Option<&str>, input: Value) -> Result<Value, String> {
        let mut s = self
            .0
            .lock()
            .map_err(|_| "Browser companion lock unavailable")?;
        s.prune();
        if input["op"] == "claim" {
            let valid = s.pairing.as_ref().is_some_and(|(code, _)| {
                crate::tokens::constant_time_eq(
                    code.as_bytes(),
                    input["code"].as_str().unwrap_or("").as_bytes(),
                )
            });
            if !valid || s.connection.is_some() {
                return Err("Pairing code expired or was already used.".into());
            }
            s.pairing = None;
            let secret = crate::web::secure_token()?;
            s.connection = Some(Connection {
                key: secret.clone(),
                seen: Instant::now(),
                expires: Instant::now() + Duration::from_secs(8 * 3600),
                session: None,
            });
            return Ok(json!({"key":secret,"expiresIn":8*3600}));
        }
        let valid = s.connection.as_ref().is_some_and(|c| {
            crate::tokens::constant_time_eq(c.key.as_bytes(), key.unwrap_or("").as_bytes())
        });
        if !valid {
            return Err(
                "Browser companion disconnected. Pair again; no action is replayed.".into(),
            );
        }
        s.connection.as_mut().unwrap().seen = Instant::now();
        match input["op"].as_str() {
            Some("poll") => {
                let job = s.job.as_mut().filter(|j| !j.delivered).map(|j| {
                    j.delivered = true;
                    json!({"id":j.id,"input":j.input})
                });
                Ok(json!({"job":job}))
            }
            Some("reply") => {
                if s.job
                    .as_ref()
                    .is_none_or(|j| j.id != input["id"].as_str().unwrap_or(""))
                {
                    return Ok(json!({"ok":false,"discarded":true}));
                }
                let job = s.job.take().unwrap();
                if !job.delivered {
                    return Err("Browser action was not delivered.".into());
                }
                let value = input["result"].clone();
                if !value.is_object() {
                    return Err("Invalid browser result.".into());
                }
                if value["running"] == true {
                    let session = value["session"]
                        .as_str()
                        .filter(|v| v.len() <= 128 && !v.is_empty())
                        .ok_or("Browser session identity missing")?;
                    s.connection.as_mut().unwrap().session = Some(session.to_owned());
                } else if value["running"] == false {
                    s.connection.as_mut().unwrap().session = None;
                }
                let _ = job.reply.send(value);
                Ok(json!({"ok":true}))
            }
            Some("disconnect") => {
                *s = State::default();
                Ok(json!({"ok":true}))
            }
            _ => Err("Unsupported companion operation.".into()),
        }
    }
    pub(crate) fn command(&self, input: Value) -> Result<Value, String> {
        if serde_json::to_vec(&input)
            .map_err(|_| "Invalid browser action")?
            .len()
            > 3500
        {
            return Err("Browser command is too large.".into());
        }
        let op = input["op"].as_str().ok_or("Browser operation required")?;
        if !matches!(
            op,
            "start"
                | "status"
                | "stop"
                | "capture"
                | "take"
                | "open"
                | "grant"
                | "back"
                | "forward"
                | "reload"
                | "tab"
                | "new"
                | "close"
                | "viewport"
                | "click"
                | "type"
                | "key"
                | "scroll"
                | "inspect"
        ) {
            return Err("Unsupported browser operation.".into());
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let id = crate::web::secure_token()?;
        {
            let mut s = self
                .0
                .lock()
                .map_err(|_| "Browser companion lock unavailable")?;
            s.prune();
            let c = s
                .connection
                .as_ref()
                .ok_or("Mac browser companion is offline. Pair it again; nothing was sent.")?;
            if !matches!(op, "start" | "status")
                && c.session.as_deref() != input["session"].as_str()
            {
                return Err("Browser session changed. Reopen it before acting.".into());
            }
            if s.job.is_some() && op != "stop" {
                return Err("Browser is busy. Your action was not queued.".into());
            }
            s.job = Some(Job {
                id: id.clone(),
                input,
                delivered: false,
                reply: sender,
            });
        }
        match receiver.recv_timeout(Duration::from_secs(25)) {
            Ok(mut value) => {
                crate::browser::redact_view(&mut value);
                Ok(value)
            }
            Err(_) => {
                let mut s = self
                    .0
                    .lock()
                    .map_err(|_| "Browser companion lock unavailable")?;
                // A replaced Stop job must survive cancellation of the older waiter.
                if s.job.as_ref().is_some_and(|j| j.id == id) {
                    *s = State::default();
                }
                Err("Browser connection interrupted; action outcome unknown. No action was replayed.".into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn connect(r: &Relay) -> String {
        let p = r.pairing("pair").unwrap();
        r.peer(None, json!({"op":"claim","code":p["code"]}))
            .unwrap()["key"]
            .as_str()
            .unwrap()
            .into()
    }
    #[test]
    fn pairing_is_one_use_and_peer_key_has_no_owner_authority() {
        let r = Relay::default();
        let code = r.pairing("pair").unwrap()["code"].clone();
        assert!(r.peer(None, json!({"op":"claim","code":"wrong"})).is_err());
        let key = r.peer(None, json!({"op":"claim","code":code})).unwrap()["key"]
            .as_str()
            .unwrap()
            .to_owned();
        assert!(r.peer(None, json!({"op":"claim","code":code})).is_err());
        assert!(r.peer(Some("wrong"), json!({"op":"poll"})).is_err());
        assert_eq!(
            r.peer(Some(&key), json!({"op":"poll"})).unwrap()["job"],
            Value::Null
        );
        r.pairing("revoke").unwrap();
        assert!(r.peer(Some(&key), json!({"op":"poll"})).is_err());
    }
    #[test]
    fn command_is_delivered_only_once_and_revocation_unblocks_waiter() {
        let r = Relay::default();
        let key = connect(&r);
        let task = r.clone();
        let thread =
            std::thread::spawn(move || task.command(json!({"op":"start","client":"test-client"})));
        let job = loop {
            let p = r.peer(Some(&key), json!({"op":"poll"})).unwrap();
            if p["job"].is_object() {
                break p["job"].clone();
            }
            std::thread::yield_now();
        };
        assert!(r.peer(Some(&key), json!({"op":"poll"})).unwrap()["job"].is_null());
        r.peer(
            Some(&key),
            json!({"op":"reply","id":job["id"],"result":{"running":true,"session":"test-session"}}),
        )
        .unwrap();
        assert_eq!(thread.join().unwrap().unwrap()["session"], "test-session");
        assert!(r.command(json!({"op":"click","session":"stale"})).is_err());
        let task = r.clone();
        let thread = std::thread::spawn(move || task.command(json!({"op":"status"})));
        loop {
            if r.peer(Some(&key), json!({"op":"poll"})).unwrap()["job"].is_object() {
                break;
            }
            std::thread::yield_now();
        }
        r.pairing("revoke").unwrap();
        assert!(thread.join().unwrap().is_err());
    }
    #[test]
    fn expired_pair_and_lost_heartbeat_fail_closed() {
        let r = Relay::default();
        r.pairing("pair").unwrap();
        r.0.lock().unwrap().pairing.as_mut().unwrap().1 = Instant::now() - Duration::from_secs(1);
        assert!(r
            .peer(None, json!({"op":"claim","code":"expired"}))
            .is_err());
        let key = connect(&r);
        r.0.lock().unwrap().connection.as_mut().unwrap().seen =
            Instant::now() - Duration::from_secs(36);
        assert!(!r.status()["connected"].as_bool().unwrap());
        assert!(r.peer(Some(&key), json!({"op":"poll"})).is_err());
    }
}
