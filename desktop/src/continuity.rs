//! Manual, evidence-only continuity between Verb hosts.
//!
//! Imported records are deliberately kept outside `sessions/`: a foreign file can contribute
//! history, never current state or a resume capability.

use crate::{event_log_path, iso8601, json_escape, load_session, state_root, Session};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Component, Path};
use std::process::Command;

const VERSION: u8 = 1;
const KIND: &str = "verb.continuity";
const MAX_BYTES: u64 = 5 * 1024 * 1024;
const MAX_LINE_BYTES: usize = 16 * 1024;
const MAX_SESSIONS: usize = 1_000;
const MAX_EVENTS: usize = 10_000;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Header {
    record_type: String,
    envelope_version: u8,
    kind: String,
    payload_sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Origin {
    record_type: String,
    host_id: String,
    host_kind: String,
    verb_version: String,
    exported_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProjectRecord {
    record_type: String,
    project_key: String,
    label: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SessionRecord {
    record_type: String,
    session_id: String,
    project_key: String,
    runtime_id: Option<String>,
    agent_type: Option<String>,
    resume_identity_ref: Option<String>,
    created_at: String,
    last_seen_at: String,
    last_observed_at: Option<String>,
    cwd_relative: Option<String>,
    recorded_state: String,
    recorded_state_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EventRecord {
    record_type: String,
    session_id: String,
    seq: u64,
    event_type: String,
    recorded_at: String,
    exit_code: Option<i64>,
    command_id: Option<String>,
    cwd_relative: Option<String>,
    state: Option<String>,
    tool: Option<String>,
    source: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawEvent {
    schema_version: u8,
    #[serde(default)]
    timestamp: serde_json::Value,
    // Logs written by the transitional writer used snake_case here amid camelCase siblings.
    #[serde(alias = "session_id")]
    session_id: String,
    seq: Option<u64>,
    #[serde(rename = "type")]
    event_type: String,
    project_id: Option<String>,
    runtime_id: Option<String>,
    agent_type: Option<String>,
    exit_code: Option<i64>,
    command_id: Option<String>,
    cwd: Option<String>,
    state: Option<String>,
    resolved_state: Option<String>,
    tool: Option<String>,
    source: Option<String>,
}

pub(crate) struct ImportedSession {
    pub host_id: String,
    pub host_kind: String,
    pub exported_at: String,
    pub session_id: String,
    pub project_label: String,
    pub runtime_id: Option<String>,
    pub recorded_state: String,
    pub recorded_state_at: String,
    pub last_seen_at: String,
}

struct ParsedEnvelope {
    header: Header,
    origin: Origin,
    projects: Vec<ProjectRecord>,
    sessions: Vec<SessionRecord>,
    events: Vec<EventRecord>,
    bytes: Vec<u8>,
}

pub(crate) fn command(project: &Path, mut args: Vec<String>) -> Result<(), String> {
    if args.is_empty() {
        return Err("usage: verb continuity export PATH | import PATH [--apply]".to_owned());
    }
    match args.remove(0).as_str() {
        "export" if args.len() == 1 => export(project, Path::new(&args[0])),
        "import" => {
            let apply = take_local_flag(&mut args, "--apply");
            if args.len() != 1 {
                return Err("usage: verb continuity import PATH [--apply]".to_owned());
            }
            import(Path::new(&args[0]), apply)
        }
        _ => Err("usage: verb continuity export PATH | import PATH [--apply]".to_owned()),
    }
}

fn export(project: &Path, destination: &Path) -> Result<(), String> {
    let host_id = load_or_create_host_id()?;
    let project_key = project_key(project);
    let project_record = ProjectRecord {
        record_type: "project".to_owned(),
        project_key: project_key.clone(),
        label: project
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("project")
            .chars()
            .take(128)
            .collect(),
    };
    let origin = Origin {
        record_type: "origin".to_owned(),
        host_id,
        host_kind: "desktop".to_owned(),
        verb_version: env!("CARGO_PKG_VERSION").to_owned(),
        exported_at: iso8601(crate::now_millis()),
    };

    let session = load_session(project)?;
    let session_record = session
        .as_ref()
        .map(|session| session_record(session, project, &project_key));
    let events = match session.as_ref() {
        Some(session) => export_events(project, session)?,
        None => Vec::new(),
    };

    let mut payload = String::new();
    push_record(&mut payload, &origin)?;
    push_record(&mut payload, &project_record)?;
    if let Some(record) = session_record.as_ref() {
        push_record(&mut payload, record)?;
    }
    for event in &events {
        push_record(&mut payload, event)?;
    }

    let checksum = sha256_hex(payload.as_bytes());
    let header = Header {
        record_type: "header".to_owned(),
        envelope_version: VERSION,
        kind: KIND.to_owned(),
        payload_sha256: checksum,
    };
    let mut bytes = serde_json::to_vec(&header).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    bytes.extend_from_slice(payload.as_bytes());
    atomic_write(destination, &bytes)?;
    println!(
        "Exported {} session and {} structural events to {}",
        usize::from(session_record.is_some()),
        events.len(),
        destination.display()
    );
    println!(
        "No transcript, command text, terminal stream, credential, or absolute path was included."
    );
    Ok(())
}

fn import(path: &Path, apply: bool) -> Result<(), String> {
    let envelope = parse_file(path)?;
    println!(
        "Recorded on another {} host ({}) at {}: {} project, {} session, {} events.",
        envelope.origin.host_kind,
        &envelope.origin.host_id[..8],
        envelope.origin.exported_at,
        envelope.projects.len(),
        envelope.sessions.len(),
        envelope.events.len()
    );
    println!("Recorded states are history only; this host has not confirmed any remote process or recovery.");
    if !apply {
        println!("Preview only. Re-run with --apply to retain this evidence read-only.");
        return Ok(());
    }

    let directory = state_root()?
        .join("imported")
        .join(&envelope.origin.host_id);
    fs::create_dir_all(&directory)
        .map_err(|error| format!("could not create imported evidence directory: {error}"))?;
    let destination = directory.join(format!("{}.vcont", envelope.header.payload_sha256));
    if destination.exists() {
        println!("This exact envelope was already imported; nothing changed.");
        return Ok(());
    }
    atomic_write(&destination, &envelope.bytes)?;
    println!("Imported read-only evidence. Local session records were not changed.");
    Ok(())
}

pub(crate) fn imported_sessions() -> Result<Vec<ImportedSession>, String> {
    let root = state_root()?.join("imported");
    let hosts = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("could not read imported evidence: {error}")),
    };
    let mut envelopes = Vec::new();
    for host in hosts.flatten().filter(|entry| entry.path().is_dir()) {
        let Ok(files) = fs::read_dir(host.path()) else {
            continue;
        };
        for file in files
            .flatten()
            .filter(|entry| entry.path().extension().is_some_and(|v| v == "vcont"))
        {
            if let Ok(envelope) = parse_file(&file.path()) {
                envelopes.push(envelope);
            }
        }
    }
    // Directory iteration order is unspecified. Read the newest origin observation first so a
    // later export supersedes an older record's summary fields without ever mutating either file.
    envelopes.sort_by(|left, right| right.origin.exported_at.cmp(&left.origin.exported_at));

    let mut seen = HashSet::new();
    let mut imported = Vec::new();
    for envelope in envelopes {
        for session in envelope.sessions {
            if !seen.insert((envelope.origin.host_id.clone(), session.session_id.clone())) {
                continue;
            }
            let label = envelope
                .projects
                .iter()
                .find(|project| project.project_key == session.project_key)
                .map(|project| project.label.clone())
                .unwrap_or_else(|| "unresolved project".to_owned());
            imported.push(ImportedSession {
                host_id: envelope.origin.host_id.clone(),
                host_kind: envelope.origin.host_kind.clone(),
                exported_at: envelope.origin.exported_at.clone(),
                session_id: session.session_id,
                project_label: label,
                runtime_id: session.runtime_id,
                recorded_state: session.recorded_state,
                recorded_state_at: session.recorded_state_at,
                last_seen_at: session.last_seen_at,
            });
        }
    }
    imported.sort_by(|left, right| right.last_seen_at.cmp(&left.last_seen_at));
    Ok(imported)
}

fn session_record(session: &Session, project: &Path, project_key: &str) -> SessionRecord {
    SessionRecord {
        record_type: "session".to_owned(),
        session_id: session.id.clone(),
        project_key: project_key.to_owned(),
        runtime_id: session.runtime_id.clone(),
        agent_type: session.agent.as_ref().map(|agent| agent.label().to_owned()),
        resume_identity_ref: session.resume_identity.clone(),
        created_at: iso8601(session.created_at),
        last_seen_at: iso8601(session.last_seen_at),
        last_observed_at: session.last_observed_at.map(iso8601),
        cwd_relative: session
            .last_known_cwd
            .as_deref()
            .and_then(|cwd| relative_path(project, cwd)),
        recorded_state: session.state.as_str().to_ascii_uppercase(),
        recorded_state_at: iso8601(session.last_seen_at),
    }
}

/// The envelope's event-state vocabulary is the uppercase shared set (the Android host validates
/// exactly those four values), while legacy local logs recorded them lowercase. Normalized here,
/// at the only place an event leaves this host.
fn envelope_event_state(raw: Option<String>, resolved: Option<String>) -> Option<String> {
    raw.or(resolved).map(|value| value.to_ascii_uppercase())
}

/// New event logs carry ISO-8601 timestamps; logs written before the shared timestamp format
/// used epoch milliseconds (the reader in `context.rs` has always taken both). The envelope's
/// `recorded_at` is a string, so legacy numbers are normalized on the way out instead of failing
/// the whole export.
fn normalized_timestamp(raw: serde_json::Value, position: usize) -> Result<String, String> {
    match raw {
        serde_json::Value::String(text) => Ok(text),
        serde_json::Value::Number(number) => number
            .as_u64()
            .map(|millis| iso8601(u128::from(millis)))
            .ok_or_else(|| format!("structural event {position} has an unusable timestamp")),
        _ => Err(format!(
            "structural event {position} has an unusable timestamp"
        )),
    }
}

fn export_events(project: &Path, session: &Session) -> Result<Vec<EventRecord>, String> {
    let path = event_log_path(project, &session.id)?;
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("could not read structural events: {error}")),
    };
    let mut records = Vec::new();
    for (index, line) in contents.lines().enumerate() {
        if line.len() > MAX_LINE_BYTES {
            return Err("a structural event exceeds the continuity line limit".to_owned());
        }
        let raw: RawEvent = serde_json::from_str(line)
            .map_err(|error| format!("structural event {} is invalid: {error}", index + 1))?;
        if raw.schema_version != 1
            || raw.session_id != session.id
            || !allowed_event(&raw.event_type)
        {
            return Err(format!(
                "structural event {} is outside the v1 contract",
                index + 1
            ));
        }
        let source = raw.source.unwrap_or_else(|| {
            if raw.event_type.starts_with("AGENT_TURN_")
                || raw.event_type.starts_with("AGENT_TOOL_")
            {
                "agentRecord".to_owned()
            } else if raw.event_type.starts_with("COMMAND_") {
                "shell".to_owned()
            } else {
                "verb".to_owned()
            }
        });
        let _ = (raw.project_id, raw.runtime_id, raw.agent_type);
        records.push(EventRecord {
            record_type: "event".to_owned(),
            session_id: raw.session_id,
            seq: raw.seq.unwrap_or(index as u64 + 1),
            event_type: raw.event_type,
            recorded_at: normalized_timestamp(raw.timestamp, index + 1)?,
            exit_code: raw.exit_code,
            command_id: raw.command_id.filter(|value| valid_opaque(value, 128)),
            cwd_relative: raw
                .cwd
                .as_deref()
                .and_then(|cwd| relative_path(project, Path::new(cwd))),
            state: envelope_event_state(raw.state, raw.resolved_state),
            tool: raw.tool.filter(|value| valid_display(value, 128)),
            source,
        });
    }
    Ok(records)
}

fn parse_file(path: &Path) -> Result<ParsedEnvelope, String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    if metadata.len() > MAX_BYTES {
        return Err("continuity file exceeds the 5 MiB limit".to_owned());
    }
    let bytes =
        fs::read(path).map_err(|error| format!("could not read {}: {error}", path.display()))?;
    let first_newline = bytes
        .iter()
        .position(|byte| *byte == b'\n')
        .ok_or("continuity header is incomplete")?;
    if first_newline > MAX_LINE_BYTES {
        return Err("continuity header exceeds the line limit".to_owned());
    }
    let header: Header = serde_json::from_slice(&bytes[..first_newline])
        .map_err(|error| format!("invalid continuity header: {error}"))?;
    if header.record_type != "header" || header.envelope_version != VERSION || header.kind != KIND {
        return Err(format!(
            "unsupported continuity envelope; expected {KIND} version {VERSION}"
        ));
    }
    let payload = &bytes[first_newline + 1..];
    if sha256_hex(payload) != header.payload_sha256 {
        return Err("continuity checksum does not match; nothing was imported".to_owned());
    }

    let text = std::str::from_utf8(payload).map_err(|_| "continuity payload is not UTF-8")?;
    let mut origin = None;
    let mut projects = Vec::new();
    let mut sessions = Vec::new();
    let mut events = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.len() > MAX_LINE_BYTES {
            return Err(format!(
                "continuity record {} exceeds the line limit",
                index + 2
            ));
        }
        let value: serde_json::Value = serde_json::from_str(line)
            .map_err(|error| format!("invalid continuity record {}: {error}", index + 2))?;
        let record_type = value
            .get("recordType")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("continuity record {} has no recordType", index + 2))?;
        match record_type {
            "origin"
                if origin.is_none()
                    && projects.is_empty()
                    && sessions.is_empty()
                    && events.is_empty() =>
            {
                origin = Some(serde_json::from_value(value).map_err(|error| error.to_string())?);
            }
            "project" => {
                projects.push(serde_json::from_value(value).map_err(|error| error.to_string())?)
            }
            "session" => {
                sessions.push(serde_json::from_value(value).map_err(|error| error.to_string())?)
            }
            "event" => {
                events.push(serde_json::from_value(value).map_err(|error| error.to_string())?)
            }
            _ => {
                return Err(format!(
                    "unexpected continuity record type or ordering at line {}",
                    index + 2
                ))
            }
        }
        if sessions.len() > MAX_SESSIONS || events.len() > MAX_EVENTS {
            return Err("continuity record count exceeds the v1 limit".to_owned());
        }
    }
    let origin: Origin = origin.ok_or("continuity payload has no origin record")?;
    validate(&header, &origin, &projects, &sessions, &events)?;
    Ok(ParsedEnvelope {
        header,
        origin,
        projects,
        sessions,
        events,
        bytes,
    })
}

fn validate(
    header: &Header,
    origin: &Origin,
    projects: &[ProjectRecord],
    sessions: &[SessionRecord],
    events: &[EventRecord],
) -> Result<(), String> {
    if origin.record_type != "origin"
        || !hex_id(&header.payload_sha256, 64)
        || !hex_id(&origin.host_id, 32)
        || !matches!(origin.host_kind.as_str(), "android" | "desktop")
        || !valid_display(&origin.verb_version, 64)
        || !valid_timestamp(&origin.exported_at)
    {
        return Err("continuity provenance is invalid".to_owned());
    }
    let project_keys: HashSet<&str> = projects
        .iter()
        .map(|project| project.project_key.as_str())
        .collect();
    if project_keys.len() != projects.len() {
        return Err("continuity contains a duplicate project identity".to_owned());
    }
    if projects.iter().any(|project| {
        project.record_type != "project"
            || !valid_project_key(&project.project_key)
            || !valid_display(&project.label, 128)
    }) {
        return Err("continuity project record is invalid".to_owned());
    }
    if sessions.iter().any(|session| {
        session.record_type != "session"
            || !valid_opaque(&session.session_id, 128)
            || !project_keys.contains(session.project_key.as_str())
            || session
                .runtime_id
                .as_deref()
                .is_some_and(|value| !valid_opaque(value, 64))
            || session
                .agent_type
                .as_deref()
                .is_some_and(|value| !valid_opaque(value, 64))
            || session
                .resume_identity_ref
                .as_deref()
                .is_some_and(|value| crate::valid_resume_identity(value).is_none())
            || !valid_timestamp(&session.created_at)
            || !valid_timestamp(&session.last_seen_at)
            || session
                .last_observed_at
                .as_deref()
                .is_some_and(|value| !valid_timestamp(value))
            || session
                .cwd_relative
                .as_deref()
                .is_some_and(|value| !valid_relative(value))
            || !matches!(
                session.recorded_state.as_str(),
                "LIVE" | "INTERRUPTED" | "RECOVERABLE" | "ENDED"
            )
            || !valid_timestamp(&session.recorded_state_at)
    }) {
        return Err("continuity session record is invalid".to_owned());
    }
    let session_ids: HashSet<&str> = sessions
        .iter()
        .map(|session| session.session_id.as_str())
        .collect();
    if session_ids.len() != sessions.len() {
        return Err("continuity contains a duplicate session identity".to_owned());
    }
    let mut identities = HashSet::new();
    let mut last_sequence = std::collections::HashMap::new();
    if events.iter().any(|event| {
        let previous = last_sequence.get(&event.session_id).copied().unwrap_or(0);
        last_sequence.insert(event.session_id.clone(), event.seq);
        event.record_type != "event"
            || !session_ids.contains(event.session_id.as_str())
            || event.seq == 0
            || event.seq <= previous
            || !identities.insert((event.session_id.as_str(), event.seq))
            || !allowed_event(&event.event_type)
            || !valid_timestamp(&event.recorded_at)
            || event
                .command_id
                .as_deref()
                .is_some_and(|value| !valid_opaque(value, 128))
            || event
                .cwd_relative
                .as_deref()
                .is_some_and(|value| !valid_relative(value))
            || event.state.as_deref().is_some_and(|value| {
                !matches!(
                    value,
                    "LIVE"
                        | "INTERRUPTED"
                        | "RECOVERABLE"
                        | "ENDED"
                        | "live"
                        | "interrupted"
                        | "recoverable"
                        | "ended"
                )
            })
            || event
                .tool
                .as_deref()
                .is_some_and(|value| !valid_display(value, 128))
            || !matches!(event.source.as_str(), "shell" | "agentRecord" | "verb")
    }) {
        return Err(
            "continuity event record is invalid or conflicts with another event".to_owned(),
        );
    }
    Ok(())
}

fn load_or_create_host_id() -> Result<String, String> {
    let path = state_root()?.join("host_id");
    if let Ok(value) = fs::read_to_string(&path) {
        let value = value.trim();
        if hex_id(value, 32) {
            return Ok(value.to_owned());
        }
        return Err(
            "Verb host_id is invalid; remove it explicitly to reset continuity identity".to_owned(),
        );
    }
    let id = crate::new_id();
    atomic_write(&path, format!("{id}\n").as_bytes())?;
    Ok(id)
}

fn project_key(project: &Path) -> String {
    let output = Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .current_dir(project)
        .output();
    let remote = output
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|value| normalize_remote(value.trim()));
    remote.map_or_else(|| "unresolved".to_owned(), |value| format!("git:{value}"))
}

fn normalize_remote(value: &str) -> Option<String> {
    if value.is_empty() || value.chars().any(char::is_control) {
        return None;
    }
    let without_scheme = value.split_once("://").map_or(value, |(_, rest)| rest);
    let without_user = without_scheme
        .rsplit_once('@')
        .map_or(without_scheme, |(_, rest)| rest);
    let mut normalized = if !value.contains("://") && without_user.contains(':') {
        without_user.replacen(':', "/", 1)
    } else {
        without_user.to_owned()
    };
    normalized = normalized
        .split(['?', '#'])
        .next()?
        .trim_matches('/')
        .to_owned();
    if let Some((host, path)) = normalized.split_once('/') {
        let host = host.split(':').next().unwrap_or(host).to_ascii_lowercase();
        normalized = format!("{host}/{}", path.trim_end_matches(".git"));
    }
    valid_project_key(&format!("git:{normalized}")).then_some(normalized)
}

fn relative_path(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    let value = relative.to_string_lossy().replace('\\', "/");
    valid_relative(&value).then_some(value)
}

fn valid_relative(value: &str) -> bool {
    if value.len() > 512 || value.contains('\0') || Path::new(value).is_absolute() {
        return false;
    }
    Path::new(value)
        .components()
        .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

fn valid_project_key(value: &str) -> bool {
    value == "unresolved"
        || (value.starts_with("git:")
            && value.len() <= 512
            && !value.chars().any(char::is_control)
            && !value.contains(".."))
}

fn valid_opaque(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | ':' | '-')
        })
}

fn valid_display(value: &str, max: usize) -> bool {
    !value.is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}

fn valid_timestamp(value: &str) -> bool {
    value.len() == 20 && value.ends_with('Z') && value.as_bytes().get(10) == Some(&b'T')
}

fn hex_id(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn allowed_event(value: &str) -> bool {
    matches!(
        value,
        "SESSION_STARTED"
            | "SESSION_STATE_CHANGED"
            | "PROCESS_STARTED"
            | "PROCESS_ENDED"
            | "AGENT_STARTED"
            | "AGENT_ENDED"
            | "COMMAND_STARTED"
            | "COMMAND_FINISHED"
            | "CWD_CHANGED"
            | "RUNTIME_CHANGED"
            | "RECOVERY_CHECKED"
            | "SESSION_ENDED"
            | "AGENT_TURN_STARTED"
            | "AGENT_TOOL_CALLED"
            | "AGENT_TOOL_SUCCEEDED"
            | "AGENT_TOOL_FAILED"
            | "AGENT_TURN_FINISHED"
    )
}

fn push_record<T: Serialize>(payload: &mut String, record: &T) -> Result<(), String> {
    let line = serde_json::to_string(record).map_err(|error| error.to_string())?;
    if line.len() > MAX_LINE_BYTES {
        return Err("continuity record exceeds the line limit".to_owned());
    }
    payload.push_str(&line);
    payload.push('\n');
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn take_local_flag(args: &mut Vec<String>, flag: &str) -> bool {
    args.iter()
        .position(|value| value == flag)
        .map(|index| args.remove(index))
        .is_some()
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or("invalid continuity path")?;
    let temporary = parent.join(format!(".{name}.tmp"));
    let mut file = File::create(&temporary)
        .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("could not finish {}: {error}", temporary.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
    }
    fs::rename(&temporary, path)
        .map_err(|error| format!("could not commit {}: {error}", path.display()))
}

pub(crate) fn imported_session_json(session: &ImportedSession) -> String {
    format!(
        "{{\"schemaVersion\":1,\"sessionId\":\"{}\",\"projectId\":null,\"projectLabel\":\"{}\",\"runtimeId\":{},\"lastKnownCwd\":null,\"lastObservedAt\":null,\"createdAt\":null,\"lastSeenAt\":\"{}\",\"state\":\"INTERRUPTED\",\"agent\":null,\"recordedElsewhere\":{{\"hostId\":\"{}\",\"hostKind\":\"{}\",\"exportedAt\":\"{}\",\"recordedState\":\"{}\",\"recordedStateAt\":\"{}\"}}}}",
        json_escape(&session.session_id),
        json_escape(&session.project_label),
        session.runtime_id.as_deref().map(|value| format!("\"{}\"", json_escape(value))).unwrap_or_else(|| "null".to_owned()),
        json_escape(&session.last_seen_at),
        json_escape(&session.host_id),
        json_escape(&session.host_kind),
        json_escape(&session.exported_at),
        json_escape(&session.recorded_state),
        json_escape(&session.recorded_state_at),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_normalization_removes_credentials_scheme_and_suffix() {
        assert_eq!(
            normalize_remote("git@GitHub.com:me/app.git"),
            Some("github.com/me/app".to_owned())
        );
        assert_eq!(
            normalize_remote("https://token@GitHub.com/me/app.git"),
            Some("github.com/me/app".to_owned())
        );
    }

    #[test]
    fn traversal_is_never_a_relative_cwd() {
        assert!(!valid_relative("../secret"));
        assert!(!valid_relative("/absolute"));
        assert!(valid_relative("packages/api"));
        assert!(valid_relative(""));
    }

    #[test]
    fn resume_identity_uses_the_same_closed_vocabulary_as_local_sessions() {
        assert!(crate::valid_resume_identity("abc-123:turn").is_some());
        assert!(crate::valid_resume_identity("--resume").is_none());
        assert!(crate::valid_resume_identity("x;touch-pwned").is_none());
    }

    #[test]
    fn legacy_millisecond_timestamps_normalize_into_the_envelope() {
        let normalized = normalized_timestamp(
            serde_json::from_str("1787392941473").unwrap(),
            1,
        )
        .unwrap();
        assert_eq!(
            normalized,
            crate::iso8601(1_787_392_941_473),
            "a legacy integer timestamp must export as the same instant iso8601 produces"
        );
        assert!(normalized.ends_with('Z'));
    }

    #[test]
    fn string_timestamps_pass_through_untouched() {
        let raw = serde_json::Value::String("2026-08-22T10:02:55Z".to_owned());
        assert_eq!(
            normalized_timestamp(raw, 1).unwrap(),
            "2026-08-22T10:02:55Z"
        );
    }

    #[test]
    fn unusable_timestamps_name_the_offending_event() {
        let raw = serde_json::Value::Bool(true);
        assert_eq!(
            normalized_timestamp(raw, 7).unwrap_err(),
            "structural event 7 has an unusable timestamp"
        );
    }

    #[test]
    fn event_states_leave_the_host_in_the_uppercase_shared_vocabulary() {
        assert_eq!(
            envelope_event_state(Some("recoverable".to_owned()), None),
            Some("RECOVERABLE".to_owned())
        );
        assert_eq!(
            envelope_event_state(None, Some("ended".to_owned())),
            Some("ENDED".to_owned()),
            "the resolved state is the fallback"
        );
        assert_eq!(envelope_event_state(None, None), None);
    }
}
