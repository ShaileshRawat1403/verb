//! Running the user's own tools against a project the user did not necessarily write.
//!
//! The observation modules (`runtime`, `gitstate`, `good`) run `git`, `node`, `python3` and friends
//! automatically: when the TUI opens, after a command, when the web page asks. Opening a project in
//! Verb must never be a way for that project to run code, so every such spawn goes through here:
//!
//! * **The program is found on absolute `PATH` entries only**, and never inside the project. A
//!   relative entry (`node_modules/.bin`, `./bin`, `.`, or an empty entry from a stray `:`) is
//!   resolved by the OS against the child's working directory -- the project -- so the repository
//!   would choose the binary.
//! * **Git runs with repository-controlled execution turned off**: `core.fsmonitor` (a hook
//!   `git status` would run), and callers add `--no-ext-diff`/`--no-textconv`/`--no-filters` where a
//!   command would otherwise apply configured drivers. A checkout copied from an archive or a shared
//!   drive is not protected by Git's ownership check, so Verb does not rely on it.
//! * stdin is closed and optional locks are off, so nothing prompts and nothing blocks a concurrent
//!   `git` the user is running.

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

mod bounded;

/// Automatic reads only. Command construction may separately perform a bounded config probe.
pub(crate) fn read_output(command: &mut Command) -> std::io::Result<std::process::Output> {
    bounded::output(command, Duration::from_secs(15), 8 * 1024 * 1024)
}

pub(crate) fn read_output_before(
    command: &mut Command,
    deadline: std::time::Instant,
) -> std::io::Result<std::process::Output> {
    let remaining = deadline
        .checked_duration_since(std::time::Instant::now())
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::TimedOut, "Git review deadline reached")
        })?;
    bounded::output(
        command,
        remaining.min(Duration::from_secs(15)),
        8 * 1024 * 1024,
    )
}

/// Explicit user operations may contact a remote, but must still release their worker.
pub(crate) fn git_operation_output(command: &mut Command) -> std::io::Result<std::process::Output> {
    bounded::output(command, Duration::from_secs(60), 1024 * 1024)
}

pub(crate) fn check_output(command: &mut Command) -> std::io::Result<std::process::Output> {
    bounded::output(command, Duration::from_secs(180), 1024 * 1024)
}

/// Decode a prepared Git command's result after the caller's chosen execution path.
pub(crate) fn git_text(output: std::io::Result<std::process::Output>) -> Result<String, String> {
    let output = output.map_err(|error| format!("could not run git: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

/// `name` resolved against absolute `PATH` entries, refusing any candidate inside `project`.
pub(crate) fn trusted_program(name: &str, project: &Path) -> Option<PathBuf> {
    trusted_program_on(&env::var_os("PATH")?, name, project)
}

fn trusted_program_on(path: &std::ffi::OsStr, name: &str, project: &Path) -> Option<PathBuf> {
    let project = project
        .canonicalize()
        .unwrap_or_else(|_| project.to_path_buf());
    env::split_paths(path)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
        .filter(|candidate| {
            let real = candidate
                .canonicalize()
                .unwrap_or_else(|_| candidate.clone());
            !real.starts_with(&project)
        })
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// A `git` command for reading `project`, or `None` when no trusted `git` exists.
///
/// Content filters are the one execution path `--no-ext-diff`/`--no-textconv` cannot switch off:
/// `git status` runs a `clean` filter on any file whose size is unchanged, and `git diff` on every
/// modified one. Filters defined in the *repository's own* config (and anything it includes) are
/// therefore blanked for Verb's commands. Filters from the user's global or system config -- git-lfs,
/// typically -- are the user's own choice and keep working.
pub(crate) fn git(project: &Path) -> Option<Command> {
    let program = trusted_program("git", project)?;
    let mut command = base_git(&program, project)?;
    for name in local_filters(&program, project)? {
        for key in ["clean", "smudge", "process"] {
            command.arg("-c").arg(format!("filter.{name}.{key}="));
        }
        command
            .arg("-c")
            .arg(format!("filter.{name}.required=false"));
    }
    Some(command)
}

/// `git` for an operation the user asked for (`verb project worktree`): found on a trusted `PATH`
/// entry, but otherwise Git as the user configured it, so their own filters and hooks behave as they
/// would from the shell.
pub(crate) fn user_git(project: &Path) -> Option<Command> {
    let program = trusted_program("git", project)?;
    let mut command = crate::isolation::command(&program, project, "").ok()?;
    command
        .current_dir(project)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null());
    Some(command)
}

fn base_git(program: &Path, project: &Path) -> Option<Command> {
    let mut command = crate::isolation::command(program, project, "").ok()?;
    command
        .args(["-c", "core.fsmonitor=false"])
        .current_dir(project)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null());
    Some(command)
}

/// Names of filter drivers the repository-local config defines. Reading config runs nothing.
fn local_filters(program: &Path, project: &Path) -> Option<Vec<String>> {
    let mut command = base_git(program, project)?;
    command
        .args([
            "config",
            "--local",
            "--includes",
            "--null",
            "--name-only",
            "--get-regexp",
            r"^filter\.",
        ])
        .env("LC_ALL", "C");
    let output = bounded::output(&mut command, Duration::from_secs(5), 65536).ok()?;
    // Exit 1 with empty output is Git's ordinary "no matching keys" result.
    // An unreadable config must never silently re-enable repository-controlled filters.
    let no_matches = output.status.code() == Some(1) && output.stdout.is_empty();
    let not_repository = output.status.code() == Some(128)
        && output.stdout.is_empty()
        && output.stderr == b"fatal: --local can only be used inside a git repository\n";
    if not_repository {
        return Some(Vec::new());
    }
    if !(output.status.success() || no_matches) {
        return None;
    }
    let raw = std::str::from_utf8(&output.stdout).ok()?;
    let mut names: Vec<String> = raw
        .split('\0')
        .filter_map(|key| {
            let rest = key.strip_prefix("filter.")?;
            let (name, _) = rest.rsplit_once('.')?;
            Some(name.to_owned())
        })
        .filter(|name| !name.is_empty())
        .collect();
    names.sort();
    names.dedup();
    Some(names)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn filter_discovery_fails_closed_on_invalid_config() {
        let root = std::env::temp_dir().join(format!("verb-filters-{}", crate::new_id()));
        fs::create_dir_all(&root).unwrap();
        assert!(Command::new("git")
            .args(["init", "-q"])
            .arg(&root)
            .status()
            .unwrap()
            .success());
        fs::write(root.join(".git/config"), "[broken\n").unwrap();
        let program = trusted_program("git", &root).unwrap();
        assert!(local_filters(&program, &root).is_none());
        assert!(git(&root).is_none());
        fs::write(
            root.join(".git/config"),
            "[core]\nrepositoryformatversion = 0\n",
        )
        .unwrap();
        assert_eq!(local_filters(&program, &root), Some(Vec::new()));
        fs::write(
            root.join(".git/config"),
            "[filter \"danger\"]\nclean = false\nprocess = false\n",
        )
        .unwrap();
        assert_eq!(local_filters(&program, &root), Some(vec!["danger".into()]));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stalled_filter_discovery_has_a_deadline() {
        let root = std::env::temp_dir().join(format!("verb-filter-stall-{}", crate::new_id()));
        fs::create_dir_all(&root).unwrap();
        let program = root.join("git-fixture");
        fs::write(&program, "#!/bin/sh\nexec sleep 30\n").unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
        let start = std::time::Instant::now();
        assert!(local_filters(&program, &root).is_none());
        assert!(start.elapsed() < Duration::from_secs(8));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_binary_inside_the_project_is_never_chosen() {
        let root = std::env::temp_dir().join(format!("verb-exec-{}", crate::new_id()));
        let bin = root.join("node_modules").join(".bin");
        fs::create_dir_all(&bin).unwrap();
        let fake = bin.join("probe");
        fs::write(&fake, "#!/bin/sh\necho owned\n").unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
        let elsewhere = std::env::temp_dir().join(format!("verb-exec-else-{}", crate::new_id()));
        fs::create_dir_all(&elsewhere).unwrap();

        let absolute = std::env::join_paths([bin.clone()]).unwrap();
        // Found through an absolute entry, for a project that does not contain it...
        assert!(trusted_program_on(&absolute, "probe", &elsewhere).is_some());
        // ...but never for the project that does,
        assert!(trusted_program_on(&absolute, "probe", &root).is_none());
        // and never through a relative or empty entry, which the OS would resolve in the project.
        let relative = std::ffi::OsString::from("node_modules/.bin::.");
        assert!(trusted_program_on(&relative, "probe", &root).is_none());
        // Not executable is not a program.
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(trusted_program_on(&absolute, "probe", &elsewhere).is_none());

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(elsewhere).unwrap();
    }
}
