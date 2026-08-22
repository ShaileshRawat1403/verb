#![cfg(unix)]

use super::shell::{ShellEvent, ShellScanner};
use super::{EventLogger, Session};
use std::ffi::CString;
use std::fs::File;
use std::io::{self, IsTerminal, Read, Write};
use std::os::raw::{c_char, c_int, c_void};
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::ptr;

const STDIN_FILENO: c_int = 0;
const POLLIN: CShort = 0x001;
const POLLERR: CShort = 0x008;
const POLLHUP: CShort = 0x010;
const EIO: i32 = 5;
const EINTR: i32 = 4;
const WNOHANG: c_int = 1;

type CShort = i16;
type PidT = c_int;

#[repr(C)]
struct PollFd {
    fd: c_int,
    events: CShort,
    revents: CShort,
}

#[link(name = "util")]
unsafe extern "C" {
    fn forkpty(
        amaster: *mut c_int,
        name: *mut c_char,
        termp: *const c_void,
        winp: *const c_void,
    ) -> PidT;
}

#[repr(C)]
pub(super) struct WinSize {
    pub rows: u16,
    pub cols: u16,
    pub x_pixels: u16,
    pub y_pixels: u16,
}

const TIOCSWINSZ: u64 = 0x8008_7467;

unsafe extern "C" {
    fn ioctl(fd: c_int, request: u64, ...) -> c_int;
    fn chdir(path: *const c_char) -> c_int;
    fn setenv(name: *const c_char, value: *const c_char, overwrite: c_int) -> c_int;
    fn execvp(file: *const c_char, argv: *const *const c_char) -> c_int;
    fn _exit(status: c_int) -> !;
    fn waitpid(pid: PidT, status: *mut c_int, options: c_int) -> PidT;
    fn poll(fds: *mut PollFd, nfds: usize, timeout: c_int) -> c_int;
    fn read(fd: c_int, buffer: *mut c_void, count: usize) -> isize;
}

/// A process running on its own PTY, with the master side handed back to the caller.
///
/// Extracted so the two hosts of a session -- the blocking CLI proxy and the TUI, which draws the
/// same terminal inside a pane -- start processes exactly the same way. Neither owns a private
/// notion of how a session is launched.
pub(super) struct PtyProcess {
    pub master: File,
    pub pid: PidT,
}

pub(super) fn spawn(
    project: &Path,
    session_id: &str,
    command: &str,
    args: &[String],
    env: &[(String, String)],
    size: Option<(u16, u16)>,
) -> Result<PtyProcess, String> {
    let (master, pid) = fork_pty(project, session_id, command, args, env)?;
    if let Some((rows, cols)) = size {
        set_window_size(&master, rows, cols);
    }
    Ok(PtyProcess { master, pid })
}

/// Tells the kernel how big the PTY is, so full-screen agents lay themselves out to the pane they
/// are actually drawn in. Failure is not fatal: the session still runs, at the default 80x24.
pub(super) fn set_window_size(master: &File, rows: u16, cols: u16) {
    let size = WinSize {
        rows,
        cols,
        x_pixels: 0,
        y_pixels: 0,
    };
    unsafe {
        ioctl(master.as_raw_fd(), TIOCSWINSZ, &size);
    }
}

pub(super) fn reap(pid: PidT) -> Result<Option<i32>, String> {
    wait_nonblocking(pid)
}

pub(super) fn run(
    project: &Path,
    session: &mut Session,
    command: &str,
    args: &[String],
    env: &[(String, String)],
    is_new_session: bool,
) -> Result<i32, String> {
    let (mut master, pid) = fork_pty(project, &session.id, command, args, env)?;

    let mut logger = EventLogger::new(session)?;
    if is_new_session {
        logger.session_started(session)?;
    } else if let Some(agent) = session.agent.as_ref() {
        logger.agent_started(agent.label())?;
    }
    logger.process_started()?;
    super::save_session(session)?;

    let _terminal_mode = TerminalMode::new()?;
    proxy_terminal(&mut master, pid, session, &mut logger)
}

fn fork_pty(
    project: &Path,
    session_id: &str,
    command: &str,
    args: &[String],
    env: &[(String, String)],
) -> Result<(File, PidT), String> {
    let project_value = CString::new(project.to_string_lossy().as_bytes())
        .map_err(|_| "project path contains a NUL byte".to_owned())?;
    let command_value =
        CString::new(command).map_err(|_| "command contains a NUL byte".to_owned())?;
    let mut argument_values = Vec::with_capacity(args.len() + 1);
    argument_values.push(command_value.clone());
    for arg in args {
        argument_values.push(
            CString::new(arg.as_str())
                .map_err(|_| "command argument contains a NUL byte".to_owned())?,
        );
    }
    let mut argument_pointers = argument_values
        .iter()
        .map(|value| value.as_ptr())
        .collect::<Vec<_>>();
    argument_pointers.push(ptr::null());

    // Converted before the fork: allocating in the child of a fork is not safe.
    let child_env = env
        .iter()
        .map(|(name, value)| {
            let name = CString::new(name.as_str())
                .map_err(|_| "environment name contains a NUL byte".to_owned())?;
            let value = CString::new(value.as_str())
                .map_err(|_| "environment value contains a NUL byte".to_owned())?;
            Ok((name, value))
        })
        .collect::<Result<Vec<_>, String>>()?;

    let mut master_fd = -1;
    let pid = unsafe { forkpty(&mut master_fd, ptr::null_mut(), ptr::null(), ptr::null()) };
    if pid < 0 {
        return Err(io::Error::last_os_error().to_string());
    }

    if pid == 0 {
        unsafe {
            if chdir(project_value.as_ptr()) != 0 {
                _exit(126);
            }
            set_child_environment(session_id, project, &child_env);
            execvp(command_value.as_ptr(), argument_pointers.as_ptr());
            _exit(127);
        }
    }

    Ok((unsafe { File::from_raw_fd(master_fd) }, pid))
}

/// Turns the shell's own markers into the durable structural facts `docs/VERB_SESSION_SCHEMA.md`
/// allows, and keeps the session's remembered working directory current.
///
/// Only a shell with integration enabled emits these at all; agents like Claude and Codex do not.
/// Verb records what the shell actually reported and invents nothing when it reports nothing --
/// silence here means "unknown", never a fabricated boundary.
/// A structural fact the caller may want to react to, in addition to it being logged.
///
/// Only what the shell actually reported: which command boundary closed and with what status. The
/// command *text* is not here and cannot be -- `OSC 633;E` is skipped by the scanner itself.
pub(super) enum Structural {
    /// A tool the agent ran, which the agent's own record says failed.
    ///
    /// Distinct from `CommandFinished` on purpose: that one Verb watched happen through shell
    /// integration, this one Verb read afterwards in a file the agent wrote. The contract calls
    /// that difference out -- an agent's claim is not verified execution -- so the two must not
    /// arrive as the same kind of fact.
    AgentToolFailed {
        millis: u128,
        /// The tool's name, when the record named one. Never its arguments or its output.
        tool: Option<String>,
    },
    CommandFinished {
        exit_code: i32,
        millis: u128,
        /// What the shell said it was running, when it said so. **Volatile display state**: it is
        /// handed to the screen and dropped. It is never written to the session record, the event
        /// log, or anything else durable -- the schema has no field for it.
        label: Option<String>,
    },
}

pub(super) struct ShellIntegration {
    scanner: ShellScanner,
    command_count: u64,
    open_command: Option<String>,
    started_at: Option<u128>,
    /// The command line for the command currently running, held only until it finishes.
    volatile_text: Option<String>,
}

impl ShellIntegration {
    pub(super) fn new() -> Self {
        Self {
            scanner: ShellScanner::default(),
            command_count: 0,
            open_command: None,
            started_at: None,
            volatile_text: None,
        }
    }

    pub(super) fn observe(
        &mut self,
        bytes: &[u8],
        session: &mut Session,
        logger: &mut EventLogger,
    ) -> Result<Vec<Structural>, String> {
        let mut structural = Vec::new();
        for event in self.scanner.feed(bytes) {
            match event {
                ShellEvent::CurrentDirectory(path) => {
                    let path = PathBuf::from(path);
                    if session.last_known_cwd.as_deref() == Some(path.as_path()) {
                        continue;
                    }
                    logger.cwd_changed(&path.to_string_lossy())?;
                    session.last_known_cwd = Some(path);
                    session.last_observed_at = Some(super::now_millis());
                    super::save_session(session)?;
                }
                // Arrives just before CommandStart. Kept in memory for the length of one command.
                ShellEvent::CommandText(text) => self.volatile_text = Some(text),
                ShellEvent::CommandStart => {
                    self.command_count += 1;
                    self.started_at = Some(super::now_millis());
                    let command_id = format!("{}-c{}", session.id, self.command_count);
                    logger.command_started(
                        &command_id,
                        session
                            .last_known_cwd
                            .as_ref()
                            .map(|path| path.to_string_lossy())
                            .as_deref(),
                    )?;
                    self.open_command = Some(command_id);
                }
                ShellEvent::CommandEnd(exit_code) => {
                    // A finish with no start is a marker Verb never saw the other half of (the
                    // session attached mid-command, say). Recording it against an invented id would
                    // put a command in the log that Verb cannot account for.
                    if let Some(command_id) = self.open_command.take() {
                        logger.command_finished(&command_id, exit_code)?;
                        let millis = self
                            .started_at
                            .take()
                            .map(|started| super::now_millis().saturating_sub(started))
                            .unwrap_or(0);
                        // Taken, not cloned: the text leaves with the outcome and nothing here
                        // keeps a copy.
                        structural.push(Structural::CommandFinished {
                            exit_code,
                            millis,
                            label: self.volatile_text.take(),
                        });
                    }
                }
                ShellEvent::PromptStart | ShellEvent::PromptEnd => {}
            }
        }
        Ok(structural)
    }
}

unsafe fn set_child_environment(session_id: &str, project: &Path, extra: &[(CString, CString)]) {
    let Ok(session_id) = CString::new(session_id) else {
        _exit(126);
    };
    let Ok(project) = CString::new(project.to_string_lossy().as_bytes()) else {
        _exit(126);
    };
    let verb_session_id = CString::new("VERB_SESSION_ID").expect("literal has no NUL");
    let verb_project_root = CString::new("VERB_PROJECT_ROOT").expect("literal has no NUL");
    setenv(verb_session_id.as_ptr(), session_id.as_ptr(), 1);
    setenv(verb_project_root.as_ptr(), project.as_ptr(), 1);
    for (name, value) in extra {
        setenv(name.as_ptr(), value.as_ptr(), 1);
    }
}

fn proxy_terminal(
    master: &mut File,
    pid: PidT,
    session: &mut Session,
    logger: &mut EventLogger,
) -> Result<i32, String> {
    let master_fd = master.as_raw_fd();
    let mut integration = ShellIntegration::new();
    let mut input_open = true;
    let mut child_code = None;
    let mut output_buffer = [0_u8; 16 * 1024];
    let mut input_buffer = [0_u8; 16 * 1024];

    loop {
        let mut descriptors = [
            PollFd {
                fd: master_fd,
                events: POLLIN,
                revents: 0,
            },
            PollFd {
                fd: STDIN_FILENO,
                events: if input_open { POLLIN } else { 0 },
                revents: 0,
            },
        ];
        let poll_result = unsafe { poll(descriptors.as_mut_ptr(), descriptors.len(), 100) };
        if poll_result < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(EINTR) {
                continue;
            }
            return Err(format!("terminal poll failed: {error}"));
        }

        if input_open && descriptors[1].revents & (POLLIN | POLLERR | POLLHUP) != 0 {
            let count = unsafe {
                read(
                    STDIN_FILENO,
                    input_buffer.as_mut_ptr().cast::<c_void>(),
                    input_buffer.len(),
                )
            };
            if count > 0 {
                let bytes = &input_buffer[..count as usize];
                master
                    .write_all(bytes)
                    .map_err(|error| format!("could not write to PTY: {error}"))?;
            } else {
                input_open = false;
            }
        }

        if descriptors[0].revents & (POLLIN | POLLERR | POLLHUP) != 0 {
            match master.read(&mut output_buffer) {
                Ok(0) => break,
                Ok(count) => {
                    let bytes = &output_buffer[..count];
                    // The bytes go to the terminal first and are scanned for structural markers
                    // afterwards, so nothing Verb does here can delay what the user sees. They are
                    // never retained: the scanner keeps only an in-flight marker, if any.
                    // The CLI proxy has no surface to react on: the events are logged, and the
                    // structural outcomes are of no further use to it.
                    let _ = integration.observe(bytes, session, logger)?;
                    io::stdout()
                        .write_all(bytes)
                        .map_err(|error| format!("could not write terminal output: {error}"))?;
                    io::stdout()
                        .flush()
                        .map_err(|error| format!("could not flush terminal output: {error}"))?;
                }
                Err(error) if error.raw_os_error() == Some(EIO) => break,
                Err(error) => return Err(format!("could not read PTY output: {error}")),
            }
        }

        if child_code.is_none() {
            child_code = wait_nonblocking(pid)?;
        }
        if child_code.is_some() && descriptors[0].revents & (POLLHUP | POLLERR) != 0 {
            break;
        }
    }

    child_code
        .or_else(|| wait_blocking(pid).ok())
        .ok_or_else(|| "PTY child did not report an exit status".to_owned())
}

fn wait_nonblocking(pid: PidT) -> Result<Option<i32>, String> {
    let mut status = 0;
    let result = unsafe { waitpid(pid, &mut status, WNOHANG) };
    if result == 0 {
        Ok(None)
    } else if result == pid {
        Ok(Some(decode_wait_status(status)))
    } else {
        Err(format!("waitpid failed: {}", io::Error::last_os_error()))
    }
}

fn wait_blocking(pid: PidT) -> Result<i32, String> {
    let mut status = 0;
    let result = unsafe { waitpid(pid, &mut status, 0) };
    if result == pid {
        Ok(decode_wait_status(status))
    } else {
        Err(format!("waitpid failed: {}", io::Error::last_os_error()))
    }
}

fn decode_wait_status(status: c_int) -> i32 {
    if status & 0x7f == 0 {
        (status >> 8) & 0xff
    } else {
        128 + (status & 0x7f)
    }
}

struct TerminalMode {
    saved: Option<String>,
}

impl TerminalMode {
    fn new() -> Result<Self, String> {
        if !io::stdin().is_terminal() {
            return Ok(Self { saved: None });
        }

        let saved = Command::new("stty")
            .arg("-g")
            .stdin(Stdio::inherit())
            .output()
            .map_err(|error| format!("could not inspect terminal mode: {error}"))?;
        if !saved.status.success() {
            return Err("could not inspect terminal mode with stty".to_owned());
        }
        let saved = String::from_utf8_lossy(&saved.stdout).trim().to_owned();
        let raw = Command::new("stty")
            .args(["raw", "-echo"])
            .stdin(Stdio::inherit())
            .status()
            .map_err(|error| format!("could not enter raw terminal mode: {error}"))?;
        if !raw.success() {
            return Err("could not enter raw terminal mode with stty".to_owned());
        }
        Ok(Self { saved: Some(saved) })
    }
}

impl Drop for TerminalMode {
    fn drop(&mut self) {
        if let Some(saved) = &self.saved {
            let _ = Command::new("stty")
                .arg(saved)
                .stdin(Stdio::inherit())
                .status();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Agent, Session};
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "verb-pty-{name}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    /// Volatile means volatile: the command line is handed to the caller once, with the command it
    /// belongs to, and nothing keeps a copy afterwards. A second command with no `E` sequence must
    /// not inherit the first one's label.
    #[test]
    fn command_text_is_handed_over_once_and_never_retained() {
        let root = scratch("volatile");
        std::env::set_var("VERB_STATE_DIR", &root);

        let session = Session::new(root.join("project"), Agent::Shell);
        let mut session = session;
        let mut logger = EventLogger::new(&session).unwrap();
        let mut integration = ShellIntegration::new();

        let first = integration
            .observe(
                b"\x1b]633;E;npm test\x07\x1b]633;C\x07\x1b]633;D;1\x07",
                &mut session,
                &mut logger,
            )
            .unwrap();
        match first.as_slice() {
            [Structural::CommandFinished {
                exit_code, label, ..
            }] => {
                assert_eq!(*exit_code, 1);
                assert_eq!(label.as_deref(), Some("npm test"));
            }
            other => panic!("expected one finished command, got {}", other.len()),
        }

        // A second command that reports no text must come back with none, not with the last one.
        let second = integration
            .observe(b"\x1b]633;C\x07\x1b]633;D;0\x07", &mut session, &mut logger)
            .unwrap();
        match second.as_slice() {
            [Structural::CommandFinished { label, .. }] => assert_eq!(label.as_deref(), None),
            other => panic!("expected one finished command, got {}", other.len()),
        }

        // And none of it reached the event log.
        let mut written = String::new();
        crate::tests_support::collect_files(&root, &mut written);
        assert!(
            !written.contains("npm test"),
            "command text must not be written anywhere durable:\n{written}"
        );
    }
}
