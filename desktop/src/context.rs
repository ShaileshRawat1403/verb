//! What Verb knows about a moment, gathered into one place.
//!
//! This is the layer every M2 variant needs and none of them is: explanation, comparison and guided
//! action all begin by assembling the evidence, and only then differ in what they do with it. So the
//! assembly exists first, on its own, with no model behind it and nothing interpreting anything.
//!
//! Three properties make it useful rather than merely tidy:
//!
//! * **Everything here was observed.** The project and Git state are read at the moment of asking;
//!   the session record and the structural events are what Verb wrote down when they happened.
//!   Nothing is inferred, and there is no field for a conclusion.
//! * **It is an evidence bundle, not a snapshot of one moment.** Three different times are mixed in
//!   here on purpose, and each is labelled: Git state observed *now*, a session record reconciled
//!   *now* from evidence written earlier, and events recorded *when they happened*. Presented
//!   flat, "Git: main · 0 changed" sitting above "COMMAND_FINISHED · exit 1" reads as *the tree was
//!   clean when the command failed* -- which Verb does not know and never claimed. A later reader,
//!   human or model, must not be able to make that mistake from the output alone.
//! * **It carries no command text, terminal output, prompts or credentials** -- not by filtering
//!   them out, but because the durable sources it reads never contained them. See
//!   `docs/VERB_SESSION_SCHEMA.md`.
//!
//! `verb context` prints it. The name is deliberate: it assembles context and explains nothing, and
//! a command that promised explanation while doing none would be exactly the kind of thing Verb
//! exists to stop happening to people.

use crate::json::{json_number, json_string};
use crate::{GitSnapshot, Session};
use std::fs;
use std::path::{Path, PathBuf};

/// One structural fact from the event log, as it was recorded.
pub(crate) struct Event {
    pub kind: String,
    pub timestamp: u128,
    pub exit_code: Option<i64>,
    pub command_id: Option<String>,
    pub cwd: Option<String>,
    pub state: Option<String>,
    /// The tool an agent's record named. A name only -- never its arguments or its output.
    pub tool: Option<String>,
}

/// Everything Verb can currently say about a project, without being asked to interpret it.
pub(crate) struct Context {
    pub project: PathBuf,
    pub git: GitSnapshot,
    pub session: Option<Session>,
    pub events: Vec<Event>,
    /// When the live half of this bundle was read. Everything under "observed now" is true as of
    /// this instant and of no other; everything else carries its own recorded time.
    pub assembled_at: u128,
}

/// How many recent events are worth carrying. Enough to cover the last few commands and the state
/// transitions around them; not so many that the answer becomes a log dump.
const RECENT_EVENTS: usize = 20;

pub(crate) fn assemble(project: &Path) -> Result<Context, String> {
    assemble_for(project, None)
}

/// As above, but told which session the caller is currently hosting.
///
/// Reconciling a record whose process this very program is running would resolve it from disk
/// evidence -- correctly "nothing to recover" for a shell -- and report a session as ended while it
/// is running in front of the user. The host holding the binding is the authority for that one
/// record.
pub(crate) fn assemble_for(project: &Path, hosting: Option<&Session>) -> Result<Context, String> {
    let session = match crate::load_session(project)? {
        Some(record) if Some(record.id.as_str()) == hosting.map(|live| live.id.as_str()) => {
            hosting.cloned()
        }
        Some(record) => Some(crate::reconcile_session(record)?),
        None => hosting.cloned(),
    };
    let events = match session.as_ref() {
        Some(session) => read_events(project, &session.id)?,
        None => Vec::new(),
    };
    Ok(Context {
        project: project.to_path_buf(),
        git: crate::git_snapshot(project),
        session,
        events,
        assembled_at: crate::now_millis(),
    })
}

/// The tail of one session's structural event log.
fn read_events(project: &Path, session_id: &str) -> Result<Vec<Event>, String> {
    let path = crate::event_log_path(project, session_id)?;
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("could not read {}: {error}", path.display())),
    };

    let lines: Vec<&str> = contents.lines().collect();
    let start = lines.len().saturating_sub(RECENT_EVENTS);
    Ok(lines[start..]
        .iter()
        .filter_map(|line| {
            Some(Event {
                kind: json_string(line, "type")?,
                timestamp: json_number(line, "timestamp").unwrap_or(0),
                // Read field by field, so a log that somehow grew a field Verb does not know about
                // is ignored rather than carried along.
                exit_code: json_integer(line, "exitCode"),
                command_id: json_string(line, "commandId"),
                cwd: json_string(line, "cwd"),
                state: json_string(line, "state").or_else(|| json_string(line, "resolvedState")),
                tool: json_string(line, "tool"),
            })
        })
        .collect())
}

/// Exit codes can be negative, which the unsigned reader cannot express.
fn json_integer(input: &str, key: &str) -> Option<i64> {
    let needle = format!("\"{key}\":");
    let index = input.find(&needle)?;
    let after = input[index + needle.len()..].trim_start();
    let negative = after.starts_with('-');
    let digits: String = after
        .trim_start_matches('-')
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    let value: i64 = digits.parse().ok()?;
    Some(if negative { -value } else { value })
}

impl Context {
    /// The same field names the durable schema uses, so a consumer of this and a consumer of
    /// `verb status --json` are reading one vocabulary.
    pub(crate) fn to_json(&self) -> String {
        let session = match self.session.as_ref() {
            Some(session) => crate::session_json(session),
            None => "null".to_owned(),
        };
        let events: Vec<String> = self.events.iter().map(Event::to_json).collect();
        // `observedNow` names what it is: a reading taken at `assembledAt` and true of no other
        // moment. The session record and the events keep their own times, so nothing here can be
        // mistaken for state-at-event.
        format!(
            "{{\"schemaVersion\":1,\"assembledAt\":\"{}\",\"observedNow\":{{\"path\":\"{}\",\"gitRoot\":{},\"branch\":{},\"changedFiles\":{}}},\"session\":{},\"recentEvents\":[{}]}}",
            crate::iso8601(self.assembled_at),
            crate::json_escape(&self.project.to_string_lossy()),
            self.git
                .root
                .as_ref()
                .map(|root| format!("\"{}\"", crate::json_escape(&root.to_string_lossy())))
                .unwrap_or_else(|| "null".to_owned()),
            self.git
                .branch
                .as_ref()
                .map(|branch| format!("\"{}\"", crate::json_escape(branch)))
                .unwrap_or_else(|| "null".to_owned()),
            self.git.changed_files,
            session,
            events.join(",")
        )
    }

    /// The same facts for a person rather than a program.
    pub(crate) fn to_text(&self) -> String {
        // Grouped by *when each fact was true*, not by subject. Flat output invites a reader to
        // treat a reading taken now as the state at the time of an event recorded earlier.
        let mut lines = vec![format!(
            "Observed now ({})",
            crate::iso8601(self.assembled_at)
        )];
        lines.push(format!("  Project: {}", self.project.display()));
        match self.git.root {
            Some(_) => lines.push(format!(
                "  Git: {} · {} changed",
                self.git.branch.as_deref().unwrap_or("detached/unknown"),
                self.git.changed_files
            )),
            None => lines.push("  Git: not a repository".to_owned()),
        }

        lines.push(String::new());
        lines.push("Recorded state".to_owned());
        match self.session.as_ref() {
            Some(session) => {
                lines.push(format!(
                    "  Session: {} ({}) · {}",
                    session.runtime_id.as_deref().unwrap_or("shell"),
                    session.state.as_str(),
                    session.id
                ));
                lines.push(format!(
                    "  Last seen: {}",
                    crate::iso8601(session.last_seen_at)
                ));
                if let Some(identity) = session.resume_identity.as_deref() {
                    lines.push(format!("  Agent conversation: {identity}"));
                }
            }
            None => lines.push("  Session: none".to_owned()),
        }

        lines.push(String::new());
        if self.events.is_empty() {
            lines.push("Recorded events: none".to_owned());
        } else {
            lines.push("Recorded events (each true when it was written)".to_owned());
            for event in &self.events {
                lines.push(format!("  {}", event.to_text()));
            }
        }

        // Said plainly rather than left to be discovered: these are the boundaries of what Verb can
        // answer from, and anything beyond them would be a guess.
        lines.push(String::new());
        lines.push(
            "Verb records structural facts only: no command text, terminal output, prompts or credentials."
                .to_owned(),
        );
        lines.push(
            "This is an evidence bundle, not a snapshot: what was observed now was not observed then."
                .to_owned(),
        );
        lines.join("\n")
    }
}

impl Event {
    fn to_json(&self) -> String {
        let mut fields = vec![
            format!("\"type\":\"{}\"", crate::json_escape(&self.kind)),
            format!("\"timestamp\":\"{}\"", crate::iso8601(self.timestamp)),
        ];
        if let Some(exit_code) = self.exit_code {
            fields.push(format!("\"exitCode\":{exit_code}"));
        }
        if let Some(command_id) = self.command_id.as_deref() {
            fields.push(format!(
                "\"commandId\":\"{}\"",
                crate::json_escape(command_id)
            ));
        }
        if let Some(cwd) = self.cwd.as_deref() {
            fields.push(format!("\"cwd\":\"{}\"", crate::json_escape(cwd)));
        }
        if let Some(state) = self.state.as_deref() {
            fields.push(format!("\"state\":\"{}\"", crate::json_escape(state)));
        }
        format!("{{{}}}", fields.join(","))
    }

    fn to_text(&self) -> String {
        let mut line = format!("{}  {}", crate::iso8601(self.timestamp), self.kind);
        if let Some(exit_code) = self.exit_code {
            line.push_str(&format!(" · exit {exit_code}"));
        }
        if let Some(state) = self.state.as_deref() {
            line.push_str(&format!(" · {state}"));
        }
        if let Some(tool) = self.tool.as_deref() {
            line.push_str(&format!(" · {tool}"));
        }
        if let Some(cwd) = self.cwd.as_deref() {
            line.push_str(&format!(" · {cwd}"));
        }
        line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle() -> Context {
        Context {
            project: PathBuf::from("/tmp/project"),
            git: GitSnapshot {
                root: Some(PathBuf::from("/tmp/project")),
                branch: Some("main".to_owned()),
                changed_files: 0,
            },
            session: None,
            events: vec![Event {
                kind: "COMMAND_FINISHED".to_owned(),
                timestamp: 1_787_320_092_493,
                exit_code: Some(1),
                command_id: Some("c1".to_owned()),
                cwd: None,
                state: None,
                tool: None,
            }],
            assembled_at: 1_787_334_976_000,
        }
    }

    #[test]
    fn a_reading_taken_now_is_never_presented_as_the_state_at_an_event() {
        // The hazard this guards: "Git: main · 0 changed" printed above "COMMAND_FINISHED · exit 1"
        // reads as *the tree was clean when it failed*, which Verb does not know. Each fact carries
        // the time it was true.
        let bundle = bundle();

        let text = bundle.to_text();
        assert!(text.contains("Observed now (2026-08-21T17:56"), "{text}");
        assert!(text.contains("Recorded events"), "{text}");
        assert!(text.contains("evidence bundle, not a snapshot"), "{text}");
        // The Git reading appears under the live heading, above anything recorded earlier.
        let observed = text.find("Observed now").unwrap();
        let recorded = text.find("Recorded events").unwrap();
        assert!(text[observed..recorded].contains("Git: main"), "{text}");

        let json = bundle.to_json();
        assert!(
            json.contains("\"assembledAt\":\"2026-08-21T17:56"),
            "{json}"
        );
        assert!(json.contains("\"observedNow\":{"), "{json}");
        // The event keeps its own, earlier time rather than borrowing the bundle's.
        assert!(json.contains("\"timestamp\":\"2026-08-21T13:48"), "{json}");
    }

    #[test]
    fn exit_codes_survive_being_negative() {
        assert_eq!(json_integer(r#"{"exitCode":-9}"#, "exitCode"), Some(-9));
        assert_eq!(json_integer(r#"{"exitCode":127}"#, "exitCode"), Some(127));
        assert_eq!(json_integer(r#"{"other":1}"#, "exitCode"), None);
    }

    #[test]
    fn an_event_carries_only_the_fields_verb_records() {
        // A log line that somehow contained command text must not smuggle it through: the reader
        // asks for known fields rather than copying whatever it finds.
        let line = r#"{"type":"COMMAND_FINISHED","timestamp":1787320092493,"exitCode":1,"commandId":"c1","commandText":"rm -rf /secret"}"#;
        let event = Event {
            kind: json_string(line, "type").unwrap(),
            timestamp: json_number(line, "timestamp").unwrap(),
            exit_code: json_integer(line, "exitCode"),
            command_id: json_string(line, "commandId"),
            cwd: json_string(line, "cwd"),
            state: json_string(line, "state"),
            tool: json_string(line, "tool"),
        };

        let json = event.to_json();
        assert!(json.contains("\"exitCode\":1"), "{json}");
        assert!(!json.contains("commandText"), "{json}");
        assert!(!json.contains("secret"), "{json}");
        assert!(!event.to_text().contains("secret"));
    }
}
