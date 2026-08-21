//! End-to-end regression tests for the shell integration Verb hosts shells with.
//!
//! These run the real binary against a real shell on a real PTY, because the thing being tested is
//! whether a shell Verb launched actually reports what it did -- which no amount of unit testing
//! can establish.
//!
//! Each test gets its own `HOME` and `VERB_STATE_DIR`, so nothing here can touch the machine's own
//! shell configuration or session records. That isolation is also the point of one of the tests.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// A command that does not exist, so it fails, and is distinctive enough to search every file Verb
/// wrote for any trace of it.
const SECRET_COMMAND: &str = "verb-secret-command-9f3xq";
const MARKER: &str = "verb-user-config-loaded";

struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "verb-integration-{name}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("home")).unwrap();
        fs::create_dir_all(root.join("state")).unwrap();
        fs::create_dir_all(root.join("project")).unwrap();
        Self { root }
    }

    fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    fn state(&self) -> PathBuf {
        self.root.join("state")
    }

    fn project(&self) -> PathBuf {
        self.root.join("project")
    }

    /// Everything Verb wrote, concatenated: session records and event logs alike.
    fn everything_written(&self) -> String {
        let mut contents = String::new();
        collect(&self.state(), &mut contents);
        contents
    }

    fn events(&self) -> String {
        let mut contents = String::new();
        collect(&self.state().join("events"), &mut contents);
        contents
    }
}

fn collect(directory: &Path, into: &mut String) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, into);
        } else if let Ok(text) = fs::read_to_string(&path) {
            into.push_str(&text);
            into.push('\n');
        }
    }
}

fn shell_path(name: &str) -> Option<PathBuf> {
    let output = Command::new("which").arg(name).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    path.is_file().then_some(path)
}

/// Runs `verb shell` with the given shell, feeding it a short script, and returns its output.
fn run_shell(sandbox: &Sandbox, shell: &Path, script: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_verb"))
        .arg("shell")
        .current_dir(sandbox.project())
        .env_clear()
        .env("HOME", sandbox.home())
        .env("SHELL", shell)
        .env("VERB_STATE_DIR", sandbox.state())
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("TERM", "xterm-256color")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("verb should start");

    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(script.as_bytes())
        .expect("script should be written");

    let output = child.wait_with_output().expect("verb should finish");
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn assert_reports_structurally(sandbox: &Sandbox, output: &str) {
    let events = sandbox.events();
    // CWD_CHANGED is written when the directory *changes*, which is why each script below moves
    // into a subdirectory: a shell reporting the directory it was started in has told Verb nothing
    // new, and an event for it would be noise rather than a fact.
    assert!(
        events.contains("CWD_CHANGED"),
        "the shell should report its working directory\nevents:\n{events}\noutput:\n{output}"
    );
    assert!(
        events.contains("COMMAND_STARTED"),
        "the shell should report command starts\nevents:\n{events}\noutput:\n{output}"
    );
    assert!(
        events.contains("COMMAND_FINISHED"),
        "the shell should report command finishes\nevents:\n{events}\noutput:\n{output}"
    );
    assert!(
        events.contains("\"exitCode\":127") || events.contains("\"exitCode\":1"),
        "a failed command's status should be recorded\nevents:\n{events}"
    );
}

/// The rule from docs/VERB_SESSION_SCHEMA.md: the command line is display state and never reaches
/// anything durable.
fn assert_no_command_text(sandbox: &Sandbox) {
    let written = sandbox.everything_written();
    assert!(
        !written.contains(SECRET_COMMAND),
        "command text must never be written to a session record or event log:\n{written}"
    );
    assert!(
        !written.contains("commandText") && !written.contains("commandLine"),
        "the durable schema must not grow a field for command text:\n{written}"
    );
}

#[test]
fn zsh_reports_its_directory_and_command_boundaries() {
    let Some(shell) = shell_path("zsh") else {
        eprintln!("zsh not installed; skipping");
        return;
    };
    let sandbox = Sandbox::new("zsh");
    fs::write(
        sandbox.home().join(".zshrc"),
        format!("export VERB_TEST_MARKER={MARKER}\nPS1='%% '\n"),
    )
    .unwrap();

    let output = run_shell(
        &sandbox,
        &shell,
        &format!("mkdir -p sub && cd sub\necho $VERB_TEST_MARKER\n{SECRET_COMMAND}\nexit\n"),
    );

    assert_reports_structurally(&sandbox, &output);
    assert_no_command_text(&sandbox);
    // The user's own configuration still loads, and loads first.
    assert!(
        output.contains(MARKER),
        "the user's .zshrc should still be sourced\noutput:\n{output}"
    );
}

#[test]
fn bash_reports_its_directory_and_command_boundaries() {
    let Some(shell) = shell_path("bash") else {
        eprintln!("bash not installed; skipping");
        return;
    };
    let sandbox = Sandbox::new("bash");
    fs::write(
        sandbox.home().join(".bashrc"),
        format!("export VERB_TEST_MARKER={MARKER}\nPS1='$ '\n"),
    )
    .unwrap();

    let output = run_shell(
        &sandbox,
        &shell,
        &format!("mkdir -p sub && cd sub\necho $VERB_TEST_MARKER\n{SECRET_COMMAND}\nexit\n"),
    );

    assert_reports_structurally(&sandbox, &output);
    assert_no_command_text(&sandbox);
    assert!(
        output.contains(MARKER),
        "the user's .bashrc should still be sourced\noutput:\n{output}"
    );
}

#[test]
fn an_uninstrumented_shell_reports_nothing_rather_than_something_invented() {
    // `sh` is not a shell Verb knows how to instrument. It must be launched exactly as it would
    // have been, and Verb must record no command boundaries at all -- silence stays unknown.
    let Some(shell) = shell_path("sh") else {
        eprintln!("sh not installed; skipping");
        return;
    };
    let sandbox = Sandbox::new("sh");

    let output = run_shell(&sandbox, &shell, &format!("{SECRET_COMMAND}\nexit\n"));

    let events = sandbox.events();
    assert!(
        events.contains("SESSION_STARTED"),
        "the session itself is still recorded\nevents:\n{events}\noutput:\n{output}"
    );
    for invented in ["COMMAND_STARTED", "COMMAND_FINISHED", "CWD_CHANGED"] {
        assert!(
            !events.contains(invented),
            "{invented} must not appear for a shell that never reported one\nevents:\n{events}"
        );
    }
    assert_no_command_text(&sandbox);
}

#[test]
fn verb_never_touches_the_users_own_shell_configuration() {
    let Some(shell) = shell_path("zsh") else {
        eprintln!("zsh not installed; skipping");
        return;
    };
    let sandbox = Sandbox::new("untouched");
    let zshrc = sandbox.home().join(".zshrc");
    let original = format!("export VERB_TEST_MARKER={MARKER}\n");
    fs::write(&zshrc, &original).unwrap();

    run_shell(&sandbox, &shell, &format!("{SECRET_COMMAND}\nexit\n"));

    assert_eq!(
        fs::read_to_string(&zshrc).unwrap(),
        original,
        "the user's .zshrc must be left exactly as it was"
    );

    // Everything Verb generated lives under its own state directory, and is marked as generated.
    assert!(sandbox.state().join("shell").is_dir());
    for entry in fs::read_dir(sandbox.home()).unwrap().flatten() {
        let contents = fs::read_to_string(entry.path()).unwrap_or_default();
        assert!(
            !contents.contains("Generated by Verb"),
            "Verb must not install anything into the user's home: {:?}",
            entry.file_name()
        );
    }

    // The shell's own artefacts stay where the shell would normally put them. zsh's history follows
    // ZDOTDIR, which Verb repoints, so without care the user's history would be split off into
    // Verb's storage -- taking command text with it, which the contract forbids outright.
    assert!(
        !sandbox.everything_written().contains(SECRET_COMMAND),
        "no command text may end up anywhere under Verb's state directory"
    );
    let history = fs::read_to_string(sandbox.home().join(".zsh_history")).unwrap_or_default();
    assert!(
        history.contains(SECRET_COMMAND),
        "the user's shell history should still be written where the user keeps it"
    );
}
