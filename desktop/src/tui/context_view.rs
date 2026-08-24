//! Rendering `verb context` inside the workspace.
//!
//! The overlay shows exactly what the command prints, grouped the same way and carrying the same
//! caveats, because it is the same assembly: `context::assemble`, rendered rather than reinterpreted.
//! Nothing here adds a conclusion, and the time boundary from `docs/VERB_SESSION_SCHEMA.md` survives
//! the trip to the screen -- what was observed now sits under its own heading, above what was
//! recorded earlier.

use crate::context::Context;

/// One line of the overlay, and how it should read.
pub(super) enum Kind {
    Heading,
    Fact,
    Caveat,
    Empty,
}

pub(super) struct EvidenceLines {
    pub lines: Vec<(Kind, String)>,
}

impl EvidenceLines {
    /// `observed` is what the hosted agent's own record has reported so far, when Verb is hosting an
    /// agent it knows how to read. `None` means it is not reading one -- a shell, or an agent with
    /// no reader -- and that is rendered as unobserved rather than as nothing having happened.
    pub(super) fn build_with(
        context: &Context,
        now: u128,
        observed: Option<&crate::observe::Observed>,
    ) -> Self {
        let mut lines = Vec::new();

        lines.push((
            Kind::Heading,
            format!("Observed now · {}", clock(context.assembled_at)),
        ));
        lines.push((
            Kind::Fact,
            format!("  {}", crate::display_path(&context.project)),
        ));
        lines.push((
            Kind::Fact,
            match context.git.root {
                Some(_) => format!(
                    "  {} · {} changed",
                    context.git.branch.as_deref().unwrap_or("detached"),
                    context.git.changed_files
                ),
                None => "  not a Git repository".to_owned(),
            },
        ));

        lines.push((Kind::Empty, String::new()));
        lines.push((Kind::Heading, "Recorded state".to_owned()));
        match context.session.as_ref() {
            Some(session) => {
                lines.push((
                    Kind::Fact,
                    format!(
                        "  {} · {} · last seen {}",
                        session.runtime_id.as_deref().unwrap_or("shell"),
                        super::render::plain_state(&session.state),
                        crate::relative_time(now.saturating_sub(session.last_seen_at))
                    ),
                ));
                if let Some(identity) = session.resume_identity.as_deref() {
                    lines.push((Kind::Fact, format!("  conversation {identity}")));
                }
            }
            None => lines.push((Kind::Fact, "  no session recorded here".to_owned())),
        }

        if let Some(observed) = observed {
            lines.push((Kind::Empty, String::new()));
            lines.push((
                Kind::Heading,
                "Agent activity · from its own record".to_owned(),
            ));
            if observed.is_empty() {
                lines.push((
                    Kind::Fact,
                    "  nothing recorded yet — not the same as nothing happening".to_owned(),
                ));
            } else {
                lines.push((
                    Kind::Fact,
                    format!(
                        "  {} turns · {} tool calls · {} failed",
                        observed.turns, observed.tools_called, observed.tools_failed
                    ),
                ));
                if let Some(at) = observed.last_failure_at {
                    lines.push((Kind::Fact, format!("  last failure {}", clock(at))));
                }
            }
        }

        lines.push((Kind::Empty, String::new()));
        if context.events.is_empty() {
            lines.push((Kind::Heading, "Recorded events · none yet".to_owned()));
        } else {
            lines.push((
                Kind::Heading,
                "Recorded events · each true when written".to_owned(),
            ));
            // Newest first: the thing that just happened is the thing being asked about.
            for event in context.events.iter().rev() {
                lines.push((Kind::Fact, format!("  {}", describe(event))));
            }
        }

        lines.push((Kind::Empty, String::new()));
        lines.push((
            Kind::Caveat,
            "Structural facts only — no command text, output, prompts or credentials.".to_owned(),
        ));
        if context
            .events
            .iter()
            .any(|event| event.kind.starts_with("AGENT_TO") || event.kind.starts_with("AGENT_TURN"))
        {
            lines.push((
                Kind::Caveat,
                "Agent lines are what the agent recorded, not what Verb watched run.".to_owned(),
            ));
        }
        lines.push((
            Kind::Caveat,
            "What was observed now was not observed then.".to_owned(),
        ));

        Self { lines }
    }
}

/// Plain words for what the event log records in capitals.
fn describe(event: &crate::context::Event) -> String {
    let subject = match event.kind.as_str() {
        "SESSION_STARTED" => "session started".to_owned(),
        "SESSION_ENDED" => "session ended".to_owned(),
        "SESSION_STATE_CHANGED" => "state changed".to_owned(),
        "PROCESS_STARTED" => "process started".to_owned(),
        "PROCESS_ENDED" => "process ended".to_owned(),
        "COMMAND_STARTED" => "command started".to_owned(),
        "COMMAND_FINISHED" => "command finished".to_owned(),
        "CWD_CHANGED" => "directory changed".to_owned(),
        "RECOVERY_CHECKED" => "recovery checked".to_owned(),
        "AGENT_STARTED" => "agent started".to_owned(),
        "AGENT_ENDED" => "agent ended".to_owned(),
        // Read from the agent's own record, so the words say who reported it. "the agent ran" would
        // claim Verb watched it happen, which is the one thing these events must never imply.
        "AGENT_TURN_STARTED" => "agent turn started".to_owned(),
        "AGENT_TURN_FINISHED" => "agent turn finished".to_owned(),
        "AGENT_TOOL_CALLED" => "agent called a tool".to_owned(),
        "AGENT_TOOL_SUCCEEDED" => "agent reported a tool succeeded".to_owned(),
        "AGENT_TOOL_FAILED" => "agent reported a tool failed".to_owned(),
        other => other.to_lowercase().replace('_', " "),
    };

    let mut line = format!("{}  {subject}", clock(event.timestamp));
    if let Some(exit_code) = event.exit_code {
        line.push_str(&format!(" · exit {exit_code}"));
    }
    if let Some(state) = event.state.as_deref() {
        line.push_str(&format!(" · {state}"));
    }
    if let Some(tool) = event.tool.as_deref() {
        line.push_str(&format!(" · {tool}"));
    }
    if let Some(cwd) = event.cwd.as_deref() {
        line.push_str(&format!(
            " · {}",
            crate::display_path(std::path::Path::new(cwd))
        ));
    }
    line
}

/// Wall-clock time, which is what a person reads off a timeline. The full ISO timestamp is in
/// `verb context --json` for anything that needs to be precise.
fn clock(millis: u128) -> String {
    let iso = crate::iso8601(millis);
    iso.get(11..19).unwrap_or(&iso).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Event;
    use crate::GitSnapshot;
    use std::path::PathBuf;

    fn context() -> Context {
        Context {
            project: PathBuf::from("/tmp/project"),
            git: GitSnapshot {
                root: Some(PathBuf::from("/tmp/project")),
                branch: Some("main".to_owned()),
                changed_files: 4,
            },
            session: None,
            events: vec![Event {
                seq: Some(1),
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
    fn the_overlay_keeps_the_time_boundary_the_schema_requires() {
        let built = EvidenceLines::build_with(&context(), 1_787_334_976_000, None);
        let text: Vec<&str> = built.lines.iter().map(|(_, line)| line.as_str()).collect();

        let observed = text
            .iter()
            .position(|line| line.starts_with("Observed now"));
        let recorded = text
            .iter()
            .position(|line| line.starts_with("Recorded events"));
        assert!(observed < recorded, "{text:?}");
        assert!(text.iter().any(|line| line.contains("main · 4 changed")));
        assert!(text
            .iter()
            .any(|line| line.contains("What was observed now was not observed then.")));
    }

    #[test]
    fn events_read_as_words_and_newest_first() {
        let built = EvidenceLines::build_with(&context(), 1_787_334_976_000, None);
        let text: Vec<&str> = built.lines.iter().map(|(_, line)| line.as_str()).collect();

        assert!(
            text.iter()
                .any(|line| line.contains("command finished · exit 1")),
            "{text:?}"
        );
        // Not the capitalised log token, which is a machine's word for it.
        assert!(!text.iter().any(|line| line.contains("COMMAND_FINISHED")));
    }
}
