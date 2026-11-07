//! HTTP-level checks of external terminal admission against a synthetic execution boundary.
//!
//! `verb web` runs with `VERB_SYNTHETIC_BOUNDARY` (test builds with the `synthetic-boundary`
//! feature only), so terminals go through the real external launch, admission and cleanup paths.
//! The stand-in client below plays the controller's part: what it does for each launch is read from
//! the `behavior` file, and every launch and stop it sees is appended to `log`. No provider, phone or
//! controller is involved.
#![cfg(all(target_os = "linux", feature = "synthetic-boundary"))]

use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// A registered project's id: the external path requires a 32-hex project directory name.
const PROJECT: &str = "0f1e2d3c4b5a69788796a5b4c3d2e1f0";

/// Stand-in for the boundary client. Arguments match the real client's:
/// `--project ID --session ID --mode terminal|pipes --program NAME -- ARGS...`, or
/// `--stop-client PID`. Receipts carry the same fields the real client writes after a lease.
const CLIENT: &str = r#"#!/bin/bash
root="$(cd "$(dirname "$0")" && pwd)"
if [ "$1" = --capabilities ]; then
  if [ -f "$root/capabilities" ]; then cat "$root/capabilities"; exit 0; fi
  exit 2 # Legacy client: no optional agents.
fi
receipt() { # pid session mtime
  printf '{"generation":"0123456789abcdef0123456789abcdef","operation":"stop","project":"%s","session":"%s","version":1}\n' \
    "$project" "$2" > "$root/receipts/.pending-$1"
  touch -d "$3" "$root/receipts/.pending-$1"
  mv "$root/receipts/.pending-$1" "$root/receipts/$1.json"
}
if [ "$1" = --stop-client ]; then
  echo "stop $2" >> "$root/log"
  if [ -f "$root/stop-fallback" ] && [ -f "$root/receipts/$2.json" ]; then
    mv "$root/receipts/$2.json" "$root/receipts/$2-0123456789abcdef0123456789abcdef.json"
  fi
  if [ -f "$root/stop-stall" ]; then sleep "$(cat "$root/stop-stall")"; exit 126; fi
  rm -f "$root/receipts/$2.json"
  exit 0
fi
while [ $# -gt 0 ]; do
  case "$1" in
    --project) project="$2"; shift 2 ;;
    --session) session="$2"; shift 2 ;;
    --mode) mode="$2"; shift 2 ;;
    --program) program="$2"; shift 2 ;;
    --) shift; break ;;
    *) exit 64 ;;
  esac
done
echo "launch $$ $mode $session" >> "$root/log"
if [ "$mode" != terminal ]; then
  if [ -f "$root/studio-enabled" ]; then
    case "$*" in
      *"<verb-studio-adapter>"*)
        if [ -f "$root/studio-reply" ]; then cat "$root/studio-reply"; exit 0; fi
        # Fixture executes the exact fixed guest command after the boundary arguments.
        exec /bin/bash "$@"
        ;;
    esac
  fi
  if [ "$program" = git ] && [ "$*" = "rev-parse --git-common-dir" ]; then
    echo "identity-probe" >> "$root/log"
  fi
  case "$*" in
    *"__project-read"*)
      if [ -f "$root/project-read-reply" ]; then cat "$root/project-read-reply"; exit 0; fi
      if [ -f "$root/project-read-capacity" ]; then echo "Verb execution unavailable: capacity" >&2; exit 75; fi
      if [ -f "$root/project-read-timeout" ]; then exit 124; fi
      if [ -f "$root/project-read-oversize" ]; then head -c 8388609 /dev/zero; exit 0; fi
      if [ -f "$root/project-read-stall" ]; then exec sleep 60; fi
      ;;
  esac
  # Background jobs (Git, file lists) never get a lease here; a stalled controller holds them.
  if [ -f "$root/pipes-stall" ]; then sleep "$(cat "$root/pipes-stall")"; fi
  echo "Verb execution connection failed; check the candidate controller." >&2
  exit 126
fi
read -r behavior delay < "$root/behavior"
case "$behavior" in
  admit)
    sleep "${delay:-0}"
    receipt $$ "$session" now
    exec "$(command -v "$program")" "$@" ;;
  refuse)
    echo "Verb execution unavailable: capacity" >&2
    exit 75 ;;
  fail)
    echo "Verb execution connection failed; check the candidate controller." >&2
    exit 126 ;;
  hang)
    exec sleep 1000 ;;
  reused-pid)
    # A receipt another session's client left under this PID yesterday.
    receipt $$ old-session "1 day ago"
    exec sleep 1000 ;;
  resumed-session)
    # This session's earlier run left a receipt under the PID this client now has.
    receipt $$ "$session" "1 minute ago"
    exec sleep 1000 ;;
esac
exit 70
"#;

struct Host {
    child: Child,
    port: u16,
    token: String,
    root: PathBuf,
}

impl Host {
    fn start(behavior: &str) -> Self {
        Self::start_with(behavior, &[])
    }

    fn start_with(behavior: &str, envs: &[(&str, &Path)]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "verb-synthetic-boundary-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        for dir in [PROJECT, "home", "boundary/receipts"] {
            fs::create_dir_all(root.join(dir)).unwrap();
        }
        let client = root.join("boundary/client");
        fs::write(&client, CLIENT).unwrap();
        fs::set_permissions(&client, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(root.join("boundary/log"), "").unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_verb"))
            .args(["web", "--port", "0"])
            .current_dir(root.join(PROJECT))
            .env("VERB_STATE_DIR", root.join("state"))
            .env("HOME", root.join("home"))
            .env("VERB_SYNTHETIC_BOUNDARY", root.join("boundary"))
            .envs(envs.iter().map(|(key, value)| (*key, *value)))
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        let url = line.strip_prefix("Verb web UI: ").unwrap().trim();
        let (origin, token) = url.split_once("/#").unwrap();
        let host = Self {
            child,
            port: origin.rsplit(':').next().unwrap().parse().unwrap(),
            token: token.to_owned(),
            root,
        };
        host.behave(behavior);
        host
    }

    fn behave(&self, behavior: &str) {
        fs::write(self.root.join("boundary/behavior"), format!("{behavior}\n")).unwrap();
    }

    fn request(&self, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .unwrap();
        let payload = body.map(|value| value.to_string()).unwrap_or_default();
        write!(
            stream,
            "{method} {path} HTTP/1.0\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\
             Origin: http://127.0.0.1:{port}\r\nX-Verb-Token: {token}\r\n\
             Content-Type: application/json\r\nContent-Length: {length}\r\n\r\n{payload}",
            port = self.port,
            token = self.token,
            length = payload.len(),
        )
        .unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        let text = String::from_utf8_lossy(&response);
        let status = text
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse().ok())
            .unwrap_or(0);
        let body = text
            .split_once("\r\n\r\n")
            .map(|(_, body)| body)
            .unwrap_or("");
        (status, serde_json::from_str(body).unwrap_or(Value::Null))
    }

    fn listed(&self) -> Vec<String> {
        let (_, workspace) = self.request("GET", "/api/workspace", None);
        workspace["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|session| session["id"].as_str().unwrap().to_owned())
            .collect()
    }

    fn log(&self) -> String {
        fs::read_to_string(self.root.join("boundary/log")).unwrap()
    }

    /// PIDs of terminal clients the stand-in saw launched, in order.
    fn client_pids(&self) -> Vec<i32> {
        self.log()
            .lines()
            .filter_map(|line| line.strip_prefix("launch "))
            .filter(|line| line.contains(" terminal "))
            .filter_map(|line| line.split_whitespace().next()?.parse().ok())
            .collect()
    }

    fn spec(&self) -> String {
        let (status, spec) = self.request(
            "POST",
            "/api/specs",
            Some(json!({"title": "Synthetic admission", "problem": "p", "criteria": ["works"]})),
        );
        assert_eq!(status, 201, "{spec}");
        spec["id"].as_str().unwrap().to_owned()
    }

    /// How many confirmed agent starts the spec's audit trail records.
    fn starts_recorded(&self) -> usize {
        let specs = self.root.join(PROJECT).join("specs");
        fs::read_dir(specs)
            .unwrap()
            .filter_map(|entry| fs::read_to_string(entry.unwrap().path()).ok())
            .map(|text| text.matches("on this spec (session").count())
            .sum()
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn alive(pid: i32) -> bool {
    // A zombie still has a /proc entry; only a running process counts.
    fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|stat| {
        !stat
            .rsplit_once(')')
            .unwrap()
            .1
            .trim_start()
            .starts_with('Z')
    })
}

fn wait_until(seconds: u64, done: impl Fn() -> bool) -> bool {
    let until = Instant::now() + Duration::from_secs(seconds);
    while Instant::now() < until {
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    done()
}

fn shell() -> Option<Value> {
    Some(json!({"agent": "shell", "isolated": false}))
}

#[test]
fn concurrent_custom_launches_reuse_the_hosts_identity_when_git_is_unavailable() {
    let host = Host::start("admit");
    let (_, workspace) = host.request("GET", "/api/workspace", None);
    let project_id = workspace["project"]["id"].as_str().unwrap().to_owned();
    // The host already established its identity. A Git marker plus the fixture's
    // failing background client now models an unavailable Git execution slot.
    fs::create_dir(host.root.join(PROJECT).join(".git")).unwrap();
    let probes_before = host.log().matches("identity-probe").count();
    let replies = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    host.request(
                        "POST",
                        "/api/terminals",
                        Some(json!({"agent":"custom", "command":"bash",
                            "args":["-c", "sleep 60"], "isolated":false})),
                    )
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>()
    });
    let records: Vec<_> = fs::read_dir(host.root.join("state/sessions"))
        .unwrap()
        .filter_map(|entry| fs::read_to_string(entry.ok()?.path()).ok())
        .collect();
    let mut sessions = Vec::new();
    for (status, reply) in replies {
        assert_eq!(status, 201, "{reply}");
        let id = reply["sessionId"].as_str().unwrap().to_owned();
        let record = records
            .iter()
            .find(|record| {
                record
                    .lines()
                    .any(|line| line == format!("session_id={id}"))
            })
            .expect("admitted custom session has a durable record");
        assert!(record
            .lines()
            .any(|line| line == format!("verb_project_id={project_id}")));
        sessions.push(id);
    }
    assert_eq!(host.log().matches("identity-probe").count(), probes_before);
    let listed = host.listed();
    assert!(sessions.iter().all(|id| listed.contains(id)));
    for id in sessions {
        assert_eq!(
            host.request("DELETE", &format!("/api/terminals/{id}"), None)
                .0,
            200
        );
    }
}

fn spec_agent() -> Option<Value> {
    Some(json!({"agent": "shell"}))
}

#[test]
fn delayed_admission_is_awaited_then_listed_and_recorded_once() {
    let host = Host::start("admit 1.5");
    let spec = host.spec();
    let started = Instant::now();
    let (status, body) = host.request("POST", &format!("/api/specs/{spec}/agent"), spec_agent());
    assert_eq!(status, 201, "{body}");
    assert!(
        started.elapsed() >= Duration::from_millis(1500),
        "answered before admission"
    );
    assert!(body.get("warning").is_none(), "{body}");
    let session = body["sessionId"].as_str().unwrap();
    assert!(host.listed().contains(&session.to_owned()));
    assert_eq!(host.starts_recorded(), 1);
    // A plain terminal launch takes the same path.
    let (status, body) = host.request("POST", "/api/terminals", shell());
    assert_eq!(status, 201, "{body}");
    assert_eq!(host.listed().len(), 2);
}

#[test]
fn no_admission_in_time_is_a_start_timeout_that_cleans_up_and_records_nothing() {
    let host = Host::start("hang");
    let spec = host.spec();
    let started = Instant::now();
    let (status, body) = host.request("POST", &format!("/api/specs/{spec}/agent"), spec_agent());
    assert_eq!(status, 503, "{body}");
    assert_eq!(body["code"], "startup_timeout", "{body}");
    assert!(started.elapsed() >= Duration::from_secs(6));
    assert!(host.listed().is_empty());
    assert_eq!(host.starts_recorded(), 0);
    let pid = host.client_pids()[0];
    assert!(
        wait_until(5, || !alive(pid)),
        "timed-out client {pid} still runs"
    );
    // Cleanup asked the boundary to stop that exact client.
    assert!(
        host.log().contains(&format!("stop {pid}")),
        "{}",
        host.log()
    );
}

#[test]
fn a_stale_receipt_under_a_reused_pid_is_not_admission() {
    let host = Host::start("reused-pid");
    let (status, body) = host.request("POST", "/api/terminals", shell());
    assert_eq!(status, 503, "{body}");
    assert_eq!(body["code"], "startup_timeout", "{body}");
    assert!(host.listed().is_empty());
    // The other session's receipt is recovery state: Verb leaves it, cleanup is the client's.
    let pid = host.client_pids()[0];
    assert!(wait_until(5, || !alive(pid)));
}

#[test]
fn a_resumed_session_is_not_admitted_by_its_earlier_runs_receipt() {
    let host = Host::start("resumed-session");
    let (status, body) = host.request("POST", "/api/terminals", shell());
    assert_eq!(status, 503, "{body}");
    assert_eq!(body["code"], "startup_timeout", "{body}");
    assert!(host.listed().is_empty());
}

#[test]
fn refusal_is_capacity_and_an_unconfirmed_exit_is_shown_without_a_start() {
    let host = Host::start("refuse");
    let spec = host.spec();
    let (status, body) = host.request("POST", &format!("/api/specs/{spec}/agent"), spec_agent());
    assert_eq!(status, 503, "{body}");
    assert_eq!(body["code"], "capacity", "{body}");
    assert!(host.listed().is_empty());
    host.behave("fail");
    let (status, body) = host.request("POST", &format!("/api/specs/{spec}/agent"), spec_agent());
    assert_eq!(status, 201, "{body}");
    let warning = body["warning"].as_str().unwrap_or_default();
    assert!(warning.contains("exit 126"), "{body}");
    assert!(warning.contains("no start was recorded"), "{body}");
    assert_eq!(host.starts_recorded(), 0);
    host.behave("admit");
    let (status, body) = host.request(
        "POST",
        &format!("/api/specs/{spec}/handoff"),
        Some(json!({"to": "shell", "note": "synthetic handoff"})),
    );
    assert_eq!(status, 201, "{body}");
    assert_eq!(host.starts_recorded(), 1);
}

#[test]
fn talks_cannot_reach_a_provider_through_the_boundary() {
    // A legacy boundary has no AGY runtime. A host-side override must never bypass its policy.
    let fixtures = std::env::temp_dir().join(format!(
        "verb-synthetic-agy-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&fixtures).unwrap();
    let marker = fixtures.join("agy-ran");
    let agy = fixtures.join("agy");
    fs::write(&agy, format!("#!/bin/sh\ntouch '{}'\n", marker.display())).unwrap();
    fs::set_permissions(&agy, fs::Permissions::from_mode(0o755)).unwrap();
    let host = Host::start_with("admit", &[("VERB_AGY_BIN", &agy)]);
    let spec = host.spec();
    let (status, body) = host.request("POST", &format!("/api/specs/{spec}/talk"), None);
    assert_eq!(status, 400, "{body}");
    assert!(
        body["error"]
            .as_str()
            .unwrap_or_default()
            .contains("unavailable"),
        "{body}"
    );
    std::thread::sleep(Duration::from_millis(500));
    assert!(!marker.exists(), "the provider stand-in ran");
    assert!(host.client_pids().is_empty());
    let _ = fs::remove_dir_all(fixtures);
}

#[test]
fn agent_catalog_uses_guest_capabilities_and_rejects_unavailable_launches_before_spawn() {
    let host = Host::start("admit");
    fs::write(host.root.join("boundary/capabilities"),
        json!({"version":1,"restricted":true,"programs":["bash","claude","codex","agy","opencode"]}).to_string()).unwrap();
    let (status, catalog) = host.request("GET", "/api/agents", None);
    assert_eq!(status, 200);
    assert_eq!(catalog["restricted"], true);
    assert_eq!(catalog["installed"]["agy"], true);
    assert_eq!(catalog["installed"]["opencode"], true);
    assert_eq!(catalog["installed"]["gemini"], false);
    assert!(host.client_pids().is_empty());

    let legacy = Host::start("admit");
    let (_, catalog) = legacy.request("GET", "/api/agents", None);
    assert_eq!(catalog["installed"]["agy"], false);
    assert_eq!(catalog["installed"]["opencode"], false);
    let (status, body) = legacy.request(
        "POST",
        "/api/terminals",
        Some(json!({"agent":"agy","isolated":false})),
    );
    assert_eq!(status, 400, "{body}");
    assert!(legacy.client_pids().is_empty());
}

/// Times `request` while `stalled` is in flight on another connection.
fn timed_during(
    host: &Host,
    stalled: impl FnOnce(&Host) + Send,
    probes: &[(&str, String)],
) -> Vec<(String, u16, Duration)> {
    std::thread::scope(|scope| {
        let worker = scope.spawn(|| stalled(host));
        // Let the stalled request reach the host first.
        std::thread::sleep(Duration::from_millis(400));
        let mut timings = Vec::new();
        for (method, path) in probes {
            let started = Instant::now();
            let (status, _) = host.request(method, path, None);
            timings.push((path.clone(), status, started.elapsed()));
        }
        worker.join().unwrap();
        timings
    })
}

fn assert_prompt(timings: &[(String, u16, Duration)]) {
    for (path, status, took) in timings {
        if std::env::var_os("VERB_PRINT_TIMINGS").is_some() {
            eprintln!("{path}: {status} in {took:?}");
        }
        assert_eq!(*status, 200, "{path}");
        assert!(
            *took < Duration::from_millis(1000),
            "{path} took {took:?} while the boundary was stalled"
        );
    }
}

#[test]
fn a_stalled_boundary_stop_does_not_freeze_health_workspace_or_terminal_output() {
    let host = Host::start("admit");
    let (_, first) = host.request("POST", "/api/terminals", shell());
    let (_, second) = host.request("POST", "/api/terminals", shell());
    let first = first["sessionId"].as_str().unwrap().to_owned();
    let second = second["sessionId"].as_str().unwrap().to_owned();
    fs::write(host.root.join("boundary/stop-stall"), "4").unwrap();
    let timings = timed_during(
        &host,
        |host| {
            let (status, body) = host.request("DELETE", &format!("/api/terminals/{first}"), None);
            // Cleanup the boundary did not confirm is reported, never claimed.
            assert_eq!(status, 200, "{body}");
            assert!(
                body["warning"]
                    .as_str()
                    .is_some_and(|warning| warning.contains("has not confirmed its cleanup")),
                "{body}"
            );
        },
        &[
            ("GET", "/healthz".to_owned()),
            ("GET", "/api/workspace".to_owned()),
            ("GET", format!("/api/terminals/{second}/output?after=0")),
        ],
    );
    assert_prompt(&timings);
}

#[test]
fn a_stalled_background_job_does_not_freeze_health_workspace_or_terminal_output() {
    let host = Host::start("admit");
    let (_, terminal) = host.request("POST", "/api/terminals", shell());
    let terminal = terminal["sessionId"].as_str().unwrap().to_owned();
    fs::write(host.root.join("boundary/pipes-stall"), "4").unwrap();
    let timings = timed_during(
        &host,
        |host| {
            host.request("GET", "/api/files", None);
        },
        &[
            ("GET", "/healthz".to_owned()),
            ("GET", "/api/workspace".to_owned()),
            ("GET", format!("/api/terminals/{terminal}/output?after=0")),
        ],
    );
    assert_prompt(&timings);
}

#[test]
fn a_stalled_launch_does_not_freeze_health_workspace_or_terminal_output() {
    let host = Host::start("admit");
    let (_, terminal) = host.request("POST", "/api/terminals", shell());
    let terminal = terminal["sessionId"].as_str().unwrap().to_owned();
    let spec = host.spec();
    // The handoff note's Git queries stall at the boundary, then admission never comes.
    fs::write(host.root.join("boundary/pipes-stall"), "2").unwrap();
    host.behave("hang");
    let timings = timed_during(
        &host,
        |host| {
            let (status, body) = host.request(
                "POST",
                &format!("/api/specs/{spec}/handoff"),
                Some(json!({"to": "shell", "note": "stalled handoff"})),
            );
            assert_eq!(status, 503, "{body}");
            assert_eq!(body["code"], "startup_timeout", "{body}");
        },
        &[
            ("GET", "/healthz".to_owned()),
            ("GET", "/api/workspace".to_owned()),
            ("GET", format!("/api/terminals/{terminal}/output?after=0")),
        ],
    );
    assert_prompt(&timings);
    assert_eq!(host.listed(), vec![terminal]);
}

#[test]
fn an_unpublished_lease_recovery_receipt_still_reports_unconfirmed_cleanup() {
    let host = Host::start("admit");
    let (status, launched) = host.request("POST", "/api/terminals", shell());
    assert_eq!(status, 201, "{launched}");
    let session = launched["sessionId"].as_str().unwrap();
    fs::write(host.root.join("boundary/stop-fallback"), "").unwrap();
    fs::write(host.root.join("boundary/stop-stall"), "1").unwrap();
    let (status, body) = host.request("DELETE", &format!("/api/terminals/{session}"), None);
    assert_eq!(status, 200, "{body}");
    assert!(
        body["warning"]
            .as_str()
            .is_some_and(|warning| warning.contains("has not confirmed its cleanup")),
        "{body}"
    );
    let names: Vec<_> = fs::read_dir(host.root.join("boundary/receipts"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names.len(), 1, "{names:?}");
    assert!(names[0].ends_with("-0123456789abcdef0123456789abcdef.json"));
}

#[test]
fn unsupported_checkout_is_rejected_before_git_or_worktree_mutations() {
    let host = Host::start("admit");
    let (status, capabilities) = host.request("GET", "/api/agents", None);
    assert_eq!(status, 200);
    assert_eq!(capabilities["isolatedCheckouts"], false);
    let before = fs::read_to_string(host.root.join("boundary/log")).unwrap();
    let (status, reply) = host.request(
        "POST",
        "/api/terminals",
        Some(json!({"agent": "shell", "isolated": true})),
    );
    assert_eq!(status, 400, "{reply}");
    assert!(reply["error"]
        .as_str()
        .unwrap()
        .contains("current checkout"));
    assert_eq!(
        fs::read_to_string(host.root.join("boundary/log")).unwrap(),
        before
    );
    assert!(!host.root.join("state/worktrees").exists());
    assert!(host.listed().is_empty());
}

#[test]
fn file_reads_cross_the_boundary_once_and_preserve_capacity_and_timeout_statuses() {
    let host = Host::start("admit");
    let reply = host.root.join("boundary/project-read-reply");
    fs::write(
        &reply,
        r#"{"files":[{"path":"synthetic.txt","status":null}],"truncated":false}"#,
    )
    .unwrap();
    let before = host.log().lines().count();
    let (status, body) = host.request("GET", "/api/files", None);
    assert_eq!(status, 200, "{body}");
    assert_eq!(body["files"][0]["path"], "synthetic.txt");
    assert_eq!(host.log().lines().count() - before, 1);
    fs::remove_file(&reply).unwrap();
    for (flag, code) in [("capacity", "capacity"), ("timeout", "read_timeout")] {
        let flag = host.root.join(format!("boundary/project-read-{flag}"));
        fs::write(&flag, "").unwrap();
        let (status, body) = host.request("GET", "/api/files", None);
        assert_eq!(status, 503, "{body}");
        assert_eq!(body["code"], code);
        fs::remove_file(flag).unwrap();
    }
    fs::write(host.root.join("boundary/project-read-oversize"), "").unwrap();
    let (status, body) = host.request("GET", "/api/files", None);
    assert_eq!(status, 400, "{body}");
    assert!(body["error"].as_str().unwrap().contains("too much data"));
    assert_eq!(host.request("GET", "/healthz", None).0, 200);
}

#[test]
fn a_stalled_project_read_has_an_overall_deadline_and_keeps_the_host_responsive() {
    let host = Host::start("admit");
    fs::write(host.root.join("boundary/project-read-stall"), "").unwrap();
    let started = Instant::now();
    let timings = timed_during(
        &host,
        |host| {
            let (status, body) = host.request("GET", "/api/files", None);
            assert_eq!(status, 503, "{body}");
            assert_eq!(body["code"], "read_timeout");
        },
        &[
            ("GET", "/healthz".to_owned()),
            ("GET", "/api/workspace".to_owned()),
        ],
    );
    assert_prompt(&timings);
    assert!(started.elapsed() < Duration::from_secs(28));
}

// This fixture exercises the real guest helper through the real host transport. It does not
// substitute a controller, identity or cleanup proof for the installed public boundary.
#[test]
fn studio_boundary_saves_as_guest_and_preserves_conflicts_without_host_fallback() {
    let host = Host::start_with(
        "admit",
        &[("VERB_STUDIO_BOUNDARY", Path::new("monitored-v1"))],
    );
    let repo = host.root.join(PROJECT);
    assert!(Command::new("git")
        .args(["init", "-q"])
        .current_dir(&repo)
        .status()
        .unwrap()
        .success());
    fs::write(repo.join("app.html"), "<h1>Before</h1>").unwrap();
    fs::write(repo.join(".gitignore"), "private.txt\n").unwrap();
    fs::write(repo.join("private.txt"), "fixture secret").unwrap();
    let boundary = host.root.join("boundary");
    fs::write(boundary.join("studio-enabled"), "").unwrap();
    let (status, cap) = host.request("GET", "/api/studio/workspace", None);
    assert_eq!(status, 200, "{cap}");
    assert_eq!(cap["editing"]["available"], true, "{cap}");
    assert_eq!(cap["transport"], "monitored-v1");
    let mut raster = b"\x89PNG\r\n\x1a\n".to_vec();
    raster.resize(2_000_000, 0);
    fs::write(repo.join("large.png"), &raster).unwrap();
    let (status, image) = host.request("GET", "/api/studio/image?path=large.png", None);
    assert_eq!(status, 200);
    assert_eq!(image["image"]["bytes"], 2_000_000);
    assert_eq!(image["image"]["mime"], "image/png");
    fs::write(repo.join("active.png"), b"<svg onload='alert(1)'/>").unwrap();
    assert_ne!(
        host.request("GET", "/api/studio/image?path=active.png", None)
            .0,
        200
    );
    let (status, read) = host.request("GET", "/api/studio/file?path=app.html", None);
    assert_eq!(status, 200, "{read}");
    let mut payload = json!({"workspaceId":cap["workspaceId"],"path":"app.html", "expectedRevision":read["file"]["revision"],"text":"<h1>After</h1>"});
    let (status, saved) = host.request("POST", "/api/studio/file", Some(payload.clone()));
    assert_eq!(status, 200, "{saved}");
    assert_eq!(
        fs::read_to_string(repo.join("app.html")).unwrap(),
        "<h1>After</h1>"
    );
    let (status, conflict) = host.request("POST", "/api/studio/file", Some(payload.clone()));
    assert_eq!(status, 409, "{conflict}");
    let (status, ticket) = host.request("POST", "/api/studio/preview", Some(json!({"workspaceId":cap["workspaceId"],"path":"app.html","expectedRevision":saved["file"]["revision"]})));
    assert_eq!(status, 200, "{ticket}");
    assert!(ticket["url"]
        .as_str()
        .unwrap()
        .starts_with("/studio-preview/"));
    // Real transport handles payloads larger than a pipe buffer and Linux's single-argv limit.
    payload["expectedRevision"] = saved["file"]["revision"].clone();
    payload["text"] = json!("x".repeat(190_000));
    let (status, large) = host.request("POST", "/api/studio/file", Some(payload.clone()));
    assert_eq!(status, 200, "{large}");
    assert_eq!(fs::metadata(repo.join("app.html")).unwrap().len(), 190_000);
    for path in ["private.txt", "../outside.txt"] {
        let (status, reply) = host.request("GET", &format!("/api/studio/file?path={path}"), None);
        assert_ne!(status, 200, "{reply}");
    }
    // Wrong protocol/version disables capability discovery. Malformed replies never cause a
    // fallback write through the host's accessible view of this synthetic project.
    fs::write(
        boundary.join("studio-reply"),
        r#"{"protocol":1,"sourceRevision":"wrong"}"#,
    )
    .unwrap();
    let (_, cap) = host.request("GET", "/api/studio/workspace", None);
    assert_eq!(cap["editing"]["available"], false);
    let (status, _) = host.request("POST", "/api/studio/file", Some(payload));
    assert_ne!(status, 200);
    assert_eq!(fs::metadata(repo.join("app.html")).unwrap().len(), 190_000);
}

#[test]
fn studio_external_boundary_is_disabled_without_opt_in() {
    let host = Host::start("admit");
    let (_, cap) = host.request("GET", "/api/studio/workspace", None);
    assert_eq!(cap["editing"]["available"], false);
    let (status, _) = host.request("GET", "/api/studio/file?path=app.html", None);
    assert_eq!(status, 403);
    assert!(!fs::read_to_string(host.root.join("boundary/log"))
        .unwrap()
        .contains("<verb-studio-adapter>"));
}
