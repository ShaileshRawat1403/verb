#![cfg(unix)]

use super::{EventLogger, Session};
use std::ffi::CString;
use std::fs::File;
use std::io::{self, IsTerminal, Read, Write};
use std::os::raw::{c_char, c_int, c_void};
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::path::Path;
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

unsafe extern "C" {
    fn chdir(path: *const c_char) -> c_int;
    fn setenv(name: *const c_char, value: *const c_char, overwrite: c_int) -> c_int;
    fn execvp(file: *const c_char, argv: *const *const c_char) -> c_int;
    fn _exit(status: c_int) -> !;
    fn waitpid(pid: PidT, status: *mut c_int, options: c_int) -> PidT;
    fn poll(fds: *mut PollFd, nfds: usize, timeout: c_int) -> c_int;
    fn read(fd: c_int, buffer: *mut c_void, count: usize) -> isize;
}

pub(super) fn run(
    project: &Path,
    session: &mut Session,
    command: &str,
    args: &[String],
    is_new_session: bool,
) -> Result<i32, String> {
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
            set_child_environment(&session.id, project);
            execvp(command_value.as_ptr(), argument_pointers.as_ptr());
            _exit(127);
        }
    }

    let mut master = unsafe { File::from_raw_fd(master_fd) };
    let mut logger = EventLogger::new(session)?;
    if is_new_session {
        logger.session_started(session)?;
    } else if let Some(agent) = session.agent.as_ref() {
        logger.agent_started(agent.label())?;
    }
    logger.process_started()?;
    super::save_session(session)?;

    let _terminal_mode = TerminalMode::new()?;
    proxy_terminal(&mut master, pid)
}

unsafe fn set_child_environment(session_id: &str, project: &Path) {
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
}

fn proxy_terminal(master: &mut File, pid: PidT) -> Result<i32, String> {
    let master_fd = master.as_raw_fd();
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
