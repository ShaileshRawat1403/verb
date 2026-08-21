use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(not(unix))]
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};

mod agents;
mod pty;
mod shell;
#[cfg(unix)]
mod ui;

const APP_NAME: &str = "Verb";

#[derive(Debug, Clone, PartialEq, Eq)]
enum Agent {
    Shell,
    Claude,
    Codex,
    OpenCode,
    Dsh,
    Custom(String),
}

impl Agent {
    fn parse(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "shell" => Self::Shell,
            "claude" => Self::Claude,
            "codex" => Self::Codex,
            "opencode" | "open-code" => Self::OpenCode,
            "dsh" | "deepseek" => Self::Dsh,
            other => Self::Custom(other.to_owned()),
        }
    }

    fn label(&self) -> &str {
        match self {
            Self::Shell => "shell",
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::OpenCode => "opencode",
            Self::Dsh => "dsh",
            Self::Custom(value) => value,
        }
    }

    fn command(&self) -> String {
        match self {
            Self::Shell => default_shell(),
            Self::Claude => "claude".to_owned(),
            Self::Codex => "codex".to_owned(),
            Self::OpenCode => "opencode".to_owned(),
            Self::Dsh => "dsh".to_owned(),
            Self::Custom(value) => value.clone(),
        }
    }

    /// Flags Verb adds whenever it starts this agent, new session or resumed.
    ///
    /// Codex boots the account's app connectors at startup, which cost tens of seconds before the
    /// user can type. Verb turns them off; MCP servers the user configures themselves are
    /// unaffected. Android's `RuntimeProfiles` makes the same choice, so an agent behaves the same
    /// on both hosts.
    fn launch_flags(&self) -> Vec<String> {
        match self {
            Self::Codex => vec!["--disable".to_owned(), "apps".to_owned()],
            _ => Vec::new(),
        }
    }

    /// How this agent is told to continue a specific conversation.
    ///
    /// Every form here was read from the installed CLI's own help output, not assumed. Where no id
    /// is known, each falls back to that agent's "most recent session" flag rather than to an
    /// interactive picker, which would sit waiting for a keystroke Verb cannot supply.
    fn resume_args(&self, resume_identity: Option<&str>) -> Vec<String> {
        let mut args = self.launch_flags();
        args.extend(self.resume_subcommand(resume_identity));
        args
    }

    fn resume_subcommand(&self, resume_identity: Option<&str>) -> Vec<String> {
        match (self, resume_identity) {
            (Self::Claude, Some(id)) => vec!["--resume".to_owned(), id.to_owned()],
            (Self::Claude, None) => vec!["--continue".to_owned()],
            (Self::Codex, Some(id)) => vec!["resume".to_owned(), id.to_owned()],
            (Self::Codex, None) => vec!["resume".to_owned(), "--last".to_owned()],
            (Self::OpenCode, Some(id)) => vec!["--session".to_owned(), id.to_owned()],
            (Self::OpenCode, None) => vec!["--continue".to_owned()],
            _ => Vec::new(),
        }
    }

    /// Whether this agent has a conversation worth recovering for `project`.
    ///
    /// `Dsh` is deliberately `Unknown` rather than `No`: its resume contract has not been observed
    /// on a real install yet, and guessing one would either strand a recoverable session or promise
    /// a recovery that does not work.
    fn resume_verdict(&self, project: &Path) -> ResumeVerdict {
        let Some(home) = home_dir() else {
            return ResumeVerdict::Unknown;
        };
        match self {
            Self::Shell | Self::Custom(_) => ResumeVerdict::No,
            Self::Claude => agents::claude_verdict(project, &home),
            Self::Codex => agents::codex_verdict(project, &home),
            Self::OpenCode => agents::opencode_verdict(project, &home),
            Self::Dsh => ResumeVerdict::Unknown,
        }
    }

    /// The agent's own stable conversation id, when this host exposes one. Never a PID: the process
    /// is gone by the time this matters.
    fn resume_identity(&self, project: &Path) -> Option<String> {
        let home = home_dir()?;
        match self {
            Self::Claude => agents::claude_identity(project, &home),
            Self::Codex => agents::codex_identity(project, &home),
            Self::OpenCode => agents::opencode_identity(project, &home),
            Self::Shell | Self::Dsh | Self::Custom(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResumeVerdict {
    Yes,
    No,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SessionState {
    Live,
    Interrupted,
    Recoverable,
    Ended,
}

impl SessionState {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Interrupted => "interrupted",
            Self::Recoverable => "recoverable",
            Self::Ended => "ended",
        }
    }

    fn parse(value: &str) -> Option<Self> {
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
struct Session {
    id: String,
    project_id: PathBuf,
    runtime_id: Option<String>,
    last_known_cwd: Option<PathBuf>,
    last_observed_at: Option<u128>,
    created_at: u128,
    last_seen_at: u128,
    state: SessionState,
    agent: Option<Agent>,
    /// The agent's own conversation id, per `docs/VERB_SESSION_SCHEMA.md`'s `agent.resumeIdentity`.
    /// Opaque to the session machinery -- only the agent interprets it.
    resume_identity: Option<String>,
}

impl Session {
    fn new(project: PathBuf, agent: Agent) -> Self {
        let now = now_millis();
        let runtime_id = Some(agent.label().to_owned());
        let tracked_agent = match agent {
            Agent::Shell | Agent::Custom(_) => None,
            _ => Some(agent),
        };
        Self {
            id: new_id(),
            project_id: project.clone(),
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

    fn serialize(&self) -> String {
        format!(
            "schema_version=1\nsession_id={}\nproject_id={}\nruntime_id={}\nlast_known_cwd={}\nlast_observed_at={}\ncreated_at={}\nlast_seen_at={}\nstate={}\nagent={}\nresume_identity={}\n",
            self.id,
            self.project_id.display(),
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

    fn deserialize(input: &str) -> Option<Self> {
        let mut values = std::collections::HashMap::new();

        for line in input.lines() {
            let (key, value) = line.split_once('=')?;
            values.insert(key, value);
        }

        let id = values.get("session_id").or_else(|| values.get("id"))?;
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
            .map(|value| PathBuf::from(value));
        Some(Self {
            id: (*id).to_owned(),
            project_id: PathBuf::from(project_value),
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
                .map(|value| (*value).to_owned()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GitSnapshot {
    root: Option<PathBuf>,
    branch: Option<String>,
    changed_files: usize,
}

/// Exit codes, kept few and documented (`verb help`) because anything that scripts against Verb
/// depends on them staying put.
///
/// The distinction that earns its place is [`EXIT_NOTHING_TO_DO`]: "there is no recoverable session
/// here" is not a failure of Verb, and a script that retries on failure should not retry on it.
mod exit {
    pub const FAILURE: i32 = 1;
    pub const USAGE: i32 = 2;
    pub const NOTHING_TO_DO: i32 = 3;
}

/// An error on its way to the exit code it should produce.
#[derive(Debug)]
struct Failure {
    message: String,
    code: i32,
}

impl Failure {
    fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code,
        }
    }
}

/// Ordinary errors are failures; the call sites that mean something more specific say so.
impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self::new(exit::FAILURE, message)
    }
}

fn main() {
    if let Err(failure) = run() {
        eprintln!("{APP_NAME}: {}", failure.message);
        std::process::exit(failure.code);
    }
}

fn run() -> Result<(), Failure> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(default_command);
    let mut rest: Vec<String> = args.collect();
    // `--json` is a global flag on the read commands rather than a per-command parser: there is one
    // spelling to learn, and a machine consumer never has to care which command it is asking.
    let json = take_flag(&mut rest, "--json");
    let project = project_root_or_current()?;

    match command.as_str() {
        "help" | "--help" | "-h" => print_help(),
        "version" | "--version" | "-V" => println!("{APP_NAME} {}", env!("CARGO_PKG_VERSION")),
        "status" => print_status(&project, json)?,
        "sessions" => print_sessions(json)?,
        #[cfg(unix)]
        "ui" => ui::run()?,
        "resume" => resume_session(&project)?,
        "shell" => launch_session(&project, Agent::Shell, rest)?,
        "claude" | "codex" | "opencode" | "open-code" | "dsh" | "deepseek" => {
            launch_session(&project, Agent::parse(&command), rest)?
        }
        "run" => {
            if rest.is_empty() {
                return Err(Failure::new(exit::USAGE, "verb run needs a command"));
            }
            let command = rest.remove(0);
            launch_session(&project, Agent::Custom(command), rest)?;
        }
        other => {
            return Err(Failure::new(
                exit::USAGE,
                format!("unknown command '{other}'. Run 'verb help'."),
            ))
        }
    }

    Ok(())
}

/// What bare `verb` does.
///
/// On a terminal it opens the UI, because the work context *is* the product and a tool whose
/// primary surface is one keystroke away is easier to keep than one whose primary surface has a
/// name you must remember. `verb shell`, which used to be the bare default, is still there.
///
/// Off a terminal -- piped, redirected, in CI -- it prints help instead and touches nothing.
/// Launching a UI or a shell into a pipe would hang waiting for input nobody is typing, and a bare
/// command should never have side effects that depend on where its output happens to be going.
/// Every action the UI offers is also a subcommand, so nothing here is reachable only by hand.
fn default_command() -> String {
    let interactive = cfg!(unix) && io::stdin().is_terminal() && io::stdout().is_terminal();
    if interactive { "ui" } else { "help" }.to_owned()
}

/// Removes `flag` from `args` if present, reporting whether it was there.
fn take_flag(args: &mut Vec<String>, flag: &str) -> bool {
    match args.iter().position(|value| value == flag) {
        Some(index) => {
            args.remove(index);
            true
        }
        None => false,
    }
}

fn print_help() {
    println!(
        r#"Verb — a work-context shell for projects, Git, and agents

Usage:
  verb                 Open the session UI (help when not run in a terminal)
  verb shell           Open the work-context shell
  verb status          Show project, Git, and last session
  verb sessions        List every project Verb has a session for
  verb ui              Browse and resume sessions on a full screen
  verb version         Print the version
  verb claude          Launch Claude in the current project
  verb codex           Launch Codex in the current project
  verb opencode        Launch OpenCode in the current project
  verb dsh              Launch DeepSeek Harness in the current project
  verb run CMD ...     Launch any command in the current project
  verb resume          Resume the last known resumable session

Options:
  --json               Machine-readable output for status and sessions

Exit codes:
  0  success
  1  something failed
  2  the command line was wrong
  3  nothing to do (no session, or recovery is not confirmed)

The current directory selects the project. Verb stores only session metadata in ~/.verb;
agent credentials and transcripts remain owned by the agent."#
    );
}

/// Lists every project Verb has a session record for, newest first.
///
/// Read-only, deliberately: unlike `verb status`, which reconciles the project you are standing in,
/// this touches no state. Listing sessions should never rewrite them, and re-resolving a dozen
/// projects would write a dozen recovery-check events for a command that was only meant to look.
///
/// A recorded `LIVE` is reported as unconfirmed rather than as fact. Nothing durable holds a process
/// handle -- that is the contract -- so a *different* process, which is what this command always is,
/// has no way to prove the session it is reading about is still running.
fn print_sessions(json: bool) -> Result<(), String> {
    let directory = sessions_directory()?;
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            // An empty list, not an error and not a special case: a machine consumer gets `[]`.
            println!("{}", if json { "[]" } else { "No sessions yet." });
            return Ok(());
        }
        Err(error) => return Err(format!("could not read {}: {error}", directory.display())),
    };

    let mut sessions: Vec<Session> = entries
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|value| value == "session"))
        .filter_map(|entry| fs::read_to_string(entry.path()).ok())
        .filter_map(|contents| Session::deserialize(&contents))
        .collect();

    if sessions.is_empty() {
        println!("{}", if json { "[]" } else { "No sessions yet." });
        return Ok(());
    }

    sessions.sort_by(|left, right| right.last_seen_at.cmp(&left.last_seen_at));

    if json {
        let rows: Vec<String> = sessions.iter().map(session_json).collect();
        println!("[{}]", rows.join(","));
        return Ok(());
    }

    let now = now_millis();
    for session in &sessions {
        println!("{}", describe_session(session, now));
    }
    Ok(())
}

/// One session as the durable record `docs/VERB_SESSION_SCHEMA.md` describes -- the same field
/// names and the same ISO-8601 timestamps Android's records use, so a consumer reading one host's
/// output does not have to learn the other's.
///
/// Deliberately absent, here as everywhere: any process handle, PID, or `processPresent`. `state`
/// is what was recorded; a reader that needs to know whether a process exists must ask the host
/// that owns it, which is exactly why the field does not exist.
fn session_json(session: &Session) -> String {
    let agent = match session.agent.as_ref() {
        Some(agent) => format!(
            "{{\"agentType\":\"{}\",\"resumeIdentity\":{}}}",
            json_escape(agent.label()),
            match session.resume_identity.as_deref() {
                Some(identity) => format!("\"{}\"", json_escape(identity)),
                None => "null".to_owned(),
            }
        ),
        None => "null".to_owned(),
    };

    format!(
        "{{\"schemaVersion\":1,\"sessionId\":\"{}\",\"projectId\":\"{}\",\"runtimeId\":{},\"lastKnownCwd\":{},\"lastObservedAt\":{},\"createdAt\":\"{}\",\"lastSeenAt\":\"{}\",\"state\":\"{}\",\"agent\":{}}}",
        json_escape(&session.id),
        json_escape(&session.project_id.to_string_lossy()),
        json_string_or_null(session.runtime_id.as_deref()),
        json_string_or_null(session.last_known_cwd.as_ref().map(|path| path.to_string_lossy()).as_deref()),
        match session.last_observed_at {
            Some(millis) => format!("\"{}\"", iso8601(millis)),
            None => "null".to_owned(),
        },
        iso8601(session.created_at),
        iso8601(session.last_seen_at),
        session.state.as_str().to_uppercase(),
        agent
    )
}

fn json_string_or_null(value: Option<&str>) -> String {
    match value {
        Some(value) => format!("\"{}\"", json_escape(value)),
        None => "null".to_owned(),
    }
}

/// Milliseconds since the epoch as an ISO-8601 UTC timestamp, which is what the schema specifies.
///
/// Hand-rolled because the crate takes no dependencies: this is Howard Hinnant's `civil_from_days`,
/// which is exact for the whole proleptic Gregorian range rather than approximating months.
fn iso8601(millis: u128) -> String {
    let total_seconds = (millis / 1_000) as i64;
    let days = total_seconds.div_euclid(86_400);
    let seconds_of_day = total_seconds.rem_euclid(86_400);

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = if month <= 2 { year + 1 } else { year };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year,
        month,
        day,
        seconds_of_day / 3_600,
        (seconds_of_day % 3_600) / 60,
        seconds_of_day % 60
    )
}

/// One line per session: what state it is in, which agent, how long ago it was seen, and where.
fn describe_session(session: &Session, now: u128) -> String {
    let state = match session.state {
        SessionState::Live => "live?".to_owned(),
        _ => session.state.as_str().to_owned(),
    };
    let mut line = format!(
        "{:<12} {:<9} {:>9}  {}",
        state,
        session.runtime_id.as_deref().unwrap_or("shell"),
        relative_time(now.saturating_sub(session.last_seen_at)),
        session.project_id.display()
    );
    if session.state == SessionState::Live {
        line.push_str("  (recorded live; another process cannot confirm it)");
    } else if session.state == SessionState::Recoverable {
        if let Some(identity) = session.resume_identity.as_deref() {
            line.push_str(&format!("  conversation {identity}"));
        }
    }
    line
}

fn relative_time(elapsed_millis: u128) -> String {
    let seconds = elapsed_millis / 1_000;
    if seconds < 60 {
        format!("{seconds}s ago")
    } else if seconds < 3_600 {
        format!("{}m ago", seconds / 60)
    } else if seconds < 86_400 {
        format!("{}h ago", seconds / 3_600)
    } else {
        format!("{}d ago", seconds / 86_400)
    }
}

fn print_status(project: &Path, json: bool) -> Result<(), String> {
    if json {
        let session = load_session(project)?.map(reconcile_session).transpose()?;
        println!(
            "{}",
            match session {
                Some(session) => session_json(&session),
                None => "null".to_owned(),
            }
        );
        return Ok(());
    }

    let git = git_snapshot(project);
    println!("Project: {}", project.display());
    match git.root {
        Some(root) => {
            println!("Git root: {}", root.display());
            println!(
                "Branch: {}",
                git.branch.as_deref().unwrap_or("detached/unknown")
            );
            println!("Changes: {} file(s)", git.changed_files);
        }
        None => println!("Git: not a repository"),
    }

    match load_session(project)?.map(reconcile_session).transpose()? {
        Some(session) => {
            println!(
                "Session: {} ({})",
                session.runtime_id.as_deref().unwrap_or("shell"),
                session.state.as_str()
            );
            println!("Session id: {}", session.id);
            if let Some(identity) = session.resume_identity.as_deref() {
                println!("Agent conversation: {identity}");
            }
            match session.state {
                SessionState::Recoverable => println!("Recovery: confirmed; run 'verb resume'"),
                SessionState::Interrupted => println!("Recovery: status unknown"),
                SessionState::Ended => println!("Recovery: not available"),
                SessionState::Live => println!("Runtime: attached in this process"),
            }
            if let Ok(path) = event_log_path(project, &session.id) {
                if path.exists() {
                    println!("Events: {}", path.display());
                }
            }
        }
        None => println!("Session: none"),
    }
    Ok(())
}

fn launch_session(project: &Path, agent: Agent, extra_args: Vec<String>) -> Result<(), String> {
    let mut session = Session::new(project.to_path_buf(), agent.clone());
    let command = agent.command();
    let args = effective_args(&agent, extra_args);
    let exit_code = run_managed(project, &mut session, &command, &args, true)
        .map_err(|error| format!("could not start {command}: {error}"))?;
    println!("Verb: {} session {}", agent.label(), session.id);
    finish_session(&mut session, exit_code)?;
    Ok(())
}

fn resume_session(project: &Path) -> Result<(), Failure> {
    // The three ways there is simply nothing to resume are reported as EXIT_NOTHING_TO_DO rather
    // than as failure: Verb worked correctly, the session just is not recoverable, and a caller
    // that retries on failure should not retry on this.
    let mut session = load_session(project)?
        .map(reconcile_session)
        .transpose()?
        .ok_or_else(|| Failure::new(exit::NOTHING_TO_DO, "no session for this project"))?;
    let agent = session.agent.clone().ok_or_else(|| {
        Failure::new(exit::NOTHING_TO_DO, "this session has no resumable agent")
    })?;
    if session.state != SessionState::Recoverable {
        return Err(Failure::new(
            exit::NOTHING_TO_DO,
            format!(
                "session recovery is not confirmed for '{}'; current state is {}",
                agent.label(),
                session.state.as_str()
            ),
        ));
    }

    let command = agent.command();
    let args = agent.resume_args(session.resume_identity.as_deref());
    let exit_code = run_managed(project, &mut session, &command, &args, false)
        .map_err(|error| format!("could not resume {command}: {error}"))?;
    println!("Verb: resumed {} session {}", agent.label(), session.id);
    finish_session(&mut session, exit_code)?;
    Ok(())
}

fn finish_session(session: &mut Session, exit_code: i32) -> Result<(), String> {
    session.last_seen_at = now_millis();
    if session.resume_identity.is_none() {
        session.resume_identity = session
            .agent
            .as_ref()
            .and_then(|agent| agent.resume_identity(&session.project_id));
    }
    session.state = resolve_without_process(session);
    save_session(session)?;
    let mut logger = EventLogger::new(session)?;
    logger.process_ended(exit_code)?;
    if let Some(agent) = session.agent.as_ref() {
        logger.agent_ended(agent.label())?;
    }
    logger.session_state_changed(session.state.as_str())?;
    logger.session_ended(session.state.as_str(), exit_code)?;
    println!(
        "Verb: {} session {} ({})",
        session.runtime_id.as_deref().unwrap_or("shell"),
        session.id,
        session.state.as_str()
    );
    Ok(())
}

fn effective_args(agent: &Agent, extra_args: Vec<String>) -> Vec<String> {
    if *agent == Agent::Shell && extra_args.is_empty() {
        return shell_args().iter().map(|arg| (*arg).to_owned()).collect();
    }
    let mut args = agent.launch_flags();
    args.extend(extra_args);
    args
}

fn run_managed(
    project: &Path,
    session: &mut Session,
    command: &str,
    args: &[String],
    is_new_session: bool,
) -> Result<i32, String> {
    #[cfg(unix)]
    {
        pty::run(project, session, command, args, is_new_session)
    }

    #[cfg(not(unix))]
    {
        run_inherited(project, session, command, args, is_new_session)
    }
}

#[cfg(not(unix))]
fn run_inherited(
    project: &Path,
    session: &mut Session,
    command: &str,
    args: &[String],
    is_new_session: bool,
) -> Result<i32, String> {
    let mut logger = EventLogger::new(session)?;
    if is_new_session {
        logger.session_started(session)?;
    } else if let Some(agent) = session.agent.as_ref() {
        logger.agent_started(agent.label())?;
    }

    let mut child = Command::new(command)
        .args(args)
        .current_dir(project)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .env("VERB_SESSION_ID", &session.id)
        .env("VERB_PROJECT_ROOT", project)
        .spawn()
        .map_err(|error| error.to_string())?;
    logger.process_started()?;
    save_session(session)?;
    let status = child.wait().map_err(|error| error.to_string())?;
    let code = status.code().unwrap_or(1);
    logger.process_ended(code)?;
    Ok(code)
}

fn reconcile_session(mut session: Session) -> Result<Session, String> {
    // A persisted LIVE state is historical evidence only. There is no durable process binding to
    // trust, so every desktop restart re-establishes the product state from host facts.
    if matches!(
        session.state,
        SessionState::Live | SessionState::Interrupted
    ) {
        // Learn the agent's own conversation id first, if it is not already known: the identity is
        // what makes a resume land on *this* conversation instead of whatever the agent happens to
        // consider most recent.
        let learned_identity = session.resume_identity.is_none()
            && session
                .agent
                .as_ref()
                .and_then(|agent| agent.resume_identity(&session.project_id))
                .is_some_and(|identity| {
                    session.resume_identity = Some(identity);
                    true
                });
        let resolved = resolve_without_process(&session);
        if learned_identity && session.state == resolved {
            save_session(&session)?;
        }
        if session.state != resolved {
            session.state = resolved;
            session.last_seen_at = now_millis();
            save_session(&session)?;
            let mut logger = EventLogger::new(&session)?;
            logger.recovery_checked(session.state.as_str())?;
            logger.session_state_changed(session.state.as_str())?;
        }
    }
    Ok(session)
}

fn resolve_without_process(session: &Session) -> SessionState {
    let Some(agent) = session.agent.as_ref() else {
        return SessionState::Ended;
    };
    match agent.resume_verdict(&session.project_id) {
        ResumeVerdict::Yes => SessionState::Recoverable,
        ResumeVerdict::No => SessionState::Ended,
        ResumeVerdict::Unknown => SessionState::Interrupted,
    }
}

fn project_root_or_current() -> Result<PathBuf, String> {
    let current =
        env::current_dir().map_err(|error| format!("could not read current directory: {error}"))?;
    Ok(git_snapshot(&current).root.unwrap_or(current))
}

fn git_snapshot(project: &Path) -> GitSnapshot {
    let root = command_output("git", &["rev-parse", "--show-toplevel"], project)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty());
    let branch = command_output("git", &["branch", "--show-current"], project)
        .filter(|value| !value.is_empty());
    let changed_files = command_output("git", &["status", "--porcelain"], project)
        .map(|value| value.lines().count())
        .unwrap_or(0);
    GitSnapshot {
        root,
        branch,
        changed_files,
    }
}

fn command_output(command: &str, args: &[&str], directory: &Path) -> Option<String> {
    let output = Command::new(command)
        .args(args)
        .current_dir(directory)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn load_session(project: &Path) -> Result<Option<Session>, String> {
    let path = session_path(project)?;
    match fs::read_to_string(path) {
        Ok(contents) => Session::deserialize(&contents)
            .map(Some)
            .ok_or_else(|| "Verb session metadata is malformed".to_owned()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("could not read session metadata: {error}")),
    }
}

fn save_session(session: &Session) -> Result<(), String> {
    let path = session_path(&session.project_id)?;
    let parent = path
        .parent()
        .ok_or_else(|| "invalid session path".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("could not create Verb state directory: {error}"))?;
    let mut file =
        File::create(path).map_err(|error| format!("could not write session metadata: {error}"))?;
    file.write_all(session.serialize().as_bytes())
        .map_err(|error| format!("could not write session metadata: {error}"))
}

fn session_path(project: &Path) -> Result<PathBuf, String> {
    Ok(sessions_directory()?.join(format!("{}.session", hex_encode(project))))
}

fn sessions_directory() -> Result<PathBuf, String> {
    let state_root = env::var_os("VERB_STATE_DIR")
        .map(PathBuf::from)
        .or_else(default_state_root)
        .ok_or_else(|| "could not determine a state directory; set VERB_STATE_DIR".to_owned())?;
    Ok(state_root.join("sessions"))
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

fn default_state_root() -> Option<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".verb"))
}

fn default_shell() -> String {
    if cfg!(windows) {
        env::var("ComSpec").unwrap_or_else(|_| "powershell".to_owned())
    } else {
        env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned())
    }
}

fn shell_args() -> &'static [&'static str] {
    if cfg!(windows) {
        &["-NoLogo"]
    } else {
        &["-il"]
    }
}

struct EventLogger {
    path: PathBuf,
    file: File,
    session_id: String,
}

impl EventLogger {
    fn new(session: &Session) -> Result<Self, String> {
        let path = event_log_path(&session.project_id, &session.id)?;
        let parent = path
            .parent()
            .ok_or_else(|| "invalid event log path".to_owned())?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create Verb event directory: {error}"))?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|error| format!("could not create event log: {error}"))?;
        Ok(Self {
            path,
            file,
            session_id: session.id.clone(),
        })
    }

    fn session_started(&mut self, session: &Session) -> Result<(), String> {
        self.write_event(
            "SESSION_STARTED",
            &format!(
                "\"projectId\":\"{}\",\"runtimeId\":\"{}\"",
                json_escape(&session.project_id.to_string_lossy()),
                json_escape(session.runtime_id.as_deref().unwrap_or("shell"))
            ),
        )?;
        if let Some(agent) = session.agent.as_ref() {
            self.agent_started(agent.label())?;
        }
        Ok(())
    }

    fn agent_started(&mut self, agent: &str) -> Result<(), String> {
        self.write_event(
            "AGENT_STARTED",
            &format!("\"agentType\":\"{}\"", json_escape(agent)),
        )
    }

    fn process_started(&mut self) -> Result<(), String> {
        self.write_event("PROCESS_STARTED", "")
    }

    fn process_ended(&mut self, code: i32) -> Result<(), String> {
        self.write_event("PROCESS_ENDED", &format!("\"exitCode\":{code}"))
    }

    /// A command boundary, with no command text: `commandId` is an opaque per-session counter, so
    /// the log can pair a start with its finish without recording what was run.
    fn command_started(&mut self, command_id: &str, cwd: Option<&str>) -> Result<(), String> {
        let cwd_field = cwd.map_or_else(String::new, |value| {
            format!(",\"cwd\":\"{}\"", json_escape(value))
        });
        self.write_event(
            "COMMAND_STARTED",
            &format!("\"commandId\":\"{}\"{cwd_field}", json_escape(command_id)),
        )
    }

    fn command_finished(&mut self, command_id: &str, exit_code: i32) -> Result<(), String> {
        self.write_event(
            "COMMAND_FINISHED",
            &format!(
                "\"commandId\":\"{}\",\"exitCode\":{exit_code}",
                json_escape(command_id)
            ),
        )
    }

    fn cwd_changed(&mut self, cwd: &str) -> Result<(), String> {
        self.write_event(
            "CWD_CHANGED",
            &format!("\"cwd\":\"{}\"", json_escape(cwd)),
        )
    }

    fn session_state_changed(&mut self, state: &str) -> Result<(), String> {
        self.write_event(
            "SESSION_STATE_CHANGED",
            &format!("\"state\":\"{}\"", json_escape(state)),
        )
    }

    fn recovery_checked(&mut self, state: &str) -> Result<(), String> {
        self.write_event(
            "RECOVERY_CHECKED",
            &format!("\"resolvedState\":\"{}\"", json_escape(state)),
        )
    }

    fn agent_ended(&mut self, agent: &str) -> Result<(), String> {
        self.write_event(
            "AGENT_ENDED",
            &format!("\"agentType\":\"{}\"", json_escape(agent)),
        )
    }

    fn session_ended(&mut self, state: &str, code: i32) -> Result<(), String> {
        self.write_event(
            "SESSION_ENDED",
            &format!("\"state\":\"{}\",\"exitCode\":{code}", json_escape(state)),
        )
    }

    fn write_event(&mut self, kind: &str, fields: &str) -> Result<(), String> {
        let field_suffix = if fields.is_empty() {
            String::new()
        } else {
            format!(",{fields}")
        };
        writeln!(
            self.file,
            "{{\"schemaVersion\":1,\"timestamp\":{},\"session_id\":\"{}\",\"type\":\"{}\"{field_suffix}}}",
            now_millis(),
            json_escape(&self.session_id),
            kind
        )
        .map_err(|error| format!("could not write event log {}: {error}", self.path.display()))?;
        self.file
            .flush()
            .map_err(|error| format!("could not flush event log {}: {error}", self.path.display()))
    }
}

fn event_log_path(project: &Path, session_id: &str) -> Result<PathBuf, String> {
    let session_file = session_path(project)?;
    let state_root = session_file
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| "invalid state directory".to_owned())?;
    Ok(state_root
        .join("events")
        .join(hex_encode(project))
        .join(format!("{}.jsonl", session_id)))
}

fn json_escape(value: &str) -> String {
    json_escape_bytes(value.as_bytes())
}

fn optional_string(value: Option<&str>) -> String {
    value.unwrap_or_default().to_owned()
}

fn optional_path(value: Option<&Path>) -> String {
    value
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn optional_number(value: Option<u128>) -> String {
    value.map(|number| number.to_string()).unwrap_or_default()
}

fn json_escape_bytes(bytes: &[u8]) -> String {
    let value = String::from_utf8_lossy(bytes);
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => escaped.push(character),
        }
    }
    escaped
}

fn new_id() -> String {
    format!("{}-{}", now_seconds(), std::process::id())
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn hex_encode(path: &Path) -> String {
    path.as_os_str()
        .to_string_lossy()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_output_matches_the_shared_schema_and_leaks_no_process_state() {
        let mut session = Session {
            id: "session-1".to_owned(),
            project_id: PathBuf::from("/tmp/project"),
            runtime_id: Some("claude".to_owned()),
            last_known_cwd: Some(PathBuf::from("/tmp/project")),
            last_observed_at: Some(1_787_320_000_000),
            created_at: 1_787_319_000_000,
            last_seen_at: 1_787_320_000_000,
            state: SessionState::Recoverable,
            agent: Some(Agent::Claude),
            resume_identity: Some("claude-conversation".to_owned()),
        };

        let json = session_json(&session);

        assert!(json.contains("\"schemaVersion\":1"), "{json}");
        assert!(json.contains("\"sessionId\":\"session-1\""), "{json}");
        assert!(json.contains("\"state\":\"RECOVERABLE\""), "{json}");
        assert!(
            json.contains("\"agent\":{\"agentType\":\"claude\",\"resumeIdentity\":\"claude-conversation\"}"),
            "{json}"
        );
        // Timestamps are ISO-8601 as the schema specifies, not raw milliseconds.
        assert!(json.contains("\"createdAt\":\"2026-08-"), "{json}");
        // The fields that must never exist anywhere durable must not appear here either.
        assert!(!json.contains("pid"), "{json}");
        assert!(!json.contains("processPresent"), "{json}");

        session.agent = None;
        session.runtime_id = None;
        let json = session_json(&session);
        assert!(json.contains("\"agent\":null"), "{json}");
        assert!(json.contains("\"runtimeId\":null"), "{json}");
    }

    #[test]
    fn timestamps_render_as_utc_iso8601() {
        assert_eq!(iso8601(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso8601(1_787_320_092_493), "2026-08-21T13:48:12Z");
        // A leap day, which an approximate month calculation would get wrong.
        assert_eq!(iso8601(1_709_164_800_000), "2024-02-29T00:00:00Z");
    }

    #[test]
    fn a_global_flag_is_removed_from_the_arguments_it_was_mixed_into() {
        let mut args = vec!["--json".to_owned(), "extra".to_owned()];
        assert!(take_flag(&mut args, "--json"));
        assert_eq!(args, vec!["extra".to_owned()]);
        assert!(!take_flag(&mut args, "--json"));
    }

    #[test]
    fn nothing_to_resume_is_its_own_exit_code_not_a_failure() {
        // A caller that retries on failure must not retry when the answer is "there is nothing
        // recoverable here" -- that is a correct result, not an error.
        let failure = Failure::new(exit::NOTHING_TO_DO, "no session for this project");
        assert_eq!(failure.code, 3);

        let ordinary: Failure = "disk exploded".to_owned().into();
        assert_eq!(ordinary.code, exit::FAILURE);
    }

    #[test]
    fn bare_verb_never_starts_something_interactive_off_a_terminal() {
        // Under `cargo test` stdout is captured, so this exercises the non-terminal branch: piped,
        // redirected or in CI, bare `verb` must print help rather than open a UI that would sit
        // waiting for keystrokes nobody is typing.
        assert_eq!(default_command(), "help");
    }

    #[test]
    fn parses_known_and_custom_agents() {
        assert_eq!(Agent::parse("Claude"), Agent::Claude);
        assert_eq!(Agent::parse("open-code"), Agent::OpenCode);
        assert_eq!(Agent::parse("my-agent").label(), "my-agent");
    }

    #[test]
    fn session_round_trips_without_external_format_dependencies() {
        let session = Session {
            id: "session-1".to_owned(),
            project_id: PathBuf::from("/tmp/project"),
            runtime_id: Some("claude".to_owned()),
            last_known_cwd: Some(PathBuf::from("/tmp/project")),
            last_observed_at: Some(41),
            created_at: 42,
            last_seen_at: 43,
            state: SessionState::Interrupted,
            agent: Some(Agent::Claude),
            resume_identity: Some("claude-conversation".to_owned()),
        };
        let serialized = session.serialize();
        assert!(!serialized.contains("pid"));
        assert!(!serialized.contains("processPresent"));
        assert!(serialized.contains("resume_identity=claude-conversation"));
        assert_eq!(Session::deserialize(&serialized), Some(session));
    }

    #[test]
    fn resume_verdict_preserves_unknown_as_unknown() {
        // `dsh` has no observed resume contract yet, so it must stay Unknown -- which the shared
        // resolver turns into INTERRUPTED rather than a guessed ENDED.
        let project = Path::new("/tmp/project");
        assert_eq!(Agent::Shell.resume_verdict(project), ResumeVerdict::No);
        assert_eq!(Agent::Dsh.resume_verdict(project), ResumeVerdict::Unknown);
    }

    #[test]
    fn resume_args_name_the_conversation_and_never_open_a_picker() {
        assert_eq!(
            Agent::Claude.resume_args(Some("claude-1")),
            vec!["--resume".to_owned(), "claude-1".to_owned()]
        );
        assert_eq!(Agent::Claude.resume_args(None), vec!["--continue".to_owned()]);
        // Codex resumes with the same flags a fresh launch uses, so a resumed conversation is not
        // quietly a differently configured Codex.
        assert_eq!(
            Agent::Codex.resume_args(Some("codex-1")),
            vec![
                "--disable".to_owned(),
                "apps".to_owned(),
                "resume".to_owned(),
                "codex-1".to_owned()
            ]
        );
        // Bare `codex resume` opens an interactive picker Verb cannot answer.
        assert_eq!(
            Agent::Codex.resume_args(None),
            vec![
                "--disable".to_owned(),
                "apps".to_owned(),
                "resume".to_owned(),
                "--last".to_owned()
            ]
        );
        assert_eq!(
            effective_args(&Agent::Codex, Vec::new()),
            vec!["--disable".to_owned(), "apps".to_owned()]
        );
        assert_eq!(
            Agent::OpenCode.resume_args(Some("opencode-1")),
            vec!["--session".to_owned(), "opencode-1".to_owned()]
        );
        assert_eq!(
            Agent::OpenCode.resume_args(None),
            vec!["--continue".to_owned()]
        );
    }

    #[test]
    fn persisted_live_state_requires_runtime_reconciliation() {
        let shell = Session::new(PathBuf::from("/tmp/project"), Agent::Shell);
        assert_eq!(shell.state, SessionState::Live);
        assert_eq!(resolve_without_process(&shell), SessionState::Ended);

        // `dsh`, whose resume contract has not been observed, is the agent that must land on
        // INTERRUPTED. Claude/Codex/OpenCode now read real evidence, so their verdict depends on
        // what is actually on the host -- which is the point of the change, and why they are
        // covered in `agents::tests` against a private HOME instead of here.
        let dsh = Session::new(PathBuf::from("/tmp/project"), Agent::Dsh);
        assert_eq!(dsh.state, SessionState::Live);
        assert_eq!(resolve_without_process(&dsh), SessionState::Interrupted);
    }

    #[test]
    fn a_listed_session_never_claims_a_live_process_it_cannot_see() {
        // Nothing durable holds a process handle, so a *different* process -- which `verb sessions`
        // always is -- cannot prove a recorded LIVE session is still running. It must say so.
        let mut session = Session::new(PathBuf::from("/tmp/project"), Agent::Claude);
        session.last_seen_at = session.created_at;

        let line = describe_session(&session, session.created_at + 5_000);

        assert!(line.contains("live?"), "{line}");
        assert!(line.contains("cannot confirm"), "{line}");
        assert!(line.contains("/tmp/project"), "{line}");
        assert!(line.contains("5s ago"), "{line}");
    }

    #[test]
    fn a_recoverable_session_lists_the_conversation_resume_would_land_on() {
        let mut session = Session::new(PathBuf::from("/tmp/project"), Agent::Codex);
        session.state = SessionState::Recoverable;
        session.resume_identity = Some("codex-1".to_owned());

        let line = describe_session(&session, session.last_seen_at);

        assert!(line.contains("recoverable"), "{line}");
        assert!(line.contains("codex"), "{line}");
        assert!(line.contains("conversation codex-1"), "{line}");
        assert!(!line.contains("cannot confirm"), "{line}");
    }

    #[test]
    fn elapsed_time_reads_in_the_largest_unit_that_fits() {
        assert_eq!(relative_time(4_000), "4s ago");
        assert_eq!(relative_time(120_000), "2m ago");
        assert_eq!(relative_time(7_200_000), "2h ago");
        assert_eq!(relative_time(172_800_000), "2d ago");
    }

    #[test]
    fn project_keys_are_stable_and_path_safe() {
        let key = hex_encode(Path::new("/Users/example/my project"));
        assert_eq!(key, "2f55736572732f6578616d706c652f6d792070726f6a656374");
        assert!(!key.contains('/'));
    }

    #[test]
    fn event_payloads_are_json_safe() {
        assert_eq!(
            json_escape("quote\" slash\\ line\n"),
            "quote\\\" slash\\\\ line\\n"
        );
        assert_eq!(json_escape_bytes(&[0xff, b'a']), "\u{fffd}a");
    }
}
