//! The conversation inside one hosted agent terminal: what Verb can show of it, and the one
//! supported way to continue it from outside the terminal.
//!
//! The rules come from requirements R1–R4 in `docs/CAPABILITY_ARCHITECTURE_MAP.md`:
//!
//! - **Content only with the stream on.** Lines come through `transcript`'s parser (redacted and
//!   bounded) and only while the project's stream setting is on. Turning it off clears everything
//!   held here at once and starts a new generation; the agent's own log is never touched.
//! - **Chat is not the terminal.** A chat message is accepted only when the agent is ready on
//!   evidence, never while an earlier message is unresolved, and never twice for the same message
//!   id. Raw keystrokes do not come through here and are never gated by readiness.
//! - **Received means the agent's log says so.** Same conversation, written after the message was
//!   sent, with the agent's own turn id and a usable timestamp, and the same text. Anything less
//!   stays "sent" or "unknown", and nothing is resent automatically.
//! - **Three identities.** The Verb session, the provider's conversation and this execution
//!   attempt are reported separately. Continuity is claimed only when the evidence names the same
//!   conversation; when it cannot be established, it says so.

use crate::observe::{Record, RecordTail};
use crate::transcript::{self, ItemLog, Parser, Settings};
use serde::Serialize;
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

const POLL_EVERY: Duration = Duration::from_millis(250);
const SETTINGS_EVERY: Duration = Duration::from_millis(400);
const STATUS_EVERY: Duration = Duration::from_millis(700);
/// How long an attempt may run without any evidence naming its conversation before Verb stops
/// waiting and says continuity was not established.
const IDENTIFY_WITHIN: Duration = Duration::from_secs(45);
const READ_LIMIT: u64 = 1024 * 1024;
const MAX_FRAGMENT: usize = 4 * 1024 * 1024;
const MAX_DELIVERIES: usize = 16;
const MAX_EVENTS: usize = 24;
pub(crate) const MAX_MESSAGE: usize = 8_000;
const MAX_MESSAGE_ID: usize = 64;

/// Screen text that looks like a provider asking for permission. A sign, never a fact: it only
/// ever closes chat, and the terminal stays the place to answer.
const PERMISSION_SIGNS: [&str; 8] = [
    "Do you want to proceed?",
    "Do you want to make this edit",
    "Do you want to create",
    "Do you want to run",
    "Allow this tool?",
    "don't ask again",
    "[y/N]",
    "[Y/n]",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Readiness {
    Ready,
    Working,
    Approval,
    Unknown,
    Ended,
    Unidentified,
    Unreadable,
    StreamOff,
    Unsupported,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReadinessView {
    pub state: Readiness,
    pub reason: String,
    /// The evidence the state rests on, in plain words.
    pub based_on: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DeliveryState {
    /// Accepted; being written to the terminal now.
    Sending,
    /// Written to the terminal; the agent's log has not shown it yet.
    Sent,
    /// The agent's log shows it (see `confirm`).
    Received,
    /// Nothing was written.
    Failed,
    /// It may or may not have reached the agent, and Verb cannot tell.
    Unknown,
    /// The person set it aside. It may still have reached the agent.
    Discarded,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Origin {
    Desktop,
    Phone,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Delivery {
    pub id: String,
    pub state: DeliveryState,
    pub origin: Origin,
    /// The message as shown (redacted). Dropped when the stream is turned off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Verb's own clock, milliseconds since the epoch.
    pub created_at: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sent_at: Option<u128>,
    /// The agent's own id for the turn that confirmed it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    /// The agent's own timestamp for that turn, exactly as written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub received_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked_at: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip)]
    log_offset: u64,
    #[serde(skip)]
    fingerprint: Option<[u8; 32]>,
    #[serde(skip)]
    conversation: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Continuity {
    /// Waiting for evidence that names this attempt's conversation.
    Pending,
    /// The first attempt of this session, in its own conversation.
    FirstAttempt,
    /// A later attempt, and the agent's evidence names the conversation it was asked to continue.
    Resumed,
    /// A later attempt in a different conversation, named by the agent's evidence.
    NewConversation,
    /// The evidence is missing, ambiguous or contradictory.
    NotEstablished,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContinuityView {
    pub state: Continuity,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_conversation: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Identity {
    pub session: String,
    pub conversation: Option<String>,
    pub attempt: u32,
    pub attempt_started_at: u128,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Event {
    pub kind: &'static str,
    /// Verb's own clock.
    pub at: u128,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderStatus {
    pub status: String,
    pub conversation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<u128>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ViewItem<'a> {
    pub seq: u64,
    #[serde(flatten)]
    pub item: &'a transcript::Item,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct View<'a> {
    pub agent: Option<&'static str>,
    pub stream: &'a Settings,
    pub identity: Identity,
    pub continuity: ContinuityView,
    pub readiness: ReadinessView,
    pub can_send: bool,
    /// `terminal`: messages are typed into the agent's terminal. `unsupported`: use the terminal.
    pub send_contract: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_note: Option<&'static str>,
    pub items: Vec<ViewItem<'a>>,
    pub earlier_omitted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_problem: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_read_at: Option<u128>,
    pub deliveries: &'a VecDeque<Delivery>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<&'a ProviderStatus>,
    pub permission_sign: bool,
    pub events: &'a VecDeque<Event>,
}

pub(crate) struct SendContext<'a> {
    pub screen: &'a str,
    pub conversation: Option<&'a str>,
    pub running: bool,
    pub bracketed_paste: bool,
}

/// Why a chat message was not accepted. Nothing was written to the terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Refusal {
    pub code: &'static str,
    pub reason: String,
}

impl Refusal {
    fn new(code: &'static str, reason: impl Into<String>) -> Self {
        Self {
            code,
            reason: reason.into(),
        }
    }
}

/// Where the engine reads from. Explicit so tests never touch a real home or store.
#[derive(Clone, Debug)]
pub(crate) struct Sources {
    pub home: Option<PathBuf>,
    pub store: Option<PathBuf>,
    /// The agent runs under another account; its records are private and never read here.
    pub private_runtime: bool,
}

impl Sources {
    pub(crate) fn for_project(project: &Path) -> Self {
        Self {
            home: crate::isolation::agent_home(),
            store: crate::project::identity(project).ok().map(|i| i.store),
            private_runtime: crate::isolation::profile().is_ok_and(|p| p.is_some()),
        }
    }
}

struct Reader {
    path: PathBuf,
    offset: u64,
    fragment: Vec<u8>,
    /// Started mid-file: the first fragment is the tail of a line already gone.
    skip_partial: bool,
}

pub(crate) struct Engine {
    record: Option<Record>,
    sendable: bool,
    project: PathBuf,
    sources: Sources,
    session: String,
    attempt: u32,
    attempt_started_at: u128,
    attempt_started: Instant,
    expected: Option<String>,
    conversation: Option<String>,
    continuity: Continuity,
    continuity_reason: String,
    settings: Settings,
    settings_stamp: Option<(SystemTime, u64)>,
    next_settings: Instant,
    reader: Option<Reader>,
    parser: Parser,
    items: ItemLog,
    read_problem: Option<String>,
    last_read_at: Option<u128>,
    deliveries: VecDeque<Delivery>,
    sent_ids: HashSet<String>,
    provider: Option<ProviderStatus>,
    next_status: Instant,
    permission_sign: bool,
    running: bool,
    pid: i32,
    events: VecDeque<Event>,
    next_poll: Instant,
}

impl Engine {
    /// The engine for a hosted terminal, or `None` for a shell (no agent, nothing to converse with).
    pub(crate) fn for_agent(
        agent: Option<&str>,
        project: &Path,
        session: &str,
        attempt: u32,
        expected: Option<&str>,
        pid: i32,
        sources: Sources,
    ) -> Option<Self> {
        let agent = agent?;
        let record = Record::for_agent(agent);
        let now = Instant::now();
        let mut engine = Self {
            record,
            // Only Claude Code gives each typed turn an id and timestamp Verb can confirm against.
            sendable: record == Some(Record::Claude),
            project: project.to_path_buf(),
            sources,
            session: session.to_owned(),
            attempt: attempt.max(1),
            attempt_started_at: crate::now_millis(),
            attempt_started: now,
            expected: expected.map(str::to_owned),
            conversation: None,
            continuity: Continuity::Pending,
            continuity_reason: "Waiting for the agent to name its conversation.".to_owned(),
            settings: Settings::default(),
            settings_stamp: None,
            next_settings: now,
            reader: None,
            parser: Parser::new(record.unwrap_or(Record::Claude)),
            items: ItemLog::bounded(transcript::MAX_ITEMS),
            read_problem: None,
            last_read_at: None,
            deliveries: VecDeque::new(),
            sent_ids: HashSet::new(),
            provider: None,
            next_status: now,
            permission_sign: false,
            running: true,
            pid,
            events: VecDeque::new(),
            next_poll: now,
        };
        engine.refresh_settings(true);
        if engine.attempt > 1 {
            engine.event(
                "restart",
                format!("Attempt {} started in this session.", engine.attempt),
            );
        }
        Some(engine)
    }

    fn event(&mut self, kind: &'static str, detail: String) {
        self.events.push_back(Event {
            kind,
            at: crate::now_millis(),
            detail,
        });
        while self.events.len() > MAX_EVENTS {
            self.events.pop_front();
        }
    }

    #[cfg(test)]
    fn generation(&self) -> u64 {
        self.settings.generation
    }

    /// Called on every pass of the host's loop; does real work at most every `POLL_EVERY`.
    /// `conversation` is what the agent's own record has named so far (`AgentWatch`).
    pub(crate) fn poll(&mut self, screen: &str, conversation: Option<&str>, running: bool) -> bool {
        let now = Instant::now();
        if now < self.next_poll {
            return false;
        }
        self.next_poll = now + POLL_EVERY;
        self.update(now, screen, conversation, running, false)
    }

    /// The same work without waiting for the next tick: before accepting a message, and for an
    /// explicit "check the conversation".
    fn update(
        &mut self,
        now: Instant,
        screen: &str,
        conversation: Option<&str>,
        running: bool,
        force: bool,
    ) -> bool {
        let before = self.fingerprint();
        if self.running && !running {
            self.event("ended", "The agent's process ended.".to_owned());
            for delivery in self.deliveries.iter_mut() {
                if matches!(delivery.state, DeliveryState::Sending | DeliveryState::Sent) {
                    delivery.state = DeliveryState::Unknown;
                    delivery.reason =
                        Some("The agent stopped before its log showed this message.".to_owned());
                }
            }
        }
        self.running = running;
        self.permission_sign = PERMISSION_SIGNS.iter().any(|sign| screen.contains(sign));
        self.refresh_settings(force);
        if force || now >= self.next_status {
            self.next_status = now + STATUS_EVERY;
            self.provider = self.read_provider_status();
        }
        self.identify(conversation, now);
        if self.settings.enabled && !self.sources.private_runtime && self.record.is_some() {
            self.read();
            self.confirm();
        }
        before != self.fingerprint()
    }

    fn fingerprint(&self) -> (u64, usize, u64, Readiness, Continuity, usize, bool) {
        let deliveries = self
            .deliveries
            .iter()
            .map(|d| d.state as usize + d.checked_at.unwrap_or(0) as usize)
            .sum();
        (
            self.settings.generation,
            self.items.len(),
            self.reader.as_ref().map_or(0, |r| r.offset),
            self.readiness().state,
            self.continuity,
            deliveries,
            self.read_problem.is_some(),
        )
    }

    // ---------------------------------------------------------------------------------- stream

    fn refresh_settings(&mut self, force: bool) {
        let now = Instant::now();
        if !force && now < self.next_settings {
            return;
        }
        self.next_settings = now + SETTINGS_EVERY;
        let Some(store) = &self.sources.store else {
            self.apply_settings(Settings::default());
            return;
        };
        let path = store.join("stream.json");
        let stamp = fs::metadata(&path)
            .ok()
            .and_then(|m| Some((m.modified().ok()?, m.len())));
        if !force && stamp.is_some() && stamp == self.settings_stamp {
            return;
        }
        self.settings_stamp = stamp;
        let settings = transcript::load(store);
        self.apply_settings(settings);
    }

    /// Applies a newer setting. Off, or any new generation, clears every piece of conversation
    /// content this engine holds before anything else can read it.
    pub(crate) fn apply_settings(&mut self, settings: Settings) {
        if settings == self.settings {
            return;
        }
        let was = self.settings.enabled;
        self.settings = settings;
        self.clear_content();
        if was && !self.settings.enabled {
            self.event(
                "stream_off",
                "The stream was turned off. Conversation text was cleared here.".to_owned(),
            );
            for delivery in self.deliveries.iter_mut() {
                if matches!(delivery.state, DeliveryState::Sending | DeliveryState::Sent) {
                    delivery.state = DeliveryState::Unknown;
                    delivery.reason = Some(
                        "The stream was turned off before the agent's log showed this message."
                            .to_owned(),
                    );
                }
            }
        } else if !was && self.settings.enabled {
            self.event(
                "stream_on",
                "The stream was turned on. The conversation is read again from the agent's log."
                    .to_owned(),
            );
        }
    }

    fn clear_content(&mut self) {
        self.reader = None;
        self.parser = Parser::new(self.record.unwrap_or(Record::Claude));
        self.items = ItemLog::bounded(transcript::MAX_ITEMS);
        self.read_problem = None;
        self.last_read_at = None;
        for delivery in self.deliveries.iter_mut() {
            delivery.text = None;
            delivery.fingerprint = None;
        }
    }

    // ------------------------------------------------------------------------------- identity

    fn identify(&mut self, watched: Option<&str>, now: Instant) {
        let reported = self
            .provider
            .as_ref()
            .and_then(|status| status.conversation.clone());
        let found = match (watched, reported.as_deref()) {
            (Some(a), Some(b)) if a != b => {
                self.set_continuity(
                    Continuity::NotEstablished,
                    format!(
                        "The agent's log names conversation {} but its status names {}.",
                        short(a),
                        short(b)
                    ),
                );
                return;
            }
            (Some(a), _) => Some(a.to_owned()),
            (None, Some(b)) => Some(b.to_owned()),
            (None, None) => None,
        };
        match (&self.conversation, found) {
            (None, Some(id)) => {
                self.conversation = Some(id.clone());
                let (state, reason) = match (&self.expected, self.attempt) {
                    (Some(expected), _) if *expected == id => (
                        Continuity::Resumed,
                        format!(
                            "The agent's evidence names the same conversation, {}.",
                            short(&id)
                        ),
                    ),
                    (Some(expected), _) => (
                        Continuity::NewConversation,
                        format!(
                            "Asked to continue {}, the agent is writing {}.",
                            short(expected),
                            short(&id)
                        ),
                    ),
                    (None, 1) => (
                        Continuity::FirstAttempt,
                        format!("First attempt, conversation {}.", short(&id)),
                    ),
                    (None, _) => (
                        Continuity::NewConversation,
                        format!("This attempt started conversation {}.", short(&id)),
                    ),
                };
                self.set_continuity(state, reason);
            }
            (Some(current), Some(id)) if *current != id => {
                self.set_continuity(
                    Continuity::NotEstablished,
                    format!(
                        "The agent's evidence changed from conversation {} to {}.",
                        short(current),
                        short(&id)
                    ),
                );
            }
            (None, None)
                if self.continuity == Continuity::Pending
                    && now.duration_since(self.attempt_started) >= IDENTIFY_WITHIN =>
            {
                self.set_continuity(
                    Continuity::NotEstablished,
                    "Nothing the agent wrote names this attempt's conversation yet.".to_owned(),
                );
            }
            _ => {}
        }
    }

    fn set_continuity(&mut self, state: Continuity, reason: String) {
        if self.continuity == state && self.continuity_reason == reason {
            return;
        }
        self.continuity = state;
        self.continuity_reason = reason.clone();
        let kind = match state {
            Continuity::Resumed => "resumed",
            Continuity::NewConversation => "new_conversation",
            Continuity::NotEstablished => "not_established",
            Continuity::FirstAttempt => "identified",
            Continuity::Pending => return,
        };
        self.event(kind, reason);
    }

    // ------------------------------------------------------------------------------- reading

    fn read(&mut self) {
        let (Some(record), Some(conversation), Some(home)) = (
            self.record,
            self.conversation.clone(),
            self.sources.home.clone(),
        ) else {
            return;
        };
        if self.continuity == Continuity::NotEstablished {
            return;
        }
        if self.reader.is_none() {
            let Some(path) = record_path(record, &home, &self.project, &conversation) else {
                // No log yet: an agent writes it with the first message. Not a read failure.
                return;
            };
            let len = match fs::metadata(&path) {
                Ok(meta) => meta.len(),
                Err(error) => {
                    self.read_problem = Some(read_problem(&error));
                    return;
                }
            };
            let start = len.saturating_sub(transcript::TAIL_BYTES);
            self.items.omitted = start > 0;
            self.reader = Some(Reader {
                path,
                offset: start,
                fragment: Vec::new(),
                skip_partial: start > 0,
            });
        }
        let Some(reader) = self.reader.as_mut() else {
            return;
        };
        let mut file = match fs::File::open(&reader.path) {
            Ok(file) => file,
            Err(error) => {
                self.read_problem = Some(read_problem(&error));
                return;
            }
        };
        let len = match file.metadata() {
            Ok(meta) => meta.len(),
            Err(error) => {
                self.read_problem = Some(read_problem(&error));
                return;
            }
        };
        if len < reader.offset {
            self.event(
                "log_replaced",
                "The agent's log became shorter; Verb read it again from the start.".to_owned(),
            );
            self.reader = None;
            self.parser = Parser::new(record);
            self.items = ItemLog::bounded(transcript::MAX_ITEMS);
            return;
        }
        if len == reader.offset {
            self.read_problem = None;
            self.last_read_at = Some(crate::now_millis());
            return;
        }
        let want = (len - reader.offset).min(READ_LIMIT);
        let mut chunk = Vec::with_capacity(want as usize);
        let read = file
            .seek(SeekFrom::Start(reader.offset))
            .and_then(|_| file.by_ref().take(want).read_to_end(&mut chunk));
        if let Err(error) = read {
            self.read_problem = Some(read_problem(&error));
            return;
        }
        let fragment_start = reader.offset - reader.fragment.len() as u64;
        reader.offset += chunk.len() as u64;
        reader.fragment.extend_from_slice(&chunk);
        let mut consumed = 0;
        let mut lines: Vec<(u64, String)> = Vec::new();
        while let Some(end) = reader.fragment[consumed..].iter().position(|b| *b == b'\n') {
            let line = &reader.fragment[consumed..consumed + end];
            let start = fragment_start + consumed as u64;
            consumed += end + 1;
            if reader.skip_partial {
                reader.skip_partial = false;
                continue;
            }
            lines.push((start, String::from_utf8_lossy(line).into_owned()));
        }
        reader.fragment.drain(..consumed);
        if reader.fragment.len() > MAX_FRAGMENT {
            // One line larger than any agent writes: drop it rather than grow without limit.
            reader.fragment.clear();
            reader.skip_partial = true;
        }
        for (start, line) in lines {
            self.parser
                .line(&line, start, &self.project, &mut self.items);
        }
        self.read_problem = None;
        self.last_read_at = Some(crate::now_millis());
    }

    // ------------------------------------------------------------------------------ readiness

    pub(crate) fn readiness(&self) -> ReadinessView {
        let view = |state, reason: &str, based_on: Vec<String>| ReadinessView {
            state,
            reason: reason.to_owned(),
            based_on,
        };
        if self.record.is_none() {
            return view(
                Readiness::Unsupported,
                "Verb has no reader for this agent's conversation. Work in the terminal.",
                vec!["No conversation reader exists for this agent.".to_owned()],
            );
        }
        if self.sources.private_runtime {
            return view(
                Readiness::Unsupported,
                "This agent runs in an isolated account, so its conversation stays private to it.",
                vec!["The isolated runtime keeps agent records under its own account.".to_owned()],
            );
        }
        if !self.settings.enabled {
            return view(
                Readiness::StreamOff,
                "The stream is off for this project, so Verb does not read the conversation.",
                vec!["The project's stream setting is off.".to_owned()],
            );
        }
        if !self.running {
            return view(
                Readiness::Ended,
                "The agent's process has ended.",
                vec!["Verb saw the process exit.".to_owned()],
            );
        }
        if self.continuity == Continuity::NotEstablished {
            return view(
                Readiness::Unidentified,
                "Verb cannot tell which conversation this terminal is in.",
                vec![self.continuity_reason.clone()],
            );
        }
        if self.conversation.is_none() {
            return view(
                Readiness::Unidentified,
                "Verb has not yet seen which conversation this terminal is in.",
                vec!["Neither the agent's log nor its status names a conversation yet.".to_owned()],
            );
        }
        if let Some(problem) = &self.read_problem {
            return view(
                Readiness::Unreadable,
                "The conversation can't be read, so a message could not be confirmed.",
                vec![problem.clone()],
            );
        }
        if self.permission_sign {
            return view(
                Readiness::Approval,
                "The terminal looks like it is asking for permission. Answer it in the terminal.",
                vec!["Screen text looks like a permission prompt (a sign, not a fact).".to_owned()],
            );
        }
        match &self.provider {
            Some(status) if status.status == "idle"
                && status.conversation == self.conversation
                && status.updated_at.is_some_and(|at| at >= self.attempt_started_at.saturating_sub(5000)
                    && at <= crate::now_millis().saturating_add(5000))
                && self.parser.turn_complete == Some(true)
                && self.reader.as_ref().is_none_or(|r| r.fragment.is_empty()
                    && fs::metadata(&r.path).is_ok_and(|m| m.len() == r.offset)) => view(
                Readiness::Ready,
                "Waiting for you.",
                vec![format!(
                    "The agent reports its status as idle{}.",
                    status
                        .updated_at
                        .map(|_| String::new())
                        .unwrap_or_else(|| " (no time given)".to_owned())
                )],
            ),
            Some(status) if status.status == "idle" => view(
                Readiness::Unknown,
                "Waiting for a completed turn in the agent’s log and a current idle status. Use the terminal if this is a new conversation.",
                vec!["Chat waits for the complete log and a timestamped idle status for this conversation.".to_owned()],
            ),
            Some(status) => view(
                Readiness::Working,
                "The agent is busy.",
                vec![format!("The agent reports its status as \"{}\".", status.status)],
            ),
            None => view(
                Readiness::Unknown,
                "Verb can't tell whether the agent is ready.",
                vec!["The agent has not reported a status Verb can match to this terminal."
                    .to_owned()],
            ),
        }
    }

    fn unresolved(&self) -> Option<&Delivery> {
        self.deliveries.iter().find(|d| {
            matches!(
                d.state,
                DeliveryState::Sending | DeliveryState::Sent | DeliveryState::Unknown
            )
        })
    }

    pub(crate) fn can_send(&self) -> bool {
        self.sendable && self.readiness().state == Readiness::Ready && self.unresolved().is_none()
    }

    fn send_note(&self) -> Option<&'static str> {
        match self.record {
            None => Some("Verb can't read this agent's conversation. Use the terminal."),
            Some(Record::Codex) => Some(
                "Codex's log gives typed messages no id, so Verb could never confirm one. Send from the terminal.",
            ),
            Some(Record::Claude) => None,
        }
    }

    // -------------------------------------------------------------------------------- sending

    /// Accepts a message and returns the bytes to type into the terminal. The caller writes them
    /// under its own input lease and reports back with `finish_send`. Refused messages write
    /// nothing. The same id twice returns the earlier outcome and writes nothing.
    pub(crate) fn prepare_send(
        &mut self,
        id: &str,
        text: &str,
        origin: Origin,
        context: SendContext<'_>,
    ) -> Result<Option<Vec<u8>>, Refusal> {
        let SendContext {
            screen,
            conversation,
            running,
            bracketed_paste,
        } = context;
        if id.is_empty()
            || id.len() > MAX_MESSAGE_ID
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(Refusal::new("invalid_id", "The message id is not valid."));
        }
        if self.sent_ids.contains(id) {
            return Ok(None);
        }
        if self.sent_ids.len() >= 4096 {
            return Err(Refusal::new(
                "receipt_limit",
                "This attempt has reached its message limit. Use the terminal.",
            ));
        }
        let text = text.trim();
        if text.is_empty() {
            return Err(Refusal::new("empty", "Write a message first."));
        }
        if text.len() > MAX_MESSAGE {
            return Err(Refusal::new(
                "too_long",
                format!("Messages are limited to {MAX_MESSAGE} bytes. Paste longer text in the terminal."),
            ));
        }
        if text
            .chars()
            .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err(Refusal::new(
                "control",
                "The message contains control characters. Use the terminal for keystrokes.",
            ));
        }
        if !self.sendable {
            return Err(Refusal::new(
                "unsupported",
                self.send_note().unwrap_or("Use the terminal."),
            ));
        }
        self.update(Instant::now(), screen, conversation, running, true);
        if let Some(earlier) = self.unresolved() {
            return Err(Refusal::new(
                "unresolved",
                format!(
                    "An earlier message is still {}. Resolve it first so this one can't overtake it.",
                    match earlier.state {
                        DeliveryState::Unknown => "of unknown delivery",
                        _ => "waiting for confirmation",
                    }
                ),
            ));
        }
        let readiness = self.readiness();
        if readiness.state != Readiness::Ready {
            return Err(Refusal::new("not_ready", readiness.reason));
        }
        if text.contains('\n') && !bracketed_paste {
            return Err(Refusal::new(
                "multiline",
                "The agent is not accepting pasted text right now, so a message with line breaks can't be sent as one. Remove the line breaks or use the terminal.",
            ));
        }
        let log_offset = self
            .reader
            .as_ref()
            .and_then(|r| fs::metadata(&r.path).ok())
            .map_or(0, |m| m.len());
        self.sent_ids.insert(id.to_owned());
        self.deliveries.push_back(Delivery {
            id: id.to_owned(),
            state: DeliveryState::Sending,
            origin,
            text: Some(transcript::as_shown(text)),
            created_at: crate::now_millis(),
            sent_at: None,
            turn_id: None,
            received_at: None,
            checked_at: None,
            reason: None,
            log_offset,
            fingerprint: Some(transcript::message_fingerprint(text)),
            conversation: self.conversation.clone(),
        });
        while self.deliveries.len() > MAX_DELIVERIES {
            let Some(index) = self
                .deliveries
                .iter()
                .position(|d| !matches!(d.state, DeliveryState::Sending | DeliveryState::Sent))
            else {
                break;
            };
            self.deliveries.remove(index);
        }
        let mut bytes = Vec::with_capacity(text.len() + 16);
        if bracketed_paste {
            bytes.extend_from_slice(b"\x1b[200~");
            bytes.extend_from_slice(text.as_bytes());
            bytes.extend_from_slice(b"\x1b[201~");
        } else {
            bytes.extend_from_slice(text.as_bytes());
        }
        bytes.push(b'\r');
        Ok(Some(bytes))
    }

    /// A message that was refused after the sender had already been told it was queued (the
    /// phone). Recorded as not sent, without its text, so the sender can see why.
    pub(crate) fn record_refusal(&mut self, id: &str, origin: Origin, refusal: &Refusal) {
        if self.deliveries.iter().any(|d| d.id == id) {
            return;
        }
        self.deliveries.push_back(Delivery {
            id: id.to_owned(),
            state: DeliveryState::Failed,
            origin,
            text: None,
            created_at: crate::now_millis(),
            sent_at: None,
            turn_id: None,
            received_at: None,
            checked_at: None,
            reason: Some(format!("Not sent: {}", refusal.reason)),
            log_offset: 0,
            fingerprint: None,
            conversation: None,
        });
        while self.deliveries.len() > MAX_DELIVERIES {
            self.deliveries.pop_front();
        }
    }

    pub(crate) fn finish_send(
        &mut self,
        id: &str,
        written: Result<(), crate::mobile::ChatWriteError>,
    ) {
        let Some(delivery) = self.deliveries.iter_mut().find(|d| d.id == id) else {
            return;
        };
        match written {
            Ok(()) => {
                delivery.state = DeliveryState::Sent;
                delivery.sent_at = Some(crate::now_millis());
            }
            Err(error) => {
                delivery.state = if error.started {
                    DeliveryState::Unknown
                } else {
                    DeliveryState::Failed
                };
                delivery.reason = Some(if error.started {
                    format!(
                        "Delivery unknown: a terminal write was attempted. {}",
                        error.reason
                    )
                } else {
                    format!("Not sent: {}", error.reason)
                });
            }
        }
    }

    /// Re-reads the log now and reports what it shows about one message.
    pub(crate) fn check(
        &mut self,
        id: &str,
        screen: &str,
        conversation: Option<&str>,
        running: bool,
    ) -> Result<(), String> {
        if !self.deliveries.iter().any(|d| d.id == id) {
            return Err("Verb has no record of that message here. It may have been sent before Verb or the agent restarted.".to_owned());
        }
        self.update(Instant::now(), screen, conversation, running, true);
        let explanation = self.explain(id);
        let now = crate::now_millis();
        if let Some(delivery) = self.deliveries.iter_mut().find(|d| d.id == id) {
            delivery.checked_at = Some(now);
            if delivery.state != DeliveryState::Received {
                delivery.reason = Some(explanation);
            }
        }
        Ok(())
    }

    pub(crate) fn discard(&mut self, id: &str) -> Result<(), String> {
        let delivery = self
            .deliveries
            .iter_mut()
            .find(|d| d.id == id)
            .ok_or("Verb has no record of that message here.")?;
        if delivery.state == DeliveryState::Received {
            return Err("That message has already been received by the agent.".to_owned());
        }
        if matches!(delivery.state, DeliveryState::Sent | DeliveryState::Unknown) {
            delivery.reason =
                Some("Discarded by you. It may still have reached the agent.".to_owned());
        }
        delivery.state = DeliveryState::Discarded;
        Ok(())
    }

    /// Marks messages received when, and only when, the agent's log proves it.
    fn confirm(&mut self) {
        let claimed: Vec<String> = self
            .deliveries
            .iter()
            .filter_map(|d| d.turn_id.clone())
            .collect();
        let mut newly: Vec<(usize, String, String)> = Vec::new();
        for (index, delivery) in self.deliveries.iter().enumerate() {
            if !matches!(delivery.state, DeliveryState::Sent | DeliveryState::Unknown) {
                continue;
            }
            if let Some((turn, at)) = self.proof(delivery, &claimed, &newly) {
                newly.push((index, turn, at));
            }
        }
        for (index, turn, at) in newly {
            let delivery = &mut self.deliveries[index];
            delivery.state = DeliveryState::Received;
            delivery.turn_id = Some(turn);
            delivery.received_at = Some(at);
            delivery.reason = None;
        }
    }

    /// The turn that proves `delivery` reached the agent: all four tests at once.
    fn proof(
        &self,
        delivery: &Delivery,
        claimed: &[String],
        newly: &[(usize, String, String)],
    ) -> Option<(String, String)> {
        let fingerprint = delivery.fingerprint?;
        self.items.iter().find_map(|(_, item)| {
            if item.kind != "you"
                || item.offset < delivery.log_offset
                || item.fingerprint != Some(fingerprint)
            {
                return None;
            }
            let turn = item.id.clone().filter(|id| !id.is_empty())?;
            let at = item.at.clone()?;
            crate::context::parse_iso8601_utc(&at)?;
            if item.conversation.is_none() || item.conversation != delivery.conversation {
                return None;
            }
            if claimed.contains(&turn) || newly.iter().any(|(_, t, _)| *t == turn) {
                return None;
            }
            Some((turn, at))
        })
    }

    /// Why a message is not (yet) confirmed, in words that do not overclaim.
    fn explain(&self, id: &str) -> String {
        let Some(delivery) = self.deliveries.iter().find(|d| d.id == id) else {
            return String::new();
        };
        if delivery.state == DeliveryState::Received {
            return String::new();
        }
        let Some(text) = &delivery.text else {
            return "No proof either way: the stream is off, so Verb is not reading the agent's log."
                .to_owned();
        };
        if !self.settings.enabled {
            return "No proof either way: the stream is off.".to_owned();
        }
        if let Some(problem) = &self.read_problem {
            return format!("No proof either way: {problem}");
        }
        let matching: Vec<_> = self
            .items
            .iter()
            .filter(|(_, item)| item.kind == "you" && item.text.as_ref() == Some(text))
            .map(|(_, item)| item)
            .collect();
        if let Some(after) = matching.iter().find(|i| i.offset >= delivery.log_offset) {
            if after.id.is_none() {
                return "No proof either way: the log shows matching text, but gives that turn no id.".to_owned();
            }
            if after
                .at
                .as_deref()
                .and_then(crate::context::parse_iso8601_utc)
                .is_none()
            {
                return "No proof either way: the log shows matching text, but no usable time for it.".to_owned();
            }
            if after.conversation != delivery.conversation {
                return "No proof either way: matching text appears in a different conversation."
                    .to_owned();
            }
            return "No proof either way: the matching turn is already claimed by another message."
                .to_owned();
        }
        if !matching.is_empty() {
            return "No proof either way: only an earlier identical message is in the log."
                .to_owned();
        }
        "No proof either way: the agent's log does not show this message.".to_owned()
    }

    // ----------------------------------------------------------------------- provider status

    /// Claude Code's own status file for the process this terminal hosts. Only an entry for this
    /// exact process (or one it started as session leader) in this project counts.
    fn read_provider_status(&self) -> Option<ProviderStatus> {
        if self.record != Some(Record::Claude) || self.sources.private_runtime {
            return None;
        }
        let dir = self.sources.home.as_ref()?.join(".claude").join("sessions");
        let mut found = None;
        for entry in fs::read_dir(dir).ok()?.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Ok(raw) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(meta) = serde_json::from_str::<serde_json::Value>(&raw) else {
                continue;
            };
            let Some(pid) = meta["pid"].as_i64().and_then(|p| i32::try_from(p).ok()) else {
                continue;
            };
            if !belongs_to(pid, self.pid) {
                continue;
            }
            if !meta["cwd"]
                .as_str()
                .is_some_and(|cwd| crate::agents::same_directory(cwd, &self.project))
            {
                continue;
            }
            let Some(status) = meta["status"].as_str() else {
                continue;
            };
            if found.is_some() {
                // Two entries claim this process: neither is evidence.
                return None;
            }
            found = Some(ProviderStatus {
                status: status.to_owned(),
                conversation: meta["sessionId"].as_str().map(str::to_owned),
                updated_at: meta["statusUpdatedAt"].as_u64().map(u128::from),
            });
        }
        found
    }

    // ----------------------------------------------------------------------------------- view

    pub(crate) fn view(&self, max_items: usize) -> View<'_> {
        let content = self.settings.enabled && !self.sources.private_runtime;
        let skip = self.items.len().saturating_sub(max_items);
        View {
            agent: self.record.map(|r| match r {
                Record::Claude => "claude",
                Record::Codex => "codex",
            }),
            stream: &self.settings,
            identity: Identity {
                session: self.session.clone(),
                conversation: self.conversation.clone(),
                attempt: self.attempt,
                attempt_started_at: self.attempt_started_at,
            },
            continuity: ContinuityView {
                state: self.continuity,
                reason: self.continuity_reason.clone(),
                expected_conversation: self.expected.clone(),
            },
            readiness: self.readiness(),
            can_send: self.can_send(),
            send_contract: if self.sendable {
                "terminal"
            } else {
                "unsupported"
            },
            send_note: self.send_note(),
            items: if content {
                self.items
                    .iter()
                    .skip(skip)
                    .map(|(seq, item)| ViewItem { seq, item })
                    .collect()
            } else {
                Vec::new()
            },
            earlier_omitted: content && (self.items.omitted || skip > 0),
            read_problem: self.read_problem.as_deref(),
            last_read_at: self.last_read_at,
            deliveries: &self.deliveries,
            provider: self.provider.as_ref(),
            permission_sign: self.permission_sign,
            events: &self.events,
        }
    }
}

/// Whether a process is the hosted one, or was started by it as session leader.
fn belongs_to(pid: i32, hosted: i32) -> bool {
    if pid == hosted {
        return true;
    }
    #[cfg(unix)]
    {
        // SAFETY: getsid only reads the session id of a process; a dead pid returns -1.
        let sid = unsafe { libc::getsid(pid) };
        sid == hosted
    }
    #[cfg(not(unix))]
    {
        false
    }
}

fn record_path(record: Record, home: &Path, project: &Path, conversation: &str) -> Option<PathBuf> {
    match record {
        Record::Claude => Some(
            home.join(".claude")
                .join("projects")
                .join(crate::agents::claude_project_dir(project))
                .join(format!("{conversation}.jsonl")),
        )
        .filter(|path| path.exists()),
        Record::Codex => RecordTail::find_existing(record, home, project, conversation)
            .map(|tail| tail.path().to_path_buf()),
    }
}

fn read_problem(error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => {
            "The agent's log could not be read: permission denied.".to_owned()
        }
        std::io::ErrorKind::NotFound => "The agent's log is no longer where it was.".to_owned(),
        _ => format!("The agent's log could not be read: {error}"),
    }
}

fn short(id: &str) -> String {
    if id.len() <= 12 {
        id.to_owned()
    } else {
        format!("{}…{}", &id[..4], &id[id.len() - 4..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    struct Fixture {
        root: PathBuf,
        home: PathBuf,
        store: PathBuf,
        project: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("verb-conversation-{}", crate::new_id()));
            let home = root.join("home");
            let store = root.join("store");
            let project = root.join("project");
            for dir in [&home, &store, &project] {
                fs::create_dir_all(dir).unwrap();
            }
            Self {
                root,
                home,
                store,
                project,
            }
        }

        fn sources(&self) -> Sources {
            Sources {
                home: Some(self.home.clone()),
                store: Some(self.store.clone()),
                private_runtime: false,
            }
        }

        fn stream(&self, enabled: bool) {
            transcript::set_enabled(&self.store, enabled).unwrap();
        }

        fn log_path(&self, conversation: &str) -> PathBuf {
            let dir = self
                .home
                .join(".claude")
                .join("projects")
                .join(crate::agents::claude_project_dir(&self.project));
            fs::create_dir_all(&dir).unwrap();
            dir.join(format!("{conversation}.jsonl"))
        }

        fn append(&self, conversation: &str, text: &str) {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.log_path(conversation))
                .unwrap();
            file.write_all(text.as_bytes()).unwrap();
        }

        fn status(&self, pid: i32, conversation: &str, status: &str) {
            let dir = self.home.join(".claude").join("sessions");
            fs::create_dir_all(&dir).unwrap();
            fs::write(
                dir.join(format!("{pid}.json")),
                serde_json::json!({"pid": pid, "sessionId": conversation, "cwd": self.project,
                    "status": status, "statusUpdatedAt": crate::now_millis()})
                .to_string(),
            )
            .unwrap();
        }

        fn engine(&self, attempt: u32, expected: Option<&str>) -> Engine {
            Engine::for_agent(
                Some("claude"),
                &self.project,
                "verb-session",
                attempt,
                expected,
                PID,
                self.sources(),
            )
            .unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    // A pid no real process uses, so `getsid` never matches by accident.
    const PID: i32 = 999_999;

    fn user(uuid: Option<&str>, conversation: &str, at: Option<&str>, text: &str) -> String {
        let mut v = serde_json::json!({"type": "user", "sessionId": conversation,
            "message": {"role": "user", "content": text}});
        if let Some(uuid) = uuid {
            v["uuid"] = serde_json::json!(uuid);
        }
        if let Some(at) = at {
            v["timestamp"] = serde_json::json!(at);
        }
        format!("{v}\n")
    }

    fn agent(conversation: &str, text: &str) -> String {
        format!(
            "{}\n",
            serde_json::json!({"type": "assistant", "sessionId": conversation, "uuid": "a-1",
                "timestamp": "2026-10-27T10:00:01.000Z",
                "message": {"stop_reason": "end_turn", "content": [{"type": "text", "text": text}]}})
        )
    }

    fn tick(engine: &mut Engine, conversation: Option<&str>) {
        engine.update(Instant::now(), "", conversation, true, true);
    }

    fn ready(f: &Fixture) -> Engine {
        f.stream(true);
        f.status(PID, "conv-1", "idle");
        f.append("conv-1", &agent("conv-1", "hello"));
        let mut engine = f.engine(1, None);
        tick(&mut engine, Some("conv-1"));
        assert_eq!(
            engine.readiness().state,
            Readiness::Ready,
            "{:?}",
            engine.readiness()
        );
        engine
    }

    fn send(engine: &mut Engine, id: &str, text: &str) -> Result<Option<Vec<u8>>, Refusal> {
        engine.prepare_send(
            id,
            text,
            Origin::Desktop,
            SendContext {
                screen: "",
                conversation: Some("conv-1"),
                running: true,
                bracketed_paste: true,
            },
        )
    }

    #[test]
    fn content_is_read_only_while_the_stream_is_on() {
        let f = Fixture::new();
        f.status(PID, "conv-1", "idle");
        f.append("conv-1", &agent("conv-1", "secret plan"));
        let mut engine = f.engine(1, None);
        tick(&mut engine, Some("conv-1"));
        assert_eq!(engine.readiness().state, Readiness::StreamOff);
        assert!(engine.view(100).items.is_empty());
        f.stream(true);
        tick(&mut engine, Some("conv-1"));
        assert_eq!(engine.view(100).items.len(), 1);
    }

    #[test]
    fn turning_the_stream_off_clears_text_at_once_and_leaves_the_log_alone() {
        let f = Fixture::new();
        let mut engine = ready(&f);
        let bytes = send(&mut engine, "m1", "please fix the tests").unwrap();
        assert!(bytes.is_some());
        engine.finish_send("m1", Ok(()));
        let before = fs::read(f.log_path("conv-1")).unwrap();
        let generation = engine.generation();
        f.stream(false);
        tick(&mut engine, Some("conv-1"));
        assert!(engine.generation() > generation);
        let view = engine.view(100);
        assert!(view.items.is_empty());
        assert!(view.deliveries.iter().all(|d| d.text.is_none()));
        assert_eq!(view.deliveries[0].state, DeliveryState::Unknown);
        assert_eq!(
            fs::read(f.log_path("conv-1")).unwrap(),
            before,
            "the agent's log is untouched"
        );
        let json = serde_json::to_string(&view).unwrap();
        assert!(
            !json.contains("hello") && !json.contains("please fix"),
            "{json}"
        );
        // Turning it back on reads the conversation again from the agent's own record.
        f.stream(true);
        tick(&mut engine, Some("conv-1"));
        assert!(!engine.view(100).items.is_empty());
    }

    #[test]
    fn a_torn_last_line_waits_for_the_rest_of_itself() {
        let f = Fixture::new();
        let mut engine = ready(&f);
        let line = agent("conv-1", "second answer");
        let (head, tail) = line.split_at(line.len() / 2);
        f.append("conv-1", head);
        tick(&mut engine, Some("conv-1"));
        assert_eq!(engine.view(100).items.len(), 1, "half a line is not parsed");
        f.append("conv-1", tail);
        tick(&mut engine, Some("conv-1"));
        let view = engine.view(100);
        assert_eq!(view.items.len(), 2, "and is not lost");
        assert_eq!(view.items[1].item.text.as_deref(), Some("second answer"));
    }

    #[test]
    fn a_message_is_received_only_with_all_four_proofs() {
        let f = Fixture::new();
        let mut engine = ready(&f);
        send(&mut engine, "m1", "run the tests").unwrap();
        engine.finish_send("m1", Ok(()));
        tick(&mut engine, Some("conv-1"));
        assert_eq!(engine.deliveries[0].state, DeliveryState::Sent);
        f.append(
            "conv-1",
            &user(
                Some("u-9"),
                "conv-1",
                Some("2026-10-27T10:05:00.120Z"),
                "run the tests",
            ),
        );
        tick(&mut engine, Some("conv-1"));
        let d = &engine.deliveries[0];
        assert_eq!(d.state, DeliveryState::Received);
        assert_eq!(d.turn_id.as_deref(), Some("u-9"));
        assert_eq!(d.received_at.as_deref(), Some("2026-10-27T10:05:00.120Z"));
    }

    #[test]
    fn an_earlier_identical_message_never_confirms_a_new_one() {
        let f = Fixture::new();
        f.append(
            "conv-1",
            &user(Some("u-old"), "conv-1", Some("2026-10-27T09:00:00Z"), "yes"),
        );
        let mut engine = ready(&f);
        send(&mut engine, "m1", "yes").unwrap();
        engine.finish_send("m1", Ok(()));
        tick(&mut engine, Some("conv-1"));
        assert_eq!(engine.deliveries[0].state, DeliveryState::Sent);
        engine.check("m1", "", Some("conv-1"), true).unwrap();
        let d = &engine.deliveries[0];
        assert_eq!(d.state, DeliveryState::Sent);
        assert!(d.checked_at.is_some());
        assert!(
            d.reason.as_deref().unwrap().contains("earlier identical"),
            "{d:?}"
        );
    }

    #[test]
    fn missing_ids_and_times_leave_delivery_unproven() {
        let f = Fixture::new();
        let mut engine = ready(&f);
        send(&mut engine, "m1", "first").unwrap();
        engine.finish_send("m1", Ok(()));
        f.append(
            "conv-1",
            &user(None, "conv-1", Some("2026-10-27T10:00:00Z"), "first"),
        );
        engine.check("m1", "", Some("conv-1"), true).unwrap();
        assert_eq!(engine.deliveries[0].state, DeliveryState::Sent);
        assert!(engine.deliveries[0]
            .reason
            .as_deref()
            .unwrap()
            .contains("no id"));

        // A time that is not a time is never replaced with "now".
        engine.discard("m1").unwrap();
        f.append("conv-1", &agent("conv-1", "turn complete"));
        send(&mut engine, "m2", "second").unwrap();
        engine.finish_send("m2", Ok(()));
        f.append(
            "conv-1",
            &user(Some("u-2"), "conv-1", Some("yesterday"), "second"),
        );
        engine.check("m2", "", Some("conv-1"), true).unwrap();
        let d = engine.deliveries.iter().find(|d| d.id == "m2").unwrap();
        assert_eq!(d.state, DeliveryState::Sent);
        assert!(d.reason.as_deref().unwrap().contains("no usable time"));
        assert!(d.received_at.is_none());
    }

    #[test]
    fn the_same_text_in_another_conversation_proves_nothing() {
        let f = Fixture::new();
        let mut engine = ready(&f);
        send(&mut engine, "m1", "ship it").unwrap();
        engine.finish_send("m1", Ok(()));
        // A decoy: the same words, in this file, but naming another conversation.
        f.append(
            "conv-1",
            &user(
                Some("u-x"),
                "conv-other",
                Some("2026-10-27T10:00:00Z"),
                "ship it",
            ),
        );
        engine.check("m1", "", Some("conv-1"), true).unwrap();
        assert_eq!(engine.deliveries[0].state, DeliveryState::Sent);
        assert!(engine.deliveries[0]
            .reason
            .as_deref()
            .unwrap()
            .contains("different conversation"));
    }

    #[test]
    fn chat_fails_closed_whenever_readiness_is_not_established() {
        let f = Fixture::new();
        f.stream(true);
        f.append("conv-1", &agent("conv-1", "hi"));
        let mut engine = f.engine(1, None);
        // No status file at all: unknown, so refused.
        let refusal = send(&mut engine, "m1", "hello").unwrap_err();
        assert_eq!(refusal.code, "not_ready");
        assert!(
            engine.deliveries.is_empty(),
            "a refusal records and writes nothing"
        );
        // A busy agent: refused.
        f.status(PID, "conv-1", "busy");
        assert_eq!(
            send(&mut engine, "m1", "hello").unwrap_err().code,
            "not_ready"
        );
        // Idle, but the screen looks like a permission prompt: refused.
        f.status(PID, "conv-1", "idle");
        let refusal = engine
            .prepare_send(
                "m1",
                "hello",
                Origin::Desktop,
                SendContext {
                    screen: "Do you want to proceed?",
                    conversation: Some("conv-1"),
                    running: true,
                    bracketed_paste: true,
                },
            )
            .unwrap_err();
        assert_eq!(refusal.code, "not_ready");
        assert_eq!(engine.readiness().state, Readiness::Approval);
        // Ended: refused.
        let refusal = engine
            .prepare_send(
                "m1",
                "hello",
                Origin::Desktop,
                SendContext {
                    screen: "",
                    conversation: Some("conv-1"),
                    running: false,
                    bracketed_paste: true,
                },
            )
            .unwrap_err();
        assert_eq!(refusal.code, "not_ready");
    }

    #[test]
    fn another_processes_status_is_not_this_terminals_readiness() {
        let f = Fixture::new();
        f.stream(true);
        f.append("conv-1", &agent("conv-1", "hi"));
        // A decoy: an idle Claude in the same project, but a different process.
        f.status(PID + 1, "conv-1", "idle");
        let mut engine = f.engine(1, None);
        tick(&mut engine, Some("conv-1"));
        assert_eq!(engine.readiness().state, Readiness::Unknown);
    }

    #[test]
    fn a_message_id_is_written_at_most_once_and_a_second_waits() {
        let f = Fixture::new();
        let mut engine = ready(&f);
        assert!(send(&mut engine, "m1", "one").unwrap().is_some());
        engine.finish_send("m1", Ok(()));
        // A retried request after a dropped connection writes nothing.
        assert_eq!(send(&mut engine, "m1", "one").unwrap(), None);
        // A different message cannot overtake the unconfirmed one.
        assert_eq!(
            send(&mut engine, "m2", "two").unwrap_err().code,
            "unresolved"
        );
    }

    #[test]
    fn a_failed_write_is_not_sent_and_does_not_block_the_next() {
        let f = Fixture::new();
        let mut engine = ready(&f);
        send(&mut engine, "m1", "one").unwrap();
        engine.finish_send(
            "m1",
            Err(crate::mobile::ChatWriteError {
                started: false,
                reason: "phone controls input for this session".to_owned(),
            }),
        );
        assert_eq!(engine.deliveries[0].state, DeliveryState::Failed);
        assert!(send(&mut engine, "m2", "two").unwrap().is_some());
    }

    #[test]
    fn multi_line_messages_need_the_agents_paste_mode() {
        let f = Fixture::new();
        let mut engine = ready(&f);
        let refusal = engine
            .prepare_send(
                "m1",
                "a\nb",
                Origin::Desktop,
                SendContext {
                    screen: "",
                    conversation: Some("conv-1"),
                    running: true,
                    bracketed_paste: false,
                },
            )
            .unwrap_err();
        assert_eq!(refusal.code, "multiline");
        let bytes = send(&mut engine, "m2", "a\nb").unwrap().unwrap();
        assert_eq!(bytes, b"\x1b[200~a\nb\x1b[201~\r");
    }

    #[test]
    fn codex_conversations_are_shown_but_never_sent_from_chat() {
        let f = Fixture::new();
        let engine =
            Engine::for_agent(Some("codex"), &f.project, "s", 1, None, PID, f.sources()).unwrap();
        assert_eq!(engine.view(10).send_contract, "unsupported");
        assert!(!engine.can_send());
        let opencode =
            Engine::for_agent(Some("opencode"), &f.project, "s", 1, None, PID, f.sources())
                .unwrap();
        assert_eq!(opencode.readiness().state, Readiness::Unsupported);
        assert!(Engine::for_agent(None, &f.project, "s", 1, None, PID, f.sources()).is_none());
    }

    #[test]
    fn continuity_is_claimed_only_from_the_agents_evidence() {
        let f = Fixture::new();
        f.stream(true);
        let mut resumed = f.engine(2, Some("conv-1"));
        tick(&mut resumed, Some("conv-1"));
        assert_eq!(resumed.continuity, Continuity::Resumed);

        let mut moved = f.engine(3, Some("conv-1"));
        tick(&mut moved, Some("conv-2"));
        assert_eq!(moved.continuity, Continuity::NewConversation);

        // The log and the status disagree: neither wins.
        f.status(PID, "conv-9", "idle");
        let mut torn = f.engine(4, Some("conv-1"));
        tick(&mut torn, Some("conv-1"));
        assert_eq!(torn.continuity, Continuity::NotEstablished);
        assert_eq!(torn.readiness().state, Readiness::Unidentified);
        assert!(torn.view(10).items.is_empty());
    }

    #[test]
    fn an_isolated_runtime_is_never_read() {
        let f = Fixture::new();
        f.stream(true);
        f.append("conv-1", &agent("conv-1", "private"));
        let mut sources = f.sources();
        sources.private_runtime = true;
        let mut engine =
            Engine::for_agent(Some("claude"), &f.project, "s", 1, None, PID, sources).unwrap();
        tick(&mut engine, Some("conv-1"));
        assert_eq!(engine.readiness().state, Readiness::Unsupported);
        assert!(engine.view(10).items.is_empty());
    }

    #[test]
    fn a_process_that_ends_leaves_unconfirmed_messages_unknown() {
        let f = Fixture::new();
        let mut engine = ready(&f);
        send(&mut engine, "m1", "go").unwrap();
        engine.finish_send("m1", Ok(()));
        engine.update(Instant::now(), "", Some("conv-1"), false, true);
        assert_eq!(engine.deliveries[0].state, DeliveryState::Unknown);
        assert_eq!(engine.readiness().state, Readiness::Ended);
        engine.discard("m1").unwrap();
        assert_eq!(engine.deliveries[0].state, DeliveryState::Discarded);
        assert!(engine.deliveries[0]
            .reason
            .as_deref()
            .unwrap()
            .contains("may still"));
    }

    #[test]
    fn redaction_and_truncation_never_supply_confirmation() {
        for (sent, other) in [
            (
                "password=original123".to_owned(),
                "password=different456".to_owned(),
            ),
            (
                format!("{}A", "a".repeat(6500)),
                format!("{}B", "a".repeat(6500)),
            ),
        ] {
            let f = Fixture::new();
            let mut engine = ready(&f);
            send(&mut engine, "m1", &sent).unwrap();
            engine.finish_send("m1", Ok(()));
            f.append(
                "conv-1",
                &user(
                    Some("wrong"),
                    "conv-1",
                    Some("2026-10-27T10:01:00Z"),
                    &other,
                ),
            );
            tick(&mut engine, Some("conv-1"));
            assert_eq!(engine.deliveries[0].state, DeliveryState::Sent);
            f.append(
                "conv-1",
                &user(Some("right"), "conv-1", Some("2026-10-27T10:01:01Z"), &sent),
            );
            tick(&mut engine, Some("conv-1"));
            assert_eq!(engine.deliveries[0].turn_id.as_deref(), Some("right"));
            let json = serde_json::to_string(&engine.view(100)).unwrap();
            assert!(!json.contains("fingerprint"));
        }
    }

    #[test]
    fn write_attempt_errors_stay_unknown_until_resolved() {
        let f = Fixture::new();
        let mut engine = ready(&f);
        send(&mut engine, "m1", "hello").unwrap();
        engine.finish_send(
            "m1",
            Err(crate::mobile::ChatWriteError {
                started: true,
                reason: "broken pipe after prefix".to_owned(),
            }),
        );
        assert_eq!(engine.deliveries[0].state, DeliveryState::Unknown);
        assert_eq!(
            send(&mut engine, "m2", "another").unwrap_err().code,
            "unresolved"
        );
    }

    #[test]
    fn idle_without_completion_or_current_metadata_is_not_ready() {
        let f = Fixture::new();
        let mut engine = ready(&f);
        f.append(
            "conv-1",
            &user(
                Some("u1"),
                "conv-1",
                Some("2026-10-27T10:00:00Z"),
                "still running",
            ),
        );
        tick(&mut engine, Some("conv-1"));
        assert!(!engine.can_send());
        f.append("conv-1", &agent("conv-1", "complete"));
        tick(&mut engine, Some("conv-1"));
        assert!(engine.can_send());
        engine.provider.as_mut().unwrap().updated_at = None;
        assert!(!engine.can_send());
        engine.provider.as_mut().unwrap().updated_at = Some(1);
        assert!(!engine.can_send());
    }

    #[test]
    fn evicted_receipts_do_not_allow_replaying_an_old_message_id() {
        let f = Fixture::new();
        let mut engine = ready(&f);
        for n in 0..20 {
            let id = format!("m{n}");
            send(&mut engine, &id, "hello").unwrap();
            engine.finish_send(&id, Ok(()));
            engine.discard(&id).unwrap();
        }
        assert!(!engine.deliveries.iter().any(|d| d.id == "m0"));
        assert_eq!(send(&mut engine, "m0", "hello").unwrap(), None);
    }

    #[test]
    fn agent_times_with_fractions_parse_and_others_do_not() {
        assert_eq!(
            crate::context::parse_iso8601_utc("1970-01-01T00:00:01.250Z"),
            Some(1250)
        );
        assert_eq!(
            crate::context::parse_iso8601_utc("1970-01-01T00:00:01Z"),
            Some(1000)
        );
        assert_eq!(
            crate::context::parse_iso8601_utc("1970-01-01T00:00:01.Z"),
            None
        );
        assert_eq!(crate::context::parse_iso8601_utc("yesterday"), None);
    }
}
