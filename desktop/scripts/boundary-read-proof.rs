//! Operator-only Linux proof driver. Uses the production collector without running a Verb host.
#[cfg(not(target_os = "linux"))]
compile_error!("the installed boundary proof requires Linux");

#[path = "../src/exec/bounded.rs"]
mod bounded;

// Same nonblocking pipe setup as pty.rs, without linking the whole terminal host into this tool.
mod pty {
    use std::io;
    unsafe extern "C" {
        fn fcntl(fd: i32, command: i32, ...) -> i32;
    }
    pub(crate) fn set_nonblocking(fd: i32) -> io::Result<()> {
        let flags = unsafe { fcntl(fd, 3) };
        if flags < 0 || unsafe { fcntl(fd, 4, flags | 0o4000) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

fn main() {
    use std::process::Command;
    use std::time::{Duration, Instant};
    let args: Vec<String> = std::env::args().skip(1).collect();
    assert!(args.len() == 2 && args[0].len() == 32);
    assert!(args[0].bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)));
    assert!(args[1].starts_with("read-proof-") && args[1].bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-'));
    let mut command = Command::new("/usr/local/libexec/verb-boundary-client");
    command.args([
        "--project", &args[0], "--session", &args[1], "--mode", "pipes",
        "--program", "bash", "--", "-c", "printf READ_PROOF_ADMITTED; /usr/bin/setsid /bin/sleep 60 & wait",
    ]);
    let start = Instant::now();
    match bounded::output(&mut command, Duration::from_secs(15), 8 * 1024 * 1024) {
        Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {
            println!("{{\"timedOut\":true,\"elapsedMs\":{}}}", start.elapsed().as_millis());
        }
        _ => {
            eprintln!("proof did not exercise the collector timeout");
            std::process::exit(1);
        }
    }
}
