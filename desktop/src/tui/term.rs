//! The embedded terminal: a real session, hosted inside a pane rather than handed the whole screen.
//!
//! Same launch decision, same records, same events as the CLI -- `begin_session` and `begin_resume`
//! decide *what* runs, `pty::spawn` starts it, and `finish_session` closes it out. What differs is
//! only the hosting: bytes are parsed into a terminal state (`vt100`) that Verb draws, instead of
//! being proxied to a terminal Verb had given away.

use crate::observe::{AgentEvent, AgentWatch, Observed};
use crate::pty::{self, ShellIntegration, Structural};
use crate::{EventLogger, Session};
use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

pub struct Hosted {
    pub session: Session,
    logger: EventLogger,
    integration: ShellIntegration,
    master: std::fs::File,
    pid: i32,
    parser: vt100::Parser,
    output: Receiver<Vec<u8>>,
    reader_finished: bool,
    exit_code: Option<i32>,
    /// Structural outcomes the workspace has not yet reacted to. The band reads these; nothing
    /// stores them.
    pending: Vec<Structural>,
    /// The agent's own record, once it has begun one. Shared with the CLI proxy, which observes
    /// the same way from its own loop.
    watch: AgentWatch,
}

impl Hosted {
    /// Starts `command` for `session` on its own PTY, sized to the pane it will be drawn in.
    #[allow(clippy::too_many_arguments)] // Each argument is a distinct fact about the launch;
                                         // bundling them would only move the list somewhere else.
    pub fn start(
        project: &std::path::Path,
        mut session: Session,
        command: &str,
        args: &[String],
        env: &[(String, String)],
        is_new: bool,
        rows: u16,
        cols: u16,
    ) -> Result<Self, String> {
        let process = pty::spawn(project, &session.id, command, args, env, Some((rows, cols)))?;

        let mut logger = EventLogger::new(&session)?;
        if is_new {
            logger.session_started(&session)?;
        } else if let Some(agent) = session.agent.as_ref() {
            logger.agent_started(agent.label())?;
        }
        logger.process_started()?;
        crate::save_session(&session)?;

        // A reader thread exists so drawing never waits on a process that has nothing to say. It
        // only moves bytes; every decision about them is made on the main thread.
        let (sender, output) = mpsc::channel();
        let mut reader = process
            .master
            .try_clone()
            .map_err(|error| format!("could not observe the session: {error}"))?;
        thread::spawn(move || {
            let mut buffer = [0_u8; 8 * 1024];
            while let Ok(count) = reader.read(&mut buffer) {
                if count == 0 || sender.send(buffer[..count].to_vec()).is_err() {
                    break;
                }
            }
        });

        session.state = crate::SessionState::Live;
        let watch =
            AgentWatch::for_agent(session.agent.as_ref().map(|agent| agent.label()), project);
        Ok(Self {
            session,
            logger,
            integration: ShellIntegration::new(),
            master: process.master,
            pid: process.pid,
            parser: vt100::Parser::new(rows, cols, SCROLLBACK_LINES),
            output,
            reader_finished: false,
            exit_code: None,
            pending: Vec::new(),
            watch,
        })
    }

    /// Drains whatever the session has produced and returns its exit code once it has finished.
    ///
    /// The same bytes go two places and nowhere else: into the terminal state Verb draws, and past
    /// the shell-integration scanner that turns markers into structural events. Neither retains
    /// them.
    pub fn poll(&mut self) -> Result<Option<i32>, String> {
        loop {
            match self.output.try_recv() {
                Ok(bytes) => {
                    self.parser.process(&bytes);
                    let structural =
                        self.integration
                            .observe(&bytes, &mut self.session, &mut self.logger)?;
                    self.pending.extend(structural);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.reader_finished = true;
                    break;
                }
            }
        }

        self.poll_agent_record()?;

        if self.exit_code.is_none() {
            self.exit_code = pty::reap(self.pid)?;
        }
        Ok(self.exit_code)
    }

    /// Reads whatever the agent has appended to its own record since the last tick.
    ///
    /// A failure to read is not reported as "nothing happened": the tail simply produces no events,
    /// and [`Hosted::observed`] stays empty, which the overlay renders as unobserved.
    fn poll_agent_record(&mut self) -> Result<(), String> {
        let now = crate::now_millis();
        for event in self.watch.poll(now) {
            self.logger.agent_observed(&event)?;
            if let AgentEvent::ToolOutcome { failed: true, .. } = event {
                // The band's whole purpose is the moment something went wrong, and until now it
                // could not speak about anything that happened inside an agent.
                self.pending.push(Structural::AgentToolFailed {
                    millis: now,
                    tool: self.watch.last_tool(),
                });
            }
        }
        if self.session.resume_identity.is_none() {
            // Known only once a line has been read, so this follows the poll rather than preceding
            // it.
            self.session.resume_identity = self.watch.conversation_id();
        }
        Ok(())
    }

    /// What the agent's record says it has done so far, when Verb is reading one at all.
    ///
    /// `None` for a shell, or for an agent whose record Verb has no reader for. The distinction is
    /// the point: an empty observation and an absent one mean different things, and only one of
    /// them may be shown as "nothing has gone wrong".
    pub fn observed_if_read(&self) -> Option<&Observed> {
        self.watch.observed_if_read()
    }

    /// Takes whatever the shell reported since the last call. Command boundaries only arrive from a
    /// shell with integration enabled; a shell that reports nothing produces nothing here, which is
    /// why the band can stay silent rather than inventing a boundary.
    pub fn take_structural(&mut self) -> Vec<Structural> {
        std::mem::take(&mut self.pending)
    }

    /// Moves the view back through the scrollback the parser already keeps.
    ///
    /// The session itself is untouched: this changes what Verb draws, not what the program below
    /// believes about its screen. Returns the offset actually applied, which is clamped to what
    /// exists -- scrolling past the top is a no-op rather than an empty screen.
    pub fn scroll_to(&mut self, offset: usize) -> usize {
        let offset = offset.min(SCROLLBACK_LINES);
        self.parser.screen_mut().set_scrollback(offset);
        self.parser.screen().scrollback()
    }

    /// The visible rows at a given scrollback offset, for searching without disturbing the view.
    pub fn rows_at(&mut self, offset: usize) -> Vec<String> {
        let restore = self.parser.screen().scrollback();
        self.parser.screen_mut().set_scrollback(offset);
        let (_, cols) = self.parser.screen().size();
        let rows: Vec<String> = self.parser.screen().rows(0, cols).collect();
        self.parser.screen_mut().set_scrollback(restore);
        rows
    }

    /// True when the hosted program has taken the alternate screen -- vim, less, htop, Claude,
    /// Codex, OpenCode. It is the one reliable signal that a full-screen application owns the
    /// terminal, and Verb uses it to decide whether an accelerator key is Verb's or theirs.
    pub fn full_screen_app(&self) -> bool {
        self.parser.screen().alternate_screen()
    }

    pub fn scrollback_limit(&self) -> usize {
        SCROLLBACK_LINES
    }

    pub fn screen(&self) -> &vt100::Screen {
        self.parser.screen()
    }

    pub fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.master
            .write_all(bytes)
            .map_err(|error| format!("could not write to the session: {error}"))
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.parser.screen_mut().set_size(rows, cols);
        pty::set_window_size(&self.master, rows, cols);
    }

    /// Closes the record out exactly as the CLI does: process ended, agent ended, state resolved
    /// from the agent's own evidence, session saved.
    pub fn finish(mut self, exit_code: i32) -> Result<Session, String> {
        crate::finish_session_quietly(&mut self.session, exit_code)?;
        Ok(self.session)
    }
}

const SCROLLBACK_LINES: usize = 1_000;
