//! Opt-in Linux account separation through a fixed operator-provisioned launcher.
use std::path::{Path, PathBuf};
use std::process::Command;

mod external;

const LAUNCHER: &str = "/usr/local/libexec/verb-agent-launch";
pub(crate) const SHELL_DIR: &str = "/var/lib/verb-shell";

#[derive(Clone, Debug)]
pub(crate) struct AccountProfile {
    pub user: String,
    pub home: PathBuf,
    uid: u32,
}

#[derive(Clone, Debug)]
pub(crate) enum Profile {
    Account(AccountProfile),
    External,
}

pub(crate) fn profile() -> Result<Option<Profile>, String> {
    #[cfg(feature = "synthetic-boundary")]
    if external::synthetic().is_some() {
        return Ok(Some(Profile::External));
    }
    if let Some(value) = std::env::var_os("VERB_EXECUTION_BOUNDARY") {
        if value != "monitored-v1" || !cfg!(target_os = "linux") {
            return Err("unsupported execution boundary; refusing fallback".into());
        }
        if std::env::var_os("VERB_AGENT_USER").is_some()
            || std::env::var_os("VERB_AGENT_SUDO").is_some()
        {
            return Err("choose one execution boundary".into());
        }
        external::validate_host()?;
        secure_launcher(Path::new(external::CLIENT))?;
        return Ok(Some(Profile::External));
    }
    account_profile().map(|profile| profile.map(Profile::Account))
}

pub(crate) fn external() -> bool {
    matches!(profile(), Ok(Some(Profile::External)))
}

/// Programs exposed inside external terminals, never inferred from the host account's PATH.
pub(crate) fn external_programs() -> Option<Vec<String>> {
    external().then(external::programs)
}

/// Exit status of a boundary client whose launch the controller refused before issuing a lease.
pub(crate) const REFUSED_EXIT: i32 = 75;

/// Whether a boundary client's result is an admission refusal (capacity or busy), as opposed to a
/// failure of the program it was asked to run. Only the client's own pre-lease refusal qualifies.
pub(crate) fn refused(code: Option<i32>, stderr: &[u8]) -> bool {
    code == Some(REFUSED_EXIT) && capacity_message(&String::from_utf8_lossy(stderr))
}

/// The boundary's own wording for a refused admission.
pub(crate) fn capacity_message(text: &str) -> bool {
    text.contains("Verb execution unavailable: capacity")
        || text.contains("Verb execution unavailable: busy")
}

/// One external terminal launch: the client Verb spawned, for which project and session, and when.
/// The controller's receipt is accepted as this launch's admission only if it matches all of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Launch {
    pub(crate) pid: i32,
    /// The client's kernel start time, read right after spawn. A reused PID has another one.
    pub(crate) started: Option<u64>,
    /// Registered project id the client was asked to run in.
    pub(crate) project: String,
    pub(crate) session: String,
    /// Taken before the client was spawned: its receipt cannot be older than this.
    pub(crate) spawned_at: std::time::SystemTime,
}

/// The identity of an external terminal launch whose client runs as `pid`, or `None` when
/// terminals do not cross the external boundary (they are then admitted at once).
pub(crate) fn launch_identity(
    pid: i32,
    project: &Path,
    session: &str,
    spawned_at: std::time::SystemTime,
) -> Option<Launch> {
    external().then(|| Launch {
        pid,
        started: external::process_start(pid),
        project: project
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        session: session.to_owned(),
        spawned_at,
    })
}

/// Whether the controller has admitted `launch`. The receipt is written only after the controller
/// issues a lease, so concurrent launches are decided by the controller rather than by a host-side
/// count; it must also belong to this launch, not merely to its PID.
pub(crate) fn admitted(launch: &Launch) -> bool {
    external() && external::admitted(launch)
}

/// Whether the execution boundary has yet to confirm cleanup of the external terminal that ran as
/// `pid` for `session`. Its slot stays reserved by the controller until it does.
pub(crate) fn cleanup_unconfirmed(pid: i32, session: &str) -> bool {
    external() && external::receipt_held(pid, session)
}

fn account_profile() -> Result<Option<AccountProfile>, String> {
    let Some(user) = std::env::var_os("VERB_AGENT_USER") else {
        return Ok(None);
    };
    let user = user.into_string().map_err(|_| "invalid VERB_AGENT_USER")?;
    if !cfg!(target_os = "linux") {
        return Err("VERB_AGENT_USER requires Linux; isolation cannot silently fall back".into());
    }
    let passwd = std::fs::read_to_string("/etc/passwd").map_err(|e| e.to_string())?;
    let status = std::fs::read_to_string("/proc/self/status").map_err(|e| e.to_string())?;
    let uid = status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|uid| uid.parse::<u32>().ok())
        .ok_or("cannot determine host UID")?;
    let profile = parse_profile(&user, &passwd, uid)?;
    secure_launcher(Path::new(LAUNCHER))?;
    let sudo = sudo_path();
    if !sudo.is_absolute() {
        return Err("VERB_AGENT_SUDO must be an absolute root-owned path".into());
    }
    secure_launcher(&sudo)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        validate_host_state(uid)?;
        let agent_home = std::fs::symlink_metadata(&profile.home).map_err(|e| e.to_string())?;
        if !agent_home.is_dir() || agent_home.uid() != profile.uid || agent_home.mode() & 0o022 != 0
        {
            return Err("agent HOME must belong to the agent and not be writable by others".into());
        }
    }
    Ok(Some(profile))
}

fn validate_host_state(uid: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let host_home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or("host HOME is missing")?;
        let home_metadata = std::fs::symlink_metadata(&host_home).map_err(|e| e.to_string())?;
        if !home_metadata.is_dir()
            || home_metadata.uid() != uid
            || home_metadata.mode() & 0o077 != 0
        {
            return Err(
                "isolated host HOME must be an owner-only directory owned by the host account"
                    .into(),
            );
        }
        let state = std::env::var_os("VERB_STATE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| host_home.join(".verb"));
        if !state.is_absolute()
            || !state.starts_with(&host_home)
            || state
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err("isolated host state must remain under its private HOME".into());
        }
        let existing = state
            .ancestors()
            .find(|path| path.exists())
            .ok_or("host state parent is missing")?;
        if !existing
            .canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(host_home.canonicalize().map_err(|e| e.to_string())?)
        {
            return Err("host state cannot follow a symlink outside its private HOME".into());
        }
        if !state.exists() {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&state)
                .map_err(|e| e.to_string())?;
        }
        if state.exists() {
            let metadata = std::fs::metadata(&state).map_err(|e| e.to_string())?;
            if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o077 != 0 {
                return Err("isolated host state must be an owner-only directory".into());
            }
        }
    }
    Ok(())
}

fn parse_profile(user: &str, passwd: &str, host_uid: u32) -> Result<AccountProfile, String> {
    if host_uid == 0 {
        return Err("isolated Verb must run as a dedicated non-root host account".into());
    }
    if user.is_empty()
        || user.len() > 32
        || user.starts_with('-')
        || !user
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
    {
        return Err("invalid VERB_AGENT_USER".into());
    }
    let fields: Vec<_> = passwd
        .lines()
        .find(|line| line.split(':').next() == Some(user))
        .ok_or("agent account does not exist")?
        .split(':')
        .collect();
    if fields.len() != 7 {
        return Err("invalid agent account record".into());
    }
    let uid = fields[2].parse::<u32>().map_err(|_| "invalid agent UID")?;
    let gid = fields[3].parse::<u32>().map_err(|_| "invalid agent GID")?;
    let home = PathBuf::from(fields[5]);
    if uid == 0 || gid == 0 || uid == host_uid || !home.is_absolute() || home == Path::new("/") {
        return Err("agent must have a separate non-root UID/GID and absolute home".into());
    }
    Ok(AccountProfile {
        user: user.into(),
        home,
        uid,
    })
}

fn secure_launcher(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        for entry in path.ancestors() {
            let metadata =
                std::fs::symlink_metadata(entry).map_err(|e| format!("launcher: {e}"))?;
            if metadata.file_type().is_symlink()
                || metadata.uid() != 0
                || metadata.mode() & 0o022 != 0
            {
                return Err(
                    "agent launcher and parents must be root-owned and not writable by others"
                        .into(),
                );
            }
        }
        let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
        if !metadata.is_file() || metadata.mode() & 0o111 == 0 {
            return Err("agent launcher is not executable".into());
        }
    }
    Ok(())
}

pub(crate) fn agent_home() -> Option<PathBuf> {
    match profile() {
        Ok(Some(Profile::Account(profile))) => Some(profile.home),
        Ok(Some(Profile::External)) => None,
        Ok(None) => std::env::var_os("HOME").map(PathBuf::from),
        Err(_) => None,
    }
}

pub(crate) fn launch(
    program: &str,
    args: &[String],
    project: &Path,
    session: &str,
    env: &[(String, String)],
    lock_fd: Option<i32>,
) -> Result<(String, Vec<String>), String> {
    match profile()? {
        None => Ok((program.into(), args.to_vec())),
        Some(Profile::External) => Ok((
            external::client(),
            external::arguments(program, args, project, session, "terminal")?,
        )),
        Some(Profile::Account(profile)) => Ok((
            sudo_path().to_string_lossy().into_owned(),
            launcher_args(&profile, program, args, project, session, env, lock_fd)?,
        )),
    }
}

fn launcher_args(
    profile: &AccountProfile,
    program: &str,
    args: &[String],
    project: &Path,
    session: &str,
    env: &[(String, String)],
    lock_fd: Option<i32>,
) -> Result<Vec<String>, String> {
    let mut out = vec!["-n".into(), "-H".into(), "-u".into(), profile.user.clone()];
    if let Some(fd) = lock_fd {
        out.extend(["-C".into(), (fd + 1).to_string()]);
    }
    out.extend([
        "--".into(),
        LAUNCHER.into(),
        project.display().to_string(),
        session.into(),
    ]);
    for (key, value) in env {
        // Native CLI bridge storage is private to the host account in this mode. Do not point
        // agents at that storage or propagate the host's executable search path.
        if matches!(key.as_str(), "VERB_PROJECT_MEMORY_PATH" | "VERB_BIN") {
            continue;
        }
        if !matches!(
            key.as_str(),
            "ZDOTDIR"
                | "VERB_ZDOTDIR_ORIGINAL"
                | "VERB_SHELL_DIR"
                | "RUSTUP_AUTO_INSTALL"
                | "GOTOOLCHAIN"
        ) {
            return Err(format!(
                "isolated launch refuses host environment override {key}"
            ));
        }
        out.extend(["--env".into(), key.clone(), value.clone()]);
    }
    out.push("--".into());
    out.push(program.into());
    out.extend_from_slice(args);
    Ok(out)
}

pub(crate) fn command(program: &Path, project: &Path, session: &str) -> Result<Command, String> {
    let (program, args) = if external() {
        (
            external::client(),
            external::arguments(&program.to_string_lossy(), &[], project, session, "pipes")?,
        )
    } else {
        launch(&program.to_string_lossy(), &[], project, session, &[], None)?
    };
    // Talk gets its own session so the unprivileged host can ask the fixed agent launcher to
    // stop that exact session's descendants. Project probes do not need another session.
    let mut command = if !session.is_empty() && profile()?.is_some() {
        let mut command = Command::new("/usr/bin/setsid");
        command.args(["--wait", &program]);
        command
    } else {
        Command::new(program)
    };
    command.args(args).current_dir(project);
    if profile()?.is_some() {
        command
            .env_clear()
            .env("PATH", "/usr/local/bin:/usr/bin:/bin")
            .env("TERM", "xterm-256color");
    }
    Ok(command)
}

pub(crate) fn signal_session(sid: i32, signal: i32) -> Result<(), String> {
    let profile = match profile()? {
        None => return Ok(()),
        Some(Profile::External) => return external::stop_client(sid),
        Some(Profile::Account(profile)) => profile,
    };
    let signal = match signal {
        1 => "HUP",
        15 => "TERM",
        9 => "KILL",
        _ => return Err("unsupported isolation signal".into()),
    };
    if sid <= 1 {
        return Err("invalid isolated session".into());
    }
    let status = Command::new(sudo_path())
        .env_clear()
        .env("PATH", "/usr/local/bin:/usr/bin:/bin")
        .args([
            "-n",
            "-H",
            "-u",
            &profile.user,
            "--",
            LAUNCHER,
            "--signal",
            &sid.to_string(),
            signal,
        ])
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if status.status.success() {
        Ok(())
    } else {
        Err("could not stop isolated agent session".into())
    }
}

fn sudo_path() -> PathBuf {
    std::env::var_os("VERB_AGENT_SUDO")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/bin/sudo"))
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct RecoveryEvidence {
    pub identity: Option<String>,
    pub status: String,
}

/// Ask the same root-owned executable under the agent UID. Return metadata, never record text.
pub(crate) fn recovery_evidence(
    agent: &str,
    project: &Path,
    since: Option<std::time::SystemTime>,
    identity: Option<&str>,
) -> Option<RecoveryEvidence> {
    let since = since
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis());
    let mut probe = if external() {
        let mut probe = command(Path::new("recovery"), project, "").ok()?;
        probe.args([agent, &since.to_string(), identity.unwrap_or("")]);
        probe
    } else {
        let executable = std::env::current_exe().ok()?;
        secure_launcher(&executable).ok()?;
        let mut probe = command(Path::new("/usr/bin/timeout"), project, "").ok()?;
        probe.args([
            "--kill-after=1s",
            "2s",
            executable.to_str()?,
            "__agent-recovery",
            agent,
            &since.to_string(),
            identity.unwrap_or(""),
        ]);
        probe
    };
    let output = probe.stdin(std::process::Stdio::null()).output().ok()?;
    if !output.status.success() || output.stdout.len() > 4096 {
        return None;
    }
    let evidence: RecoveryEvidence = serde_json::from_slice(&output.stdout).ok()?;
    if evidence
        .identity
        .as_deref()
        .is_some_and(|id| crate::valid_resume_identity(id).is_none())
    {
        return None;
    }
    if identity.is_some() && evidence.identity.as_deref() != identity {
        return None;
    }
    Some(evidence)
}

pub(crate) fn recovery_command(project: &Path, args: &[String]) -> Result<(), String> {
    // The launcher clears the isolation flag before exec. Never recurse or read the host's home.
    if profile()?.is_some()
        || args.len() != 3
        || !matches!(args[0].as_str(), "claude" | "codex" | "opencode")
    {
        return Err("invalid agent recovery metadata request".into());
    }
    let since = args[1]
        .parse::<u64>()
        .map_err(|_| "invalid record timestamp")?;
    let identity = if args[2].is_empty() {
        crate::observe::recovery_identity(
            &args[0],
            project,
            std::time::UNIX_EPOCH + std::time::Duration::from_millis(since),
        )
    } else {
        Some(
            crate::valid_resume_identity(&args[2])
                .ok_or("invalid conversation identity")?
                .to_owned(),
        )
    };
    let verdict = crate::Agent::parse(&args[0]).resume_verdict(project, identity.as_deref());
    let status = match verdict {
        crate::ResumeVerdict::Yes => "yes",
        crate::ResumeVerdict::No => "no",
        crate::ResumeVerdict::Unknown => "unknown",
    };
    println!(
        "{}",
        serde_json::to_string(&RecoveryEvidence {
            identity,
            status: status.into()
        })
        .map_err(|e| e.to_string())?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_clients_own_capacity_refusal_counts_as_refused() {
        let refusal = b"Verb execution unavailable: capacity\n";
        assert!(refused(Some(REFUSED_EXIT), refusal));
        assert!(refused(
            Some(REFUSED_EXIT),
            b"Verb execution unavailable: busy\n"
        ));
        // A program may exit 75 itself, and the message alone is not a client verdict.
        assert!(!refused(Some(REFUSED_EXIT), b"temporary failure"));
        assert!(!refused(Some(1), refusal));
        assert!(!refused(None, refusal));
    }
    const PASSWD: &str = "root:x:0:0::/root:/bin/bash\nagent:x:1101:1101::/home/agent:/bin/bash\nhost:x:1102:1102::/home/host:/bin/bash\n";
    #[test]
    fn refuses_root_same_user_missing_and_option_shaped_accounts() {
        for (user, uid) in [
            ("root", 1102),
            ("host", 1102),
            ("agent", 0),
            ("missing", 1102),
            ("-agent", 1102),
        ] {
            assert!(parse_profile(user, PASSWD, uid).is_err());
        }
        assert_eq!(
            parse_profile("agent", PASSWD, 1102).unwrap().home,
            Path::new("/home/agent")
        );
    }
    #[test]
    fn launch_keeps_arguments_literal_and_refuses_host_secrets() {
        let profile = parse_profile("agent", PASSWD, 1102).unwrap();
        let args = launcher_args(
            &profile,
            "bash",
            &["literal; $(touch secret)".into()],
            Path::new("/srv/work space"),
            "abc",
            &[],
            Some(12),
        )
        .unwrap();
        assert_eq!(&args[..6], ["-n", "-H", "-u", "agent", "-C", "13"]);
        assert_eq!(args.last().unwrap(), "literal; $(touch secret)");
        assert!(launcher_args(
            &profile,
            "bash",
            &[],
            Path::new("/srv/work"),
            "abc",
            &[("VERB_TOKEN".into(), "synthetic".into())],
            None
        )
        .is_err());
    }
}
