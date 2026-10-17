//! Fixed unprivileged client for the explicitly selected private candidate.
use std::path::Path;
use std::process::{Command, Stdio};

pub(super) const CLIENT: &str = "/usr/local/libexec/verb-boundary-client";
const PROJECTS: &str = "/srv/verb-boundary/projects";
/// The client writes `<pid>.json` here only after the controller has issued the session's lease.
const RECEIPTS: &str = "/home/verb-host/.verb-boundary-clients";

pub(super) fn validate_host() -> Result<(), String> {
    let status = std::fs::read_to_string("/proc/self/status").map_err(|e| e.to_string())?;
    let uid = status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .and_then(|value| value.split_whitespace().nth(1));
    if uid != Some("995") {
        return Err("external execution requires the dedicated Verb host account".into());
    }
    super::validate_host_state(995)
}

fn project_id(project: &Path) -> Result<String, String> {
    if project.parent() != Some(Path::new(PROJECTS)) {
        return Err("external execution requires an operator-registered project".into());
    }
    let id = project
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or("invalid registered project")?;
    if id.len() != 32
        || !id
            .bytes()
            .all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
    {
        return Err("invalid registered project identity".into());
    }
    if project.canonicalize().map_err(|e| e.to_string())? != project {
        return Err("registered project cannot follow a symbolic link".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let info = std::fs::symlink_metadata(project).map_err(|e| e.to_string())?;
        if !info.is_dir()
            || !(62000..=62999).contains(&info.uid())
            || info.gid() != 995
            || info.mode() & 0o7777 != 0o2770
        {
            return Err("registered project ownership or mode changed".into());
        }
    }
    Ok(id.to_owned())
}

fn program_name(program: &str) -> Result<&str, String> {
    match program {
        "bash" | "/bin/bash" | "/usr/bin/bash" => Ok("bash"),
        "git" | "/usr/bin/git" | "/bin/git" => Ok("git"),
        "node" | "/usr/bin/node" => Ok("node"),
        "python3" | "/usr/bin/python3" => Ok("python3"),
        "claude" | "/usr/local/bin/claude" => Ok("claude"),
        "codex" | "/usr/local/bin/codex" => Ok("codex"),
        "timeout" | "/usr/bin/timeout" => Ok("timeout"),
        "recovery" => Ok("recovery"),
        _ => Err("program is unavailable in the private execution candidate".into()),
    }
}

pub(super) fn arguments(
    program: &str,
    args: &[String],
    project: &Path,
    session: &str,
    mode: &str,
) -> Result<Vec<String>, String> {
    let id = project_id(project)?;
    let session = if session.is_empty() {
        crate::new_id()
    } else {
        session.to_owned()
    };
    if session.len() > 100
        || !session
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
        || !matches!(mode, "pipes" | "terminal")
    {
        return Err("invalid external session request".into());
    }
    let program = program_name(program)?;
    let mut out = vec![
        "--project".into(),
        id,
        "--session".into(),
        session,
        "--mode".into(),
        mode.into(),
        "--program".into(),
        if program == "timeout" {
            "bash"
        } else {
            program
        }
        .into(),
        "--".into(),
    ];
    if program == "timeout" {
        // Arguments appended by Command::args remain literal positional values.
        out.extend([
            "--noprofile".into(),
            "--norc".into(),
            "-c".into(),
            "exec /usr/bin/timeout \"$@\"".into(),
            "verb-timeout".into(),
        ]);
    }
    out.extend_from_slice(args);
    Ok(out)
}

/// File modification times come from a coarse kernel clock that can trail the time Verb read just
/// before spawning by up to a scheduler tick. A client takes far longer than this to be admitted.
const TIMESTAMP_SLACK: std::time::Duration = std::time::Duration::from_millis(50);

/// Kernel start time of `pid` in clock ticks since boot (`/proc/<pid>/stat` field 22), which stays
/// the same for the life of the process, including as a zombie until Verb reaps it.
pub(super) fn process_start(pid: i32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The command name (field 2) is parenthesized and may itself contain spaces or parentheses.
    let (_, rest) = stat.rsplit_once(')')?;
    rest.split_whitespace().nth(19)?.parse().ok()
}

/// Controller-authoritative startup acknowledgement for `launch`.
pub(super) fn admitted(launch: &super::Launch) -> bool {
    admitted_in(Path::new(RECEIPTS), launch, process_start(launch.pid))
}

/// The client writes `<pid>.json` only after the controller issued the session's lease, but a PID
/// is only a lookup key. Receipts outlive clients that ended before their cleanup was acknowledged
/// (they are what recovery uses to stop that exact lease, so Verb never removes them), and PIDs are
/// reused. A receipt is this launch's admission only when
/// - `pid` still runs the process this launch spawned (`running` is its current start time),
/// - the receipt was written after this launch was spawned, which also rejects one left by an
///   earlier run of a resumed session with the same session id, and
/// - it is a stop request for this launch's project and session with a controller-issued
///   generation.
fn admitted_in(receipts: &Path, launch: &super::Launch, running: Option<u64>) -> bool {
    if launch.pid <= 1 || launch.started.is_none() || running != launch.started {
        return false;
    }
    let path = receipts.join(format!("{}.json", launch.pid));
    let Ok(info) = std::fs::symlink_metadata(&path) else {
        return false;
    };
    if !info.is_file() || info.len() > 4096 {
        return false;
    }
    if !info
        .modified()
        .is_ok_and(|written| written + TIMESTAMP_SLACK >= launch.spawned_at)
    {
        return false;
    }
    let Some(receipt) = std::fs::read(&path)
        .ok()
        .and_then(|raw| serde_json::from_slice::<serde_json::Value>(&raw).ok())
    else {
        return false;
    };
    let generation = receipt["generation"].as_str().unwrap_or_default();
    receipt["version"] == 1
        && receipt["operation"] == "stop"
        && receipt["project"] == launch.project.as_str()
        && receipt["session"] == launch.session.as_str()
        && generation.len() == 32
        && generation
            .bytes()
            .all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
}

pub(super) fn stop_client(pid: i32) -> Result<(), String> {
    if pid <= 1 {
        return Err("invalid host client identity".into());
    }
    let result = Command::new(CLIENT)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .args(["--stop-client", &pid.to_string()])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if result.status.success() {
        Ok(())
    } else {
        Err("native session cleanup was not confirmed".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};
    #[test]
    fn rejects_arbitrary_program_paths_and_unregistered_roots() {
        for program in ["/tmp/bash", "sudo", "/usr/bin/su", "../codex"] {
            assert!(program_name(program).is_err());
        }
        for project in [
            "/root",
            "/srv/verb-boundary/projects/../root",
            "/srv/verb-boundary/projects/not-an-id",
        ] {
            assert!(project_id(Path::new(project)).is_err());
        }
        assert_eq!(program_name("/usr/local/bin/codex").unwrap(), "codex");
        assert!(stop_client(1).is_err());
    }

    const PROJECT: &str = "0f1e2d3c4b5a69788796a5b4c3d2e1f0";
    const GENERATION: &str = "0123456789abcdef0123456789abcdef";

    struct Receipts(std::path::PathBuf);

    impl Receipts {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("verb-receipts-{}", crate::new_id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        /// Writes the stop receipt a client leaves for `session`, last modified at `written`.
        fn write(&self, pid: i32, session: &str, written: std::time::SystemTime) -> PathBuf {
            let path = self.0.join(format!("{pid}.json"));
            let receipt = serde_json::json!({"generation": GENERATION, "operation": "stop",
                "project": PROJECT, "session": session, "version": 1});
            std::fs::write(&path, receipt.to_string()).unwrap();
            std::fs::File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(written)
                .unwrap();
            path
        }
    }

    impl Drop for Receipts {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn launch(pid: i32, session: &str, spawned_at: SystemTime) -> super::super::Launch {
        super::super::Launch {
            pid,
            started: Some(777),
            project: PROJECT.to_owned(),
            session: session.to_owned(),
            spawned_at,
        }
    }

    #[test]
    fn a_receipt_written_for_this_launch_after_it_was_spawned_is_its_admission() {
        let receipts = Receipts::new();
        let spawned = SystemTime::now() - Duration::from_secs(1);
        receipts.write(4242, "new-session", SystemTime::now());
        assert!(admitted_in(
            &receipts.0,
            &launch(4242, "new-session", spawned),
            Some(777)
        ));
    }

    #[test]
    fn a_stale_receipt_for_a_reused_pid_is_not_admission_and_is_kept_for_recovery() {
        let receipts = Receipts::new();
        let spawned = SystemTime::now();
        // A client that ran as 4242 yesterday ended before its cleanup was acknowledged.
        let stale = receipts.write(4242, "old-session", spawned - Duration::from_secs(86_400));
        assert!(!admitted_in(
            &receipts.0,
            &launch(4242, "new-session", spawned),
            Some(777)
        ));
        // Even rewritten now, it names another session.
        receipts.write(4242, "old-session", SystemTime::now());
        assert!(!admitted_in(
            &receipts.0,
            &launch(4242, "new-session", spawned),
            Some(777)
        ));
        assert!(stale.is_file(), "Verb must never remove a receipt");
    }

    #[test]
    fn a_resumed_session_needs_a_receipt_from_this_run_not_the_earlier_one() {
        let receipts = Receipts::new();
        let spawned = SystemTime::now();
        // The earlier run of the same session left a receipt under the PID the new client got.
        receipts.write(4242, "resumed", spawned - Duration::from_secs(60));
        let resumed = launch(4242, "resumed", spawned);
        assert!(!admitted_in(&receipts.0, &resumed, Some(777)));
        // This run's own receipt, written once the controller issues the new lease.
        receipts.write(4242, "resumed", spawned + Duration::from_millis(300));
        assert!(admitted_in(&receipts.0, &resumed, Some(777)));
    }

    #[test]
    fn a_receipt_does_not_count_once_the_pid_runs_another_process() {
        let receipts = Receipts::new();
        let spawned = SystemTime::now() - Duration::from_secs(1);
        receipts.write(4242, "s", SystemTime::now());
        let ours = launch(4242, "s", spawned);
        // The client ended and was reaped, and the PID now belongs to a different process.
        assert!(!admitted_in(&receipts.0, &ours, Some(778)));
        assert!(!admitted_in(&receipts.0, &ours, None));
        let unknown = super::super::Launch {
            started: None,
            ..ours.clone()
        };
        assert!(!admitted_in(&receipts.0, &unknown, None));
        assert!(admitted_in(&receipts.0, &ours, Some(777)));
    }

    #[test]
    fn only_a_well_formed_stop_receipt_for_this_project_counts() {
        let receipts = Receipts::new();
        let spawned = SystemTime::now() - Duration::from_secs(1);
        let ours = launch(4242, "s", spawned);
        let path = receipts.0.join("4242.json");
        for receipt in [
            serde_json::json!({"generation": GENERATION, "operation": "stop",
                "project": "ffffffffffffffffffffffffffffffff", "session": "s", "version": 1}),
            serde_json::json!({"operation": "launch", "project": PROJECT, "session": "s",
                "version": 1}),
            serde_json::json!({"generation": "", "operation": "stop", "project": PROJECT,
                "session": "s", "version": 1}),
        ] {
            std::fs::write(&path, receipt.to_string()).unwrap();
            assert!(!admitted_in(&receipts.0, &ours, Some(777)), "{receipt}");
        }
        std::fs::write(&path, "not json").unwrap();
        assert!(!admitted_in(&receipts.0, &ours, Some(777)));
        std::fs::remove_file(&path).unwrap();
        #[cfg(unix)]
        {
            let real = receipts.write(9999, "s", SystemTime::now());
            std::os::unix::fs::symlink(real, &path).unwrap();
            assert!(!admitted_in(&receipts.0, &ours, Some(777)));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn process_start_is_stable_for_a_running_process() {
        let me = std::process::id() as i32;
        let first = process_start(me).unwrap();
        assert_eq!(process_start(me), Some(first));
    }
}
