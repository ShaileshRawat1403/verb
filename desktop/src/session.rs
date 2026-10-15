//! Durable session records and provider recovery contracts. Process ownership is tracked by
//! each host separately; a persisted live label is evidence to reconcile, not a PTY handle.
use crate::{
    agents, default_shell, isolation, new_id, now_millis, optional_number, optional_path,
    optional_string, program_on_path, valid_resume_identity,
};
use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Agent {
    Shell,
    Claude,
    Codex,
    Gemini,
    Agy,
    OpenCode,
    Dsh,
    External,
    Custom(String),
}

impl Agent {
    pub(crate) fn parse(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "shell" => Self::Shell,
            "claude" => Self::Claude,
            "codex" => Self::Codex,
            "gemini" => Self::Gemini,
            "agy" | "antigravity" => Self::Agy,
            "opencode" | "open-code" => Self::OpenCode,
            "dsh" | "deepseek" => Self::Dsh,
            "external" => Self::External,
            other => Self::Custom(other.to_owned()),
        }
    }

    pub(crate) fn label(&self) -> &str {
        match self {
            Self::Shell => "shell",
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::Agy => "agy",
            Self::OpenCode => "opencode",
            Self::Dsh => "dsh",
            Self::External => "external",
            // The executable is volatile launch input, never a durable runtime identifier. Using
            // it here would write full command text into the session record and event log.
            Self::Custom(_) => "custom",
        }
    }

    pub(crate) fn command(&self) -> String {
        match self {
            Self::Shell => default_shell(),
            Self::Claude => "claude".to_owned(),
            Self::Codex => "codex".to_owned(),
            Self::Gemini => "gemini".to_owned(),
            Self::Agy => "agy".to_owned(),
            Self::OpenCode => "opencode".to_owned(),
            Self::Dsh => "dsh".to_owned(),
            Self::External => "external".to_owned(),
            Self::Custom(value) => value.clone(),
        }
    }

    /// Whether this agent's program can be started here: found on Verb's own `PATH` (the one a
    /// hosted terminal is started with), or an executable path. A missing agent used to start a
    /// terminal that failed at exec and showed an empty tile (Antigravity on Node 1).
    pub(crate) fn installed(&self) -> bool {
        if matches!(self, Self::Shell | Self::External | Self::Custom(_)) {
            return true;
        }
        program_on_path(&self.command(), env::var_os("PATH").as_deref())
    }

    /// Flags Verb adds whenever it starts this agent, new session or resumed.
    ///
    /// Codex boots the account's app connectors at startup, which cost tens of seconds before the
    /// user can type. Verb turns them off; MCP servers the user configures themselves are
    /// unaffected. Android's `RuntimeProfiles` makes the same choice, so an agent behaves the same
    /// on both hosts.
    pub(crate) fn launch_flags(&self) -> Vec<String> {
        match self {
            Self::Codex => vec!["--disable".to_owned(), "apps".to_owned()],
            _ => Vec::new(),
        }
    }

    /// How this agent is told to continue a specific conversation.
    ///
    /// Every form here was read from the installed CLI's own help output, not assumed. A resume
    /// must name this session's exact conversation; an absent or unsafe id never falls back to the
    /// agent's latest conversation.
    pub(crate) fn resume_args(&self, resume_identity: &str) -> Option<Vec<String>> {
        let id = valid_resume_identity(resume_identity)?;
        let mut args = self.launch_flags();
        args.extend(self.resume_subcommand(id)?);
        Some(args)
    }

    pub(crate) fn resume_subcommand(&self, id: &str) -> Option<Vec<String>> {
        match self {
            Self::Claude => Some(vec!["--resume".to_owned(), id.to_owned()]),
            Self::Codex => Some(vec!["resume".to_owned(), id.to_owned()]),
            Self::OpenCode => Some(vec!["--session".to_owned(), id.to_owned()]),
            _ => None,
        }
    }

    /// Whether this agent has a conversation worth recovering for `project`.
    ///
    /// `Dsh` is deliberately `Unknown` rather than `No`: its resume contract has not been observed
    /// on a real install yet, and guessing one would either strand a recoverable session or promise
    /// a recovery that does not work.
    pub(crate) fn resume_verdict(&self, project: &Path, identity: Option<&str>) -> ResumeVerdict {
        if matches!(self, Self::Shell | Self::External | Self::Custom(_)) {
            return ResumeVerdict::No;
        }
        if identity.is_some_and(|id| valid_resume_identity(id).is_none()) {
            return ResumeVerdict::Unknown;
        }
        if isolation::profile().is_ok_and(|p| p.is_some()) {
            if identity.is_none() {
                return ResumeVerdict::Unknown;
            }
            return match isolation::recovery_evidence(self.label(), project, None, identity)
                .map(|e| e.status)
                .as_deref()
            {
                Some("yes") => ResumeVerdict::Yes,
                Some("no") => ResumeVerdict::No,
                _ => ResumeVerdict::Unknown,
            };
        }
        let Some(home) = isolation::agent_home() else {
            return ResumeVerdict::Unknown;
        };
        match (self, identity) {
            (Self::Claude, Some(id)) => agents::claude_verdict_for(project, &home, id),
            (Self::Codex, Some(id)) => agents::codex_verdict_for(project, &home, id),
            (Self::OpenCode, Some(id)) => agents::opencode_verdict_for(project, &home, id),
            // A project may have many conversations. Without this session's exact identity,
            // another agent session's evidence must never make this one recoverable.
            (Self::Claude | Self::Codex | Self::OpenCode, None) => ResumeVerdict::Unknown,
            (Self::Shell | Self::External | Self::Custom(_), _) => ResumeVerdict::No,
            (Self::Gemini | Self::Dsh | Self::Agy, _) => ResumeVerdict::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResumeVerdict {
    Yes,
    No,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SessionState {
    Live,
    Interrupted,
    Recoverable,
    Ended,
}

impl SessionState {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Interrupted => "interrupted",
            Self::Recoverable => "recoverable",
            Self::Ended => "ended",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "live" => Some(Self::Live),
            "interrupted" => Some(Self::Interrupted),
            "recoverable" => Some(Self::Recoverable),
            "ended" => Some(Self::Ended),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Session {
    pub(crate) id: String,
    pub(crate) project_id: PathBuf,
    /// Stable Verb project ID. Older records have none and are matched by checkout path.
    pub(crate) verb_project_id: Option<String>,
    pub(crate) runtime_id: Option<String>,
    pub(crate) last_known_cwd: Option<PathBuf>,
    pub(crate) last_observed_at: Option<u128>,
    pub(crate) created_at: u128,
    pub(crate) last_seen_at: u128,
    pub(crate) state: SessionState,
    pub(crate) agent: Option<Agent>,
    /// The agent's own conversation id, per `docs/VERB_SESSION_SCHEMA.md`'s `agent.resumeIdentity`.
    /// Opaque to the session machinery -- only the agent interprets it.
    pub(crate) resume_identity: Option<String>,
}

impl Session {
    pub(crate) fn display_agent(&self) -> &str {
        match self.agent.as_ref() {
            Some(Agent::External) => self.runtime_id.as_deref().unwrap_or("external"),
            Some(agent) => agent.label(),
            None => "shell",
        }
    }

    pub(crate) fn new(project: PathBuf, agent: Agent) -> Self {
        let now = now_millis();
        let runtime_id = Some(agent.label().to_owned());
        let tracked_agent = match agent {
            Agent::Shell | Agent::Custom(_) => None,
            _ => Some(agent),
        };
        Self {
            id: new_id(),
            project_id: project.clone(),
            verb_project_id: None,
            runtime_id,
            last_known_cwd: Some(project),
            last_observed_at: Some(now),
            created_at: now,
            last_seen_at: now,
            state: SessionState::Live,
            agent: tracked_agent,
            resume_identity: None,
        }
    }

    pub(crate) fn serialize(&self) -> String {
        format!(
            "schema_version=1\nsession_id={}\nproject_id={}\nverb_project_id={}\nruntime_id={}\nlast_known_cwd={}\nlast_observed_at={}\ncreated_at={}\nlast_seen_at={}\nstate={}\nagent={}\nresume_identity={}\n",
            self.id,
            self.project_id.display(),
            optional_string(self.verb_project_id.as_deref()),
            optional_string(self.runtime_id.as_deref()),
            optional_path(self.last_known_cwd.as_deref()),
            optional_number(self.last_observed_at),
            self.created_at,
            self.last_seen_at,
            self.state.as_str(),
            self.agent
                .as_ref()
                .map_or_else(String::new, |agent| agent.label().to_owned()),
            optional_string(self.resume_identity.as_deref()),
        )
    }

    pub(crate) fn deserialize(input: &str) -> Option<Self> {
        let mut values = std::collections::HashMap::new();

        for line in input.lines() {
            let (key, value) = line.split_once('=')?;
            // A repeated key means the record was written with a value that contained a newline.
            // Letting the later one win would let that value rewrite other fields.
            if values.insert(key, value).is_some() {
                return None;
            }
        }

        let id = values.get("session_id").or_else(|| values.get("id"))?;
        // The ID is also an event-log filename. Reject malformed or oversized records before a
        // later save can turn them into a path outside the supported session store.
        let id = valid_resume_identity(id).filter(|id| id.len() <= 100)?;
        let project_value = values.get("project_id").or_else(|| values.get("project"))?;
        let agent_value = values.get("agent").copied().unwrap_or_default();
        let agent = if agent_value.is_empty() {
            None
        } else {
            match Agent::parse(agent_value) {
                Agent::Shell | Agent::Custom(_) => None,
                parsed => Some(parsed),
            }
        };
        let legacy_started_at = values
            .get("started_at")
            .and_then(|value| value.parse::<u128>().ok())
            .map(|seconds| seconds * 1_000);
        let created_at = values
            .get("created_at")
            .and_then(|value| value.parse().ok())
            .or(legacy_started_at)
            .unwrap_or_else(now_millis);
        let last_seen_at = values
            .get("last_seen_at")
            .and_then(|value| value.parse().ok())
            .unwrap_or(created_at);
        let runtime_id = values
            .get("runtime_id")
            .or_else(|| values.get("agent"))
            .filter(|value| !value.is_empty())
            .map(|value| (*value).to_owned());
        let last_known_cwd = values
            .get("last_known_cwd")
            .or_else(|| values.get("project"))
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        let verb_project_id = match values.get("verb_project_id") {
            Some(value) if !value.is_empty() => {
                if value.len() != 32 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                    return None;
                }
                Some((*value).to_owned())
            }
            _ => None,
        };
        Some(Self {
            id: (*id).to_owned(),
            project_id: PathBuf::from(project_value),
            verb_project_id,
            runtime_id,
            last_known_cwd,
            last_observed_at: values
                .get("last_observed_at")
                .and_then(|value| value.parse().ok()),
            created_at,
            last_seen_at,
            state: values
                .get("state")
                .and_then(|value| SessionState::parse(value))?,
            agent,
            resume_identity: values
                .get("resume_identity")
                .filter(|value| !value.is_empty())
                .and_then(|value| valid_resume_identity(value))
                .map(str::to_owned),
        })
    }
}
