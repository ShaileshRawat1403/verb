//! Project memory and a durable, agent-neutral task ledger.
//!
//! Verb records explicit work agreements, not agent transcripts. A task has one owner at a time;
//! help requests and handoffs remain in its history. Every mutation takes a project file lock and
//! publishes a complete replacement, so concurrent CLI agents cannot lose each other's updates.

use crate::fsutil::atomic_write;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use crate::{load_session_by_id, new_id, now_millis, SessionState};

const SCHEMA_VERSION: u8 = 1;
const MAX_MEMORY: usize = 64 * 1024;
const MAX_NOTE: usize = 8 * 1024;
const MAX_TITLE: usize = 200;
const MAX_EVENTS: usize = 200;
const MAX_TASK_BYTES: u64 = 2 * 1024 * 1024;
const MAX_SHARED_SNAPSHOT: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Task {
    schema_version: u8,
    id: String,
    project_id: PathBuf,
    #[serde(default)]
    verb_project_id: Option<String>,
    title: String,
    brief: String,
    created_at: u128,
    events: Vec<TaskEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TaskEvent {
    id: String,
    at: u128,
    kind: EventKind,
    session_id: String,
    note: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EventKind {
    Claimed,
    Reassigned,
    HelpRequested,
    HelpReplied,
    HandedOff,
    Completed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TaskAction {
    Claim,
    Reassign,
    RequestHelp,
    Reply,
    Handoff,
    Done,
}

impl TaskAction {
    fn event(self) -> EventKind {
        match self {
            Self::Claim => EventKind::Claimed,
            Self::Reassign => EventKind::Reassigned,
            Self::RequestHelp => EventKind::HelpRequested,
            Self::Reply => EventKind::HelpReplied,
            Self::Handoff => EventKind::HandedOff,
            Self::Done => EventKind::Completed,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TaskSnapshot {
    pub id: String,
    pub title: String,
    pub brief: String,
    pub status: &'static str,
    pub owner: Option<String>,
    pub needs_help: bool,
    pub created_at: u128,
    pub events: Vec<TaskHistory>,
}

#[derive(Clone, Debug)]
pub(crate) struct TaskHistory {
    pub at: u128,
    pub kind: EventKind,
    pub session_id: String,
    pub note: Option<String>,
}

pub(crate) fn snapshots(project: &Path) -> Result<Vec<TaskSnapshot>, String> {
    let mut tasks = read_tasks(project)?;
    tasks.sort_by_key(|task| std::cmp::Reverse(task.created_at));
    Ok(tasks
        .into_iter()
        .map(|task| {
            let status = task.status();
            let needs_help = task.needs_help();
            TaskSnapshot {
                id: task.id,
                title: task.title,
                brief: task.brief,
                status: status.label(),
                owner: status.owner().map(str::to_owned),
                needs_help,
                created_at: task.created_at,
                events: task
                    .events
                    .into_iter()
                    .map(|event| TaskHistory {
                        at: event.at,
                        kind: event.kind,
                        session_id: event.session_id,
                        note: event.note,
                    })
                    .collect(),
            }
        })
        .collect())
}

pub(crate) fn shared_memory(project: &Path) -> Result<String, String> {
    read_memory(project)
}

pub(crate) fn append_ui_memory(project: &Path, note: &str) -> Result<String, String> {
    if note.trim().is_empty() {
        return Err("a memory note cannot be empty".to_owned());
    }
    let root = project_root(project)?;
    let _lock = lock_project(&root)?;
    let mut current = read_memory(project)?;
    if !current.is_empty() && !current.ends_with('\n') {
        current.push('\n');
    }
    current.push_str(note);
    if !current.ends_with('\n') {
        current.push('\n');
    }
    if current.len() > MAX_MEMORY {
        return Err(format!("project memory exceeds {MAX_MEMORY} bytes"));
    }
    atomic_write(&root.join("memory.md"), current.as_bytes())?;
    Ok("Shared project memory saved.".to_owned())
}

pub(crate) fn create_ui_task(project: &Path, title: &str, brief: &str) -> Result<String, String> {
    create_task_record(project, title, brief)
}

pub(crate) fn apply_ui_action(
    project: &Path,
    task_id: &str,
    session_id: &str,
    action: TaskAction,
    note: Option<String>,
    expected_revision: Option<&str>,
) -> Result<String, String> {
    if action != TaskAction::Claim && note.as_ref().is_none_or(|note| note.trim().is_empty()) {
        return Err("a task note cannot be empty".to_owned());
    }
    update_task_inner(
        project,
        task_id,
        session_id,
        action.event(),
        note,
        expected_revision,
    )
}

pub(crate) fn handoff_revision(project: &Path, task_id: &str) -> Result<String, String> {
    let root = project_root(project)?;
    let _lock = lock_project(&root)?;
    let task = read_task(&root, project, task_id)?;
    revision_for_handoff(&read_memory(project)?, &task)
}

impl EventKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Claimed => "claimed",
            Self::Reassigned => "reassigned",
            Self::HelpRequested => "requested help",
            Self::HelpReplied => "replied",
            Self::HandedOff => "handed off for review",
            Self::Completed => "completed",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TaskStatus {
    Open,
    Active(String),
    Review,
    Done,
}

impl TaskStatus {
    fn label(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Active(_) => "active",
            Self::Review => "needs review",
            Self::Done => "done",
        }
    }

    fn owner(&self) -> Option<&str> {
        match self {
            Self::Active(id) => Some(id),
            _ => None,
        }
    }
}

impl Task {
    fn status(&self) -> TaskStatus {
        let mut status = TaskStatus::Open;
        for event in &self.events {
            match event.kind {
                EventKind::Claimed | EventKind::Reassigned => {
                    status = TaskStatus::Active(event.session_id.clone())
                }
                EventKind::HandedOff => status = TaskStatus::Review,
                EventKind::Completed => status = TaskStatus::Done,
                EventKind::HelpRequested | EventKind::HelpReplied => {}
            }
        }
        status
    }

    fn needs_help(&self) -> bool {
        let mut pending = false;
        for event in &self.events {
            match event.kind {
                EventKind::HelpRequested => pending = true,
                EventKind::HelpReplied | EventKind::HandedOff | EventKind::Completed => {
                    pending = false
                }
                EventKind::Claimed | EventKind::Reassigned => pending = false,
            }
        }
        pending
    }

    fn record(
        &mut self,
        kind: EventKind,
        session_id: &str,
        note: Option<String>,
    ) -> Result<(), String> {
        if self.events.len() >= MAX_EVENTS {
            return Err(format!(
                "task history is full (maximum {MAX_EVENTS} events)"
            ));
        }
        self.events.push(TaskEvent {
            id: new_id(),
            at: now_millis(),
            kind,
            session_id: session_id.to_owned(),
            note,
        });
        Ok(())
    }
}

pub(crate) fn memory_command(project: &Path, args: &[String]) -> Result<(), String> {
    match args {
        [show] if show == "show" => {
            let memory = read_memory(project)?;
            if memory.is_empty() {
                println!("No shared memory recorded for this project.");
            } else {
                print!("{memory}");
            }
            Ok(())
        }
        [hash] if hash == "hash" => {
            let root = project_root(project)?;
            let _lock = lock_project(&root)?;
            println!("{}", memory_hash(&read_memory(project)?));
            Ok(())
        }
        [action, source] if action == "set" || action == "append" => {
            write_memory(project, action, source, None, false)
        }
        [action, source, flag, value] if action == "set" && flag == "--base-hash" => {
            write_memory(project, action, source, Some(value), false)
        }
        [action, source, flag] if action == "set" && flag == "--force" => {
            write_memory(project, action, source, None, true)
        }
        _ => Err("usage: verb memory show|hash|append FILE|set FILE [--base-hash HASH|--force] (FILE may be - for stdin)".to_owned()),
    }
}

fn memory_hash(memory: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(memory.as_bytes()))
}

fn write_memory(
    project: &Path,
    action: &str,
    source: &str,
    base_hash: Option<&str>,
    force: bool,
) -> Result<(), String> {
    let input = read_input(source, MAX_MEMORY)?;
    let root = project_root(project)?;
    let _lock = lock_project(&root)?;
    let path = root.join("memory.md");
    let current = read_memory(project)?;
    let updated = if action == "append" {
        let mut current = current.clone();
        if !current.is_empty() && !current.ends_with('\n') {
            current.push('\n');
        }
        current.push_str(&input);
        current
    } else {
        input
    };
    if action == "set" && !force {
        if let Some(expected) = base_hash {
            if expected != memory_hash(&current) {
                return Err(
                    "project memory changed; read 'verb memory hash' and retry with --base-hash"
                        .to_owned(),
                );
            }
        } else if updated != current && !current.is_empty() {
            return Err("project memory already exists; read 'verb memory hash' and set with --base-hash, or use --force for an intentional replacement".to_owned());
        }
    }
    if updated.len() > MAX_MEMORY {
        return Err(format!("project memory exceeds {MAX_MEMORY} bytes"));
    }
    if updated != current {
        atomic_write(&path, updated.as_bytes())?;
    }
    println!("Shared project memory saved ({} bytes).", updated.len());
    Ok(())
}

/// The small agent-neutral bridge. `read` writes one complete, internally consistent snapshot;
/// a receipt records delivery to the caller's CLI process, never model comprehension. The digest
/// is a content revision, so legacy memory/task writers also change it without a migration.
pub(crate) fn shared_command(
    project: &Path,
    args: &[String],
    json_output: bool,
) -> Result<(), String> {
    // An agent can move to a subdirectory or another working directory during its conversation.
    // The project inherited from its Verb host remains the collaboration scope.
    let hosted_project = match (
        std::env::var("VERB_SESSION_ID"),
        std::env::var_os("VERB_PROJECT_ROOT"),
    ) {
        (Ok(id), Some(root)) if !id.is_empty() => {
            let root = PathBuf::from(root);
            verify_agent_session(&root, &id)?;
            root
        }
        _ => project.to_path_buf(),
    };
    match args {
        [action] if action == "read" => shared_read(&hosted_project, json_output),
        [action] if action == "status" => shared_status(&hosted_project, json_output),
        [action, source] if action == "publish" => shared_publish(&hosted_project, source),
        _ => Err("usage: verb shared read|status [--json] | publish FILE".to_owned()),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SharedSnapshot {
    schema_version: u8,
    revision: String,
    project_id: PathBuf,
    verb_project_id: String,
    memory: String,
    tasks: Vec<Task>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FetchReceipt {
    schema_version: u8,
    session_id: String,
    revision: String,
    fetched_at: u128,
    /// The memory and each owned task's history at the time this agent fetched. A change to an
    /// unrelated task does not invalidate an owner's review handoff.
    #[serde(default)]
    handoff_revisions: BTreeMap<String, String>,
    /// The last event for each task actually included in the delivered snapshot. `None` means a
    /// receipt written before this field existed, so event-level unread state is unknown.
    #[serde(default)]
    event_cursors: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InboxKind {
    ReviewAvailable,
    HelpAvailable,
    HelpReply,
    AssignedToYou,
}

impl InboxKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::ReviewAvailable => "Review available",
            Self::HelpAvailable => "Help requested",
            Self::HelpReply => "Help reply received",
            Self::AssignedToYou => "Assigned to this session",
        }
    }

    fn priority(self) -> u8 {
        match self {
            Self::ReviewAvailable => 0,
            Self::HelpAvailable => 1,
            Self::HelpReply => 2,
            Self::AssignedToYou => 3,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InboxItem {
    pub task_id: String,
    pub title: String,
    pub kind: InboxKind,
    pub linked_session_id: String,
    /// `None` for legacy receipts whose task-event cursor was never recorded.
    pub new_since_fetch: Option<bool>,
    pub at: u128,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InboxSnapshot {
    pub session_id: String,
    pub revision: String,
    pub context_state: &'static str,
    pub fetched_at: Option<u128>,
    pub items: Vec<InboxItem>,
}

fn revision_for_handoff(memory: &str, task: &Task) -> Result<String, String> {
    let content = serde_json::to_vec(&(memory, task))
        .map_err(|error| format!("could not encode handoff context: {error}"))?;
    Ok(format!("sha256:{:x}", Sha256::digest(&content)))
}

fn shared_snapshot(project: &Path) -> Result<SharedSnapshot, String> {
    let root = project_root(project)?;
    let _lock = lock_project(&root)?;
    shared_snapshot_unlocked(project)
}

fn shared_snapshot_unlocked(project: &Path) -> Result<SharedSnapshot, String> {
    let memory = read_memory(project)?;
    let tasks = read_tasks(project)?;
    build_shared_snapshot(project, memory, tasks)
}

fn build_shared_snapshot(
    project: &Path,
    memory: String,
    mut tasks: Vec<Task>,
) -> Result<SharedSnapshot, String> {
    tasks.sort_by(|left, right| left.id.cmp(&right.id));
    let content = serde_json::to_vec(&(&memory, &tasks))
        .map_err(|error| format!("could not encode shared context: {error}"))?;
    if content.len() > MAX_SHARED_SNAPSHOT {
        return Err(format!(
            "shared context exceeds {} bytes; reduce old task history or project memory",
            MAX_SHARED_SNAPSHOT
        ));
    }
    let revision = format!("sha256:{:x}", Sha256::digest(&content));
    let identity = crate::project::identity(project)?;
    Ok(SharedSnapshot {
        schema_version: SCHEMA_VERSION,
        revision,
        project_id: identity.anchor,
        verb_project_id: identity.id,
        memory,
        tasks,
    })
}

fn shared_read(project: &Path, json_output: bool) -> Result<(), String> {
    let caller = std::env::var("VERB_SESSION_ID")
        .ok()
        .filter(|id| !id.is_empty());
    if let Some(id) = &caller {
        verify_agent_session(project, id)?;
    }
    let snapshot = shared_snapshot(project)?;
    let output = if json_output {
        serde_json::to_string_pretty(&snapshot)
            .map_err(|error| format!("could not encode shared context: {error}"))?
    } else {
        format_shared(&snapshot)
    };
    let mut stdout = io::stdout().lock();
    stdout
        .write_all(output.as_bytes())
        .and_then(|()| stdout.flush())
        .map_err(|error| format!("could not deliver shared context: {error}"))?;
    if let Some(id) = caller {
        let mut handoff_revisions = BTreeMap::new();
        let mut event_cursors = BTreeMap::new();
        for task in &snapshot.tasks {
            if let Some(event) = task.events.last() {
                event_cursors.insert(task.id.clone(), event.id.clone());
            }
            if task.status().owner() == Some(id.as_str()) {
                handoff_revisions.insert(
                    task.id.clone(),
                    revision_for_handoff(&snapshot.memory, task)?,
                );
            }
        }
        let receipt = FetchReceipt {
            schema_version: SCHEMA_VERSION,
            session_id: id.clone(),
            revision: snapshot.revision,
            fetched_at: now_millis(),
            handoff_revisions,
            event_cursors: Some(event_cursors),
        };
        let bytes = serde_json::to_vec_pretty(&receipt)
            .map_err(|error| format!("could not encode fetch receipt: {error}"))?;
        atomic_write(&receipt_path(&project_root(project)?, &id)?, &bytes)?;
    }
    Ok(())
}

fn format_shared(snapshot: &SharedSnapshot) -> String {
    use std::fmt::Write as _;
    let mut output = format!("SHARED CONTEXT · {}\n\nPROJECT MEMORY\n", snapshot.revision);
    if snapshot.memory.is_empty() {
        output.push_str("(empty)\n");
    } else {
        output.push_str(&snapshot.memory);
        if !output.ends_with('\n') {
            output.push('\n');
        }
    }
    output.push_str("\nTASKS AND HANDOFFS\n");
    if snapshot.tasks.is_empty() {
        output.push_str("(none)\n");
    }
    for task in &snapshot.tasks {
        let _ = writeln!(
            output,
            "{} · {} · {}",
            task.id,
            task.status().label(),
            task.title
        );
        if !task.brief.is_empty() {
            let _ = writeln!(output, "Brief: {}", task.brief);
        }
        for event in &task.events {
            let _ = writeln!(
                output,
                "  {} · {} · session {}",
                crate::iso8601(event.at),
                event.kind.label(),
                event.session_id
            );
            if let Some(note) = &event.note {
                let _ = writeln!(output, "  {note}");
            }
        }
    }
    output
}

fn shared_publish(project: &Path, source: &str) -> Result<(), String> {
    let note = read_input(source, MAX_NOTE)?;
    let note = note.trim();
    if note.is_empty() {
        return Err("a shared note cannot be empty".to_owned());
    }
    let author = match std::env::var("VERB_SESSION_ID")
        .ok()
        .filter(|id| !id.is_empty())
    {
        Some(id) => {
            verify_agent_session(project, &id)?;
            format!("agent session {id}")
        }
        None => "person".to_owned(),
    };
    let identity = crate::project::identity(project)?;
    let root = identity.store;
    let _lock = lock_project(&root)?;
    let mut memory = read_memory(project)?;
    if !memory.is_empty() && !memory.ends_with('\n') {
        memory.push('\n');
    }
    if !memory.is_empty() {
        memory.push('\n');
    }
    memory.push_str(&format!(
        "### {} · {}\n{}\n",
        crate::iso8601(now_millis()),
        author,
        note
    ));
    if memory.len() > MAX_MEMORY {
        return Err(format!("project memory exceeds {MAX_MEMORY} bytes"));
    }
    let revision = build_shared_snapshot(project, memory.clone(), read_tasks(project)?)?.revision;
    atomic_write(&root.join("memory.md"), memory.as_bytes())?;
    drop(_lock);
    println!("Shared note published. Revision: {revision}");
    Ok(())
}

fn shared_status(project: &Path, json_output: bool) -> Result<(), String> {
    let snapshot = shared_snapshot(project)?;
    let root = project_root(project)?;
    let sessions = crate::read_sessions()?;
    let mut rows = Vec::new();
    for session in sessions {
        if !crate::session_in_project(&session, project) || session.agent.is_none() {
            continue;
        }
        let receipt = read_receipt(&root, &session.id)?;
        let state = match receipt.as_ref() {
            None => "never fetched",
            Some(receipt) if receipt.revision == snapshot.revision => "fetched current revision",
            Some(_) => "new revision available",
        };
        rows.push(json!({
            "sessionId": session.id,
            "agent": session.agent.as_ref().map(|agent| agent.label()),
            "state": state,
            "fetchedRevision": receipt.as_ref().map(|receipt| &receipt.revision),
            "fetchedAt": receipt.as_ref().map(|receipt| receipt.fetched_at),
        }));
    }
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "revision": snapshot.revision, "sessions": rows
            }))
            .map_err(|error| error.to_string())?
        );
    } else {
        println!("Shared revision: {}", snapshot.revision);
        if rows.is_empty() {
            println!("No agent sessions in this project.");
        }
        for row in rows {
            println!(
                "{} · {} · {}",
                row["agent"].as_str().unwrap_or("agent"),
                row["sessionId"].as_str().unwrap_or("unknown"),
                row["state"].as_str().unwrap_or("unknown")
            );
        }
    }
    Ok(())
}

/// A session's actionable work, derived from task state and its last successful context fetch.
/// Reading the inbox never acknowledges an item or causes an agent to execute a command.
pub(crate) fn inbox_snapshot(project: &Path, session_id: &str) -> Result<InboxSnapshot, String> {
    inbox_snapshots(project, &[session_id.to_owned()])?
        .pop()
        .ok_or_else(|| "could not load the session inbox".to_owned())
}

pub(crate) fn inbox_snapshots(
    project: &Path,
    session_ids: &[String],
) -> Result<Vec<InboxSnapshot>, String> {
    for session_id in session_ids {
        let session = load_session_by_id(session_id)?
            .ok_or_else(|| format!("no Verb session with id {session_id}"))?;
        if session.agent.is_none() || !crate::session_in_project(&session, project) {
            return Err("the inbox needs an agent session in this project".to_owned());
        }
    }
    let root = project_root(project)?;
    let _lock = lock_project(&root)?;
    let snapshot = shared_snapshot_unlocked(project)?;
    session_ids
        .iter()
        .map(|session_id| {
            let receipt = read_receipt(&root, session_id)?;
            Ok(build_inbox(&snapshot, session_id, receipt))
        })
        .collect()
}

fn build_inbox(
    snapshot: &SharedSnapshot,
    session_id: &str,
    receipt: Option<FetchReceipt>,
) -> InboxSnapshot {
    let context_state = match receipt.as_ref() {
        None => "never fetched",
        Some(receipt) if receipt.revision == snapshot.revision => "fetched current revision",
        Some(_) => "new revision available",
    };
    let mut items = Vec::new();
    for task in &snapshot.tasks {
        let unseen = unseen_task_events(task, receipt.as_ref());
        let status = task.status();
        if status == TaskStatus::Review {
            if let Some(event) = task
                .events
                .iter()
                .rev()
                .find(|event| event.kind == EventKind::HandedOff)
                .filter(|event| event.session_id != session_id)
            {
                items.push(inbox_item(task, event, InboxKind::ReviewAvailable, unseen));
            }
            continue;
        }
        if let TaskStatus::Active(owner) = status {
            if task.needs_help() && owner != session_id {
                if let Some(event) = task
                    .events
                    .iter()
                    .rev()
                    .find(|event| event.kind == EventKind::HelpRequested)
                {
                    items.push(inbox_item(task, event, InboxKind::HelpAvailable, unseen));
                }
            }
            if owner == session_id {
                if let Some(events) = unseen {
                    if let Some(event) = (!task.needs_help())
                        .then(|| {
                            events
                                .iter()
                                .rev()
                                .find(|event| event.kind == EventKind::HelpReplied)
                        })
                        .flatten()
                    {
                        items.push(inbox_item(task, event, InboxKind::HelpReply, unseen));
                    } else if let Some(event) = events.iter().rev().find(|event| {
                        matches!(event.kind, EventKind::Claimed | EventKind::Reassigned)
                            && event.session_id == session_id
                    }) {
                        items.push(inbox_item(task, event, InboxKind::AssignedToYou, unseen));
                    }
                }
            }
        }
    }
    items.sort_by(|left, right| {
        left.kind
            .priority()
            .cmp(&right.kind.priority())
            .then_with(|| right.at.cmp(&left.at))
            .then_with(|| left.task_id.cmp(&right.task_id))
    });
    InboxSnapshot {
        session_id: session_id.to_owned(),
        revision: snapshot.revision.clone(),
        context_state,
        fetched_at: receipt.map(|receipt| receipt.fetched_at),
        items,
    }
}

fn unseen_task_events<'a>(
    task: &'a Task,
    receipt: Option<&FetchReceipt>,
) -> Option<&'a [TaskEvent]> {
    let Some(receipt) = receipt else {
        return Some(&task.events);
    };
    let cursors = receipt.event_cursors.as_ref()?;
    let Some(cursor) = cursors.get(&task.id) else {
        return Some(&task.events);
    };
    task.events
        .iter()
        .position(|event| event.id == *cursor)
        .map(|position| &task.events[position + 1..])
}

fn inbox_item(
    task: &Task,
    event: &TaskEvent,
    kind: InboxKind,
    unseen: Option<&[TaskEvent]>,
) -> InboxItem {
    InboxItem {
        task_id: task.id.clone(),
        title: task.title.clone(),
        kind,
        linked_session_id: event.session_id.clone(),
        new_since_fetch: unseen.map(|events| events.iter().any(|item| item.id == event.id)),
        at: event.at,
    }
}

pub(crate) fn inbox_command(
    project: &Path,
    args: &[String],
    json_output: bool,
) -> Result<(), String> {
    let session_id = match args {
        [] => current_session()?,
        [id] => id.clone(),
        _ => return Err("usage: verb inbox [SESSION_ID] [--json]".to_owned()),
    };
    let project = match std::env::var_os("VERB_PROJECT_ROOT") {
        Some(root)
            if std::env::var("VERB_SESSION_ID").ok().as_deref() == Some(session_id.as_str()) =>
        {
            PathBuf::from(root)
        }
        _ => project.to_path_buf(),
    };
    let inbox = inbox_snapshot(&project, &session_id)?;
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&inbox)
                .map_err(|error| format!("could not encode inbox: {error}"))?
        );
    } else {
        println!("SESSION INBOX · {}", inbox.session_id);
        println!("Shared context: {}", inbox.context_state);
        if inbox.items.is_empty() {
            println!("No task attention for this session.");
        }
        for item in inbox.items {
            let fresh = match item.new_since_fetch {
                Some(true) => " · new since fetch",
                Some(false) => "",
                None => " · event freshness unknown",
            };
            println!(
                "{} · {} · {}{}",
                item.kind.label(),
                item.task_id,
                item.title,
                fresh
            );
        }
        println!("Read a task with: verb task context TASK_ID");
        println!("Fetch current shared context with: verb shared read");
    }
    Ok(())
}

fn receipt_path(root: &Path, id: &str) -> Result<PathBuf, String> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err("invalid session id for shared context receipt".to_owned());
    }
    Ok(root.join("fetches").join(format!("{id}.json")))
}

fn read_receipt(root: &Path, id: &str) -> Result<Option<FetchReceipt>, String> {
    let path = receipt_path(root, id)?;
    let text = match read_bounded(&path, MAX_SHARED_SNAPSHOT as u64) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("could not read fetch receipt for {id}: {error}")),
    };
    let receipt: FetchReceipt = serde_json::from_str(&text)
        .map_err(|error| format!("invalid fetch receipt for {id}: {error}"))?;
    if receipt.schema_version != SCHEMA_VERSION || receipt.session_id != id {
        return Err(format!("invalid fetch receipt identity for {id}"));
    }
    Ok(Some(receipt))
}

pub(crate) fn task_command(
    project: &Path,
    args: &[String],
    json_output: bool,
) -> Result<(), String> {
    match args {
        [action] if action == "list" => list_tasks(project, json_output),
        [action, title] if action == "create" => create_task(project, title, ""),
        [action, title, source] if action == "create" => {
            let brief = read_input(source, MAX_NOTE)?;
            create_task(project, title, &brief)
        }
        [action, id] if action == "show" => show_task(project, id, json_output, false),
        [action, id] if action == "context" => show_task(project, id, json_output, true),
        [action, id] if action == "claim" => {
            let session = current_session()?;
            update_task(project, id, &session, EventKind::Claimed, None)
        }
        [action, id, session] if action == "claim" => {
            update_task(project, id, session, EventKind::Claimed, None)
        }
        [action, id, session, source] if action == "reassign" => {
            let note = read_input(source, MAX_NOTE)?;
            if note.trim().is_empty() {
                return Err("a reassignment reason cannot be empty".to_owned());
            }
            update_task(project, id, session, EventKind::Reassigned, Some(note))
        }
        [action, id, source]
            if matches!(
                action.as_str(),
                "request-help" | "reply" | "handoff" | "done"
            ) =>
        {
            let session = current_session()?;
            let kind = event_command(action)?;
            let note = read_input(source, MAX_NOTE)?;
            if note.trim().is_empty() {
                return Err("a task note cannot be empty".to_owned());
            }
            update_task(project, id, &session, kind, Some(note))
        }
        [action, id, session, source] => {
            let kind = event_command(action)?;
            let note = read_input(source, MAX_NOTE)?;
            if note.trim().is_empty() {
                return Err("a task note cannot be empty".to_owned());
            }
            update_task(project, id, session, kind, Some(note))
        }
        _ => Err(task_usage()),
    }
}

fn task_usage() -> String {
    "usage: verb task list | create TITLE [BRIEF_FILE] | show ID | context ID | claim ID [SESSION_ID] | reassign ID NEW_SESSION_ID REASON_FILE | request-help|reply|handoff|done ID [SESSION_ID] FILE".to_owned()
}

fn current_session() -> Result<String, String> {
    std::env::var("VERB_SESSION_ID")
        .ok()
        .filter(|id| !id.is_empty())
        .ok_or_else(|| "no active Verb session; pass SESSION_ID explicitly".to_owned())
}

fn event_command(action: &str) -> Result<EventKind, String> {
    match action {
        "request-help" => Ok(EventKind::HelpRequested),
        "reply" => Ok(EventKind::HelpReplied),
        "handoff" => Ok(EventKind::HandedOff),
        "done" => Ok(EventKind::Completed),
        _ => Err(task_usage()),
    }
}

fn create_task(project: &Path, title: &str, brief: &str) -> Result<(), String> {
    println!("{}", create_task_inner(project, title, brief)?);
    Ok(())
}

fn create_task_inner(project: &Path, title: &str, brief: &str) -> Result<String, String> {
    let id = create_task_record(project, title, brief)?;
    Ok(format!("Created task {id}: {}", title.trim()))
}

fn create_task_record(project: &Path, title: &str, brief: &str) -> Result<String, String> {
    if title.trim().is_empty() || title.len() > MAX_TITLE || title.chars().any(char::is_control) {
        return Err(format!(
            "task title must be one line and at most {MAX_TITLE} bytes"
        ));
    }
    if brief.len() > MAX_NOTE {
        return Err(format!("task brief exceeds {MAX_NOTE} bytes"));
    }
    let identity = crate::project::identity(project)?;
    let root = identity.store.clone();
    let _lock = lock_project(&root)?;
    let id = (0..8)
        .map(|_| new_id())
        .find(|id| task_path(&root, id).is_ok_and(|path| !path.exists()))
        .ok_or_else(|| "could not allocate a unique task id".to_owned())?;
    let task = Task {
        schema_version: SCHEMA_VERSION,
        id,
        project_id: identity.anchor,
        verb_project_id: Some(identity.id),
        title: title.trim().to_owned(),
        brief: brief.to_owned(),
        created_at: now_millis(),
        events: Vec::new(),
    };
    write_task(&root, &task)?;
    Ok(task.id)
}

fn list_tasks(project: &Path, json_output: bool) -> Result<(), String> {
    let mut tasks = read_tasks(project)?;
    tasks.sort_by_key(|task| std::cmp::Reverse(task.created_at));
    if json_output {
        let summaries: Vec<_> = tasks.iter().map(|task| {
            let status = task.status();
            json!({"taskId":task.id,"title":task.title,"status":status.label(),
                "ownerSessionId":status.owner(),"needsHelp":task.needs_help(),"createdAt":task.created_at})
        }).collect();
        println!(
            "{}",
            serde_json::to_string(&summaries).map_err(|error| error.to_string())?
        );
    } else if tasks.is_empty() {
        println!("No tasks in this project.");
    } else {
        for task in tasks {
            let status = task.status();
            let help = if task.needs_help() {
                " · help requested"
            } else {
                ""
            };
            println!("{:<13} {}  {}{}", status.label(), task.id, task.title, help);
        }
    }
    Ok(())
}

fn show_task(
    project: &Path,
    id: &str,
    json_output: bool,
    include_memory: bool,
) -> Result<(), String> {
    let task = read_task(&project_root(project)?, project, id)?;
    let memory = include_memory.then(|| read_memory(project)).transpose()?;
    if json_output {
        let status = task.status();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "task":task,"status":status.label(),"ownerSessionId":status.owner(),
                "needsHelp":task.needs_help(),"projectMemory":memory
            }))
            .map_err(|error| error.to_string())?
        );
        return Ok(());
    }
    if let Some(memory) = memory {
        println!(
            "PROJECT MEMORY\n{}\n",
            if memory.is_empty() {
                "(empty)"
            } else {
                &memory
            }
        );
    }
    println!("TASK {} · {}", task.id, task.status().label());
    println!("{}", task.title);
    if !task.brief.is_empty() {
        println!("\nBRIEF\n{}", task.brief);
    }
    if !task.events.is_empty() {
        println!("\nHISTORY");
        for event in &task.events {
            println!(
                "{} · {} · session {}",
                crate::iso8601(event.at),
                event.kind.label(),
                event.session_id
            );
            if let Some(note) = &event.note {
                println!("{note}");
            }
        }
    }
    Ok(())
}

fn update_task(
    project: &Path,
    id: &str,
    session_id: &str,
    kind: EventKind,
    note: Option<String>,
) -> Result<(), String> {
    println!(
        "{}",
        update_task_inner(project, id, session_id, kind, note, None)?
    );
    Ok(())
}

fn update_task_inner(
    project: &Path,
    id: &str,
    session_id: &str,
    kind: EventKind,
    note: Option<String>,
    expected_revision: Option<&str>,
) -> Result<String, String> {
    verify_agent_session(project, session_id)?;
    let root = project_root(project)?;
    let _lock = lock_project(&root)?;
    let mut task = read_task(&root, project, id)?;
    let status = task.status();
    match kind {
        EventKind::Claimed => match status {
            TaskStatus::Open | TaskStatus::Review => {}
            TaskStatus::Active(ref owner) if owner == session_id => {
                return Ok(format!(
                    "Task {id} is already claimed by session {session_id}."
                ));
            }
            TaskStatus::Active(owner) => {
                return Err(format!(
                    "task is already claimed by session {owner}; ask for a handoff"
                ))
            }
            TaskStatus::Done => return Err("a completed task cannot be claimed".to_owned()),
        },
        EventKind::Reassigned => match status {
            TaskStatus::Done => return Err("a completed task cannot be reassigned".to_owned()),
            TaskStatus::Active(ref owner) if owner == session_id => {
                return Err("the new owner already owns this task".to_owned())
            }
            TaskStatus::Open | TaskStatus::Active(_) | TaskStatus::Review => {}
        },
        EventKind::HelpRequested | EventKind::HandedOff | EventKind::Completed => {
            if status.owner() != Some(session_id) {
                return Err("only the session currently owning this task can do that".to_owned());
            }
            if kind == EventKind::HelpRequested && task.needs_help() {
                return Err("this task already has an open help request".to_owned());
            }
        }
        EventKind::HelpReplied => {
            if !matches!(status, TaskStatus::Active(ref owner) if owner != session_id)
                || !task.needs_help()
            {
                return Err(
                    "a different session must have an open help request on this task".to_owned(),
                );
            }
        }
    }
    if kind == EventKind::HandedOff {
        let expected = match expected_revision {
            Some(revision) => revision.to_owned(),
            None => read_receipt(&root, session_id)?
                .and_then(|receipt| receipt.handoff_revisions.get(id).cloned())
                .ok_or_else(|| {
                    "the owning agent must fetch this task's shared context before a handoff; run 'verb shared read' in that session".to_owned()
                })?,
        };
        let current = revision_for_handoff(&read_memory(project)?, &task)?;
        if expected != current {
            return Err(format!(
                "this task or project memory changed since this handoff began (expected {expected}, current {current}); refresh shared context and retry"
            ));
        }
    }
    task.record(kind, session_id, note)?;
    write_task(&root, &task)?;
    Ok(format!(
        "Task {id}: {} by session {session_id}.",
        kind.label()
    ))
}

fn verify_agent_session(project: &Path, id: &str) -> Result<(), String> {
    let session = load_session_by_id(id)?.ok_or_else(|| format!("no Verb session with id {id}"))?;
    if !crate::session_in_project(&session, project) {
        return Err("that session belongs to another project".to_owned());
    }
    if session.agent.is_none() || session.state == SessionState::Ended {
        return Err("task actors must be agent sessions that have not ended".to_owned());
    }
    Ok(())
}

fn same_project(left: &Path, right: &Path) -> bool {
    crate::project::same_project(left, right)
}

fn read_memory(project: &Path) -> Result<String, String> {
    let path = project_root(project)?.join("memory.md");
    read_bounded(&path, MAX_MEMORY as u64).or_else(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            Ok(String::new())
        } else {
            Err(format!("could not read shared project memory: {error}"))
        }
    })
}

fn read_tasks(project: &Path) -> Result<Vec<Task>, String> {
    let identity = crate::project::identity(project)?;
    let root = identity.store;
    let directory = root.join("tasks");
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("could not read tasks: {error}")),
    };
    let mut tasks = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("could not read task entry: {error}"))?;
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let id = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| format!("invalid task filename: {}", path.display()))?;
        tasks.push(read_task_checked(
            &root,
            project,
            &identity.anchor,
            &identity.id,
            id,
        )?);
    }
    Ok(tasks)
}

fn read_task(root: &Path, project: &Path, id: &str) -> Result<Task, String> {
    let identity = crate::project::identity(project)?;
    read_task_checked(root, project, &identity.anchor, &identity.id, id)
}

fn read_task_checked(
    root: &Path,
    project: &Path,
    anchor: &Path,
    project_id: &str,
    id: &str,
) -> Result<Task, String> {
    let path = task_path(root, id)?;
    let bytes = read_bounded(&path, MAX_TASK_BYTES)
        .map_err(|error| format!("could not read task {id}: {error}"))?;
    let task: Task =
        serde_json::from_str(&bytes).map_err(|error| format!("invalid task {id}: {error}"))?;
    if task.schema_version != SCHEMA_VERSION
        || task.id != id
        || match task.verb_project_id.as_deref() {
            Some(recorded) => recorded != project_id,
            None => task.project_id != anchor && !same_project(&task.project_id, project),
        }
        || task.events.len() > MAX_EVENTS
    {
        return Err(format!(
            "task {id} has invalid identity, version, or history length"
        ));
    }
    Ok(task)
}

fn write_task(root: &Path, task: &Task) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(task).map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_TASK_BYTES {
        return Err(format!("task {} exceeds the storage limit", task.id));
    }
    atomic_write(&task_path(root, &task.id)?, &bytes)
}

fn task_path(root: &Path, id: &str) -> Result<PathBuf, String> {
    if id.len() != 32 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("invalid task id".to_owned());
    }
    Ok(root.join("tasks").join(format!("{id}.json")))
}

fn project_root(project: &Path) -> Result<PathBuf, String> {
    Ok(crate::project::identity(project)?.store)
}

pub(crate) fn memory_path(project: &Path) -> Result<PathBuf, String> {
    Ok(project_root(project)?.join("memory.md"))
}

fn lock_project(root: &Path) -> Result<File, String> {
    fs::create_dir_all(root)
        .map_err(|error| format!("could not create project work store: {error}"))?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root.join(".lock"))
        .map_err(|error| format!("could not open project work lock: {error}"))?;
    lock.lock()
        .map_err(|error| format!("could not lock project work store: {error}"))?;
    Ok(lock)
}

fn read_bounded(path: &Path, limit: u64) -> io::Result<String> {
    let mut bytes = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "record exceeds size limit",
        ));
    }
    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn read_input(source: &str, limit: usize) -> Result<String, String> {
    let mut bytes = Vec::new();
    if source == "-" {
        io::stdin()
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("could not read stdin: {error}"))?;
    } else {
        File::open(source)
            .map_err(|error| format!("could not open {source}: {error}"))?
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("could not read {source}: {error}"))?;
    }
    if bytes.len() > limit {
        return Err(format!("input exceeds {limit} bytes"));
    }
    String::from_utf8(bytes).map_err(|error| format!("input must be UTF-8: {error}"))
}
