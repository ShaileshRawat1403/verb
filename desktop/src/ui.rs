#![cfg(unix)]

//! Verb's terminal UI: the work context as a screen rather than a command you remember.
//!
//! Everything here is presentation over the machinery that was already proven -- the session
//! records, the agent adapters, the PTY host. It computes no state of its own and defines no
//! session semantics; when it needs to know whether something is recoverable it asks the same
//! resolver `verb status` asks, and when it launches or resumes it calls the same functions
//! `verb claude` and `verb resume` call.
//!
//! Dependency-free like the rest of the crate: raw mode through `stty` (as the PTY host already
//! does), drawing through ANSI escapes, input read a byte at a time.

use super::{
    git_snapshot, launch_session, reconcile_session, resume_session, sessions_directory, Agent,
    Session, SessionState,
};
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Entry point for `verb ui`.
pub(super) fn run() -> Result<(), String> {
    if !io::stdin().is_terminal() {
        return Err("verb ui needs a terminal; run it directly rather than through a pipe".to_owned());
    }

    let mut app = App::load()?;
    let _screen = Screen::enter()?;

    loop {
        app.draw()?;
        match read_key()? {
            Key::Quit => break,
            Key::Up => app.move_selection(-1),
            Key::Down => app.move_selection(1),
            Key::Refresh => app.refresh()?,
            Key::Resume => {
                if let Some(project) = app.selected_project() {
                    // The screen is given back before the agent runs: it owns the terminal from
                    // here, and a TUI drawing underneath an interactive agent would fight it for
                    // the cursor.
                    Screen::leave();
                    let outcome = resume_session(&project).map_err(|failure| failure.message);
                    Screen::enter_again()?;
                    app.report(outcome);
                    app.refresh()?;
                }
            }
            Key::New => {
                if let Some((project, agent)) = app.selected_launch() {
                    Screen::leave();
                    let outcome = launch_session(&project, agent, Vec::new());
                    Screen::enter_again()?;
                    app.report(outcome);
                    app.refresh()?;
                }
            }
            Key::Ignored => {}
        }
    }

    Ok(())
}

struct App {
    sessions: Vec<Session>,
    selected: usize,
    message: Option<String>,
}

impl App {
    fn load() -> Result<Self, String> {
        Ok(Self {
            sessions: read_sessions()?,
            selected: 0,
            message: None,
        })
    }

    fn refresh(&mut self) -> Result<(), String> {
        self.sessions = read_sessions()?;
        if self.selected >= self.sessions.len() {
            self.selected = self.sessions.len().saturating_sub(1);
        }
        Ok(())
    }

    fn move_selection(&mut self, delta: isize) {
        if self.sessions.is_empty() {
            return;
        }
        let last = self.sessions.len() - 1;
        self.selected = match delta {
            d if d < 0 => self.selected.saturating_sub(1),
            _ => (self.selected + 1).min(last),
        };
        self.message = None;
    }

    fn selected_session(&self) -> Option<&Session> {
        self.sessions.get(self.selected)
    }

    fn selected_project(&self) -> Option<PathBuf> {
        self.selected_session()
            .map(|session| session.project_id.clone())
    }

    /// The project and agent a "new session" would start: the same agent this project last used,
    /// because that is the only agent choice the record actually justifies.
    fn selected_launch(&self) -> Option<(PathBuf, Agent)> {
        let session = self.selected_session()?;
        let agent = session.agent.clone()?;
        Some((session.project_id.clone(), agent))
    }

    fn report(&mut self, outcome: Result<(), String>) {
        self.message = match outcome {
            Ok(()) => None,
            Err(error) => Some(error),
        };
    }

    fn draw(&self) -> Result<(), String> {
        let screen = self.frame();
        let mut stdout = io::stdout();
        stdout
            .write_all(screen.as_bytes())
            .map_err(|error| format!("could not draw: {error}"))?;
        stdout
            .flush()
            .map_err(|error| format!("could not draw: {error}"))
    }

    /// The whole screen as a string, so what the UI shows can be tested without a terminal.
    fn frame(&self) -> String {
        let mut screen = String::new();
        screen.push_str("\x1b[H\x1b[2J");
        screen.push_str(&format!(
            "  {}Verb{} — sessions{}    ↑↓ move   enter resume   n new   r refresh   q quit{}\r\n",
            colour("\x1b[1m"),
            reset(),
            colour("\x1b[2m"),
            reset()
        ));
        screen.push_str(&rule());

        if self.sessions.is_empty() {
            screen.push_str("\r\n  No sessions yet. Run an agent in a project first.\r\n");
        }

        for (index, session) in self.sessions.iter().enumerate() {
            let selected = index == self.selected;
            screen.push_str(&row(session, selected));
            screen.push_str("\r\n");
        }

        screen.push_str(&rule());
        screen.push_str(&self.footer());
        screen.push_str("\r\n");
        screen
    }

    fn footer(&self) -> String {
        if let Some(message) = self.message.as_deref() {
            return format!("  {}{message}{}", colour("\x1b[31m"), reset());
        }
        let Some(session) = self.selected_session() else {
            return String::new();
        };
        let git = git_snapshot(&session.project_id);
        let branch = git.branch.as_deref().unwrap_or("no branch");
        let mut footer = format!(
            "  {} · {} · {} changed",
            display_path(&session.project_id),
            branch,
            git.changed_files
        );
        match session.state {
            // What each key would actually do here, rather than a fixed legend that is wrong for
            // three rows out of four.
            SessionState::Recoverable => footer.push_str(&format!(
                "\r\n  {}enter{} resumes this conversation",
                colour("\x1b[1m"),
                reset()
            )),
            SessionState::Live => footer.push_str(&format!(
                "\r\n  {}recorded live; this process cannot confirm it is running{}",
                colour("\x1b[2m"),
                reset()
            )),
            SessionState::Interrupted => footer.push_str(&format!(
                "\r\n  {}recovery status unknown; r re-checks{}",
                colour("\x1b[2m"),
                reset()
            )),
            SessionState::Ended => footer.push_str(&format!(
                "\r\n  {}n{} starts a new session here",
                colour("\x1b[1m"),
                reset()
            )),
        }
        footer
    }
}

/// Honours `NO_COLOR` (no-color.org): any non-empty value means draw without colour.
///
/// The selection marker is a `▸` glyph rather than reverse video alone, so the UI stays readable
/// when colour is off -- which is the point of the convention, not a formality.
fn colour(code: &str) -> &str {
    match std::env::var_os("NO_COLOR") {
        Some(value) if !value.is_empty() => "",
        _ => code,
    }
}

fn row(session: &Session, selected: bool) -> String {
    let (glyph, state_colour) = match session.state {
        SessionState::Live => ("●", colour("\x1b[32m")),
        SessionState::Recoverable => ("◐", colour("\x1b[33m")),
        SessionState::Interrupted => ("◌", colour("\x1b[2m")),
        SessionState::Ended => ("○", colour("\x1b[2m")),
    };
    // The same honesty `verb sessions` prints: a persisted LIVE is evidence, not proof.
    let state = match session.state {
        SessionState::Live => "live?",
        _ => session.state.as_str(),
    };
    let conversation = session
        .resume_identity
        .as_deref()
        .filter(|_| session.state == SessionState::Recoverable)
        .map(short_identity)
        .unwrap_or_default();

    // The marker is always two columns wide, selected or not, so the rows stay in line.
    format!(
        "{}{}{} {:<14}{} {:<9} {:>7}  {}{}{}",
        if selected { colour("\x1b[7m") } else { "" },
        if selected { "▸ " } else { "  " },
        state_colour,
        format!("{glyph} {state}"),
        reset(),
        session.runtime_id.as_deref().unwrap_or("shell"),
        super::relative_time(super::now_millis().saturating_sub(session.last_seen_at)),
        display_path(&session.project_id),
        if conversation.is_empty() {
            String::new()
        } else {
            format!("  {}{conversation}{}", colour("\x1b[2m"), reset())
        },
        reset()
    )
}

/// The reset only has to be emitted when something was set; with `NO_COLOR` there is nothing to
/// undo, and emitting escapes anyway would defeat the point for a terminal that shows them raw.
fn reset() -> &'static str {
    colour("\x1b[0m")
}

fn rule() -> String {
    format!(
        "{} {}{}\r\n",
        colour("\x1b[2m"),
        "─".repeat(rule_width(terminal_width())),
        reset()
    )
}

/// A terminal that reports no size at all (some pty allocations do) must still get a visible rule,
/// so this floors the width rather than drawing nothing.
fn rule_width(terminal_width: usize) -> usize {
    terminal_width.clamp(40, 120).saturating_sub(2)
}

/// `~` for the home directory, because a column of identical prefixes is not information.
fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy().into_owned();
    match super::home_dir() {
        Some(home) => {
            let home = home.to_string_lossy().into_owned();
            match text.strip_prefix(&home) {
                Some(rest) => format!("~{rest}"),
                None => text,
            }
        }
        None => text,
    }
}

fn short_identity(identity: &str) -> String {
    match identity.char_indices().nth(8) {
        Some((index, _)) => format!("{}…", &identity[..index]),
        None => identity.to_owned(),
    }
}

fn read_sessions() -> Result<Vec<Session>, String> {
    let directory = sessions_directory()?;
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("could not read {}: {error}", directory.display())),
    };

    let mut sessions: Vec<Session> = entries
        .flatten()
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|value| value == "session")
        })
        .filter_map(|entry| fs::read_to_string(entry.path()).ok())
        .filter_map(|contents| Session::deserialize(&contents))
        // Unlike `verb sessions`, this *is* interactive: the user is looking at the screen and
        // expects it to be current, so each row is reconciled against the agent's real evidence.
        .filter_map(|session| reconcile_session(session).ok())
        .collect();

    sessions.sort_by(|left, right| right.last_seen_at.cmp(&left.last_seen_at));
    Ok(sessions)
}

enum Key {
    Up,
    Down,
    Resume,
    New,
    Refresh,
    Quit,
    Ignored,
}

fn read_key() -> Result<Key, String> {
    let mut buffer = [0_u8; 3];
    let count = io::stdin()
        .read(&mut buffer)
        .map_err(|error| format!("could not read input: {error}"))?;
    if count == 0 {
        return Ok(Key::Quit);
    }
    Ok(match &buffer[..count] {
        [b'q'] | [0x03] => Key::Quit,
        [b'k'] | [0x1b, b'[', b'A'] => Key::Up,
        [b'j'] | [0x1b, b'[', b'B'] => Key::Down,
        [b'\r'] | [b'\n'] => Key::Resume,
        [b'n'] => Key::New,
        [b'r'] => Key::Refresh,
        _ => Key::Ignored,
    })
}

/// Owns the terminal for as long as the UI is on screen: alternate buffer, raw mode, hidden cursor.
///
/// [`Screen::leave`] and [`Screen::enter_again`] exist because launching an agent hands the whole
/// terminal to that agent and then takes it back; `Drop` alone cannot express that, and leaving the
/// user in raw mode with no cursor would be the worst possible way to fail.
struct Screen;

impl Screen {
    fn enter() -> Result<Self, String> {
        Self::enter_again()?;
        Ok(Self)
    }

    fn enter_again() -> Result<(), String> {
        stty(&["raw", "-echo"])?;
        print!("\x1b[?1049h\x1b[?25l");
        io::stdout()
            .flush()
            .map_err(|error| format!("could not take the screen: {error}"))
    }

    fn leave() {
        print!("\x1b[?25h\x1b[?1049l");
        let _ = io::stdout().flush();
        let _ = stty(&["sane"]);
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        Self::leave();
    }
}

fn stty(args: &[&str]) -> Result<(), String> {
    let status = Command::new("stty")
        .args(args)
        .stdin(Stdio::inherit())
        .status()
        .map_err(|error| format!("could not configure the terminal: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("could not configure the terminal with stty".to_owned())
    }
}

fn terminal_width() -> usize {
    Command::new("stty")
        .arg("size")
        .stdin(Stdio::inherit())
        .output()
        .ok()
        .and_then(|output| {
            String::from_utf8_lossy(&output.stdout)
                .split_whitespace()
                .nth(1)
                .and_then(|value| value.parse().ok())
        })
        .unwrap_or(80)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn session(state: SessionState, agent: Agent, project: &str) -> Session {
        let mut session = Session::new(PathBuf::from(project), agent);
        session.state = state;
        session.last_seen_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        session
    }

    fn app(sessions: Vec<Session>) -> App {
        App {
            sessions,
            selected: 0,
            message: None,
        }
    }

    #[test]
    fn a_live_row_is_marked_as_unconfirmed_rather_than_as_fact() {
        // The same rule the rest of Verb follows: nothing durable holds a process handle, so a
        // persisted LIVE is evidence about the past, not proof about now.
        let app = app(vec![session(SessionState::Live, Agent::Claude, "/tmp/api")]);

        let frame = app.frame();

        assert!(frame.contains("live?"), "{frame}");
        assert!(frame.contains("cannot confirm"), "{frame}");
    }

    #[test]
    fn a_recoverable_row_shows_the_conversation_and_offers_resume() {
        let mut recoverable = session(SessionState::Recoverable, Agent::Codex, "/tmp/site");
        recoverable.resume_identity = Some("01a0247b-3507-7522".to_owned());
        let app = app(vec![recoverable]);

        let frame = app.frame();

        assert!(frame.contains("recoverable"), "{frame}");
        assert!(frame.contains("01a0247b…"), "{frame}");
        assert!(frame.contains("resumes this conversation"), "{frame}");
    }

    #[test]
    fn an_ended_row_offers_a_new_session_and_never_resume() {
        let app = app(vec![session(SessionState::Ended, Agent::OpenCode, "/tmp/scratch")]);

        let frame = app.frame();

        assert!(frame.contains("starts a new session"), "{frame}");
        assert!(!frame.contains("resumes this conversation"), "{frame}");
    }

    #[test]
    fn an_interrupted_row_says_the_status_is_unknown_and_never_offers_resume() {
        let app = app(vec![session(
            SessionState::Interrupted,
            Agent::Claude,
            "/tmp/unknown",
        )]);

        let frame = app.frame();

        assert!(frame.contains("recovery status unknown"), "{frame}");
        assert!(!frame.contains("resumes this conversation"), "{frame}");
    }

    #[test]
    fn selection_moves_within_bounds_and_never_off_either_end() {
        let mut app = app(vec![
            session(SessionState::Live, Agent::Claude, "/tmp/a"),
            session(SessionState::Ended, Agent::Codex, "/tmp/b"),
        ]);

        app.move_selection(-1);
        assert_eq!(app.selected, 0);
        app.move_selection(1);
        assert_eq!(app.selected, 1);
        app.move_selection(1);
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn an_empty_state_explains_itself_instead_of_showing_a_blank_screen() {
        let frame = app(Vec::new()).frame();

        assert!(frame.contains("No sessions yet"), "{frame}");
    }

    #[test]
    fn new_session_reuses_the_agent_that_project_actually_used() {
        // Not a guess and not a picker: the record justifies exactly one agent.
        let app = app(vec![session(SessionState::Ended, Agent::Codex, "/tmp/site")]);

        let (project, agent) = app.selected_launch().unwrap();

        assert_eq!(project, PathBuf::from("/tmp/site"));
        assert_eq!(agent, Agent::Codex);
    }

    #[test]
    fn a_shell_session_offers_no_agent_to_start() {
        // `Session::new` records no agent for a plain shell, so there is nothing for "new" to
        // launch and the UI must not invent one.
        let app = app(vec![session(SessionState::Ended, Agent::Shell, "/tmp/shell")]);

        assert!(app.selected_launch().is_none());
    }

    #[test]
    fn the_rule_is_drawn_even_when_the_terminal_reports_no_size() {
        assert_eq!(rule_width(0), 38);
        assert_eq!(rule_width(80), 78);
        assert_eq!(rule_width(400), 118);
    }

    #[test]
    fn no_color_leaves_the_screen_free_of_escape_sequences() {
        // The other frame tests assert on text, so they pass either way; this one asserts the
        // convention itself. Selection stays legible because the marker is a glyph, not colour.
        let app = app(vec![
            session(SessionState::Recoverable, Agent::Claude, "/tmp/a"),
            session(SessionState::Ended, Agent::Codex, "/tmp/b"),
        ]);

        std::env::set_var("NO_COLOR", "1");
        let frame = app.frame();
        std::env::remove_var("NO_COLOR");

        let body = frame.trim_start_matches("\x1b[H\x1b[2J");
        assert!(!body.contains('\x1b'), "{body:?}");
        assert!(body.contains("▸ "), "{body}");
    }

    #[test]
    fn identities_are_shortened_for_the_column_but_never_invented() {
        assert_eq!(short_identity("01a0247b-3507-7522"), "01a0247b…");
        assert_eq!(short_identity("short"), "short");
    }
}
