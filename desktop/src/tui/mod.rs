#![cfg(unix)]

//! The Verb workspace: the terminal you were already using, with Verb's knowledge of it one leader
//! key away.
//!
//! Built against `docs/TUI_VISION.md`, and the rules there are the ones that matter here:
//!
//! * **Terminal first.** The session owns most of the screen and every keystroke Verb has not
//!   claimed. Verb reserves exactly one chord (see [`leader`]).
//! * **Context second.** The band under the terminal appears only from an observed fact -- a command
//!   that actually failed, a session state that actually changed -- never from a suspicion.
//! * **Power stays reachable.** Everything is in the palette, by name.
//! * **Every action maps to a capability.** Resume, start, reconcile and list are the same functions
//!   `verb resume`, `verb claude`, `verb status` and `verb sessions` call. The UI decides nothing
//!   about sessions on its own.

mod keys;
mod leader;
mod render;
mod term;

use crate::{Agent, Session, SessionState};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use leader::{Command, Leader, Outcome};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use term::Hosted;

/// How long a pressed leader waits for its command key before being forwarded to the terminal.
const LEADER_TIMEOUT: Duration = Duration::from_millis(900);
const TICK: Duration = Duration::from_millis(30);

pub(super) fn run(project: &Path) -> Result<(), String> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("verb ui needs a terminal; run it directly rather than through a pipe".to_owned());
    }

    let mut screen = Screen::enter()?;
    let result = App::new(project)?.run(&mut screen.terminal);
    screen.leave();
    result
}

/// Which Verb surface, if any, is in front of the terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Surface {
    None,
    Palette { filter: String, selected: usize },
    Sessions { selected: usize },
    Help,
}

/// A fact Verb observed, worth a line under the terminal. Only these two exist in M1, because only
/// these two are things Verb can currently *observe* rather than infer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Context {
    None,
    /// A command the shell itself reported finishing with a non-zero status. Structural: Verb knows
    /// that it failed and how long it took, and deliberately does not know what was typed.
    CommandFailed {
        exit_code: i32,
        millis: u128,
        /// Volatile: shown, never stored. Absent when the shell reported no command line, and the
        /// band says so rather than inventing one.
        label: Option<String>,
    },
    SessionEnded {
        exit_code: i32,
    },
    SessionState(SessionState),
}

pub(crate) struct App {
    project: PathBuf,
    leader: Leader,
    surface: Surface,
    context: Context,
    hosted: Option<Hosted>,
    sessions: Vec<Session>,
    message: Option<String>,
    leader_pressed_at: Option<Instant>,
    quit: bool,
}

impl App {
    /// Session records, with the one this process is hosting shown as it actually is.
    ///
    /// `read_sessions` reconciles every record from disk, where a shell session correctly resolves
    /// to ENDED because there is nothing to recover. But Verb is *hosting* this one and holds its
    /// process binding, which is precisely the evidence the contract says turns a record into LIVE.
    /// Listing it as ended while it runs in front of the user would be the one kind of lie Verb is
    /// built to avoid.
    fn refresh_sessions(&mut self) -> Result<(), String> {
        let hosting = self.hosted.as_ref().map(|hosted| hosted.session.id.clone());
        let mut sessions = crate::read_sessions_except(hosting.as_deref())?;
        if let Some(hosted) = self.hosted.as_ref() {
            let hosted_id = hosted.session.id.clone();
            match sessions
                .iter_mut()
                .find(|session| session.id == hosted_id)
            {
                Some(session) => session.state = SessionState::Live,
                None => sessions.insert(0, hosted.session.clone()),
            }
        }
        self.sessions = sessions;
        Ok(())
    }

    fn new(project: &Path) -> Result<Self, String> {
        Ok(Self {
            project: project.to_path_buf(),
            leader: Leader::from_environment(),
            surface: Surface::None,
            context: Context::None,
            hosted: None,
            sessions: crate::read_sessions()?,
            message: None,
            leader_pressed_at: None,
            quit: false,
        })
    }

    fn run(mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<(), String> {
        // The workspace opens with the work-context shell already running, which is what `verb
        // shell` starts -- the terminal region is not a placeholder waiting to be filled.
        let size = terminal
            .size()
            .map_err(|error| format!("could not read the terminal size: {error}"))?;
        let (rows, cols) = render::hosting_size(size.width, size.height);
        self.start_shell(rows, cols)?;

        while !self.quit {
            // The rendered rectangle is the authority for how big the session thinks it is.
            let drawn = std::cell::Cell::new((0_u16, 0_u16));
            terminal
                .draw(|frame| {
                    let area = render::workspace(frame, &self);
                    drawn.set((area.height, area.width));
                })
                .map_err(|error| format!("could not draw: {error}"))?;

            let (rows, cols) = drawn.get();
            if rows > 0 && cols > 0 {
                if let Some(hosted) = self.hosted.as_mut() {
                    if hosted.screen().size() != (rows, cols) {
                        hosted.resize(rows, cols);
                    }
                }
            }

            self.pump()?;

            if event::poll(TICK).map_err(|error| format!("could not read input: {error}"))? {
                match event::read().map_err(|error| format!("could not read input: {error}"))? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => self.on_key(key)?,
                    // A terminal resize needs no special handling: the next draw reports the new
                    // rectangle, and the session is resized to exactly that. One path, not two.
                    Event::Resize(_, _) => {}
                    _ => {}
                }
            }

            if let Some(pressed) = self.leader_pressed_at.filter(|_| self.leader.is_pending()) {
                if pressed.elapsed() >= LEADER_TIMEOUT {
                    self.leader_pressed_at = None;
                    if let Some(bytes) = self.leader.timeout() {
                        self.forward(&bytes)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Moves the hosted session forward, and closes it out when it ends.
    fn pump(&mut self) -> Result<(), String> {
        let Some(hosted) = self.hosted.as_mut() else {
            return Ok(());
        };
        let previous_state = hosted.session.state.clone();
        let exit = hosted.poll()?;

        // A failed command outranks a state change in the band: it is the thing the user just
        // watched happen.
        for outcome in hosted.take_structural() {
            match outcome {
                crate::pty::Structural::CommandFinished {
                    exit_code,
                    millis,
                    label,
                } if exit_code != 0 => {
                    self.context = Context::CommandFailed {
                        exit_code,
                        millis,
                        label,
                    };
                }
                // A command that succeeded is not news. The band stays as it was.
                crate::pty::Structural::CommandFinished { .. } => {}
            }
        }

        if hosted.session.state != previous_state {
            self.context = Context::SessionState(hosted.session.state.clone());
        }

        if let Some(exit_code) = exit {
            // The session is over, so any command label it left on screen goes with it.
            self.context = Context::None;
            let hosted = self.hosted.take().expect("checked above");
            let session = hosted.finish(exit_code)?;
            self.context = if exit_code == 0 {
                Context::SessionState(session.state.clone())
            } else {
                Context::SessionEnded { exit_code }
            };
            self.refresh_sessions()?;
        }
        Ok(())
    }

    fn on_key(&mut self, key: KeyEvent) -> Result<(), String> {
        if self.surface != Surface::None {
            return self.on_surface_key(key);
        }

        // No surface is open, so the terminal has the keyboard and Verb sees only its own chord.
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        let character = match key.code {
            KeyCode::Char(character) => Some(character),
            _ => None,
        };

        if let Some(character) = character {
            match self.leader.key(control, character) {
                Outcome::Pending => {
                    self.leader_pressed_at = Some(Instant::now());
                    return Ok(());
                }
                Outcome::Forward(bytes) => {
                    self.leader_pressed_at = None;
                    return self.forward(&bytes);
                }
                Outcome::Run(command) => {
                    self.leader_pressed_at = None;
                    return self.run_command(command);
                }
            }
        }

        // Everything that is not a character key cannot be the leader, so it belongs to the
        // terminal untouched -- arrows, function keys, Escape and the rest.
        if let Some(bytes) = keys::encode(key) {
            self.forward(&bytes)?;
        }
        Ok(())
    }

    fn on_surface_key(&mut self, key: KeyEvent) -> Result<(), String> {
        match (&mut self.surface, key.code) {
            (_, KeyCode::Esc) => self.surface = Surface::None,
            (Surface::Help, _) => self.surface = Surface::None,
            (Surface::Sessions { selected }, KeyCode::Up | KeyCode::Char('k')) => {
                *selected = selected.saturating_sub(1);
            }
            (Surface::Sessions { selected }, KeyCode::Down | KeyCode::Char('j')) => {
                *selected = (*selected + 1).min(self.sessions.len().saturating_sub(1));
            }
            (Surface::Sessions { selected }, KeyCode::Enter) => {
                let index = *selected;
                self.surface = Surface::None;
                self.resume_selected(index)?;
            }
            (Surface::Sessions { selected }, KeyCode::Char('n')) => {
                let index = *selected;
                self.surface = Surface::None;
                self.start_selected(index)?;
            }
            (Surface::Palette { filter, selected }, KeyCode::Char(character)) => {
                filter.push(character);
                *selected = 0;
            }
            (Surface::Palette { filter, selected }, KeyCode::Backspace) => {
                filter.pop();
                *selected = 0;
            }
            (Surface::Palette { filter, selected }, KeyCode::Up) => {
                let _ = filter;
                *selected = selected.saturating_sub(1);
            }
            (Surface::Palette { filter, selected }, KeyCode::Down) => {
                let count = render::palette_entries(filter).len();
                *selected = (*selected + 1).min(count.saturating_sub(1));
            }
            (Surface::Palette { filter, selected }, KeyCode::Enter) => {
                let entries = render::palette_entries(filter);
                let action = entries.get(*selected).map(|entry| entry.action.clone());
                self.surface = Surface::None;
                if let Some(action) = action {
                    self.run_action(action)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn run_command(&mut self, command: Command) -> Result<(), String> {
        match command {
            Command::Palette => {
                self.surface = Surface::Palette {
                    filter: String::new(),
                    selected: 0,
                }
            }
            Command::Sessions => {
                self.refresh_sessions()?;
                self.surface = Surface::Sessions { selected: 0 };
            }
            Command::Help => self.surface = Surface::Help,
            Command::Contextual => {
                // The contextual surface is the band, which is already on screen when there is
                // something to say. Asking for it when there is nothing says so rather than opening
                // an empty panel.
                if self.context == Context::None {
                    self.message = Some("Nothing to report: no failure or state change observed yet.".to_owned());
                }
            }
        }
        Ok(())
    }

    fn run_action(&mut self, action: Action) -> Result<(), String> {
        match action {
            Action::Sessions => self.run_command(Command::Sessions),
            Action::Help => self.run_command(Command::Help),
            Action::Resume => self.resume_here(),
            Action::NewShell => {
                let (rows, cols) = self.last_size();
                self.start_shell(rows, cols)
            }
            Action::NewAgent(agent) => {
                let (rows, cols) = self.last_size();
                self.start(crate::begin_session(&self.project.clone(), agent, Vec::new()), rows, cols)
            }
            Action::Reconcile => {
                self.refresh_sessions()?;
                self.message = Some("Recovery re-checked from each agent's own evidence.".to_owned());
                Ok(())
            }
            Action::Quit => {
                self.quit = true;
                Ok(())
            }
        }
    }

    fn resume_here(&mut self) -> Result<(), String> {
        let project = self.project.clone();
        self.resume_project(&project)
    }

    fn resume_selected(&mut self, index: usize) -> Result<(), String> {
        let Some(project) = self.sessions.get(index).map(|session| session.project_id.clone()) else {
            return Ok(());
        };
        self.resume_project(&project)
    }

    fn start_selected(&mut self, index: usize) -> Result<(), String> {
        let Some(session) = self.sessions.get(index) else {
            return Ok(());
        };
        let project = session.project_id.clone();
        let agent = session.agent.clone();
        let (rows, cols) = self.last_size();
        match agent {
            Some(agent) => self.start(crate::begin_session(&project, agent, Vec::new()), rows, cols),
            // A shell session records no agent, so there is nothing to start a *new* one of.
            None => {
                self.message = Some("That session has no agent to start.".to_owned());
                Ok(())
            }
        }
    }

    fn resume_project(&mut self, project: &Path) -> Result<(), String> {
        let (rows, cols) = self.last_size();
        match crate::begin_resume(project) {
            Ok(start) => self.start(start, rows, cols),
            Err(failure) => {
                // Refused for the same reasons `verb resume` refuses, and reported rather than
                // worked around.
                self.message = Some(failure.message);
                Ok(())
            }
        }
    }

    fn start_shell(&mut self, rows: u16, cols: u16) -> Result<(), String> {
        let start = crate::begin_session(&self.project.clone(), Agent::Shell, Vec::new());
        self.start(start, rows, cols)
    }

    fn start(&mut self, start: crate::SessionStart, rows: u16, cols: u16) -> Result<(), String> {
        if let Some(hosted) = self.hosted.take() {
            // One hosted session at a time in M1. The outgoing one is closed out honestly rather
            // than abandoned: its record must not be left claiming LIVE.
            let session = hosted.finish(0)?;
            self.context = Context::SessionState(session.state.clone());
        }
        let project = start.session.project_id.clone();
        self.hosted = Some(Hosted::start(
            &project,
            start.session,
            &start.command,
            &start.args,
            &start.env,
            start.is_new,
            rows,
            cols,
        )?);
        self.context = Context::None;
        self.message = None;
        self.refresh_sessions()?;
        Ok(())
    }

    fn forward(&mut self, bytes: &[u8]) -> Result<(), String> {
        if let Some(hosted) = self.hosted.as_mut() {
            hosted.write(bytes)?;
        }
        Ok(())
    }

    fn last_size(&self) -> (u16, u16) {
        self.hosted
            .as_ref()
            .map(|hosted| {
                let (rows, cols) = hosted.screen().size();
                (rows.max(1), cols.max(1))
            })
            .unwrap_or((24, 80))
    }

    pub(crate) fn project(&self) -> &Path {
        &self.project
    }

    pub(crate) fn leader(&self) -> &Leader {
        &self.leader
    }

    pub(crate) fn surface(&self) -> &Surface {
        &self.surface
    }

    pub(crate) fn context(&self) -> &Context {
        &self.context
    }

    pub(crate) fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    pub(crate) fn sessions(&self) -> &[Session] {
        &self.sessions
    }

    pub(crate) fn hosted(&self) -> Option<&Hosted> {
        self.hosted.as_ref()
    }
}

/// What a palette entry does. Every variant is an existing capability, reachable from the CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Action {
    Resume,
    NewShell,
    NewAgent(Agent),
    Sessions,
    Reconcile,
    Help,
    Quit,
}

/// Owns the alternate screen and raw mode for as long as the workspace is up.
struct Screen {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl Screen {
    fn enter() -> Result<Self, String> {
        ratatui::crossterm::terminal::enable_raw_mode()
            .map_err(|error| format!("could not take the terminal: {error}"))?;
        let mut stdout = io::stdout();
        ratatui::crossterm::execute!(stdout, ratatui::crossterm::terminal::EnterAlternateScreen)
            .map_err(|error| format!("could not take the screen: {error}"))?;
        let terminal = Terminal::new(CrosstermBackend::new(stdout))
            .map_err(|error| format!("could not start the renderer: {error}"))?;
        Ok(Self { terminal })
    }

    fn leave(&mut self) {
        let _ = ratatui::crossterm::execute!(
            io::stdout(),
            ratatui::crossterm::terminal::LeaveAlternateScreen
        );
        let _ = ratatui::crossterm::terminal::disable_raw_mode();
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        self.leave();
    }
}
