//! What an agent did, read from the record the agent itself keeps.
//!
//! Verb hosts a PTY, so inside `claude` or `codex` it can see a process running and, eventually, an
//! exit code -- and nothing else. Every tool the agent ran, every one that failed, every turn it
//! took, happened inside that process where Verb's shell integration cannot reach. `docs/BACKLOG.md`
//! records this as C1: the failure band is silent inside an agent session, which is most of what a
//! person wants explained.
//!
//! Both agents already write a structured record of their own work: Claude Code under
//! `~/.claude/projects/<project>/<session>.jsonl`, Codex under `~/.codex/sessions/.../rollout-*.jsonl`.
//! That record is the agent's, not Verb's. Verb reads it the way it reads everything else: to learn
//! *structure*, never content.
//!
//! # What is read, and what is not
//!
//! Only these scalar fields, by name: `type`, `name`, `is_error`, `status`, `timestamp`. Prompts,
//! messages, tool arguments, tool output, file contents and diffs are never read, so they can never
//! be written into a Verb event, shown in an overlay, or handed to an assistant.
//!
//! That is enforced by construction rather than by discipline. `crate::json` reads a named scalar
//! and nothing else; there is no parse step here that produces a document a later change could
//! start reaching into. Its own doc comment says it: anything that needed real parsing would be a
//! sign Verb was reading something it should not be.
//!
//! # What the events mean
//!
//! A tool *call* is the agent saying it intends to run something. A tool *outcome* is the agent's
//! record of what came back. Neither is Verb watching the command run -- it did not, and
//! `docs/VERB_SESSION_CONTRACT.md` is explicit that an agent's claim is not verified execution. So
//! these are reported as what they are: observed from the agent's record.

use crate::json::{json_number, json_string};

/// One structural thing an agent did, as its own record describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentEvent {
    /// The agent began working on something.
    TurnStarted { at: u128 },
    /// The agent asked to run a tool, named but never described.
    ToolCalled { at: u128, tool: String },
    /// The agent recorded how a tool call came back.
    ToolOutcome {
        at: u128,
        tool: Option<String>,
        failed: bool,
    },
    /// The agent finished and handed the turn back.
    TurnFinished { at: u128 },
}

impl AgentEvent {
    pub fn at(&self) -> u128 {
        match self {
            AgentEvent::TurnStarted { at }
            | AgentEvent::ToolCalled { at, .. }
            | AgentEvent::ToolOutcome { at, .. }
            | AgentEvent::TurnFinished { at } => *at,
        }
    }

    /// The name Verb writes into its own event log.
    pub fn kind(&self) -> &'static str {
        match self {
            AgentEvent::TurnStarted { .. } => "AGENT_TURN_STARTED",
            AgentEvent::ToolCalled { .. } => "AGENT_TOOL_CALLED",
            AgentEvent::ToolOutcome { failed: false, .. } => "AGENT_TOOL_SUCCEEDED",
            AgentEvent::ToolOutcome { failed: true, .. } => "AGENT_TOOL_FAILED",
            AgentEvent::TurnFinished { .. } => "AGENT_TURN_FINISHED",
        }
    }
}

/// Which agent's record a line came from. The two write different shapes for the same facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Record {
    Claude,
    Codex,
}

impl Record {
    /// The structural events one line of the record describes, if it describes any.
    ///
    /// A line Verb does not understand yields nothing rather than a guess. Both agents version
    /// their own formats independently of Verb, and inventing an event from an unrecognised shape
    /// would be exactly the inference the contract forbids.
    pub fn events(&self, line: &str, fallback_at: u128) -> Vec<AgentEvent> {
        match self {
            Record::Claude => claude_events(line, fallback_at),
            Record::Codex => codex_events(line, fallback_at),
        }
    }
}

/// Claude Code writes one record per message. A turn is a `user` record answered by an `assistant`
/// record; tool calls appear as `tool_use` blocks inside an assistant message, and their outcomes as
/// `tool_result` blocks carrying `is_error` in the next user record.
///
/// The record kind is found by looking for the exact top-level marker rather than by reading the
/// first `"type"` in the line: the message object carries `"type":"message"` of its own, and it can
/// appear first. That mistake is why the first run of this against a real transcript saw 821 tool
/// calls as none.
fn claude_events(line: &str, fallback_at: u128) -> Vec<AgentEvent> {
    let at = timestamp(line, fallback_at);
    let is_user = line.contains(r#""type":"user""#);
    let is_assistant = line.contains(r#""type":"assistant""#);
    if !is_user && !is_assistant {
        return Vec::new();
    }

    let mut events = Vec::new();
    let calls = blocks(line, r#""type":"tool_use""#);
    let results = blocks(line, r#""type":"tool_result""#);

    if is_assistant {
        for block in &calls {
            // The name is read from inside that block, so it cannot be picked up from a neighbour.
            if let Some(tool) = json_string(block, "name") {
                events.push(AgentEvent::ToolCalled { at, tool });
            }
        }
        if calls.is_empty() {
            events.push(AgentEvent::TurnFinished { at });
        }
        return events;
    }

    if results.is_empty() {
        events.push(AgentEvent::TurnStarted { at });
        return events;
    }
    for block in &results {
        // Absent means the tool worked: Claude writes the field only on failure.
        let failed = block.contains(r#""is_error":true"#);
        events.push(AgentEvent::ToolOutcome {
            at,
            tool: None,
            failed,
        });
    }
    events
}

/// The slices of `line` that each begin at `marker` and end where the next block begins.
///
/// This is deliberately not a parser. It bounds a lookup to one block so a field cannot be read
/// from the wrong one, and that is all it does -- the moment Verb needed to understand the
/// structure inside a block, it would be reading content it has no business reading.
fn blocks<'line>(line: &'line str, marker: &str) -> Vec<&'line str> {
    let mut found = Vec::new();
    let mut offset = 0;
    while let Some(index) = line[offset..].find(marker) {
        let start = offset + index;
        let rest = &line[start + marker.len()..];
        let end = rest
            .find(r#"{"type":"#)
            .map(|next| start + marker.len() + next)
            .unwrap_or(line.len());
        found.push(&line[start..end]);
        offset = start + marker.len();
    }
    found
}

/// Codex writes one record per event, with the interesting part under `payload`. Turns are
/// `task_started` and `task_complete`; calls are `function_call` and `custom_tool_call`, each
/// carrying a `name`; outcomes are the matching `*_output` records.
fn codex_events(line: &str, fallback_at: u128) -> Vec<AgentEvent> {
    let at = timestamp(line, fallback_at);
    let mut events = Vec::new();

    // One payload per line, so the checks are on the line itself -- ordered so that the `_output`
    // forms are recognised before the call forms they contain as a prefix.
    if line.contains(r#""type":"task_started""#) {
        events.push(AgentEvent::TurnStarted { at });
    }
    if line.contains(r#""type":"task_complete""#) {
        events.push(AgentEvent::TurnFinished { at });
    }
    if line.contains(r#""type":"function_call_output""#)
        || line.contains(r#""type":"custom_tool_call_output""#)
    {
        // Codex reports an outcome's fate in `status`; anything that is not an explicit failure is
        // left as not-failed rather than guessed at.
        let failed = json_string(line, "status")
            .map(|status| status == "failed" || status == "error")
            .unwrap_or(false);
        events.push(AgentEvent::ToolOutcome {
            at,
            tool: None,
            failed,
        });
    } else if line.contains(r#""type":"function_call""#)
        || line.contains(r#""type":"custom_tool_call""#)
    {
        if let Some(tool) = json_string(line, "name") {
            events.push(AgentEvent::ToolCalled { at, tool });
        }
    }
    events
}

/// Both agents timestamp their records; Claude in ISO-8601, Codex the same. Verb keeps its own
/// milliseconds, so a record whose time cannot be read is stamped with when Verb read it -- and
/// never with a time invented from the text.
fn timestamp(line: &str, fallback_at: u128) -> u128 {
    json_number(line, "timestampMs").unwrap_or(fallback_at)
}

/// What a session's events add up to, for the evidence overlay.
///
/// Counts and the last failure, which is the thing a person is looking for. No content, so there is
/// nothing here that could carry a prompt or an output by accident.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Observed {
    pub turns: usize,
    pub tools_called: usize,
    pub tools_failed: usize,
    pub last_failure_at: Option<u128>,
    pub last_tool: Option<String>,
}

impl Observed {
    pub fn absorb(&mut self, event: &AgentEvent) {
        match event {
            AgentEvent::TurnStarted { .. } => self.turns += 1,
            AgentEvent::ToolCalled { tool, .. } => {
                self.tools_called += 1;
                self.last_tool = Some(tool.clone());
            }
            AgentEvent::ToolOutcome { at, failed, .. } => {
                if *failed {
                    self.tools_failed += 1;
                    self.last_failure_at = Some(*at);
                }
            }
            AgentEvent::TurnFinished { .. } => {}
        }
    }

    /// Whether anything was observed at all. An empty observation is reported as empty, never as
    /// "nothing went wrong".
    pub fn is_empty(&self) -> bool {
        self.turns == 0 && self.tools_called == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_claude_user_record_opens_a_turn() {
        let line = r#"{"type":"user","timestampMs":1000,"message":{"role":"user","content":"deploy the thing"}}"#;

        assert_eq!(
            Record::Claude.events(line, 0),
            vec![AgentEvent::TurnStarted { at: 1000 }]
        );
    }

    #[test]
    fn a_claude_tool_use_is_named_and_nothing_else_is_carried() {
        let line = r#"{"type":"assistant","timestampMs":2000,"message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"rm -rf /","description":"secret"}}]}}"#;

        let events = Record::Claude.events(line, 0);

        assert_eq!(
            events,
            vec![AgentEvent::ToolCalled {
                at: 2000,
                tool: "Bash".to_owned()
            }]
        );
    }

    #[test]
    fn nothing_a_person_typed_or_a_tool_printed_can_reach_an_event() {
        // The load-bearing test. Verb reads an agent's own record, which is full of prompts, tool
        // arguments and output; none of it may end up in a Verb event, an overlay or an assistant's
        // context. Structure leaves this module. Content does not.
        let secrets = [
            "sk-live-4242424242",
            "my database password is hunter2",
            "/Users/someone/private/notes.txt",
            "diff --git a/secrets.env",
        ];
        let line = format!(
            r#"{{"type":"assistant","timestampMs":3000,"message":{{"content":[
                {{"type":"text","text":"{}"}},
                {{"type":"tool_use","name":"Bash","input":{{"command":"{}","description":"{}"}}}}
            ]}},"cwd":"{}"}}"#,
            secrets[1], secrets[0], secrets[3], secrets[2]
        );

        let rendered = format!("{:?}", Record::Claude.events(&line, 0));

        for secret in secrets {
            assert!(
                !rendered.contains(secret),
                "an agent event carried content out of the record: {secret}"
            );
        }
    }

    #[test]
    fn a_failed_tool_is_the_thing_verb_could_not_see_before() {
        // The whole point of C1: inside an agent session, a failure used to be invisible to Verb.
        let line = r#"{"type":"user","timestampMs":4000,"message":{"content":[{"type":"tool_result","tool_use_id":"x","is_error":true,"content":"command not found"}]}}"#;

        assert_eq!(
            Record::Claude.events(line, 0),
            vec![AgentEvent::ToolOutcome {
                at: 4000,
                tool: None,
                failed: true
            }]
        );
    }

    #[test]
    fn a_tool_result_without_is_error_is_a_success_not_an_unknown() {
        // Claude omits the field entirely when the tool worked, so absence is meaningful here --
        // unlike the session states, where absence of evidence is never a "no".
        let line = r#"{"type":"user","timestampMs":5000,"message":{"content":[{"type":"tool_result","tool_use_id":"x","content":"ok"}]}}"#;

        assert_eq!(
            Record::Claude.events(line, 0),
            vec![AgentEvent::ToolOutcome {
                at: 5000,
                tool: None,
                failed: false
            }]
        );
    }

    #[test]
    fn codex_turns_and_calls_are_read_from_its_payloads() {
        let started = r#"{"timestamp":"2026-08-22T08:00:00Z","type":"event_msg","payload":{"type":"task_started"}}"#;
        let called = r#"{"timestamp":"2026-08-22T08:00:01Z","type":"response_item","payload":{"type":"custom_tool_call","name":"exec","input":"cat /etc/passwd"}}"#;
        let done = r#"{"timestamp":"2026-08-22T08:00:02Z","type":"event_msg","payload":{"type":"task_complete"}}"#;

        assert_eq!(
            Record::Codex.events(started, 10),
            vec![AgentEvent::TurnStarted { at: 10 }]
        );
        assert_eq!(
            Record::Codex.events(called, 11),
            vec![AgentEvent::ToolCalled {
                at: 11,
                tool: "exec".to_owned()
            }]
        );
        assert_eq!(
            Record::Codex.events(done, 12),
            vec![AgentEvent::TurnFinished { at: 12 }]
        );
    }

    #[test]
    fn codex_reports_a_failure_only_when_it_says_so() {
        let failed = r#"{"timestamp":"t","payload":{"type":"function_call_output","status":"failed","output":"boom"}}"#;
        let quiet =
            r#"{"timestamp":"t","payload":{"type":"function_call_output","output":"fine"}}"#;

        assert_eq!(
            Record::Codex.events(failed, 1),
            vec![AgentEvent::ToolOutcome {
                at: 1,
                tool: None,
                failed: true
            }]
        );
        assert_eq!(
            Record::Codex.events(quiet, 1),
            vec![AgentEvent::ToolOutcome {
                at: 1,
                tool: None,
                failed: false
            }]
        );
    }

    #[test]
    fn a_line_verb_does_not_understand_produces_no_event_rather_than_a_guess() {
        // Both agents version their own formats. A shape Verb has never seen must read as silence,
        // not as an invented fact.
        for line in [
            r#"{"type":"file-history-snapshot","timestampMs":1}"#,
            r#"{"type":"world_state","payload":{"type":"something_new"}}"#,
            "not json at all",
            "",
        ] {
            assert!(Record::Claude.events(line, 0).is_empty(), "claude: {line}");
            assert!(Record::Codex.events(line, 0).is_empty(), "codex: {line}");
        }
    }

    #[test]
    fn an_observation_with_nothing_in_it_says_so_rather_than_saying_all_is_well() {
        let observed = Observed::default();

        assert!(observed.is_empty());
        assert_eq!(observed.tools_failed, 0);
        assert_eq!(observed.last_failure_at, None);
    }

    #[test]
    fn an_observation_keeps_counts_and_the_last_failure() {
        let mut observed = Observed::default();
        for event in [
            AgentEvent::TurnStarted { at: 1 },
            AgentEvent::ToolCalled {
                at: 2,
                tool: "Bash".to_owned(),
            },
            AgentEvent::ToolOutcome {
                at: 3,
                tool: None,
                failed: true,
            },
            AgentEvent::ToolCalled {
                at: 4,
                tool: "Read".to_owned(),
            },
            AgentEvent::ToolOutcome {
                at: 5,
                tool: None,
                failed: false,
            },
        ] {
            observed.absorb(&event);
        }

        assert!(!observed.is_empty());
        assert_eq!(observed.turns, 1);
        assert_eq!(observed.tools_called, 2);
        assert_eq!(observed.tools_failed, 1);
        assert_eq!(observed.last_failure_at, Some(3));
        assert_eq!(observed.last_tool.as_deref(), Some("Read"));
    }

    /// Run against a real record on this machine, to check the parsers against the shape the agents
    /// actually write rather than against the shape this file assumes:
    ///
    /// ```text
    /// VERB_CLAUDE_RECORD=~/.claude/projects/<project>/<session>.jsonl \
    ///   cargo test observed_counts -- --ignored --nocapture
    /// ```
    ///
    /// Ignored by default: it reads a person's own agent transcript, which belongs to them and is
    /// not something CI should ever open.
    #[test]
    #[ignore]
    fn observed_counts_against_a_real_record() {
        for (variable, record) in [
            ("VERB_CLAUDE_RECORD", Record::Claude),
            ("VERB_CODEX_RECORD", Record::Codex),
        ] {
            let Ok(path) = std::env::var(variable) else {
                continue;
            };
            let text = std::fs::read_to_string(&path).expect("record readable");
            let mut observed = Observed::default();
            for (index, line) in text.lines().enumerate() {
                for event in record.events(line, index as u128) {
                    observed.absorb(&event);
                }
            }
            println!("{variable}: {observed:?}");
            assert!(!observed.is_empty(), "{variable} produced no observations");
        }
    }
}
