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

/// Controller-authoritative startup acknowledgement for the client running as `pid`.
pub(super) fn admitted(pid: i32) -> bool {
    pid > 1 && Path::new(RECEIPTS).join(format!("{pid}.json")).is_file()
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
}
