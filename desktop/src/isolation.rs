//! Opt-in Linux account separation through a fixed operator-provisioned launcher.
use std::path::{Path, PathBuf};
use std::process::Command;

const LAUNCHER: &str = "/usr/local/libexec/verb-agent-launch";
pub(crate) const SHELL_DIR: &str = "/var/lib/verb-shell";

#[derive(Clone, Debug)]
pub(crate) struct Profile {
    pub user: String,
    pub home: PathBuf,
    uid: u32,
}

pub(crate) fn profile() -> Result<Option<Profile>, String> {
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
        let agent_home = std::fs::symlink_metadata(&profile.home).map_err(|e| e.to_string())?;
        if !agent_home.is_dir() || agent_home.uid() != profile.uid || agent_home.mode() & 0o022 != 0
        {
            return Err("agent HOME must belong to the agent and not be writable by others".into());
        }
    }
    Ok(Some(profile))
}

fn parse_profile(user: &str, passwd: &str, host_uid: u32) -> Result<Profile, String> {
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
    Ok(Profile {
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
        Ok(Some(profile)) => Some(profile.home),
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
        Some(profile) => Ok((
            sudo_path().to_string_lossy().into_owned(),
            launcher_args(&profile, program, args, project, session, env, lock_fd)?,
        )),
    }
}

fn launcher_args(
    profile: &Profile,
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
    let (program, args) = launch(&program.to_string_lossy(), &[], project, session, &[], None)?;
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
    let Some(profile) = profile()? else {
        return Ok(());
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

#[cfg(test)]
mod tests {
    use super::*;
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
