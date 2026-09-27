//! The Verb workspace: a durable task/session Workbench beside a hosted terminal.
//!
//! Built against `docs/TUI_VISION.md`, and the rules there are the ones that matter here:
//!
//! * **Workbench first.** Durable sessions and tasks are visible on entry. In Terminal mode, the
//!   hosted session owns most of the screen and every keystroke except Verb's leader chord.
//! * **Context second.** The band under the terminal appears only from an observed fact -- a command
//!   that actually failed, a session state that actually changed -- never from a suspicion.
//! * **Power stays reachable.** Everything is in the palette, by name.
//! * **Every action maps to a capability.** Resume, start, reconcile and list are the same functions
//!   `verb resume`, `verb claude`, `verb status` and `verb sessions` call. The UI decides nothing
//!   about sessions on its own.

mod context_view;
mod input;
mod keys;
mod leader;
mod mouse;
mod render;
pub(crate) mod term;
mod theme;
mod workbench_view;

use crate::{Agent, Session, SessionState};
use leader::{Command, Leader, Outcome};
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use ratatui::Terminal;
use std::collections::HashMap;
use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use term::Hosted;

const TICK: Duration = Duration::from_millis(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Workbench,
    Terminal,
    Activity,
    Memory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkFocus {
    Sessions,
    Tasks,
}

#[derive(Debug)]
pub(crate) struct WorkState {
    focus: WorkFocus,
    session: usize,
    task: usize,
    detail: bool,
    inbox: Option<crate::workbench::InboxSnapshot>,
    inbox_index: usize,
    search: Option<String>,
    filter: String,
    composer: Option<Composer>,
    actor_picker: Option<ActorPicker>,
    new_task: Option<NewTask>,
    memory_editor: Option<String>,
    scroll: u16,
}

#[derive(Debug)]
pub(crate) struct Composer {
    task_id: String,
    action: crate::workbench::TaskAction,
    actor: String,
    text: String,
    expected_revision: Option<String>,
}

#[derive(Debug)]
pub(crate) struct ActorPicker {
    action: crate::workbench::TaskAction,
    selected: usize,
}

#[derive(Debug, Default)]
pub(crate) struct NewTask {
    title: String,
    brief: String,
    editing_brief: bool,
}

impl Default for WorkState {
    fn default() -> Self {
        Self {
            focus: WorkFocus::Tasks,
            session: 0,
            task: 0,
            detail: false,
            inbox: None,
            inbox_index: 0,
            search: None,
            filter: String::new(),
            composer: None,
            actor_picker: None,
            new_task: None,
            memory_editor: None,
            scroll: 0,
        }
    }
}

pub(super) fn run(project: &Path) -> Result<(), String> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(
            "verb ui needs a terminal; run it directly rather than through a pipe".to_owned(),
        );
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
    Palette {
        filter: String,
        selected: usize,
    },
    ExternalAgent {
        command: String,
        isolated: bool,
    },
    Sessions {
        selected: usize,
    },
    Help,
    /// What Verb has observed, as `verb context` assembles it.
    Evidence,
    /// What Git reports as changed here, as `verb changes` lists it.
    Changes,
    /// Looking back through output that has scrolled away. A Verb surface rather than a terminal
    /// mode: while it is open the keyboard and the mouse belong to Verb, and `Esc` gives them back.
    Scrollback {
        offset: usize,
        search: Option<String>,
        /// The last term searched for, kept so `n` can repeat it after the prompt closes.
        last_search: Option<String>,
    },
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
        /// How far the tree has moved from the user's last-known-good mark, read when the command
        /// failed. Absent when nothing is marked or Git could not say.
        since_good: Option<String>,
    },
    /// A tool the agent ran and its own record marked as failed.
    ///
    /// Read from the agent's transcript after the fact, not watched happening -- which is why it is
    /// worded as what the agent reported rather than as what Verb saw.
    AgentToolFailed {
        millis: u128,
        /// The last tool the record named, when it named one.
        tool: Option<String>,
    },
    SessionEnded {
        exit_code: i32,
    },
    SessionState(SessionState),
    /// The repository is in a state where the obvious next Git command can lose work or be refused:
    /// an unfinished rebase or merge, conflicts, a detached HEAD, a diverged upstream. Read from Git
    /// after a command finished; see `gitstate.rs`.
    RepoWarning {
        fact: String,
        safe_next: String,
    },
    /// A runtime the project declares is missing or the wrong version, per `runtime.rs`. Only from a
    /// declaration in the project's own files; never from a guess about what it wants.
    RuntimeMismatch(String),
}

pub(crate) struct App {
    project: PathBuf,
    project_anchor: PathBuf,
    logical_project_id: String,
    git: crate::GitSnapshot,
    mode: Mode,
    work: WorkState,
    tasks: Vec<crate::workbench::TaskSnapshot>,
    memory: String,
    inboxes: HashMap<String, crate::workbench::InboxSnapshot>,
    workspace_status: HashMap<PathBuf, crate::GitSnapshot>,
    leader: Leader,
    surface: Surface,
    context: Context,
    hosted: Option<Hosted>,
    parked: Vec<Hosted>,
    pane_order: Vec<String>,
    pane_areas: Vec<(String, ratatui::layout::Rect)>,
    terminal_zoom: bool,
    sessions: Vec<Session>,
    imported_sessions: Vec<crate::continuity::ImportedSession>,
    message: Option<String>,
    quit: bool,
    mouse_captured: bool,
    /// Whether Verb should hold the mouse at all. On by default: the action bar is the primary
    /// visible affordance, and an affordance you can see but not click is a worse lie than no
    /// affordance at all.
    mouse_enabled: bool,
    /// The height of the last frame drawn, so a click can be told which row it landed on. Recorded
    /// from the draw itself rather than queried separately: the rendered frame is the authority for
    /// where things are, exactly as it already is for how large the session believes it is.
    frame_height: u16,
    frame_width: u16,
    /// Observations that run Git or a runtime's `--version`, gathered off the UI thread: in a large
    /// repository or on a network drive they take long enough to freeze typing.
    background: Background,
}

/// Results arriving from background observation threads.
enum Observation {
    Runtime(Vec<crate::runtime::Fact>),
    Repo(Option<Context>),
    SinceGood {
        generation: u64,
        text: Option<String>,
    },
}

struct Background {
    sender: std::sync::mpsc::Sender<Observation>,
    receiver: std::sync::mpsc::Receiver<Observation>,
    /// Bumped for every failed command, so a slow answer for an older failure is dropped.
    failure_generation: u64,
    repo_running: bool,
    /// A command finished while a repository read was in flight; read again when it returns.
    repo_again: bool,
}

impl Background {
    fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        Self {
            sender,
            receiver,
            failure_generation: 0,
            repo_running: false,
            repo_again: false,
        }
    }

    fn spawn(&self, work: impl FnOnce() -> Observation + Send + 'static) {
        let sender = self.sender.clone();
        std::thread::spawn(move || {
            let _ = sender.send(work());
        });
    }

    fn runtime(&self, project: &Path) {
        let project = project.to_path_buf();
        self.spawn(move || Observation::Runtime(crate::runtime::observe(&project)));
    }

    fn repo(&mut self, project: &Path) {
        if self.repo_running {
            self.repo_again = true;
            return;
        }
        self.repo_running = true;
        let project = project.to_path_buf();
        self.spawn(move || Observation::Repo(repo_warning(&project)));
    }

    fn since_good(&mut self, project: &Path) -> u64 {
        self.failure_generation += 1;
        let generation = self.failure_generation;
        let project = project.to_path_buf();
        self.spawn(move || Observation::SinceGood {
            generation,
            text: since_good(&project),
        });
        generation
    }
}

/// The first repository warning, if the repository has one.
fn repo_warning(project: &Path) -> Option<Context> {
    let warning = crate::gitstate::observe(project)
        .state()?
        .warnings()
        .into_iter()
        .next()?;
    Some(Context::RepoWarning {
        fact: warning.fact,
        safe_next: warning.safe_next,
    })
}

/// A short "since last known good" line, when the user has marked one. No fingerprint, so the answer
/// arrives while the failure is still on screen.
fn since_good(project: &Path) -> Option<String> {
    let mark = crate::good::load(project).ok()??;
    let distance = crate::good::distance(project, &mark, false);
    Some(format!(
        "last known good {}: {}",
        mark.short_head().unwrap_or("(no commit)"),
        distance.summary()
    ))
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
        let hosting: Vec<&str> = self
            .hosted
            .iter()
            .chain(self.parked.iter())
            .map(|hosted| hosted.session.id.as_str())
            .collect();
        let mut sessions = crate::read_sessions_except(&hosting)?;
        for hosted in self.hosted.iter().chain(self.parked.iter()) {
            let hosted_id = hosted.session.id.clone();
            match sessions.iter_mut().find(|session| session.id == hosted_id) {
                Some(session) => session.state = SessionState::Live,
                None => sessions.insert(0, hosted.session.clone()),
            }
        }
        self.sessions = sessions;
        self.imported_sessions = crate::continuity::imported_sessions()?;
        Ok(())
    }

    fn new(project: &Path) -> Result<Self, String> {
        let identity = crate::project::identity(project)?;
        let mut app = Self {
            project: project.to_path_buf(),
            project_anchor: identity.anchor,
            logical_project_id: identity.id,
            git: crate::git_snapshot(project),
            mode: Mode::Workbench,
            work: WorkState::default(),
            tasks: crate::workbench::snapshots(project)?,
            memory: crate::workbench::shared_memory(project)?,
            inboxes: HashMap::new(),
            workspace_status: HashMap::new(),
            leader: Leader::from_environment(),
            surface: Surface::None,
            context: Context::None,
            hosted: None,
            parked: Vec::new(),
            pane_order: Vec::new(),
            pane_areas: Vec::new(),
            terminal_zoom: false,
            sessions: crate::read_sessions()?,
            imported_sessions: crate::continuity::imported_sessions()?,
            message: None,
            quit: false,
            mouse_captured: false,
            mouse_enabled: true,
            frame_height: 0,
            frame_width: 0,
            background: Background::new(),
        };
        app.background.runtime(project);
        // A repository left mid-merge or detached is worth knowing before the first command.
        app.background.repo(project);
        app.refresh_workbench()?;
        Ok(app)
    }

    fn run(mut self, terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<(), String> {
        let size = terminal
            .size()
            .map_err(|error| format!("could not read the terminal size: {error}"))?;
        // Workbench is the landing view. No idle PTY is created until the user opens Terminal.

        let mut redraw = true;
        let mut last_draw = Instant::now();
        let mut last_workbench_refresh = Instant::now();
        let mut last_git_refresh = Instant::now();
        let mut undersized = render::is_too_small(size.width, size.height);
        let mut drew_once = false;
        while !self.quit {
            // The rendered rectangle is the authority for how big the session thinks it is.
            let drawn = std::cell::RefCell::new(Vec::new());
            let frame_height = std::cell::Cell::new(0_u16);
            let frame_width = std::cell::Cell::new(0_u16);
            let frame_undersized = std::cell::Cell::new(undersized);
            let draw_due = !drew_once
                || last_draw.elapsed() >= Duration::from_secs(1)
                || (redraw && !undersized);
            if draw_due {
                if last_git_refresh.elapsed() >= Duration::from_secs(1) {
                    self.refresh_git();
                    last_git_refresh = Instant::now();
                }
                if self.mode != Mode::Terminal
                    && last_workbench_refresh.elapsed() >= Duration::from_secs(1)
                {
                    self.refresh_workbench()?;
                    last_workbench_refresh = Instant::now();
                }
                terminal
                    .draw(|frame| {
                        *drawn.borrow_mut() = render::workspace(frame, &self);
                        frame_height.set(frame.area().height);
                        frame_width.set(frame.area().width);
                        frame_undersized.set(render::is_too_small(
                            frame.area().width,
                            frame.area().height,
                        ));
                    })
                    .map_err(|error| format!("could not draw: {error}"))?;
                last_draw = Instant::now();
                drew_once = true;
                redraw = false;
                undersized = frame_undersized.get();
                self.frame_height = frame_height.get();
                self.frame_width = frame_width.get();

                self.sync_mouse_capture()?;

                self.pane_areas = drawn.into_inner();
                if self.mode == Mode::Terminal {
                    for (id, area) in &self.pane_areas {
                        if area.height == 0 || area.width == 0 {
                            continue;
                        }
                        if let Some(hosted) = self
                            .hosted
                            .iter_mut()
                            .chain(self.parked.iter_mut())
                            .find(|hosted| hosted.session.id == *id)
                        {
                            if hosted.screen().size() != (area.height, area.width) {
                                hosted.resize(area.height, area.width);
                            }
                        }
                    }
                }
            }

            redraw |= self.pump()?;

            if event::poll(TICK).map_err(|error| format!("could not read input: {error}"))? {
                redraw = true;
                match event::read().map_err(|error| format!("could not read input: {error}"))? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => self.on_key(key)?,
                    Event::Mouse(mouse) => self.on_mouse(mouse)?,
                    // A terminal resize needs no special handling: the next draw reports the new
                    // rectangle, and the session is resized to exactly that. One path, not two.
                    Event::Resize(_, _) => {}
                    _ => {}
                }
            }
            // While the only usable view is the minimum-size notice, PTY output can continue to
            // arrive but cannot change that notice. Do not turn a static guard into a 30 fps stream
            // of terminal control bytes. A one-second refresh and resize events remain enough to
            // recover immediately when the terminal becomes usable again.
            if undersized {
                redraw = false;
            }
        }
        Ok(())
    }

    /// Applies whatever the background observers have finished. Each only fills a band that is
    /// quiet or already showing its own kind of fact: a failed command is never overwritten.
    fn take_observations(&mut self) -> bool {
        let mut changed = false;
        while let Ok(observation) = self.background.receiver.try_recv() {
            match observation {
                Observation::Runtime(facts) => {
                    let problem = facts.iter().find(|fact| {
                        matches!(
                            fact.verdict,
                            crate::runtime::Verdict::Mismatch | crate::runtime::Verdict::Missing
                        )
                    });
                    if let (Some(fact), Context::None) = (problem, &self.context) {
                        self.context = Context::RuntimeMismatch(fact.to_text());
                        changed = true;
                    }
                }
                Observation::Repo(warning) => {
                    self.background.repo_running = false;
                    if matches!(self.context, Context::None | Context::RepoWarning { .. }) {
                        let next = warning.unwrap_or(Context::None);
                        changed |= next != self.context;
                        self.context = next;
                    }
                    if std::mem::take(&mut self.background.repo_again) {
                        let project = self.project.clone();
                        self.background.repo(&project);
                    }
                }
                Observation::SinceGood { generation, text } => {
                    if generation != self.background.failure_generation {
                        continue;
                    }
                    if let Context::CommandFailed { since_good, .. } = &mut self.context {
                        *since_good = text;
                        changed = true;
                    }
                }
            }
        }
        changed
    }

    /// Moves every hosted session forward, including terminals hidden behind the active one.
    fn pump(&mut self) -> Result<bool, String> {
        let mut changed = self.take_observations();
        let mut index = 0;
        while index < self.parked.len() {
            let (exit, output_changed) = self.parked[index].poll()?;
            changed |= output_changed && self.mode == Mode::Terminal;
            self.parked[index].take_structural();
            if let Some(exit_code) = exit {
                let hosted = self.parked.remove(index);
                self.pane_order.retain(|id| id != &hosted.session.id);
                hosted.finish(exit_code)?;
                self.refresh_sessions()?;
                changed = true;
            } else {
                index += 1;
            }
        }
        if self.pane_order.len() <= 1 {
            self.terminal_zoom = false;
        }
        let Some(hosted) = self.hosted.as_mut() else {
            return Ok(changed);
        };
        let previous_state = hosted.session.state.clone();
        let (exit, active_changed) = hosted.poll()?;
        changed |= active_changed && self.mode == Mode::Terminal;

        // A failed command outranks a state change in the band: it is the thing the user just
        // watched happen.
        for outcome in hosted.take_structural() {
            changed = true;
            match outcome {
                crate::pty::Structural::CommandFinished {
                    exit_code,
                    millis,
                    label,
                } if exit_code != 0 => {
                    self.background.since_good(&self.project);
                    self.context = Context::CommandFailed {
                        exit_code,
                        millis,
                        label,
                        since_good: None,
                    };
                }
                // A command that succeeded is not news in itself, but it may have left the
                // repository in a risky state (`git switch --detach` exits 0). Only replaces a
                // quiet band or an older warning.
                crate::pty::Structural::CommandFinished { .. } => {
                    if matches!(self.context, Context::None | Context::RepoWarning { .. }) {
                        let project = self.project.clone();
                        self.background.repo(&project);
                    }
                }
                // Inside an agent, this is the only kind of failure Verb can see at all, and until
                // now it could not see even this.
                crate::pty::Structural::AgentToolFailed { millis, tool } => {
                    self.context = Context::AgentToolFailed { millis, tool };
                }
            }
        }

        if hosted.session.state != previous_state {
            changed = true;
            self.context = Context::SessionState(hosted.session.state.clone());
        }

        if let Some(exit_code) = exit {
            changed = true;
            // The session is over, so any command label it left on screen goes with it.
            self.context = Context::None;
            let hosted = self.hosted.take().expect("checked above");
            self.pane_order.retain(|id| id != &hosted.session.id);
            let session = hosted.finish(exit_code)?;
            self.context = if exit_code == 0 {
                Context::SessionState(session.state.clone())
            } else {
                Context::SessionEnded { exit_code }
            };
            self.refresh_sessions()?;
            if let Some(next) = self.parked.pop() {
                self.project = next.session.project_id.clone();
                let identity = crate::project::identity(&self.project)?;
                self.project_anchor = identity.anchor;
                self.logical_project_id = identity.id;
                self.refresh_git();
                self.hosted = Some(next);
                self.refresh_sessions()?;
            }
            if self.pane_order.len() <= 1 {
                self.terminal_zoom = false;
            }
        }
        Ok(changed)
    }

    fn on_key(&mut self, key: KeyEvent) -> Result<(), String> {
        // Some PTYs report Return as Ctrl+J. Verb overlays treat it as Enter; terminal mode
        // forwards the original key to the hosted program.
        let key = if key.code == KeyCode::Char('j')
            && key.modifiers.contains(KeyModifiers::CONTROL)
            && self.surface != Surface::None
        {
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
        } else {
            key
        };
        if self.surface != Surface::None {
            return self.on_surface_key(key);
        }

        if self.mode != Mode::Terminal {
            return self.on_workbench_key(key);
        }

        // Accelerators, but only at a shell prompt. A full-screen application -- vim, less, an
        // agent -- has the alternate screen, and people map F-keys inside those constantly, so
        // there they belong to the application and the leader remains the way into Verb.
        if let Some(command) = self.accelerator(key) {
            return self.run_command(command);
        }

        // No surface is open, so the terminal has the keyboard and Verb sees only its own chord.
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        let character = match key.code {
            KeyCode::Char(character) => Some(character),
            _ => None,
        };

        // Escape while the menu is open cancels it, rather than reaching the terminal.
        if key.code == KeyCode::Esc && self.leader.is_pending() {
            self.leader.cancel();
            return Ok(());
        }

        if let Some(character) = character {
            let outcome = self.leader.key(control, character);
            match outcome {
                // Sticky: the menu stays until a command key or Esc. No timer, so nothing is lost
                // by pausing to read it.
                Outcome::Pending => return Ok(()),
                Outcome::Run(command) => return self.run_command(command),
                Outcome::SendLeader => {
                    let bytes = self.leader.chord().bytes();
                    return self.forward(&bytes);
                }
                // The key is encoded by the terminal layer, which knows how to write a multi-byte
                // character; the leader deliberately does not try.
                Outcome::SendLeaderThen => {
                    let mut bytes = self.leader.chord().bytes();
                    if let Some(encoded) = keys::encode(key) {
                        bytes.extend(encoded);
                    }
                    return self.forward(&bytes);
                }
                Outcome::Passthrough => {}
            }
        }

        // Everything that is not a character key cannot be the leader, so it belongs to the
        // terminal untouched -- arrows, function keys, Escape and the rest.
        if let Some(bytes) = keys::encode(key) {
            self.forward(&bytes)?;
        }
        Ok(())
    }

    fn refresh_workbench(&mut self) -> Result<(), String> {
        let selected_session = self.selected_actor();
        let selected_task = self.selected_task().map(|task| task.id.clone());
        self.refresh_sessions()?;
        self.tasks = crate::workbench::snapshots(&self.project)?;
        self.memory = crate::workbench::shared_memory(&self.project)?;
        let ids = self
            .project_session_indices()
            .iter()
            .map(|index| self.sessions[*index].id.clone())
            .collect::<Vec<_>>();
        self.inboxes = crate::workbench::inbox_snapshots(&self.project, &ids)?
            .into_iter()
            .map(|inbox| (inbox.session_id.clone(), inbox))
            .collect();
        self.workspace_status.clear();
        for index in self.project_session_indices() {
            let workspace = self.sessions[index].project_id.clone();
            self.workspace_status
                .entry(workspace.clone())
                .or_insert_with(|| crate::git_snapshot(&workspace));
        }
        if let Some(open) = &self.work.inbox {
            self.work.inbox = self.inboxes.get(&open.session_id).cloned();
            self.work.inbox_index = self.work.inbox_index.min(
                self.work
                    .inbox
                    .as_ref()
                    .map_or(0, |inbox| inbox.items.len().saturating_sub(1)),
            );
        }
        let sessions = self.project_session_indices();
        self.work.session = selected_session
            .and_then(|id| {
                sessions
                    .iter()
                    .position(|index| self.sessions[*index].id == id)
            })
            .unwrap_or_else(|| self.work.session.min(sessions.len().saturating_sub(1)));
        let tasks = self.visible_task_indices();
        self.work.task = selected_task
            .and_then(|id| tasks.iter().position(|index| self.tasks[*index].id == id))
            .unwrap_or_else(|| self.work.task.min(tasks.len().saturating_sub(1)));
        Ok(())
    }

    fn refresh_git(&mut self) {
        self.git = crate::git_snapshot(&self.project);
    }

    fn project_session_indices(&self) -> Vec<usize> {
        self.sessions
            .iter()
            .enumerate()
            .filter(|(_, session)| {
                crate::session_in_project_with_id(session, &self.project, &self.logical_project_id)
                    && session.agent.is_some()
            })
            .map(|(index, _)| index)
            .collect()
    }

    fn visible_task_indices(&self) -> Vec<usize> {
        let query = self.work.filter.to_lowercase();
        let mut indices: Vec<usize> = self
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, task)| {
                query.is_empty()
                    || task.title.to_lowercase().contains(&query)
                    || task.id.to_lowercase().contains(&query)
            })
            .map(|(index, _)| index)
            .collect();
        indices.sort_by_key(|index| {
            let task = &self.tasks[*index];
            let priority = if task.needs_help {
                0
            } else {
                match task.status {
                    "needs review" => 1,
                    "active" => 2,
                    "open" => 3,
                    _ => 4,
                }
            };
            (priority, std::cmp::Reverse(task.created_at))
        });
        indices
    }

    fn selected_task(&self) -> Option<&crate::workbench::TaskSnapshot> {
        self.visible_task_indices()
            .get(self.work.task)
            .and_then(|index| self.tasks.get(*index))
    }

    fn open_task_by_id(&mut self, id: &str) {
        self.work.filter.clear();
        self.work.search = None;
        self.work.focus = WorkFocus::Tasks;
        if let Some(position) = self
            .visible_task_indices()
            .iter()
            .position(|index| self.tasks[*index].id == id)
        {
            self.work.task = position;
            self.work.detail = true;
            self.work.scroll = 0;
        }
    }

    fn selected_actor(&self) -> Option<String> {
        self.project_session_indices()
            .get(self.work.session)
            .and_then(|index| self.sessions.get(*index))
            .map(|session| session.id.clone())
    }

    fn session_can_open(&self, session: &crate::Session) -> bool {
        self.hosted
            .iter()
            .chain(self.parked.iter())
            .any(|hosted| hosted.session.id == session.id)
            || (session.state == SessionState::Recoverable && session.project_id.is_dir())
    }

    fn open_workbench_session(&mut self, index: usize) -> Result<(), String> {
        let Some(session) = self.sessions.get(index) else {
            return Ok(());
        };
        if self.session_can_open(session) {
            self.resume_selected(index)
        } else {
            self.open_session_inbox()
        }
    }

    fn enter_workbench(&mut self) -> Result<(), String> {
        self.refresh_workbench()?;
        self.refresh_git();
        self.mode = Mode::Workbench;
        self.work.detail = false;
        self.work.inbox = None;
        self.work.scroll = 0;
        self.surface = Surface::None;
        Ok(())
    }

    fn open_session_inbox(&mut self) -> Result<(), String> {
        let Some(id) = self.selected_actor() else {
            return Ok(());
        };
        let inbox = crate::workbench::inbox_snapshot(&self.project, &id)?;
        self.inboxes.insert(id, inbox.clone());
        self.work.inbox = Some(inbox);
        self.work.inbox_index = 0;
        self.work.detail = false;
        self.work.scroll = 0;
        Ok(())
    }

    fn open_inbox_task(&mut self, index: usize) {
        if let Some(id) = self
            .work
            .inbox
            .as_ref()
            .and_then(|inbox| inbox.items.get(index))
            .map(|item| item.task_id.clone())
        {
            self.open_task_by_id(&id);
        }
    }

    fn enter_terminal(&mut self) -> Result<(), String> {
        if self.hosted.is_none() {
            let (rows, cols) = self.last_size();
            self.start_shell(rows, cols)?;
        }
        self.mode = Mode::Terminal;
        self.surface = Surface::None;
        Ok(())
    }

    fn focus_pane(&mut self, id: &str) -> Result<(), String> {
        if self
            .hosted
            .as_ref()
            .is_some_and(|hosted| hosted.session.id == id)
        {
            return Ok(());
        }
        let Some(index) = self
            .parked
            .iter()
            .position(|hosted| hosted.session.id == id)
        else {
            return Ok(());
        };
        let selected = self.parked.remove(index);
        let project = selected.session.project_id.clone();
        if let Some(current) = self.hosted.replace(selected) {
            self.parked.push(current);
        }
        self.project = project;
        let identity = crate::project::identity(&self.project)?;
        self.project_anchor = identity.anchor;
        self.logical_project_id = identity.id;
        self.refresh_git();
        self.context = Context::None;
        self.message = None;
        self.refresh_sessions()?;
        Ok(())
    }

    fn focus_pane_number(&mut self, number: usize) -> Result<(), String> {
        if let Some(id) = self.pane_order.get(number).cloned() {
            self.focus_pane(&id)?;
        }
        Ok(())
    }

    pub(crate) fn pane_hosts(&self) -> Vec<&Hosted> {
        self.pane_order
            .iter()
            .filter_map(|id| {
                self.hosted
                    .iter()
                    .chain(self.parked.iter())
                    .find(|hosted| hosted.session.id == *id)
            })
            .collect()
    }

    fn task_action(&mut self, action: crate::workbench::TaskAction) -> Result<(), String> {
        let Some(task) = self.selected_task().cloned() else {
            return Ok(());
        };
        let available = match action {
            crate::workbench::TaskAction::Claim => matches!(task.status, "open" | "needs review"),
            crate::workbench::TaskAction::Reassign => task.status != "done",
            crate::workbench::TaskAction::Reply => task.status == "active" && task.needs_help,
            crate::workbench::TaskAction::RequestHelp => {
                task.status == "active" && !task.needs_help
            }
            crate::workbench::TaskAction::Handoff | crate::workbench::TaskAction::Done => {
                task.status == "active"
            }
        };
        if !available {
            self.message = Some(format!(
                "That action is unavailable while this task is {}.",
                task.status
            ));
            return Ok(());
        }
        match action {
            crate::workbench::TaskAction::Claim
            | crate::workbench::TaskAction::Reassign
            | crate::workbench::TaskAction::Reply => {
                let candidates = self.actor_candidates(action);
                if candidates.is_empty() {
                    self.message = Some(
                        "No eligible agent session. Start or resume an agent first.".to_owned(),
                    );
                    return Ok(());
                }
                let selected = self
                    .selected_actor()
                    .and_then(|id| {
                        candidates
                            .iter()
                            .position(|index| self.sessions[*index].id == id)
                    })
                    .unwrap_or(0);
                self.work.actor_picker = Some(ActorPicker { action, selected });
                return Ok(());
            }
            _ => {}
        }
        let actor = task.owner.clone();
        let Some(actor) = actor else {
            self.message =
                Some("This task has no owner. Claim it with an agent session first.".to_owned());
            return Ok(());
        };
        self.start_task_action(action, actor)
    }

    fn actor_candidates(&self, action: crate::workbench::TaskAction) -> Vec<usize> {
        let owner = self.selected_task().and_then(|task| task.owner.as_deref());
        self.project_session_indices()
            .into_iter()
            .filter(|index| {
                let session = &self.sessions[*index];
                session.state != SessionState::Ended
                    && !(matches!(
                        action,
                        crate::workbench::TaskAction::Reply
                            | crate::workbench::TaskAction::Reassign
                    ) && owner == Some(session.id.as_str()))
            })
            .collect()
    }

    fn start_task_action(
        &mut self,
        action: crate::workbench::TaskAction,
        actor: String,
    ) -> Result<(), String> {
        let Some(task) = self.selected_task().cloned() else {
            return Ok(());
        };
        if action == crate::workbench::TaskAction::Claim {
            match crate::workbench::apply_ui_action(
                &self.project,
                &task.id,
                &actor,
                action,
                None,
                None,
            ) {
                Ok(message) => {
                    self.message = Some(message);
                    self.refresh_workbench()?;
                }
                Err(error) => self.message = Some(error),
            }
        } else {
            self.work.composer = Some(Composer {
                task_id: task.id.clone(),
                action,
                actor,
                text: String::new(),
                expected_revision: (action == crate::workbench::TaskAction::Handoff)
                    .then(|| crate::workbench::handoff_revision(&self.project, &task.id))
                    .transpose()?,
            });
        }
        Ok(())
    }

    /// Moves the scrollback view; positive is further back.
    fn scroll_by(&mut self, lines: isize) -> Result<(), String> {
        let Surface::Scrollback { offset, .. } = &self.surface else {
            return Ok(());
        };
        let target = if lines >= 0 {
            offset.saturating_add(lines as usize)
        } else {
            offset.saturating_sub(lines.unsigned_abs())
        };
        let applied = match self.hosted.as_mut() {
            Some(hosted) => hosted.scroll_to(target),
            None => 0,
        };
        if let Surface::Scrollback { offset, .. } = &mut self.surface {
            *offset = applied;
        }
        Ok(())
    }

    fn scroll_to_end(&mut self) -> Result<(), String> {
        if let Some(hosted) = self.hosted.as_mut() {
            hosted.scroll_to(0);
        }
        if let Surface::Scrollback { offset, .. } = &mut self.surface {
            *offset = 0;
        }
        Ok(())
    }

    /// Finds `term` in the scrollback and moves the view to it.
    ///
    /// Searches the rendered rows rather than a copy of the stream, so what is searched is exactly
    /// what was on screen -- and nothing of the session has to be retained to make search work.
    fn search(&mut self, term: &str, direction: isize) -> Result<(), String> {
        if term.is_empty() {
            return Ok(());
        }
        let Surface::Scrollback { offset, .. } = &self.surface else {
            return Ok(());
        };
        let start = *offset;
        let Some(hosted) = self.hosted.as_mut() else {
            return Ok(());
        };
        let limit = hosted.scrollback_limit();
        let needle = term.to_lowercase();

        let mut candidate = start as isize;
        for _ in 0..limit {
            candidate += direction;
            if candidate < 0 || candidate as usize > limit {
                break;
            }
            let found = hosted
                .rows_at(candidate as usize)
                .iter()
                .any(|row| row.to_lowercase().contains(&needle));
            if found {
                let applied = hosted.scroll_to(candidate as usize);
                if let Surface::Scrollback { offset, .. } = &mut self.surface {
                    *offset = applied;
                }
                self.message = None;
                return Ok(());
            }
        }
        self.message = Some(format!("No further match for \"{term}\"."));
        Ok(())
    }

    fn run_command(&mut self, command: Command) -> Result<(), String> {
        match command {
            Command::Workbench => self.enter_workbench()?,
            Command::FocusPane(number) => self.focus_pane_number(number)?,
            Command::ToggleZoom => {
                if self.pane_order.len() > 1 {
                    self.terminal_zoom = !self.terminal_zoom;
                } else {
                    self.message = Some("Open another agent session to use pane zoom.".to_owned());
                }
            }
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
            Command::ToggleMouse => {
                self.mouse_enabled = !self.mouse_enabled;
                self.message = Some(if self.mouse_enabled {
                    "Mouse is Verb's: the bar is clickable. Option-drag still selects text."
                        .to_owned()
                } else {
                    format!(
                        "Mouse belongs to the terminal: selection works, the bar does not. {} m to take it back.",
                        self.leader.chord()
                    )
                });
            }
            // Everything Verb has observed, assembled the same way `verb context` assembles it.
            Command::Contextual => self.surface = Surface::Evidence,
            Command::Scrollback => {
                if self.hosted.is_some() {
                    self.surface = Surface::Scrollback {
                        offset: 0,
                        search: None,
                        last_search: None,
                    };
                    self.scroll_by(1)?;
                } else {
                    self.message = Some(
                        "No session running, so there is nothing to scroll back through."
                            .to_owned(),
                    );
                }
            }
        }
        Ok(())
    }

    fn run_action(&mut self, action: Action) -> Result<(), String> {
        match action {
            Action::Workbench => self.enter_workbench(),
            Action::Sessions => self.run_command(Command::Sessions),
            Action::Help => self.run_command(Command::Help),
            Action::Evidence => self.run_command(Command::Contextual),
            Action::Changes => {
                self.surface = Surface::Changes;
                Ok(())
            }
            Action::Scrollback => self.run_command(Command::Scrollback),
            Action::ToggleMouse => self.run_command(Command::ToggleMouse),
            Action::Resume => self.resume_here(),
            Action::NewShell => {
                let (rows, cols) = self.last_size();
                self.start_shell(rows, cols)
            }
            Action::NewAgent(agent) => {
                let (rows, cols) = self.last_size();
                self.start(
                    crate::begin_session(&self.project.clone(), agent, Vec::new()),
                    rows,
                    cols,
                )
            }
            Action::NewIsolatedAgent(agent) => {
                let workspace = crate::project::create_isolated_checkout(&self.project)?;
                let (rows, cols) = self.last_size();
                self.start(
                    crate::begin_session(&workspace, agent, Vec::new()),
                    rows,
                    cols,
                )
                .map_err(|error| {
                    format!(
                        "{error}; isolated workspace kept at {}",
                        workspace.display()
                    )
                })
            }
            Action::NewExternalAgent => {
                self.surface = Surface::ExternalAgent {
                    command: String::new(),
                    isolated: false,
                };
                Ok(())
            }
            Action::NewIsolatedExternalAgent => {
                self.surface = Surface::ExternalAgent {
                    command: String::new(),
                    isolated: true,
                };
                Ok(())
            }
            Action::NewExternalPreset(command, args) => {
                let (rows, cols) = self.last_size();
                self.start(
                    crate::begin_external_session(
                        &self.project,
                        command.to_owned(),
                        args.iter().map(|arg| (*arg).to_owned()).collect(),
                    )?,
                    rows,
                    cols,
                )
            }
            Action::Reconcile => {
                self.refresh_sessions()?;
                self.message =
                    Some("Recovery re-checked from each agent's own evidence.".to_owned());
                Ok(())
            }
            Action::Quit => {
                if let Some(hosted) = self.hosted.take() {
                    let session = hosted.stop()?;
                    self.context = Context::SessionState(session.state.clone());
                }
                for hosted in self.parked.drain(..) {
                    hosted.stop()?;
                }
                self.pane_order.clear();
                self.refresh_sessions()?;
                self.quit = true;
                Ok(())
            }
        }
    }

    fn resume_here(&mut self) -> Result<(), String> {
        let project = self.project.clone();
        if self.hosted.as_ref().is_some_and(|hosted| {
            crate::session_in_project_with_id(&hosted.session, &project, &self.logical_project_id)
                && hosted.session.agent.is_some()
        }) {
            self.mode = Mode::Terminal;
            return Ok(());
        }
        if let Some(id) = self
            .parked
            .iter()
            .rev()
            .find(|hosted| {
                crate::session_in_project_with_id(
                    &hosted.session,
                    &project,
                    &self.logical_project_id,
                ) && hosted.session.agent.is_some()
            })
            .map(|hosted| hosted.session.id.clone())
        {
            if let Some(index) = self.sessions.iter().position(|session| session.id == id) {
                return self.resume_selected(index);
            }
        }
        self.resume_project(&project, None)
    }

    fn resume_selected(&mut self, index: usize) -> Result<(), String> {
        let Some(session) = self.sessions.get(index) else {
            return Ok(());
        };
        let id = session.id.clone();
        let project = session.project_id.clone();
        if self
            .hosted
            .as_ref()
            .is_some_and(|hosted| hosted.session.id == id)
        {
            self.mode = Mode::Terminal;
            return Ok(());
        }
        if self.parked.iter().any(|hosted| hosted.session.id == id) {
            self.focus_pane(&id)?;
            self.mode = Mode::Terminal;
            return Ok(());
        }
        self.resume_project(&project, Some(&id))
    }

    fn start_selected(&mut self, index: usize) -> Result<(), String> {
        let Some(session) = self.sessions.get(index) else {
            return Ok(());
        };
        let project = session.project_id.clone();
        let agent = session.agent.clone();
        let (rows, cols) = self.last_size();
        match agent {
            Some(Agent::External) => {
                self.message = Some(
                    "Start another external agent with 'verb agent CMD' in a shell; Verb does not store its executable."
                        .to_owned(),
                );
                Ok(())
            }
            Some(agent) => self.start(
                crate::begin_session(&project, agent, Vec::new()),
                rows,
                cols,
            ),
            // A shell session records no agent, so there is nothing to start a *new* one of.
            None => {
                self.message = Some("That session has no agent to start.".to_owned());
                Ok(())
            }
        }
    }

    /// Forgets Verb's record of a session. The agent's own conversation is untouched -- Verb never
    /// owned it -- and the session Verb is currently hosting cannot be forgotten while it runs.
    fn forget_selected(&mut self, index: usize) -> Result<(), String> {
        let Some(session) = self.sessions.get(index) else {
            return Ok(());
        };
        if self
            .hosted
            .iter()
            .chain(self.parked.iter())
            .any(|hosted| hosted.session.id == session.id)
        {
            self.message = Some("That session is running here; end it first.".to_owned());
            return Ok(());
        }
        let project = session.project_id.clone();
        crate::forget_session(&session.id)?;
        self.message = Some(format!(
            "Forgot Verb's record of {}. The agent's own conversation is untouched.",
            crate::display_path(&project)
        ));
        self.refresh_sessions()?;
        if let Surface::Sessions { selected } = &mut self.surface {
            *selected = (*selected).min(self.sessions.len().saturating_sub(1));
        }
        Ok(())
    }

    fn resume_project(&mut self, project: &Path, id: Option<&str>) -> Result<(), String> {
        let (rows, cols) = self.last_size();
        match crate::begin_resume(project, id) {
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
        let project = start.session.project_id.clone();
        let next = Hosted::start(
            &project,
            start.session,
            &start.command,
            &start.args,
            &start.env,
            start.is_new,
            rows,
            cols,
        )?;
        if !self.pane_order.iter().any(|id| id == &next.session.id) {
            self.pane_order.push(next.session.id.clone());
        }
        if let Some(previous) = self.hosted.replace(next) {
            self.parked.push(previous);
        }
        self.project = project;
        let identity = crate::project::identity(&self.project)?;
        self.project_anchor = identity.anchor;
        self.logical_project_id = identity.id;
        self.refresh_git();
        self.mode = Mode::Terminal;
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

    /// A plain workspace, for tests in this module and in `render`.
    #[cfg(test)]
    pub(super) fn set_context_for_tests(&mut self, context: Context) {
        self.context = context;
    }

    #[cfg(test)]
    pub(super) fn for_tests() -> App {
        App {
            project: PathBuf::from("/tmp/project"),
            project_anchor: PathBuf::from("/tmp/project"),
            logical_project_id: String::new(),
            git: crate::git_snapshot(Path::new("/tmp/project")),
            mode: Mode::Terminal,
            work: WorkState::default(),
            tasks: Vec::new(),
            memory: String::new(),
            inboxes: HashMap::new(),
            workspace_status: HashMap::new(),
            leader: Leader::default_chord(),
            surface: Surface::None,
            context: Context::None,
            hosted: None,
            parked: Vec::new(),
            pane_order: Vec::new(),
            pane_areas: Vec::new(),
            terminal_zoom: false,
            sessions: Vec::new(),
            imported_sessions: Vec::new(),
            message: None,
            quit: false,
            mouse_captured: false,
            mouse_enabled: true,
            frame_height: 24,
            frame_width: 80,
            background: Background::new(),
        }
    }

    /// The row the action bar occupies: the last one, always.
    fn bar_row(&self) -> u16 {
        self.frame_height.saturating_sub(1)
    }

    /// Whether Verb is holding the mouse. Read by the bar, which says so only when it is not.
    pub(crate) fn mouse_enabled(&self) -> bool {
        self.mouse_enabled
    }

    fn last_size(&self) -> (u16, u16) {
        self.hosted
            .as_ref()
            .map(|hosted| {
                let (rows, cols) = hosted.screen().size();
                (rows.max(1), cols.max(1))
            })
            .unwrap_or_else(|| render::hosting_size(self.frame_width, self.frame_height))
    }

    pub(crate) fn project(&self) -> &Path {
        &self.project
    }

    pub(crate) fn leader_pending(&self) -> bool {
        self.leader.is_pending()
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

    pub(crate) fn imported_sessions(&self) -> &[crate::continuity::ImportedSession] {
        &self.imported_sessions
    }

    pub(crate) fn hosted(&self) -> Option<&Hosted> {
        self.hosted.as_ref()
    }
}

/// What a palette entry does. Every variant is an existing capability, reachable from the CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Action {
    Workbench,
    Resume,
    NewShell,
    NewAgent(Agent),
    NewIsolatedAgent(Agent),
    NewExternalAgent,
    NewIsolatedExternalAgent,
    NewExternalPreset(&'static str, &'static [&'static str]),
    Sessions,
    Reconcile,
    Evidence,
    Changes,
    Scrollback,
    Help,
    ToggleMouse,
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
            event::DisableMouseCapture,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        App::for_tests()
    }

    fn click(column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(event::MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn workbench_buttons_and_rows_complete_actions_with_one_click() {
        let mut app = app();
        app.mode = Mode::Workbench;
        app.frame_width = 120;
        app.frame_height = 30;

        app.on_mouse(click(39, 6)).unwrap();
        assert!(app.work.new_task.is_some());
        let cancel = (0..30)
            .flat_map(|row| (0..120).map(move |column| (column, row)))
            .find(|(column, row)| {
                workbench_view::modal_action_at(&app, *column, *row)
                    == Some(workbench_view::ClickAction::Cancel)
            })
            .unwrap();
        app.on_mouse(click(cancel.0, cancel.1)).unwrap();
        assert!(app.work.new_task.is_none());

        app.on_mouse(click(3, 7)).unwrap();
        assert!(matches!(app.surface, Surface::Palette { .. }));
    }

    #[test]
    fn clicking_an_action_on_the_bar_runs_it() {
        // The bar is the primary visible affordance. Something a person can see and point at, and
        // which then does nothing, is worse than not showing it at all.
        let mut app = app();
        let help = render::bar_slots(&app)
            .into_iter()
            .find(|slot| slot.label == "Help")
            .expect("Help is always on the bar");

        app.on_mouse(click(help.start + 2, app.bar_row())).unwrap();

        assert!(matches!(app.surface, Surface::Help));
    }

    #[test]
    fn the_whole_label_is_the_target_not_just_the_bracketed_key() {
        let mut app = app();
        let sessions = render::bar_slots(&app)
            .into_iter()
            .find(|slot| slot.label == "Workbench")
            .expect("Workbench is on the bar");

        // The last column of "[F2] Workbench" is clickable too.
        app.on_mouse(click(sessions.start + sessions.width - 1, app.bar_row()))
            .unwrap();

        assert_eq!(app.mode, Mode::Workbench);
    }

    #[test]
    fn a_click_on_empty_bar_space_does_nothing_at_all() {
        // Between two actions, and past the end of the last one: a miss must stay a miss rather
        // than resolving to whichever action happens to be nearest.
        let mut app = app();
        let slots = render::bar_slots(&app);
        let gap = slots[0].start + slots[0].width;
        let past_the_end = slots
            .last()
            .map(|slot| slot.start + slot.width + 5)
            .unwrap();

        app.on_mouse(click(gap, app.bar_row())).unwrap();
        assert!(matches!(app.surface, Surface::None));

        app.on_mouse(click(past_the_end, app.bar_row())).unwrap();
        assert!(matches!(app.surface, Surface::None));
    }

    #[test]
    fn a_click_in_the_terminal_region_belongs_to_the_terminal() {
        // Rows above the bar are the work. Verb takes the click event but must not act on it.
        let mut app = app();

        app.on_mouse(click(4, 0)).unwrap();
        app.on_mouse(click(4, app.bar_row().saturating_sub(1)))
            .unwrap();

        assert!(matches!(app.surface, Surface::None));
    }

    #[test]
    fn the_mouse_is_verbs_by_default_and_can_be_handed_back() {
        // Default on, because that is what makes the bar real. Reversible, because Option-drag is
        // not universal and a terminal where it fails must not be a dead end.
        let mut app = app();
        assert!(app.mouse_enabled());

        app.run_command(Command::ToggleMouse).unwrap();
        assert!(!app.mouse_enabled());
        assert!(app.message.as_deref().unwrap().contains("selection works"));

        app.run_command(Command::ToggleMouse).unwrap();
        assert!(app.mouse_enabled());
    }

    #[test]
    fn handing_the_mouse_back_leaves_the_bar_unclickable_but_still_keyed() {
        let mut app = app();
        let help = render::bar_slots(&app)
            .into_iter()
            .find(|slot| slot.label == "Help")
            .unwrap();
        app.run_command(Command::ToggleMouse).unwrap();
        app.message = None;

        // No capture means no events arrive at all; the accelerator is the way in.
        app.run_command(
            app.accelerator(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE))
                .unwrap(),
        )
        .unwrap();

        assert!(matches!(app.surface, Surface::Help));
        assert_eq!(help.command, Command::Help);
    }

    #[test]
    fn choosing_changed_files_opens_its_own_surface() {
        let mut app = app();
        app.run_action(Action::Changes).unwrap();
        assert!(matches!(app.surface, Surface::Changes));
        // Esc gives the keyboard back to the terminal, like every other Verb surface.
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .unwrap();
        assert!(matches!(app.surface, Surface::None));
    }

    #[test]
    fn arbitrary_agent_launcher_accepts_an_executable_without_shell_parsing() {
        let mut app = app();
        app.frame_width = 120;
        app.frame_height = 30;
        app.run_action(Action::NewExternalAgent).unwrap();
        for character in "agy".chars() {
            app.on_surface_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE))
                .unwrap();
        }
        assert!(matches!(
            app.surface,
            Surface::ExternalAgent { ref command, .. } if command == "agy"
        ));
        let rect = render::external_agent_rect(ratatui::layout::Rect::new(0, 0, 120, 30));
        app.on_mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: rect.x + 3,
            row: rect.y + 2,
            modifiers: KeyModifiers::NONE,
        })
        .unwrap();
        assert!(matches!(app.surface, Surface::ExternalAgent { .. }));
        app.on_surface_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .unwrap();
        assert!(matches!(app.surface, Surface::None));
    }

    #[test]
    fn isolated_launcher_keeps_the_generic_cli_choice() {
        let mut app = app();
        app.run_action(Action::NewIsolatedExternalAgent).unwrap();
        assert!(matches!(
            app.surface,
            Surface::ExternalAgent { isolated: true, .. }
        ));
    }

    #[test]
    fn only_one_verb_surface_can_be_open_at_a_time() {
        // The budget in docs/UX_FOUNDATION.md forbids stacking overlays, and the state makes it
        // impossible rather than merely discouraged: asking for a second replaces the first.
        let mut app = app();
        app.run_command(Command::Palette).unwrap();
        assert!(matches!(app.surface, Surface::Palette { .. }));

        app.run_command(Command::Help).unwrap();
        assert!(matches!(app.surface, Surface::Help));

        app.run_command(Command::Contextual).unwrap();
        assert!(matches!(app.surface, Surface::Evidence));
    }

    #[test]
    fn accelerators_belong_to_a_full_screen_application_when_one_is_running() {
        // Nothing is hosted in this test, so there is no alternate screen and the keys are Verb's.
        // The inverse -- an agent or an editor owning them -- is the case that matters, and is
        // decided by the same flag rather than by guessing at what is running.
        let app = app();
        assert!(app.accelerators_active());
        assert!(matches!(
            app.accelerator(KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE)),
            Some(Command::Help)
        ));
        assert!(app
            .accelerator(KeyEvent::new(KeyCode::F(9), KeyModifiers::NONE))
            .is_none());
    }

    #[test]
    fn the_leader_menu_does_not_close_itself() {
        let mut app = app();
        // Ctrl+Space is the leader, and it stays pending with no timer to close it.
        app.on_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::CONTROL))
            .unwrap();
        assert!(app.leader_pending());

        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .unwrap();
        assert!(!app.leader_pending());
    }

    #[test]
    fn scrollback_is_refused_when_there_is_no_session_to_scroll() {
        // Rather than opening an empty surface, which would be a panel that answers no question.
        let mut app = app();
        app.run_command(Command::Scrollback).unwrap();

        assert!(matches!(app.surface, Surface::None));
        assert!(app.message.is_some());
    }

    #[test]
    fn control_j_moves_new_task_input_from_title_to_brief() {
        let mut app = app();
        app.mode = Mode::Workbench;
        app.work.new_task = Some(NewTask::default());
        for character in "Title".chars() {
            app.on_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE))
                .unwrap();
        }
        app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL))
            .unwrap();
        app.on_key(KeyEvent::new(KeyCode::Char('B'), KeyModifiers::NONE))
            .unwrap();
        let draft = app.work.new_task.as_ref().unwrap();
        assert_eq!(draft.title, "Title");
        assert_eq!(draft.brief, "B");
    }

    #[test]
    fn workbench_commands_follow_the_configured_leader() {
        let mut app = app();
        app.mode = Mode::Workbench;
        app.leader = Leader::configured(leader::Chord::ctrl('g'));
        app.on_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL))
            .unwrap();
        assert!(matches!(app.surface, Surface::Palette { .. }));
    }

    #[test]
    fn task_claim_opens_an_explicit_agent_chooser() {
        let mut app = app();
        app.mode = Mode::Workbench;
        app.work.detail = true;
        app.tasks.push(crate::workbench::TaskSnapshot {
            id: "a".repeat(32),
            title: "Review parser".to_owned(),
            brief: String::new(),
            status: "open",
            owner: None,
            needs_help: false,
            created_at: 1,
            events: Vec::new(),
        });
        app.sessions
            .push(crate::Session::new(app.project.clone(), Agent::Claude));
        app.sessions
            .push(crate::Session::new(app.project.clone(), Agent::Codex));
        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE))
            .unwrap();
        assert!(matches!(
            app.work.actor_picker,
            Some(ActorPicker { selected: 0, .. })
        ));
        app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
            .unwrap();
        assert!(matches!(
            app.work.actor_picker,
            Some(ActorPicker { selected: 1, .. })
        ));
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .unwrap();
        assert!(app.work.actor_picker.is_none());
    }

    #[test]
    fn a_new_task_opens_even_when_an_older_task_has_higher_priority() {
        let mut app = app();
        app.mode = Mode::Workbench;
        app.work.filter = "older".to_owned();
        app.tasks.push(crate::workbench::TaskSnapshot {
            id: "a".repeat(32),
            title: "Older review".to_owned(),
            brief: String::new(),
            status: "needs review",
            owner: None,
            needs_help: false,
            created_at: 1,
            events: Vec::new(),
        });
        let id = "b".repeat(32);
        app.tasks.push(crate::workbench::TaskSnapshot {
            id: id.clone(),
            title: "New task".to_owned(),
            brief: String::new(),
            status: "open",
            owner: None,
            needs_help: false,
            created_at: 2,
            events: Vec::new(),
        });

        app.open_task_by_id(&id);

        assert_eq!(app.work.focus, WorkFocus::Tasks);
        assert!(app.work.detail);
        assert!(app.work.filter.is_empty());
        assert_eq!(app.selected_task().unwrap().id, id);
    }

    #[test]
    fn control_j_activates_a_palette_choice_in_verb_owned_ui() {
        let mut app = app();
        app.mode = Mode::Workbench;
        app.surface = Surface::Palette {
            filter: "Help".to_owned(),
            selected: 0,
        };
        app.on_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL))
            .unwrap();
        assert!(matches!(app.surface, Surface::Help));
    }
}
