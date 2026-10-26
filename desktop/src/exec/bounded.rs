//! Bounded collection for observations and explicitly requested Git transfers.
//! A stopped mutation has an unconfirmed outcome; callers must not report it as rolled back.
use std::io;
use std::process::{Command, Output};
use std::time::Duration;

pub(super) fn output(command: &mut Command, timeout: Duration, limit: usize) -> io::Result<Output> {
    #[cfg(unix)]
    {
        unix::output(command, timeout, limit, None)
    }
    #[cfg(not(unix))]
    {
        let _ = (command, timeout, limit);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "bounded observations require Unix",
        ))
    }
}

/// Bounded full-duplex exchange; stdin is closed after the supplied bytes are written.
#[cfg(unix)]
pub(super) fn exchange(
    command: &mut Command,
    timeout: Duration,
    limit: usize,
    input: &[u8],
) -> io::Result<Output> {
    unix::output(command, timeout, limit, Some(input))
}

#[cfg(unix)]
mod unix {
    use super::*;
    use std::io::{Read, Write};
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Stdio};
    use std::time::Instant;

    unsafe extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }

    struct Probe {
        child: Child,
        reaped: bool,
    }
    impl Drop for Probe {
        fn drop(&mut self) {
            if self.reaped {
                return;
            }
            // The leader has not been reaped on error: its PID cannot have been reused.
            // For external commands, closing the dedicated client also leaves descendant
            // cleanup with the boundary controller; never remove its recovery receipts.
            unsafe { kill(-(self.child.id() as i32), 9) };
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    // Limit each drain turn as well as total bytes, so a continuously writing child cannot
    // starve the deadline check or the other stream. No pipe-reader threads can be stranded.
    fn drain(reader: &mut impl Read, bytes: &mut Vec<u8>, limit: usize) -> io::Result<bool> {
        let mut buffer = [0; 8192];
        for _ in 0..8 {
            match reader.read(&mut buffer) {
                Ok(0) => return Ok(true),
                Ok(count) => {
                    if count > limit.saturating_sub(bytes.len()) {
                        return Err(io::Error::other("observation output exceeds limit"));
                    }
                    bytes.extend_from_slice(&buffer[..count]);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(false),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        Ok(false)
    }

    pub(super) fn output(
        command: &mut Command,
        timeout: Duration,
        limit: usize,
        input: Option<&[u8]>,
    ) -> io::Result<Output> {
        let deadline = Instant::now() + timeout;
        let mut probe = Probe {
            child: command
                .process_group(0)
                .stdin(if input.is_some() {
                    Stdio::piped()
                } else {
                    Stdio::null()
                })
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?,
            reaped: false,
        };
        let mut stdin = probe.child.stdin.take();
        if let Some(pipe) = stdin.as_ref() {
            crate::pty::set_nonblocking(pipe.as_raw_fd())?;
        }
        let input = input.unwrap_or_default();
        let mut sent = 0;
        let mut stdout = probe
            .child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("missing stdout"))?;
        let mut stderr = probe
            .child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("missing stderr"))?;
        crate::pty::set_nonblocking(stdout.as_raw_fd())?;
        crate::pty::set_nonblocking(stderr.as_raw_fd())?;
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let (mut out_done, mut err_done) = (false, false);
        loop {
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "observation timed out",
                ));
            }
            if let Some(pipe) = stdin.as_mut() {
                if sent < input.len() {
                    let end = (sent + 8192).min(input.len());
                    match pipe.write(&input[sent..end]) {
                        Ok(0) => {
                            return Err(io::Error::new(
                                io::ErrorKind::WriteZero,
                                "helper input closed",
                            ))
                        }
                        Ok(n) => sent += n,
                        Err(e)
                            if matches!(
                                e.kind(),
                                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                            ) => {}
                        Err(e) => return Err(e),
                    }
                }
                if sent == input.len() {
                    stdin.take();
                }
            }
            if !out_done {
                out_done = drain(&mut stdout, &mut out, limit)?;
            }
            if !err_done {
                err_done = drain(&mut stderr, &mut err, limit.min(65536))?;
            }
            // Do not reap an exited leader while a descendant holds a pipe: on timeout we
            // still need its reserved process-group ID to safely terminate that group.
            if out_done && err_done {
                if let Some(status) = probe.child.try_wait()? {
                    // Drop must not signal a PID after reaping it.
                    probe.reaped = true;
                    return Ok(Output {
                        status,
                        stdout: out,
                        stderr: err,
                    });
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn run(script: &str, timeout: Duration, limit: usize) -> io::Result<Output> {
            super::super::output(Command::new("/bin/sh").args(["-c", script]), timeout, limit)
        }

        #[test]
        fn exchanges_large_input_and_bounds_a_nonreader() {
            let input = vec![b'x'; 300_000];
            let result = super::super::exchange(
                &mut Command::new("/bin/cat"),
                Duration::from_secs(5),
                400_000,
                &input,
            )
            .unwrap();
            assert_eq!(result.stdout, input);
            let start = Instant::now();
            let error = super::super::exchange(
                Command::new("/bin/sh").args(["-c", "sleep 30"]),
                Duration::from_millis(100),
                100,
                &input,
            )
            .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::TimedOut);
            assert!(start.elapsed() < Duration::from_secs(3));
        }

        #[test]
        fn captures_both_streams_and_preserves_nonzero_exit() {
            let result = run(
                "printf ok; printf failure >&2; exit 7",
                Duration::from_secs(2),
                100,
            )
            .unwrap();
            assert_eq!(result.status.code(), Some(7));
            assert_eq!(result.stdout, b"ok");
            assert_eq!(result.stderr, b"failure");
        }

        #[test]
        fn stalled_child_and_inherited_pipe_are_bounded_and_do_not_block_next_read() {
            for script in ["exec sleep 30", "sleep 30 & exit 0"] {
                let start = Instant::now();
                let error = run(script, Duration::from_millis(100), 100).unwrap_err();
                assert_eq!(error.kind(), io::ErrorKind::TimedOut);
                assert!(start.elapsed() < Duration::from_secs(3));
                assert!(run("printf next", Duration::from_secs(2), 100)
                    .unwrap()
                    .status
                    .success());
            }
        }

        #[test]
        fn excessive_stdout_and_stderr_fail_instead_of_truncating() {
            for script in [
                "while :; do printf 12345678; done",
                "while :; do printf 12345678 >&2; done",
            ] {
                let start = Instant::now();
                let error = run(script, Duration::from_secs(2), 64).unwrap_err();
                assert!(error.to_string().contains("exceeds limit"));
                assert!(start.elapsed() < Duration::from_secs(3));
            }
        }

        #[test]
        fn timeout_stops_descendant_work_after_leader_exits() {
            let marker = std::env::temp_dir().join(format!("verb-probe-{}", crate::new_id()));
            let error = super::super::output(
                Command::new("/bin/sh")
                    .args([
                        "-c",
                        "(sleep 1; printf leaked > \"$1\") & exit 0",
                        "fixture",
                    ])
                    .arg(&marker),
                Duration::from_millis(100),
                100,
            )
            .unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::TimedOut);
            std::thread::sleep(Duration::from_millis(1200));
            assert!(
                !marker.exists(),
                "descendant survived timeout and wrote its marker"
            );
        }
    }
}
