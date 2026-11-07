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
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

pub(crate) const READ_TIMEOUT: Duration = Duration::from_secs(3);
pub(crate) const READ_LIMIT: usize = 2 * 1024 * 1024;

/// Drain both pipes without blocking or spawning readers which can outlive the request.
/// A descendant retaining a pipe is subject to the same deadline as its parent.
pub(crate) fn bounded_output(
    command: &mut Command,
    timeout: Duration,
    limit: usize,
) -> io::Result<Output> {
    bounded_output_with_input(command, timeout, limit, None)
}

pub(crate) fn bounded_output_with_input(
    command: &mut Command,
    timeout: Duration,
    limit: usize,
    input: Option<&[u8]>,
) -> io::Result<Output> {
    run_bounded(command, timeout, limit, input, None)
}

pub(crate) fn bounded_stdout(
    command: &mut Command,
    timeout: Duration,
    limit: usize,
    sink: &mut dyn FnMut(&[u8]),
) -> io::Result<Output> {
    run_bounded(command, timeout, limit, None, Some(sink))
}

type OutputSink<'a> = &'a mut dyn FnMut(&[u8]);

fn run_bounded(
    command: &mut Command,
    timeout: Duration,
    limit: usize,
    input: Option<&[u8]>,
    mut sink: Option<OutputSink<'_>>,
) -> io::Result<Output> {
    command.process_group(0);
    let mut child = command
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let result = (|| {
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("missing stdout"))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("missing stderr"))?;
        crate::pty::set_nonblocking(stdout.as_raw_fd())?;
        crate::pty::set_nonblocking(stderr.as_raw_fd())?;
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let mut seen = 0usize;
        let (mut out_done, mut err_done) = (false, false);
        let mut stdin = child.stdin.take();
        if let Some(pipe) = &stdin {
            crate::pty::set_nonblocking(pipe.as_raw_fd())?;
        }
        let mut remaining = input.unwrap_or_default();
        let deadline = Instant::now() + timeout;
        loop {
            if remaining.is_empty() {
                stdin.take();
            }
            if let Some(pipe) = &mut stdin {
                match pipe.write(&remaining[..remaining.len().min(16 * 1024)]) {
                    Ok(n) => remaining = &remaining[n..],
                    Err(e)
                        if matches!(
                            e.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                        ) => {}
                    Err(e) => return Err(e),
                }
            }
            // One bounded read per pipe per tick: a continuous producer cannot starve the clock.
            for (index, (pipe, bytes, done)) in [
                (&mut stdout as &mut dyn Read, &mut out, &mut out_done),
                (&mut stderr as &mut dyn Read, &mut err, &mut err_done),
            ]
            .into_iter()
            .enumerate()
            {
                if *done {
                    continue;
                }
                let mut buffer = [0; 16 * 1024];
                match pipe.read(&mut buffer) {
                    Ok(0) => *done = true,
                    Ok(n) => {
                        seen = seen.saturating_add(n);
                        if seen > limit {
                            return Err(io::Error::other("command output exceeded its limit"));
                        }
                        if index == 0 {
                            if let Some(consume) = sink.as_deref_mut() {
                                consume(&buffer[..n]);
                            } else {
                                bytes.extend_from_slice(&buffer[..n]);
                            }
                        } else {
                            bytes.extend_from_slice(&buffer[..n]);
                        }
                    }
                    Err(e)
                        if matches!(
                            e.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                        ) => {}
                    Err(e) => return Err(e),
                }
            }
            if let Some(status) = child.try_wait()? {
                if out_done && err_done {
                    return Ok(Output {
                        status,
                        stdout: out,
                        stderr: err,
                    });
                }
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "command deadline exceeded",
                ));
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    })();
    if result.is_err() {
        let _ = crate::pty::signal_group(child.id() as i32, 9);
        let _ = child.kill();
        // An uninterruptible filesystem wait must not hold an HTTP worker's permit.
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
    result
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
    command.env("LC_ALL", "C").args([
        "config",
        "--local",
        "--includes",
        "--null",
        "--name-only",
        "--get-regexp",
        r"^filter\.",
    ]);
    let output = bounded_output(&mut command, READ_TIMEOUT, READ_LIMIT).ok()?;
    // Exit 1 means no matching configuration keys. Other failures must fail closed.
    if !output.status.success() && output.status.code() != Some(1) {
        if output.status.code() == Some(128)
            && String::from_utf8_lossy(&output.stderr)
                .contains("--local can only be used inside a git repository")
        {
            return Some(Vec::new());
        }
        return None;
    }
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
    Some(names)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn commands_have_one_deadline_even_when_a_descendant_keeps_the_pipe_open() {
        for script in ["sleep 30", "sleep 30 & exit 0"] {
            let started = Instant::now();
            let error = bounded_output(
                Command::new("/bin/sh").args(["-c", script]),
                Duration::from_millis(80),
                4096,
            )
            .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::TimedOut);
            assert!(started.elapsed() < Duration::from_secs(2));
        }
    }

    #[test]
    fn output_limits_and_full_stdin_do_not_deadlock_the_caller() {
        let error = bounded_output(
            Command::new("/bin/sh").args(["-c", "while :; do printf 'oversized output'; done"]),
            Duration::from_secs(2),
            1024,
        )
        .unwrap_err();
        assert!(error.to_string().contains("output exceeded"));
        let input = vec![b'x'; 256 * 1024];
        let output = bounded_output_with_input(
            &mut Command::new("/bin/cat"),
            Duration::from_secs(2),
            input.len(),
            Some(&input),
        )
        .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, input);
    }

    #[test]
    fn unreadable_filter_config_fails_closed() {
        let root = std::env::temp_dir().join(format!("verb-filter-{}", crate::new_id()));
        fs::create_dir_all(&root).unwrap();
        let program = root.join("git");
        fs::write(&program, "#!/bin/sh\nexit 2\n").unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(local_filters(&program, &root).is_none());
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
