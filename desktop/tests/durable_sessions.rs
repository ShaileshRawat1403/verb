//! End to end checks for Verb-owned session selection across process restarts.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

struct Sandbox {
    root: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "verb-durable-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("home")).unwrap();
        fs::create_dir_all(root.join("state/sessions")).unwrap();
        fs::create_dir_all(root.join("project")).unwrap();
        Self { root }
    }

    fn project(&self) -> PathBuf {
        self.root.join("project")
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_verb"));
        command
            .current_dir(self.project())
            .env("HOME", self.root.join("home"))
            .env("VERB_STATE_DIR", self.root.join("state"));
        command
    }

    fn record(&self, id: &str, identity: &str, legacy: bool) {
        let project = self.project();
        let path = if legacy {
            let mut hex = project
                .to_string_lossy()
                .as_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            if hex.len() > 200 {
                let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
                for byte in project.to_string_lossy().as_bytes() {
                    hash ^= *byte as u64;
                    hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
                }
                hex = format!("{hash:016x}-{}", &hex[hex.len() - 64..]);
            }
            self.root
                .join("state/sessions")
                .join(format!("{hex}.session"))
        } else {
            let hex = id
                .as_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            self.root
                .join("state/sessions")
                .join(format!("s-{hex}.session"))
        };
        fs::write(
            path,
            format!(
                "schema_version=1\nsession_id={id}\nproject_id={}\nruntime_id=claude\nlast_known_cwd={}\nlast_observed_at=1\ncreated_at=1\nlast_seen_at=1\nstate=recoverable\nagent=claude\nresume_identity={identity}\n",
                project.display(),
                project.display()
            ),
        )
        .unwrap();
    }

    fn record_agent(&self, id: &str, agent: &str) {
        let hex = id
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let project = self.project();
        fs::write(
            self.root
                .join("state/sessions")
                .join(format!("s-{hex}.session")),
            format!(
                "schema_version=1\nsession_id={id}\nproject_id={}\nruntime_id={agent}\nlast_known_cwd={}\nlast_observed_at=1\ncreated_at=1\nlast_seen_at=1\nstate=interrupted\nagent={agent}\nresume_identity=\n",
                project.display(),
                project.display()
            ),
        )
        .unwrap();
    }

    fn transcript(&self, identity: &str) {
        let directory = self
            .project()
            .to_string_lossy()
            .chars()
            .map(|ch| {
                if matches!(ch, '/' | '.' | '_') {
                    '-'
                } else {
                    ch
                }
            })
            .collect::<String>();
        let path = self
            .root
            .join("home/.claude/projects")
            .join(directory)
            .join(format!("{identity}.jsonl"));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "{}\n").unwrap();
    }
}

fn git(project: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(project)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?}: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn old_project_key(path: &std::path::Path) -> String {
    let hex = path
        .to_string_lossy()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if hex.len() <= 200 {
        return hex;
    }
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in path.to_string_lossy().as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}-{}", &hex[hex.len() - 64..])
}

#[test]
fn older_path_keyed_work_is_migrated_without_losing_memory() {
    let sandbox = Sandbox::new("project-migration");
    let project = fs::canonicalize(sandbox.project()).unwrap();
    let legacy = sandbox
        .root
        .join("state/work")
        .join(old_project_key(&project));
    fs::create_dir_all(&legacy).unwrap();
    fs::write(legacy.join("memory.md"), "Keep the original decision.\n").unwrap();
    let old_task_id = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    fs::create_dir_all(legacy.join("tasks")).unwrap();
    fs::write(
        legacy.join("tasks").join(format!("{old_task_id}.json")),
        serde_json::to_vec(&serde_json::json!({"schemaVersion":1,"id":old_task_id,
            "projectId":project,"title":"Keep old task","brief":"Previously recorded",
            "createdAt":1,"events":[]}))
        .unwrap(),
    )
    .unwrap();
    let shown = sandbox.command().args(["memory", "show"]).output().unwrap();
    assert!(
        shown.status.success(),
        "{}",
        String::from_utf8_lossy(&shown.stderr)
    );
    assert!(String::from_utf8_lossy(&shown.stdout).contains("Keep the original decision."));
    let tasks = sandbox
        .command()
        .args(["task", "list", "--json"])
        .output()
        .unwrap();
    assert!(tasks.status.success());
    assert!(String::from_utf8_lossy(&tasks.stdout).contains("Keep old task"));
    assert!(!legacy.exists());
    let project_info = sandbox
        .command()
        .args(["project", "--json"])
        .output()
        .unwrap();
    let project_info: serde_json::Value = serde_json::from_slice(&project_info.stdout).unwrap();
    let id = project_info["projectId"].as_str().unwrap();
    assert_eq!(
        fs::read_to_string(
            sandbox
                .root
                .join("state/work")
                .join(format!("p-{id}/memory.md"))
        )
        .unwrap(),
        "Keep the original decision.\n"
    );
}

#[cfg(unix)]
#[test]
fn isolated_agent_edits_its_own_checkout_and_joins_the_same_project_ledger() {
    use std::os::unix::fs::PermissionsExt;
    let sandbox = Sandbox::new("isolated-agent");
    let project = sandbox.project();
    git(&project, &["init", "-q"]);
    git(
        &project,
        &["config", "user.email", "verb-test@example.invalid"],
    );
    git(&project, &["config", "user.name", "Verb Test"]);
    fs::write(project.join("README.md"), "main checkout\n").unwrap();
    git(&project, &["add", "README.md"]);
    git(&project, &["commit", "-qm", "initial"]);

    let original = sandbox
        .command()
        .args(["project", "--json"])
        .output()
        .unwrap();
    assert!(original.status.success());
    let original: serde_json::Value = serde_json::from_slice(&original.stdout).unwrap();
    let id = original["projectId"].as_str().unwrap();
    let prepared = sandbox
        .command()
        .args(["project", "worktree", "--json"])
        .output()
        .unwrap();
    assert!(prepared.status.success());
    let prepared: serde_json::Value = serde_json::from_slice(&prepared.stdout).unwrap();
    assert_eq!(prepared["projectId"], id);
    assert_eq!(prepared["sourceEditsCopied"], false);
    let prepared_path = prepared["workspace"].as_str().unwrap();
    assert!(std::path::Path::new(prepared_path)
        .join("README.md")
        .exists());
    git(&project, &["worktree", "remove", prepared_path]);
    sandbox.record_agent("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "codex");
    let created = sandbox
        .command()
        .args(["task", "create", "Implement safe workspace"])
        .output()
        .unwrap();
    assert!(created.status.success());
    let created = String::from_utf8(created.stdout).unwrap();
    let task_id = created
        .split_whitespace()
        .nth(2)
        .unwrap()
        .trim_end_matches(':');
    let note = sandbox.root.join("note.txt");
    fs::write(&note, "Both checkouts share this note.\n").unwrap();
    assert!(sandbox
        .command()
        .args(["memory", "append"])
        .arg(&note)
        .output()
        .unwrap()
        .status
        .success());

    let executable = sandbox.root.join("test-agent");
    fs::write(&executable, "#!/bin/sh\nset -eu\nprintf '%s' \"$PWD\" > \"$VERB_TEST_CAPTURE\"\ntest \"$VERB_PROJECT_ROOT\" = \"$PWD\"\n\"$VERB_BIN\" shared read >/dev/null\n\"$VERB_BIN\" task claim \"$VERB_TEST_TASK_ID\"\nprintf 'isolated change\\n' > feature.txt\n").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let capture = sandbox.root.join("agent-cwd.txt");
    let run = sandbox
        .command()
        .arg("isolated")
        .arg("agent")
        .arg(&executable)
        .env("VERB_TEST_CAPTURE", &capture)
        .env("VERB_TEST_TASK_ID", task_id)
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let workspace = PathBuf::from(fs::read_to_string(&capture).unwrap());
    assert_ne!(workspace, project);
    assert_eq!(
        fs::read_to_string(workspace.join("feature.txt")).unwrap(),
        "isolated change\n"
    );
    assert!(!project.join("feature.txt").exists());

    let linked = sandbox
        .command()
        .current_dir(&workspace)
        .args(["project", "--json"])
        .output()
        .unwrap();
    assert!(linked.status.success());
    let linked: serde_json::Value = serde_json::from_slice(&linked.stdout).unwrap();
    assert_eq!(linked["projectId"], id);
    assert!(linked["branch"].as_str().unwrap().starts_with("verb/"));
    assert_eq!(linked["changedFiles"], 1);
    let memory = sandbox
        .command()
        .current_dir(&workspace)
        .args(["memory", "show"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&memory.stdout).contains("Both checkouts share this note."));
    let tasks = sandbox
        .command()
        .args(["task", "list", "--json"])
        .output()
        .unwrap();
    let tasks: serde_json::Value = serde_json::from_slice(&tasks.stdout).unwrap();
    assert_eq!(tasks[0]["status"], "active");
    let shared = sandbox
        .command()
        .args(["shared", "status", "--json"])
        .output()
        .unwrap();
    let shared: serde_json::Value = serde_json::from_slice(&shared.stdout).unwrap();
    assert_eq!(shared["sessions"].as_array().unwrap().len(), 2);
    let migrated_main = session_files(&sandbox)
        .iter()
        .map(|path| fs::read_to_string(path).unwrap())
        .find(|record| record.contains("session_id=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n"))
        .unwrap();
    assert!(migrated_main.contains(&format!("verb_project_id={id}\n")));
    let isolated_row = shared["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["agent"] == "external")
        .unwrap();
    assert_eq!(isolated_row["state"], "new revision available");
    let session_id = isolated_row["sessionId"].as_str().unwrap();
    let records = session_files(&sandbox);
    let record = records
        .iter()
        .map(|path| fs::read_to_string(path).unwrap())
        .find(|record| record.contains(&format!("session_id={session_id}\n")))
        .unwrap();
    assert!(record.contains(&format!("verb_project_id={id}\n")));
    assert!(record.contains(&format!("project_id={}\n", workspace.display())));

    let export = sandbox.root.join("continuity.vcont");
    let exported = sandbox
        .command()
        .args(["continuity", "export"])
        .arg(&export)
        .output()
        .unwrap();
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    let exported = fs::read_to_string(export).unwrap();
    assert!(exported.contains(session_id));
    assert!(exported.contains("PROCESS_STARTED"));

    let context = sandbox
        .command()
        .args(["context", "--json"])
        .output()
        .unwrap();
    assert!(context.status.success());
    let context = String::from_utf8(context.stdout).unwrap();
    assert!(context.contains(session_id));
    assert!(context.contains("PROCESS_STARTED"));

    git(
        &project,
        &["worktree", "remove", "--force", workspace.to_str().unwrap()],
    );
    let after = sandbox
        .command()
        .args(["shared", "status", "--json"])
        .output()
        .unwrap();
    assert!(after.status.success());
    let after: serde_json::Value = serde_json::from_slice(&after.stdout).unwrap();
    assert!(after["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["sessionId"] == session_id));
    let task = sandbox
        .command()
        .args(["task", "show", task_id, "--json"])
        .output()
        .unwrap();
    assert!(task.status.success());
}

#[test]
fn two_agent_sessions_share_a_revision_and_detect_updates_across_processes() {
    let sandbox = Sandbox::new("shared-revision");
    let claude = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let codex = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    sandbox.record_agent(claude, "claude");
    sandbox.record_agent(codex, "codex");

    let first = sandbox
        .command()
        .args(["shared", "read", "--json"])
        .env("VERB_SESSION_ID", claude)
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    let original = first["revision"].as_str().unwrap();

    let note = sandbox.root.join("note.txt");
    fs::write(&note, "Parser ownership: Claude implements; Codex reviews.").unwrap();
    let published = sandbox
        .command()
        .args(["shared", "publish", note.to_str().unwrap()])
        .env("VERB_SESSION_ID", codex)
        .output()
        .unwrap();
    assert!(
        published.status.success(),
        "{}",
        String::from_utf8_lossy(&published.stderr)
    );

    let status = sandbox
        .command()
        .args(["shared", "status", "--json"])
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    let current = status["revision"].as_str().unwrap();
    assert_ne!(original, current);
    let sessions = status["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 2);
    assert_eq!(
        sessions
            .iter()
            .find(|row| row["sessionId"] == claude)
            .unwrap()["state"],
        "new revision available"
    );
    assert_eq!(
        sessions
            .iter()
            .find(|row| row["sessionId"] == codex)
            .unwrap()["state"],
        "never fetched"
    );

    // Each call starts a fresh Verb process, exercising persistence rather than process memory.
    for id in [claude, codex] {
        let fetched = sandbox
            .command()
            .args(["shared", "read", "--json"])
            .env("VERB_SESSION_ID", id)
            .output()
            .unwrap();
        assert!(
            fetched.status.success(),
            "{}",
            String::from_utf8_lossy(&fetched.stderr)
        );
        let fetched: serde_json::Value = serde_json::from_slice(&fetched.stdout).unwrap();
        assert_eq!(fetched["revision"], current);
        assert!(fetched["memory"]
            .as_str()
            .unwrap()
            .contains("Parser ownership"));
        assert!(fetched["memory"]
            .as_str()
            .unwrap()
            .contains(&format!("agent session {codex}")));
    }
    let status = sandbox
        .command()
        .args(["shared", "status", "--json"])
        .output()
        .unwrap();
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert!(status["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["state"] == "fetched current revision"));

    let created = sandbox
        .command()
        .args(["task", "create", "Review parser"])
        .output()
        .unwrap();
    assert!(created.status.success());
    let after_task = sandbox
        .command()
        .args(["shared", "status", "--json"])
        .output()
        .unwrap();
    let after_task: serde_json::Value = serde_json::from_slice(&after_task.stdout).unwrap();
    assert_ne!(after_task["revision"], current);
    assert!(after_task["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["state"] == "new revision available"));
}

#[cfg(unix)]
#[test]
fn hosted_claude_and_codex_can_fetch_through_their_inherited_bridge() {
    use std::os::unix::fs::PermissionsExt;

    let sandbox = Sandbox::new("hosted-bridge");
    let bin = sandbox.root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    for agent in ["claude", "codex"] {
        let script = bin.join(agent);
        fs::write(
            &script,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$VERB_TEST_DIR/$VERB_SESSION_ID.args\"\n\"$VERB_BIN\" shared read --json > \"$VERB_TEST_DIR/$VERB_SESSION_ID.json\"\n",
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    for agent in ["claude", "codex"] {
        let output = sandbox
            .command()
            .arg(agent)
            .env("PATH", &path)
            .env("VERB_TEST_DIR", &sandbox.root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{agent}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let mut prompted = 0;
    for entry in fs::read_dir(&sandbox.root).unwrap().flatten() {
        if entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "args")
        {
            let args = fs::read_to_string(entry.path()).unwrap();
            assert!(args.contains("shared read"));
            assert!(args.contains("wait for the user's task"));
            prompted += 1;
        }
    }
    assert_eq!(prompted, 2);
    let status = sandbox
        .command()
        .args(["shared", "status", "--json"])
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    let rows = status["sessions"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows
        .iter()
        .all(|row| row["state"] == "fetched current revision"));
    for row in rows {
        let id = row["sessionId"].as_str().unwrap();
        let fetched: serde_json::Value =
            serde_json::from_slice(&fs::read(sandbox.root.join(format!("{id}.json"))).unwrap())
                .unwrap();
        assert_eq!(fetched["revision"], status["revision"]);
    }
}

#[test]
fn concurrent_shared_publications_keep_both_notes_and_reject_foreign_sessions() {
    let sandbox = Sandbox::new("shared-concurrent");
    let first = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let second = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    sandbox.record_agent(first, "claude");
    sandbox.record_agent(second, "codex");
    let first_note = sandbox.root.join("first.txt");
    let second_note = sandbox.root.join("second.txt");
    fs::write(&first_note, "Claude found the parser boundary.").unwrap();
    fs::write(&second_note, "Codex found the retry boundary.").unwrap();
    let mut one = sandbox
        .command()
        .args(["shared", "publish", first_note.to_str().unwrap()])
        .env("VERB_SESSION_ID", first)
        .spawn()
        .unwrap();
    let mut two = sandbox
        .command()
        .args(["shared", "publish", second_note.to_str().unwrap()])
        .env("VERB_SESSION_ID", second)
        .spawn()
        .unwrap();
    assert!(one.wait().unwrap().success());
    assert!(two.wait().unwrap().success());
    let read = sandbox
        .command()
        .args(["shared", "read", "--json"])
        .output()
        .unwrap();
    assert!(read.status.success());
    let read: serde_json::Value = serde_json::from_slice(&read.stdout).unwrap();
    let memory = read["memory"].as_str().unwrap();
    assert!(memory.contains("Claude found the parser boundary."));
    assert!(memory.contains("Codex found the retry boundary."));

    let foreign = sandbox.root.join("foreign");
    fs::create_dir_all(&foreign).unwrap();
    let failed = sandbox
        .command()
        .current_dir(foreign)
        .args(["shared", "read"])
        .env("VERB_SESSION_ID", first)
        .output()
        .unwrap();
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("another project"));
}

fn session_files(sandbox: &Sandbox) -> Vec<PathBuf> {
    fs::read_dir(sandbox.root.join("state/sessions"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "session"))
        .collect()
}

#[test]
fn two_sessions_in_one_project_survive_restart_and_export_together() {
    let sandbox = Sandbox::new("two-sessions");
    for _ in 0..2 {
        let output = sandbox
            .command()
            .args(["run", "/bin/true"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(session_files(&sandbox).len(), 2);

    let output = sandbox
        .command()
        .args(["sessions", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let sessions: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(sessions.as_array().unwrap().len(), 2);

    let export = sandbox.root.join("sessions.vcont");
    let output = sandbox
        .command()
        .args(["continuity", "export"])
        .arg(&export)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let contents = fs::read_to_string(&export).unwrap();
    assert_eq!(
        contents
            .lines()
            .filter(|line| line.contains("\"recordType\":\"session\""))
            .count(),
        2
    );
    let preview = sandbox
        .command()
        .args(["continuity", "import"])
        .arg(&export)
        .output()
        .unwrap();
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
}

#[cfg(unix)]
#[test]
fn explicit_resume_uses_only_the_chosen_conversation_and_migrates_legacy_record() {
    use std::os::unix::fs::PermissionsExt;

    let sandbox = Sandbox::new("exact-resume");
    sandbox.record("first", "conversation-one", true);
    sandbox.record("second", "conversation-two", false);
    sandbox.transcript("conversation-one");
    sandbox.transcript("conversation-two");

    let bin = sandbox.root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let script = bin.join("claude");
    fs::write(
        &script,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$VERB_TEST_ARGS\"\n",
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let args_path = sandbox.root.join("args.txt");
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let output = sandbox
        .command()
        .args(["resume", "first"])
        .env("PATH", path)
        .env("VERB_TEST_ARGS", &args_path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let args = fs::read_to_string(args_path).unwrap();
    assert!(args.starts_with("--resume\nconversation-one\n"));
    assert!(args.contains("shared read"));
    assert!(!args.contains("conversation-two"));
    assert_eq!(session_files(&sandbox).len(), 2);
    assert!(session_files(&sandbox).iter().all(|path| path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("s-")));

    // Another transcript in this project cannot stand in for this session's missing identity.
    sandbox.record("third", "conversation-missing", false);
    let output = sandbox
        .command()
        .args(["resume", "third"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn shared_memory_and_handoff_history_survive_separate_agent_processes() {
    let sandbox = Sandbox::new("workbench");
    let first = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let second = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    sandbox.record(first, "conversation-a", false);
    sandbox.record(second, "conversation-b", false);

    let memory = sandbox.root.join("memory.txt");
    fs::write(&memory, "Use Rust for the desktop host.\n").unwrap();
    let output = sandbox
        .command()
        .args(["memory", "set"])
        .arg(&memory)
        .output()
        .unwrap();
    assert!(output.status.success());

    let memory_update = sandbox.root.join("memory-update.txt");
    fs::write(&memory_update, "Run desktop tests before handoff.\n").unwrap();
    let output = sandbox
        .command()
        .args(["memory", "append"])
        .arg(&memory_update)
        .output()
        .unwrap();
    assert!(output.status.success());
    #[cfg(unix)]
    {
        let output = sandbox
            .command()
            .args([
                "run",
                "/bin/sh",
                "-c",
                "test -f \"$VERB_PROJECT_MEMORY_PATH\" && test -n \"$VERB_SESSION_ID\"",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let brief = sandbox.root.join("brief.txt");
    fs::write(&brief, "Add a durable handoff ledger.\n").unwrap();
    let output = sandbox
        .command()
        .args(["task", "create", "Build handoffs"])
        .arg(&brief)
        .output()
        .unwrap();
    assert!(output.status.success());
    let created = String::from_utf8(output.stdout).unwrap();
    let id = created
        .split_whitespace()
        .nth(2)
        .unwrap()
        .trim_end_matches(':');
    assert_eq!(id.len(), 32);

    let other = sandbox.root.join("other-project");
    fs::create_dir_all(&other).unwrap();
    let rejected = sandbox
        .command()
        .current_dir(&other)
        .args(["task", "claim", id, first])
        .output()
        .unwrap();
    assert!(!rejected.status.success());

    let output = sandbox
        .command()
        .args(["task", "claim", id])
        .env("VERB_SESSION_ID", first)
        .output()
        .unwrap();
    assert!(output.status.success());
    let conflict = sandbox
        .command()
        .args(["task", "claim", id, second])
        .output()
        .unwrap();
    assert!(!conflict.status.success());

    let help = sandbox.root.join("help.txt");
    fs::write(&help, "Please review the storage boundary.\n").unwrap();
    let output = sandbox
        .command()
        .args(["task", "request-help", id])
        .env("VERB_SESSION_ID", first)
        .arg(&help)
        .output()
        .unwrap();
    assert!(output.status.success());
    let duplicate = sandbox
        .command()
        .args(["task", "request-help", id, first])
        .arg(&help)
        .output()
        .unwrap();
    assert!(!duplicate.status.success());
    let answer = sandbox.root.join("answer.txt");
    fs::write(&answer, "Keep records per project.\n").unwrap();
    let output = sandbox
        .command()
        .args(["task", "reply", id, second])
        .arg(&answer)
        .output()
        .unwrap();
    assert!(output.status.success());

    let fetched = sandbox
        .command()
        .args(["shared", "read"])
        .env("VERB_SESSION_ID", first)
        .output()
        .unwrap();
    assert!(fetched.status.success());

    let handoff = sandbox.root.join("handoff.txt");
    fs::write(
        &handoff,
        "Implementation done; please verify concurrency.\n",
    )
    .unwrap();
    let output = sandbox
        .command()
        .args(["task", "handoff", id, first])
        .arg(&handoff)
        .output()
        .unwrap();
    assert!(output.status.success());
    let output = sandbox
        .command()
        .args(["task", "claim", id, second])
        .output()
        .unwrap();
    assert!(output.status.success());
    let output = sandbox
        .command()
        .args(["task", "done", id, second])
        .arg(&answer)
        .output()
        .unwrap();
    assert!(output.status.success());

    let output = sandbox
        .command()
        .args(["task", "context", id])
        .output()
        .unwrap();
    assert!(output.status.success());
    let context = String::from_utf8(output.stdout).unwrap();
    assert!(context.contains("Use Rust for the desktop host."));
    assert!(context.contains("Run desktop tests before handoff."));
    assert!(context.contains("Add a durable handoff ledger."));
    assert!(context.contains("Implementation done; please verify concurrency."));
    assert!(context.contains("done"));

    let output = sandbox
        .command()
        .args(["task", "list", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let list: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(list[0]["status"], "done");
    let output = sandbox
        .command()
        .args(["task", "show", id, "--json"])
        .output()
        .unwrap();
    let detail: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(detail["task"]["events"].as_array().unwrap().len(), 6);
}

#[test]
fn session_inbox_tracks_pending_work_and_delivered_events_across_processes() {
    let sandbox = Sandbox::new("session-inbox");
    let owner = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let helper = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    sandbox.record_agent(owner, "claude");
    sandbox.record_agent(helper, "codex");
    let note = sandbox.root.join("note.txt");
    fs::write(&note, "Please inspect the boundary.\n").unwrap();

    let created = sandbox
        .command()
        .args(["task", "create", "Review inbox"])
        .arg(&note)
        .output()
        .unwrap();
    assert!(created.status.success());
    let created = String::from_utf8(created.stdout).unwrap();
    let id = created
        .split_whitespace()
        .nth(2)
        .unwrap()
        .trim_end_matches(':');
    assert!(sandbox
        .command()
        .args(["task", "claim", id, owner])
        .output()
        .unwrap()
        .status
        .success());
    assert!(sandbox
        .command()
        .args(["shared", "read"])
        .env("VERB_SESSION_ID", owner)
        .output()
        .unwrap()
        .status
        .success());
    assert!(sandbox
        .command()
        .args(["task", "request-help", id, owner])
        .arg(&note)
        .output()
        .unwrap()
        .status
        .success());

    let owner_inbox = sandbox
        .command()
        .args(["inbox", owner, "--json"])
        .output()
        .unwrap();
    assert!(owner_inbox.status.success());
    let owner_inbox: serde_json::Value = serde_json::from_slice(&owner_inbox.stdout).unwrap();
    assert_eq!(owner_inbox["contextState"], "new revision available");
    let helper_inbox = sandbox
        .command()
        .args(["inbox", "--json"])
        .env("VERB_SESSION_ID", helper)
        .output()
        .unwrap();
    assert!(helper_inbox.status.success());
    let helper_inbox: serde_json::Value = serde_json::from_slice(&helper_inbox.stdout).unwrap();
    assert_eq!(helper_inbox["items"][0]["kind"], "help_available");
    assert_eq!(helper_inbox["items"][0]["newSinceFetch"], true);
    assert_eq!(helper_inbox["contextState"], "never fetched");

    assert!(sandbox
        .command()
        .args(["task", "reply", id, helper])
        .arg(&note)
        .output()
        .unwrap()
        .status
        .success());
    let replied = sandbox
        .command()
        .args(["inbox", owner, "--json"])
        .output()
        .unwrap();
    let replied: serde_json::Value = serde_json::from_slice(&replied.stdout).unwrap();
    assert_eq!(replied["items"][0]["kind"], "help_reply");
    assert_eq!(replied["items"][0]["newSinceFetch"], true);
    assert!(sandbox
        .command()
        .args(["shared", "read"])
        .env("VERB_SESSION_ID", owner)
        .output()
        .unwrap()
        .status
        .success());
    let fetched = sandbox
        .command()
        .args(["inbox", owner, "--json"])
        .output()
        .unwrap();
    let fetched: serde_json::Value = serde_json::from_slice(&fetched.stdout).unwrap();
    assert!(fetched["items"].as_array().unwrap().is_empty());

    assert!(sandbox
        .command()
        .args(["task", "handoff", id, owner])
        .arg(&note)
        .output()
        .unwrap()
        .status
        .success());
    let review = sandbox
        .command()
        .args(["inbox", helper, "--json"])
        .output()
        .unwrap();
    let review: serde_json::Value = serde_json::from_slice(&review.stdout).unwrap();
    assert_eq!(review["items"][0]["kind"], "review_available");
    assert_eq!(review["items"][0]["newSinceFetch"], true);
    assert!(sandbox
        .command()
        .args(["shared", "read"])
        .env("VERB_SESSION_ID", helper)
        .output()
        .unwrap()
        .status
        .success());
    let review = sandbox
        .command()
        .args(["inbox", helper, "--json"])
        .output()
        .unwrap();
    let review: serde_json::Value = serde_json::from_slice(&review.stdout).unwrap();
    assert_eq!(review["items"][0]["kind"], "review_available");
    assert_eq!(review["items"][0]["newSinceFetch"], false);
    assert!(sandbox
        .command()
        .args(["task", "claim", id, helper])
        .output()
        .unwrap()
        .status
        .success());
    let claimed = sandbox
        .command()
        .args(["inbox", helper, "--json"])
        .output()
        .unwrap();
    let claimed: serde_json::Value = serde_json::from_slice(&claimed.stdout).unwrap();
    assert_eq!(claimed["items"][0]["kind"], "assigned_to_you");
    assert!(sandbox
        .command()
        .args(["shared", "read"])
        .env("VERB_SESSION_ID", helper)
        .output()
        .unwrap()
        .status
        .success());
    let done = sandbox
        .command()
        .args(["inbox", helper, "--json"])
        .output()
        .unwrap();
    let done: serde_json::Value = serde_json::from_slice(&done.stdout).unwrap();
    assert!(done["items"].as_array().unwrap().is_empty());

    let other = sandbox.root.join("other-project");
    fs::create_dir_all(&other).unwrap();
    assert!(!sandbox
        .command()
        .current_dir(other)
        .args(["inbox", owner])
        .output()
        .unwrap()
        .status
        .success());
}

#[test]
fn handoff_requires_the_owners_current_shared_revision() {
    let sandbox = Sandbox::new("stale-handoff");
    let owner = "cccccccccccccccccccccccccccccccc";
    sandbox.record_agent(owner, "claude");
    let created = sandbox
        .command()
        .args(["task", "create", "Review the parser"])
        .output()
        .unwrap();
    assert!(created.status.success());
    let created = String::from_utf8(created.stdout).unwrap();
    let id = created
        .split_whitespace()
        .nth(2)
        .unwrap()
        .trim_end_matches(':');
    let claim = sandbox
        .command()
        .args(["task", "claim", id, owner])
        .output()
        .unwrap();
    assert!(claim.status.success());
    let note = sandbox.root.join("handoff.txt");
    fs::write(&note, "Ready for review.\n").unwrap();

    let no_fetch = sandbox
        .command()
        .args(["task", "handoff", id, owner])
        .arg(&note)
        .output()
        .unwrap();
    assert!(!no_fetch.status.success());
    assert!(String::from_utf8_lossy(&no_fetch.stderr).contains("shared read"));

    let fetched = sandbox
        .command()
        .args(["shared", "read"])
        .env("VERB_SESSION_ID", owner)
        .output()
        .unwrap();
    assert!(fetched.status.success());
    let new_memory = sandbox.root.join("new-memory.txt");
    fs::write(&new_memory, "Review must include error cases.\n").unwrap();
    let published = sandbox
        .command()
        .args(["memory", "append"])
        .arg(&new_memory)
        .output()
        .unwrap();
    assert!(published.status.success());

    let stale = sandbox
        .command()
        .args(["task", "handoff", id, owner])
        .arg(&note)
        .output()
        .unwrap();
    assert!(!stale.status.success());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("project memory changed"));
    let shown = sandbox
        .command()
        .args(["task", "show", id, "--json"])
        .output()
        .unwrap();
    let shown: serde_json::Value = serde_json::from_slice(&shown.stdout).unwrap();
    assert_eq!(shown["status"], "active");

    let refreshed = sandbox
        .command()
        .args(["shared", "read"])
        .env("VERB_SESSION_ID", owner)
        .output()
        .unwrap();
    assert!(refreshed.status.success());
    let help = sandbox.root.join("help.txt");
    fs::write(&help, "Please check the parser edge cases.\n").unwrap();
    let task_changed = sandbox
        .command()
        .args(["task", "request-help", id, owner])
        .arg(&help)
        .output()
        .unwrap();
    assert!(task_changed.status.success());
    let stale_task = sandbox
        .command()
        .args(["task", "handoff", id, owner])
        .arg(&note)
        .output()
        .unwrap();
    assert!(!stale_task.status.success());
    assert!(String::from_utf8_lossy(&stale_task.stderr).contains("task or project memory changed"));
    let refreshed = sandbox
        .command()
        .args(["shared", "read"])
        .env("VERB_SESSION_ID", owner)
        .output()
        .unwrap();
    assert!(refreshed.status.success());
    let unrelated = sandbox
        .command()
        .args(["task", "create", "Unrelated documentation"])
        .output()
        .unwrap();
    assert!(unrelated.status.success());
    // A global snapshot revision changed, but this owner's memory and task did not.
    let accepted = sandbox
        .command()
        .args(["task", "handoff", id, owner])
        .arg(&note)
        .output()
        .unwrap();
    assert!(
        accepted.status.success(),
        "{}",
        String::from_utf8_lossy(&accepted.stderr)
    );
}

#[cfg(unix)]
#[test]
fn arbitrary_cli_agent_can_fetch_claim_and_handoff_without_a_native_resume_contract() {
    use std::os::unix::fs::PermissionsExt;

    let sandbox = Sandbox::new("external-agent");
    let created = sandbox
        .command()
        .args(["task", "create", "Inspect logs"])
        .output()
        .unwrap();
    assert!(created.status.success());
    let created = String::from_utf8(created.stdout).unwrap();
    let id = created
        .split_whitespace()
        .nth(2)
        .unwrap()
        .trim_end_matches(':');
    let note = sandbox.root.join("handoff.txt");
    fs::write(&note, "Logs inspected; review the findings.\n").unwrap();
    let executable = sandbox.root.join("my-agent");
    fs::write(
        &executable,
        "#!/bin/sh\ntest \"$1\" = --example || exit 9\ntest \"$2\" = --json || exit 14\n\"$VERB_BIN\" shared read > /dev/null || exit 10\n\"$VERB_BIN\" task claim \"$VERB_TEST_TASK_ID\" || exit 11\n\"$VERB_BIN\" shared read > /dev/null || exit 12\n\"$VERB_BIN\" task handoff \"$VERB_TEST_TASK_ID\" \"$VERB_TEST_NOTE\" || exit 13\n",
    )
    .unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();

    let hosted = sandbox
        .command()
        .arg("agent")
        .arg(&executable)
        .arg("--example")
        .arg("--json")
        .env("VERB_TEST_TASK_ID", id)
        .env("VERB_TEST_NOTE", &note)
        .output()
        .unwrap();
    assert!(
        hosted.status.success(),
        "{}",
        String::from_utf8_lossy(&hosted.stderr)
    );
    let shown = sandbox
        .command()
        .args(["task", "show", id, "--json"])
        .output()
        .unwrap();
    let shown: serde_json::Value = serde_json::from_slice(&shown.stdout).unwrap();
    assert_eq!(shown["status"], "needs review");

    let sessions = session_files(&sandbox);
    assert_eq!(sessions.len(), 1);
    let record = fs::read_to_string(&sessions[0]).unwrap();
    assert!(record.contains("agent=external\n"));
    assert!(record.contains("runtime_id=my-agent\n"));
    assert!(record.contains("state=ended\n"));
    assert!(!record.contains(executable.to_str().unwrap()));
    let session_id = record
        .lines()
        .find_map(|line| line.strip_prefix("session_id="))
        .unwrap();
    let resume = sandbox
        .command()
        .args(["resume", session_id])
        .output()
        .unwrap();
    assert!(!resume.status.success());
    assert!(String::from_utf8_lossy(&resume.stderr).contains("recovery is not confirmed"));
}

#[test]
fn replacing_shared_memory_requires_the_version_that_was_read() {
    let sandbox = Sandbox::new("memory-cas");
    let first = sandbox.root.join("first.txt");
    let second = sandbox.root.join("second.txt");
    fs::write(&first, "First decision.\n").unwrap();
    fs::write(&second, "Revised decision.\n").unwrap();

    let initial = sandbox
        .command()
        .args(["memory", "set"])
        .arg(&first)
        .output()
        .unwrap();
    assert!(initial.status.success());
    let hash = sandbox.command().args(["memory", "hash"]).output().unwrap();
    assert!(hash.status.success());
    let hash = String::from_utf8(hash.stdout).unwrap();
    let hash = hash.trim();
    assert!(hash.starts_with("sha256:"));

    let no_base = sandbox
        .command()
        .args(["memory", "set"])
        .arg(&second)
        .output()
        .unwrap();
    assert!(!no_base.status.success());
    let replaced = sandbox
        .command()
        .args(["memory", "set"])
        .arg(&second)
        .args(["--base-hash", hash])
        .output()
        .unwrap();
    assert!(replaced.status.success());
    let stale = sandbox
        .command()
        .args(["memory", "set"])
        .arg(&first)
        .args(["--base-hash", hash])
        .output()
        .unwrap();
    assert!(!stale.status.success());
    let shown = sandbox.command().args(["memory", "show"]).output().unwrap();
    assert_eq!(
        String::from_utf8(shown.stdout).unwrap(),
        "Revised decision.\n"
    );
}

#[test]
fn concurrent_claims_have_one_winner() {
    let sandbox = Sandbox::new("claim-race");
    let first = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let second = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    sandbox.record(first, "conversation-a", false);
    sandbox.record(second, "conversation-b", false);
    let created = sandbox
        .command()
        .args(["task", "create", "Race test"])
        .output()
        .unwrap();
    let text = String::from_utf8(created.stdout).unwrap();
    let id = text
        .split_whitespace()
        .nth(2)
        .unwrap()
        .trim_end_matches(':')
        .to_owned();

    let mut a = sandbox
        .command()
        .args(["task", "claim", &id, first])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut b = sandbox
        .command()
        .args(["task", "claim", &id, second])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let results = [a.wait().unwrap().success(), b.wait().unwrap().success()];
    assert_eq!(results.iter().filter(|success| **success).count(), 1);

    let output = sandbox
        .command()
        .args(["task", "show", &id, "--json"])
        .output()
        .unwrap();
    let detail: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(detail["task"]["events"].as_array().unwrap().len(), 1);
}

#[test]
fn an_unavailable_owner_can_be_reassigned_without_losing_history() {
    let sandbox = Sandbox::new("reassign-after-loss");
    let first = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let second = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    sandbox.record(first, "conversation-a", false);
    sandbox.record(second, "conversation-b", false);
    let created = sandbox
        .command()
        .args(["task", "create", "Finish parser"])
        .output()
        .unwrap();
    assert!(created.status.success());
    let id = String::from_utf8(created.stdout)
        .unwrap()
        .split_whitespace()
        .nth(2)
        .unwrap()
        .trim_end_matches(':')
        .to_owned();
    assert!(sandbox
        .command()
        .args(["task", "claim", &id, first])
        .output()
        .unwrap()
        .status
        .success());

    let owner_record = session_files(&sandbox)
        .into_iter()
        .find(|path| {
            fs::read_to_string(path)
                .unwrap()
                .contains(&format!("session_id={first}"))
        })
        .unwrap();
    let record = fs::read_to_string(&owner_record).unwrap();
    fs::write(
        owner_record,
        record.replace("state=recoverable", "state=ended"),
    )
    .unwrap();
    let reason = sandbox.root.join("reason.txt");
    fs::write(&reason, "Original agent session is no longer available.").unwrap();
    let moved = sandbox
        .command()
        .args(["task", "reassign", &id, second])
        .arg(&reason)
        .output()
        .unwrap();
    assert!(
        moved.status.success(),
        "{}",
        String::from_utf8_lossy(&moved.stderr)
    );
    let detail = sandbox
        .command()
        .args(["task", "show", &id, "--json"])
        .output()
        .unwrap();
    let detail: serde_json::Value = serde_json::from_slice(&detail.stdout).unwrap();
    assert_eq!(detail["ownerSessionId"], second);
    let events = detail["task"]["events"].as_array().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["kind"], "claimed");
    assert_eq!(events[1]["kind"], "reassigned");
    assert_eq!(
        events[1]["note"],
        "Original agent session is no longer available."
    );
}

#[cfg(unix)]
#[test]
fn a_live_session_cannot_be_resumed_by_a_second_verb_process() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;

    let sandbox = Sandbox::new("session-lock");
    sandbox.record("first", "conversation-one", false);
    sandbox.transcript("conversation-one");
    let bin = sandbox.root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let script = bin.join("claude");
    fs::write(&script, "#!/bin/sh\nif test -e \"$VERB_TEST_READY\"; then exit 0; fi\ntouch \"$VERB_TEST_READY\"\nwhile ! test -e \"$VERB_TEST_RELEASE\"; do sleep 0.05; done\n").unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let ready = sandbox.root.join("ready");
    let release = sandbox.root.join("release");
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut first = sandbox
        .command()
        .args(["resume", "first"])
        .env("PATH", &path)
        .env("VERB_TEST_READY", &ready)
        .env("VERB_TEST_RELEASE", &release)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    for _ in 0..300 {
        if ready.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if !ready.exists() {
        let _ = first.kill();
        let _ = first.wait();
        panic!("the first agent never became ready");
    }

    let listing = sandbox
        .command()
        .args(["sessions", "--json"])
        .output()
        .unwrap();
    assert!(listing.status.success());
    let sessions: serde_json::Value = serde_json::from_slice(&listing.stdout).unwrap();
    assert_eq!(sessions[0]["state"], "LIVE");

    let second = sandbox
        .command()
        .args(["resume", "first"])
        .env("PATH", &path)
        .env("VERB_TEST_READY", &ready)
        .env("VERB_TEST_RELEASE", &release)
        .output()
        .unwrap();
    assert_eq!(second.status.code(), Some(3));
    assert!(!release.exists());

    fs::write(&release, "").unwrap();
    assert!(first.wait().unwrap().success());
    let listing = sandbox
        .command()
        .args(["sessions", "--json"])
        .output()
        .unwrap();
    let sessions: serde_json::Value = serde_json::from_slice(&listing.stdout).unwrap();
    assert_eq!(sessions[0]["state"], "RECOVERABLE");
}

#[cfg(unix)]
#[test]
fn an_agent_keeps_the_session_lock_when_the_verb_host_crashes() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::Duration;

    let sandbox = Sandbox::new("host-crash-lock");
    sandbox.record("first", "conversation-one", false);
    sandbox.transcript("conversation-one");
    let bin = sandbox.root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    let script = bin.join("claude");
    fs::write(
        &script,
        "#!/bin/sh\ntrap '' HUP\ntouch \"$VERB_TEST_READY\"\nwhile ! test -e \"$VERB_TEST_RELEASE\"; do sleep 0.05; done\ntouch \"$VERB_TEST_EXITED\"\n",
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let ready = sandbox.root.join("ready");
    let release = sandbox.root.join("release");
    let exited = sandbox.root.join("exited");
    struct ReleaseOnDrop(PathBuf);
    impl Drop for ReleaseOnDrop {
        fn drop(&mut self) {
            let _ = fs::write(&self.0, "");
        }
    }
    let _cleanup = ReleaseOnDrop(release.clone());
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut host = sandbox
        .command()
        .args(["resume", "first"])
        .env("PATH", &path)
        .env("VERB_TEST_READY", &ready)
        .env("VERB_TEST_RELEASE", &release)
        .env("VERB_TEST_EXITED", &exited)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    for _ in 0..300 {
        if ready.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if !ready.exists() {
        let _ = host.kill();
        let _ = host.wait();
        panic!("the agent never became ready");
    }
    host.kill().unwrap();
    host.wait().unwrap();

    let listing = sandbox
        .command()
        .args(["sessions", "--json"])
        .output()
        .unwrap();
    assert!(listing.status.success());
    let sessions: serde_json::Value = serde_json::from_slice(&listing.stdout).unwrap();
    assert_eq!(sessions[0]["state"], "LIVE");
    let duplicate = sandbox
        .command()
        .args(["resume", "first"])
        .env("PATH", &path)
        .output()
        .unwrap();
    assert_eq!(duplicate.status.code(), Some(3));

    fs::write(&release, "").unwrap();
    for _ in 0..1000 {
        let listing = sandbox
            .command()
            .args(["sessions", "--json"])
            .output()
            .unwrap();
        let sessions: serde_json::Value = serde_json::from_slice(&listing.stdout).unwrap();
        if sessions[0]["state"] == "RECOVERABLE" {
            assert!(
                exited.exists(),
                "the agent must finish before the lock clears"
            );
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!(
        "session lock stayed held after release (agent exited: {})",
        exited.exists()
    );
}

#[test]
fn a_corrupt_session_record_is_reported_instead_of_silently_disappearing() {
    let sandbox = Sandbox::new("corrupt-session");
    sandbox.record("valid", "conversation-one", false);
    let broken = sandbox.root.join("state/sessions/broken.session");
    fs::write(&broken, "not a session record\n").unwrap();

    let listing = sandbox
        .command()
        .args(["sessions", "--json"])
        .output()
        .unwrap();
    assert!(!listing.status.success());
    assert!(String::from_utf8_lossy(&listing.stderr).contains("invalid session record"));

    fs::remove_file(broken).unwrap();
    let listing = sandbox
        .command()
        .args(["sessions", "--json"])
        .output()
        .unwrap();
    assert!(listing.status.success());
    let records: serde_json::Value = serde_json::from_slice(&listing.stdout).unwrap();
    assert_eq!(records.as_array().unwrap().len(), 1);
}

#[cfg(unix)]
#[test]
fn continuity_import_refuses_a_device_instead_of_reading_it_forever() {
    let sandbox = Sandbox::new("import-device");
    let mut child = sandbox
        .command()
        .args(["continuity", "import", "/dev/zero"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let started = std::time::Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(!status.success());
            break;
        }
        if started.elapsed() > std::time::Duration::from_secs(5) {
            let _ = child.kill();
            panic!("continuity import kept reading /dev/zero");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[cfg(unix)]
#[test]
fn event_logs_are_readable_by_their_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let sandbox = Sandbox::new("event-log-mode");
    let output = sandbox
        .command()
        .args(["run", "true"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let events = sandbox.root.join("state/events");
    assert_eq!(
        fs::metadata(&events).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let mut logs = 0;
    for project in fs::read_dir(&events).unwrap().flatten() {
        assert_eq!(
            fs::metadata(project.path()).unwrap().permissions().mode() & 0o777,
            0o700
        );
        for log in fs::read_dir(project.path()).unwrap().flatten() {
            assert_eq!(
                fs::metadata(log.path()).unwrap().permissions().mode() & 0o777,
                0o600,
                "{}",
                log.path().display()
            );
            logs += 1;
        }
    }
    assert!(logs > 0);
}
