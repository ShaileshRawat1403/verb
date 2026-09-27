//! Agent-specific runtime truth for the desktop host.
//!
//! Everything here answers two questions per agent, and nothing else: *is there a conversation
//! worth recovering for this project*, and *what is its stable identity*. Session lifecycle -- the
//! `LIVE / INTERRUPTED / RECOVERABLE / ENDED` machine -- lives in `main.rs` and is shared by every
//! agent, exactly as `AgentSessionCoordinator` is on Android. There is one state machine per host,
//! never one per agent.
//!
//! The layouts below were read off real installed builds (Claude Code, codex-cli 0.149.0,
//! OpenCode), not assumed. Where a host gives no readable answer, these return
//! [`ResumeVerdict::Unknown`] -- never `No`, which would claim an impossibility Verb has not
//! established.
//!
//! No function here reads a conversation into Verb. Transcripts are scanned for a marker; the
//! content is never returned, stored, or logged.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::json::{json_string, json_strings};
use crate::ResumeVerdict;

/// A conversation an agent could resume by its exact identity.
struct Conversation {
    id: String,
}

// ---------------------------------------------------------------------------
// Claude Code
// ---------------------------------------------------------------------------

/// Claude records a project's transcripts under a normalized project-path directory.
/// and (on builds that have one) session metadata under `~/.claude/sessions/*.json`. The metadata
/// filename is a PID and is never used as identity -- the `sessionId` inside it is.
/// The directory name written by the installed Claude Code build.
///
/// This is shared by recovery and live observation because a mismatch is not cosmetic: it turns a
/// real recoverable conversation into `UNKNOWN`. Slash, dot and underscore normalization are all
/// covered by local real-session evidence; other characters are preserved until evidence says
/// Claude treats them differently.
pub(crate) fn claude_project_dir(project: &Path) -> String {
    project
        .to_string_lossy()
        .chars()
        .map(|character| match character {
            '/' | '.' | '_' => '-',
            other => other,
        })
        .collect()
}

pub fn claude_verdict_for(project: &Path, home: &Path, id: &str) -> ResumeVerdict {
    let transcript_dir = home
        .join(".claude")
        .join("projects")
        .join(claude_project_dir(project));
    if has_extension(&transcript_dir.join(format!("{id}.jsonl")), "jsonl") {
        return ResumeVerdict::Yes;
    }
    exact_verdict(claude_conversations(project, home), id)
}

/// `None` when this Claude build keeps no readable session metadata store.
fn claude_conversations(project: &Path, home: &Path) -> Option<Vec<Conversation>> {
    let entries = fs::read_dir(home.join(".claude").join("sessions")).ok()?;
    let mut conversations = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !has_extension(&path, "json") {
            continue;
        }
        let Ok(metadata) = fs::read_to_string(&path) else {
            continue;
        };
        let Some(cwd) = json_string(&metadata, "cwd") else {
            continue;
        };
        if !same_directory(&cwd, project) {
            continue;
        }
        let Some(id) = json_string(&metadata, "sessionId") else {
            continue;
        };
        conversations.push(Conversation { id });
    }
    Some(conversations)
}

// ---------------------------------------------------------------------------
// Codex
// ---------------------------------------------------------------------------

/// Codex records each conversation as a rollout file,
/// `~/.codex/sessions/<yyyy>/<mm>/<dd>/rollout-<timestamp>-<id>.jsonl`, whose first line carries
/// the conversation `id` and the `cwd` it ran in.
pub fn codex_verdict_for(project: &Path, home: &Path, id: &str) -> ResumeVerdict {
    exact_verdict(codex_conversations(project, home), id)
}

fn codex_conversations(project: &Path, home: &Path) -> Option<Vec<Conversation>> {
    let root = home.join(".codex").join("sessions");
    if !root.is_dir() {
        return None;
    }
    let mut rollouts = Vec::new();
    collect_files(&root, "jsonl", 5, &mut rollouts);

    let mut conversations = Vec::new();
    for rollout in rollouts {
        let Ok(contents) = fs::read_to_string(&rollout) else {
            continue;
        };
        let mut lines = contents.lines();
        let Some(header) = lines.next() else {
            continue;
        };
        let Some(cwd) = json_string(header, "cwd") else {
            continue;
        };
        if !same_directory(&cwd, project) {
            continue;
        }
        let Some(id) = json_string(header, "id") else {
            continue;
        };
        // Opened is not used: Codex writes the rollout at startup and injects its own
        // `<environment_context>` as a user-role message, so neither proves a conversation. Only a
        // user-role record that is not one of those injected blocks does.
        if !contents.lines().any(records_user_turn) {
            continue;
        }
        conversations.push(Conversation { id });
    }
    Some(conversations)
}

fn records_user_turn(line: &str) -> bool {
    if line.contains("\"type\":\"user_message\"") {
        return true;
    }
    if !line.contains("\"role\":\"user\"") {
        return false;
    }
    json_strings(line, "text").iter().any(|text| {
        !text.starts_with("<environment_context>") && !text.starts_with("<user_instructions>")
    })
}

// ---------------------------------------------------------------------------
// OpenCode
// ---------------------------------------------------------------------------

/// OpenCode keeps no transcripts at all: its sessions live in the SQLite database at
/// `~/.local/share/opencode/opencode.db` (`session(id, directory, parent_id, time_updated, …)` and
/// `message(session_id, data, …)`). The desktop host has no SQLite of its own and Verb takes no
/// dependencies for this, so it asks the `sqlite3` the host already provides -- read-only, against
/// an immutable URI, so a running OpenCode is never disturbed. No `sqlite3` means no answer, which
/// is [`ResumeVerdict::Unknown`], not a `No`.
pub fn opencode_verdict_for(project: &Path, home: &Path, id: &str) -> ResumeVerdict {
    let wal = opencode_database(home).with_extension("db-wal");
    let wal_before = wal.exists();
    match opencode_session_ids(project, home) {
        Some(ids) if ids.iter().any(|candidate| candidate == id) => ResumeVerdict::Yes,
        // An immutable SQLite read deliberately ignores the writer's WAL. A missing ID could
        // still be present there. Check before and after the query so a checkpoint racing the
        // read cannot turn stale evidence into a false No.
        Some(_) if wal_before || wal.exists() => ResumeVerdict::Unknown,
        Some(_) => ResumeVerdict::No,
        None => ResumeVerdict::Unknown,
    }
}

fn opencode_database(home: &Path) -> PathBuf {
    home.join(".local")
        .join("share")
        .join("opencode")
        .join("opencode.db")
}

fn opencode_session_ids(project: &Path, home: &Path) -> Option<Vec<String>> {
    let database = opencode_database(home);
    if !database.is_file() {
        return None;
    }

    // `immutable=1` is what makes this safe to run against a live database: sqlite3 reads the file
    // without taking locks or touching the WAL, so Verb can never block or corrupt OpenCode's own
    // writer. When a WAL exists, a missing ID is reported as Unknown rather than a false No.
    let query = "SELECT s.id, s.directory FROM session s \
                 WHERE s.parent_id IS NULL AND EXISTS ( \
                   SELECT 1 FROM message m WHERE m.session_id = s.id \
                     AND m.data LIKE '%\"role\":\"user\"%' \
                 ) ORDER BY s.time_updated DESC;";
    // Verb usually runs from inside a project, so a relative PATH entry would let that project pick
    // the `sqlite3` binary; resolve it the same way the observation modules resolve theirs.
    let here = std::env::current_dir().ok()?;
    let sqlite = crate::exec::trusted_program("sqlite3", &here)?;
    let output = Command::new(sqlite)
        .arg("-readonly")
        .arg("-separator")
        .arg("\u{1f}")
        .arg(format!("file:{}?immutable=1", database.display()))
        .arg(query)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    Some(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|row| row.split_once('\u{1f}'))
            // Directory matching happens here rather than in SQL so it goes through the same rule
            // every other adapter uses.
            .filter(|(_, directory)| same_directory(directory, project))
            .map(|(id, _)| id.to_owned())
            .collect(),
    )
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn exact_verdict(conversations: Option<Vec<Conversation>>, id: &str) -> ResumeVerdict {
    match conversations {
        Some(conversations) if conversations.iter().any(|candidate| candidate.id == id) => {
            ResumeVerdict::Yes
        }
        Some(_) => ResumeVerdict::No,
        None => ResumeVerdict::Unknown,
    }
}

/// Compares what an agent recorded against the project Verb is tracking. Kept as one function so
/// every adapter agrees on what "the same directory" means; the desktop host has no Android-style
/// path aliases, so this is a canonicalised comparison rather than a set of spellings.
pub(crate) fn same_directory(recorded: &str, project: &Path) -> bool {
    let recorded = Path::new(recorded);
    if recorded == project {
        return true;
    }
    match (fs::canonicalize(recorded), fs::canonicalize(project)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.is_file() && path.extension().is_some_and(|value| value == extension)
}

fn collect_files(directory: &Path, extension: &str, depth: usize, found: &mut Vec<PathBuf>) {
    if depth == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, extension, depth - 1, found);
        } else if has_extension(&path, extension) {
            found.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A private HOME and project per test. No dependency on `tempfile`; the desktop crate stays
    /// dependency-free on purpose.
    fn scratch(name: &str) -> (PathBuf, PathBuf) {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir()
            .join("verb-agents-tests")
            .join(format!("{name}-{}-{unique}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let home = root.join("home");
        let project = root.join("project");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&project).unwrap();
        (home, project)
    }

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn codex_rollout(home: &Path, project: &Path, id: &str, user_turn: Option<&str>) {
        let mut contents = format!(
            "{{\"timestamp\":\"2026-08-21T13:00:00.000Z\",\"type\":\"session_meta\",\"payload\":\
             {{\"session_id\":\"{id}\",\"id\":\"{id}\",\"cwd\":\"{}\"}}}}",
            project.display()
        );
        if let Some(text) = user_turn {
            contents.push_str(&format!(
                "\n{{\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"user\",\
                 \"content\":[{{\"type\":\"input_text\",\"text\":\"{text}\"}}]}}}}"
            ));
        }
        contents.push('\n');
        write(
            &home
                .join(".codex")
                .join("sessions")
                .join("2026")
                .join("08")
                .join("21")
                .join(format!("rollout-2026-08-21T13-00-00-{id}.jsonl")),
            &contents,
        );
    }

    // --- Claude ---

    #[test]
    fn claude_transcript_for_this_project_is_recoverable() {
        let (home, project) = scratch("claude-transcript");
        write(
            &home
                .join(".claude")
                .join("projects")
                .join(claude_project_dir(&project))
                .join("conversation.jsonl"),
            "{}\n",
        );

        assert_eq!(
            claude_verdict_for(&project, &home, "conversation"),
            ResumeVerdict::Yes
        );
        assert_eq!(
            claude_verdict_for(&project, &home, "other"),
            ResumeVerdict::Unknown
        );
    }

    #[test]
    fn claude_project_directory_matches_the_installed_normalization() {
        assert_eq!(
            claude_project_dir(Path::new("/tmp/Verb_Transfer.v1")),
            "-tmp-Verb-Transfer-v1"
        );
    }

    #[test]
    fn claude_identity_is_the_session_id_never_the_pid_filename() {
        let (home, project) = scratch("claude-identity");
        write(
            &home.join(".claude").join("sessions").join("4321.json"),
            &format!(
                "{{\"pid\":4321,\"sessionId\":\"claude-conversation\",\"cwd\":\"{}\"}}",
                project.display()
            ),
        );

        assert_eq!(
            claude_verdict_for(&project, &home, "claude-conversation"),
            ResumeVerdict::Yes
        );
        assert_eq!(
            claude_verdict_for(&project, &home, "other"),
            ResumeVerdict::No
        );
    }

    #[test]
    fn claude_with_no_state_at_all_is_unknown_never_no() {
        let (home, project) = scratch("claude-nothing");

        assert_eq!(
            claude_verdict_for(&project, &home, "missing"),
            ResumeVerdict::Unknown
        );
    }

    // --- Codex ---

    #[test]
    fn codex_rollout_with_a_real_turn_is_recoverable() {
        let (home, project) = scratch("codex-real");
        codex_rollout(&home, &project, "codex-1", Some("say hi"));

        assert_eq!(
            codex_verdict_for(&project, &home, "codex-1"),
            ResumeVerdict::Yes
        );
        assert_eq!(
            codex_verdict_for(&project, &home, "other"),
            ResumeVerdict::No
        );
    }

    #[test]
    fn codex_rollout_that_was_only_opened_is_not_recoverable() {
        // Codex writes the rollout at startup, and injects <environment_context> as a user-role
        // message. Neither is evidence that a conversation happened.
        let (home, project) = scratch("codex-idle");
        codex_rollout(&home, &project, "codex-idle", None);
        codex_rollout(
            &home,
            &project,
            "codex-injected",
            Some("<environment_context>\\n  <cwd>/somewhere</cwd>"),
        );

        assert_eq!(
            codex_verdict_for(&project, &home, "codex-idle"),
            ResumeVerdict::No
        );
        assert_eq!(
            codex_verdict_for(&project, &home, "codex-injected"),
            ResumeVerdict::No
        );
    }

    #[test]
    fn codex_rollouts_for_other_projects_do_not_count() {
        let (home, project) = scratch("codex-other");
        let other = project.parent().unwrap().join("other-project");
        fs::create_dir_all(&other).unwrap();
        codex_rollout(&home, &other, "codex-elsewhere", Some("hello"));

        assert_eq!(
            codex_verdict_for(&project, &home, "codex-elsewhere"),
            ResumeVerdict::No
        );
    }

    #[test]
    fn codex_with_no_sessions_tree_is_unknown_never_no() {
        let (home, project) = scratch("codex-nothing");

        assert_eq!(
            codex_verdict_for(&project, &home, "missing"),
            ResumeVerdict::Unknown
        );
    }

    // --- OpenCode ---

    #[test]
    fn opencode_without_a_database_is_unknown_never_no() {
        let (home, project) = scratch("opencode-nothing");

        assert_eq!(
            opencode_verdict_for(&project, &home, "missing"),
            ResumeVerdict::Unknown
        );
    }

    #[test]
    fn opencode_reads_used_sessions_from_its_database() {
        let (home, project) = scratch("opencode-db");
        let database = home
            .join(".local")
            .join("share")
            .join("opencode")
            .join("opencode.db");
        fs::create_dir_all(database.parent().unwrap()).unwrap();

        let schema = format!(
            "CREATE TABLE session (id text PRIMARY KEY, parent_id text, directory text NOT NULL, \
             time_updated integer NOT NULL); \
             CREATE TABLE message (id text PRIMARY KEY, session_id text NOT NULL, data text NOT NULL); \
             INSERT INTO session VALUES ('used', NULL, '{directory}', 20); \
             INSERT INTO message VALUES ('m1', 'used', '{{\"role\":\"user\"}}'); \
             INSERT INTO session VALUES ('idle', NULL, '{directory}', 30);",
            directory = project.display()
        );
        let created = Command::new("sqlite3").arg(&database).arg(&schema).status();
        let Ok(status) = created else {
            // No sqlite3 on this machine: the adapter's own answer in that case is Unknown, which
            // is exactly what this asserts instead of failing the suite.
            assert_eq!(
                opencode_verdict_for(&project, &home, "used"),
                ResumeVerdict::Unknown
            );
            return;
        };
        assert!(status.success());

        assert_eq!(
            opencode_verdict_for(&project, &home, "used"),
            ResumeVerdict::Yes
        );
        assert_eq!(
            opencode_verdict_for(&project, &home, "idle"),
            ResumeVerdict::No
        );

        fs::write(database.with_extension("db-wal"), b"uncheckpointed").unwrap();
        assert_eq!(
            opencode_verdict_for(&project, &home, "used"),
            ResumeVerdict::Yes
        );
        assert_eq!(
            opencode_verdict_for(&project, &home, "idle"),
            ResumeVerdict::Unknown
        );
    }
}
