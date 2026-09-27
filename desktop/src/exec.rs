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
    let mut command = base_git(&program, project);
    for name in local_filters(&program, project) {
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
    let mut command = Command::new(program);
    command
        .current_dir(project)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null());
    Some(command)
}

fn base_git(program: &Path, project: &Path) -> Command {
    let mut command = Command::new(program);
    command
        .args(["-c", "core.fsmonitor=false"])
        .current_dir(project)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null());
    command
}

/// Names of filter drivers the repository-local config defines. Reading config runs nothing.
fn local_filters(program: &Path, project: &Path) -> Vec<String> {
    let Ok(output) = base_git(program, project)
        .args([
            "config",
            "--local",
            "--includes",
            "--null",
            "--name-only",
            "--get-regexp",
            r"^filter\.",
        ])
        .stderr(Stdio::null())
        .output()
    else {
        return Vec::new();
    };
    let mut names: Vec<String> = String::from_utf8_lossy(&output.stdout)
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
    names
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

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
