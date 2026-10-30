//! The embedded terminal: a real session, hosted inside a pane rather than handed the whole screen.
//!
//! Same launch decision, same records, same events as the CLI -- `begin_session` and `begin_resume`
//! decide *what* runs, `pty::spawn` starts it, and `finish_session` closes it out. What differs is
//! only the hosting: bytes are parsed into a terminal state (`vt100`) that Verb draws, instead of
//! being proxied to a terminal Verb had given away.

use crate::mobile::{LiveBridge, LocalServer};
use crate::observe::{AgentEvent, AgentWatch, Observed};
use crate::pty::{self, ShellIntegration, Structural};
use crate::{EventLogger, Session};
use std::io::Read;
use std::os::fd::AsRawFd;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

pub struct Hosted {
    pub session: Session,
    logger: EventLogger,
    integration: ShellIntegration,
    master: std::fs::File,
    pid: i32,
    /// Held for the whole PTY lifetime so no second Verb process can resume this session.
    _session_lock: std::fs::File,
    parser: vt100::Parser,
    mobile: LiveBridge,
    _mobile_server: Option<LocalServer>,
    output: Receiver<Vec<u8>>,
    web_output: Option<Vec<Vec<u8>>>,
    reader_finished: bool,
    exit_code: Option<i32>,
    /// Structural outcomes the workspace has not yet reacted to. The band reads these; nothing
    /// stores them.
    pending: Vec<Structural>,
    /// The agent's own record, once it has begun one. Shared with the CLI proxy, which observes
    /// the same way from its own loop.
    watch: AgentWatch,
    /// The agent's conversation as Verb can show and continue it: `crate::conversation`.
    conversation: Option<crate::conversation::Engine>,
    pub(crate) sink: Option<std::sync::Arc<crate::stream::TerminalStreamSink>>,
    /// Set only after the process and durable session have both been closed out.
    closed: bool,
}

impl Hosted {
    /// Starts `command` for `session` on its own PTY, sized to the pane it will be drawn in.
    #[allow(clippy::too_many_arguments)] // Each argument is a distinct fact about the launch;
                                         // bundling them would only move the list somewhere else.
    pub fn start(
        project: &std::path::Path,
        session: Session,
        command: &str,
        args: &[String],
        env: &[(String, String)],
        is_new: bool,
        rows: u16,
        cols: u16,
    ) -> Result<Self, String> {
        Self::start_with_sink(
            project, session, command, args, env, is_new, rows, cols, None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start_with_sink(
        project: &std::path::Path,
        mut session: Session,
        command: &str,
        args: &[String],
        env: &[(String, String)],
        is_new: bool,
        rows: u16,
        cols: u16,
        sink: Option<std::sync::Arc<crate::stream::TerminalStreamSink>>,
    ) -> Result<Self, String> {
        let session_lock = crate::lock_session_for_host(&session.id)?;
        if is_new {
            // The child can fetch shared context immediately after exec, before this host's
            // post-spawn setup. Make its identity visible first without claiming it is live yet.
            session.state = crate::SessionState::Interrupted;
            crate::save_session(&session)?;
        }
        // This boundary must precede process creation. A fast agent can create its transcript
        // before `spawn` returns, and then a later boundary would reject the right record as old.
        let watch = AgentWatch::for_session(
            session.agent.as_ref().map(|agent| agent.label()),
            project,
            session.resume_identity.as_deref(),
        );
        let process = pty::spawn(
            project,
            &session.id,
            command,
            args,
            env,
            Some((rows, cols)),
            Some(session_lock.as_raw_fd()),
        )?;
        let setup = (|| -> Result<_, String> {
            let reader = process
                .master
                .try_clone()
                .map_err(|error| format!("could not observe the session: {error}"))?;
            let mut logger = EventLogger::new(&session)?;
            if is_new {
                logger.session_started(&session)?;
            } else if let Some(agent) = session.agent.as_ref() {
                logger.agent_started(agent.label())?;
            }
            logger.process_started()?;
            session.state = crate::SessionState::Live;
            session.last_seen_at = crate::now_millis();
            crate::save_session(&session)?;
            Ok((reader, logger))
        })();
        let (mut reader, logger) = match setup {
            Ok(setup) => setup,
            Err(error) => {
                let _ = pty::terminate(process.pid);
                return Err(error);
            }
        };

        // A reader thread exists so drawing never waits on a process that has nothing to say. It
        // only moves bytes; every decision about them is made on the main thread.
        // Bound the reader queue so a noisy CLI cannot grow Verb without limit when rendering
        // or browser delivery is slower than its PTY output. A full queue backpressures the PTY.
        let (sender, output) = mpsc::sync_channel(2048);
        let sink_clone = sink.clone();
        thread::spawn(move || {
            let mut buffer = [0_u8; 16 * 1024];
            while let Ok(count) = reader.read(&mut buffer) {
                if count == 0 {
                    break;
                }
                let chunk = &buffer[..count];
                if let Some(s) = &sink_clone {
                    s.wait_for_drain();
                    s.broadcast_output(chunk);
                }
                if sender.send(chunk.to_vec()).is_err() {
                    break;
                }
            }
        });

        let mobile = LiveBridge::new(session.id.clone());
        // An unavailable optional bridge never prevents the agent from starting. `verb mobile
        // offer ID` will report the missing endpoint if this host could not create it.
        let mobile_server = LocalServer::bind(&session.id, mobile.clone(), process.pid).ok();
        // Each process start is one execution attempt, numbered from the session's own event log.
        let conversation = crate::conversation::Engine::for_agent(
            session.agent.as_ref().map(|agent| agent.label()),
            project,
            &session.id,
            crate::process_starts(&session),
            session.resume_identity.as_deref(),
            process.pid,
            crate::conversation::Sources::for_project(project),
        );
        Ok(Self {
            session,
            logger,
            integration: ShellIntegration::new(),
            master: process.master,
            pid: process.pid,
            _session_lock: session_lock,
            parser: vt100::Parser::new(rows, cols, SCROLLBACK_LINES),
            mobile,
            _mobile_server: mobile_server,
            output,
            web_output: None,
            sink,
            reader_finished: false,
            exit_code: None,
            pending: Vec::new(),
            watch,
            conversation,
            closed: false,
        })
    }

    /// Drains whatever the session has produced and returns its exit code once it has finished.
    ///
    /// The PTY child's process ID: for an external terminal, the boundary client itself.
    pub fn pid(&self) -> i32 {
        self.pid
    }

    /// The same bytes go two places and nowhere else: into the terminal state Verb draws, and past
    /// the shell-integration scanner that turns markers into structural events. Neither retains
    /// them.
    pub fn poll(&mut self) -> Result<(Option<i32>, bool), String> {
        self.mobile.expire_idle(std::time::Instant::now())?;
        self.mobile.deliver_phone_input(&mut self.master)?;
        self.poll_conversation()?;
        if self.mobile.needs_initial_screen()? {
            self.mobile
                .publish_screen(self.parser.screen().contents().into_bytes())?;
        }
        let mut changed = false;
        // A continuous producer must not keep this poll in the drain loop forever. The next UI
        // tick resumes where this one stopped, while the bounded queue applies backpressure.
        for _ in 0..512 {
            match self.output.try_recv() {
                Ok(bytes) => {
                    changed = true;
                    self.parser.process(&bytes);
                    if let Some(web_output) = &mut self.web_output {
                        web_output.push(bytes.clone());
                    }
                    if self.mobile.wants_screen()? {
                        self.mobile
                            .publish_screen(self.parser.screen().contents().into_bytes())?;
                    }
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

        changed |= self.poll_agent_record()?;

        if self.exit_code.is_none() {
            let previous = self.exit_code;
            self.exit_code = pty::reap(self.pid)?;
            changed |= self.exit_code != previous;
        }
        Ok((self.exit_code, changed))
    }

    /// Advances the conversation, types any phone chat messages that pass its checks, and
    /// republishes the phone's view when something changed.
    fn poll_conversation(&mut self) -> Result<(), String> {
        for request in self.mobile.take_chat()? {
            // The phone was told only "queued". A refusal writes nothing, so it is recorded as a
            // message that was not sent, with its reason, for the phone to show.
            if let Err(refusal) = self.chat_send_with_lease(
                &request.id,
                &request.text,
                crate::conversation::Origin::Phone,
                Some(request.generation),
            ) {
                if let Some(engine) = &mut self.conversation {
                    engine.record_refusal(
                        &request.id,
                        crate::conversation::Origin::Phone,
                        &refusal,
                    );
                }
            }
        }
        let running = self.exit_code.is_none();
        let watched = self.watch.conversation_id();
        let Some(engine) = &mut self.conversation else {
            return Ok(());
        };
        let screen = self.parser.screen().contents();
        let changed = engine.poll(&screen, watched.as_deref(), running);
        if changed || self.mobile.needs_initial_screen()? {
            self.publish_conversation()?;
        }
        Ok(())
    }

    fn publish_conversation(&self) -> Result<(), String> {
        let Some(engine) = &self.conversation else {
            return Ok(());
        };
        if !self.mobile.wants_screen()? {
            return Ok(());
        }
        let mut limit = 60;
        let (view, conversation) = loop {
            let view = engine.view(limit);
            let value = serde_json::to_value(&view).map_err(|e| e.to_string())?;
            if limit == 0 || value.to_string().len() <= crate::mobile::MAX_PHONE_CONVERSATION {
                break (view, value);
            }
            limit /= 2;
        };
        use crate::conversation::Readiness;
        let agent_state = match view.readiness.state {
            Readiness::Ready => "waiting",
            Readiness::Working => "working",
            Readiness::Approval => "awaiting_approval",
            _ => "uncertain",
        };
        let messages = view
            .items
            .iter()
            .filter(|i| matches!(i.item.kind, "you" | "agent"))
            .map(|i| crate::mobile::ChatMessageDto {
                id: i
                    .item
                    .id
                    .clone()
                    .unwrap_or_else(|| format!("seq-{}", i.seq)),
                sender: if i.item.kind == "you" {
                    "user"
                } else {
                    "agent"
                }
                .to_owned(),
                text: i.item.text.clone().unwrap_or_default(),
                // 0 means "no usable time": never the time Verb read it.
                timestamp: i
                    .item
                    .at
                    .as_deref()
                    .and_then(crate::context::parse_iso8601_utc)
                    .and_then(|ms| u64::try_from(ms).ok())
                    .unwrap_or(0),
                at: i.item.at.clone(),
            })
            .collect();
        self.mobile.update_chat(crate::mobile::ChatUpdate {
            agent: view.agent.unwrap_or("agent").to_owned(),
            agent_state: agent_state.to_owned(),
            can_send_prompt: view.can_send,
            messages,
            conversation,
            stream_generation: view.stream.generation,
        })
    }

    /// Sends a chat message: a separate channel from raw keystrokes, gated by readiness, written
    /// under the input lease of the side it came from, and recorded once per message id.
    pub fn chat_send(
        &mut self,
        id: &str,
        text: &str,
        origin: crate::conversation::Origin,
    ) -> Result<(), crate::conversation::Refusal> {
        self.chat_send_with_lease(id, text, origin, None)
    }

    fn chat_send_with_lease(
        &mut self,
        id: &str,
        text: &str,
        origin: crate::conversation::Origin,
        lease: Option<u64>,
    ) -> Result<(), crate::conversation::Refusal> {
        let generation = lease.or_else(|| self.mobile.chat_generation().ok());
        let running = self.exit_code.is_none();
        let watched = self.watch.conversation_id();
        let screen = self.parser.screen().contents();
        let bracketed = self.parser.screen().bracketed_paste();
        let Some(engine) = &mut self.conversation else {
            return Err(crate::conversation::Refusal {
                code: "unsupported",
                reason: "This terminal has no agent conversation. Use the terminal.".to_owned(),
            });
        };
        let Some(bytes) = engine.prepare_send(
            id,
            text,
            origin,
            crate::conversation::SendContext {
                screen: &screen,
                conversation: watched.as_deref(),
                running,
                bracketed_paste: bracketed,
            },
        )?
        else {
            return Ok(());
        };
        let written = match origin {
            crate::conversation::Origin::Desktop => {
                self.mobile.write_desktop_chat(&mut self.master, &bytes)
            }
            crate::conversation::Origin::Phone => self.mobile.write_phone_chat(
                &mut self.master,
                &bytes,
                generation.unwrap_or(u64::MAX),
            ),
        };
        engine.finish_send(id, written);
        let _ = self.publish_conversation();
        Ok(())
    }

    pub fn chat_check(&mut self, id: &str) -> Result<(), String> {
        let running = self.exit_code.is_none();
        let watched = self.watch.conversation_id();
        let screen = self.parser.screen().contents();
        let engine = self
            .conversation
            .as_mut()
            .ok_or("This terminal has no agent conversation.")?;
        engine.check(id, &screen, watched.as_deref(), running)?;
        let _ = self.publish_conversation();
        Ok(())
    }

    pub fn chat_discard(&mut self, id: &str) -> Result<(), String> {
        let engine = self
            .conversation
            .as_mut()
            .ok_or("This terminal has no agent conversation.")?;
        engine.discard(id)?;
        let _ = self.publish_conversation();
        Ok(())
    }

    /// Applies a stream setting the moment it changes, rather than at the next settings check.
    pub fn apply_stream(&mut self, settings: crate::transcript::Settings) {
        if let Some(engine) = &mut self.conversation {
            engine.apply_settings(settings);
        }
        let _ = self.publish_conversation();
    }

    /// The conversation view for the browser, with what the paired phone has acknowledged.
    pub fn conversation_view(&self, max_items: usize) -> Option<serde_json::Value> {
        let engine = self.conversation.as_ref()?;
        let mut value = serde_json::to_value(engine.view(max_items)).ok()?;
        if let Ok(ack) = self.mobile.phone_ack() {
            value["phone"] = serde_json::json!({
                "paired": ack.paired,
                "connected": ack.connected,
                "ackedGeneration": ack.applied.map(|(generation, _)| generation),
                "ackedAt": ack.applied.map(|(_, at)| at),
            });
        }
        Some(value)
    }

    /// A process can exit before its reader thread has forwarded the final PTY bytes. The web
    /// host waits for this flag before recording the terminal's final screen.
    pub fn output_drained(&self) -> bool {
        self.reader_finished
    }

    /// Reads whatever the agent has appended to its own record since the last tick.
    ///
    /// A failure to read is not reported as "nothing happened": the tail simply produces no events,
    /// and [`Hosted::observed`] stays empty, which the overlay renders as unobserved.
    fn poll_agent_record(&mut self) -> Result<bool, String> {
        let now = crate::now_millis();
        let events = self.watch.poll(now);
        let mut changed = !events.is_empty();
        for event in events {
            self.logger.agent_observed(&event)?;
            if let AgentEvent::ToolOutcome {
                at, failed: true, ..
            } = event
            {
                // The band's whole purpose is the moment something went wrong, and until now it
                // could not speak about anything that happened inside an agent.
                self.pending.push(Structural::AgentToolFailed {
                    millis: now.saturating_sub(at),
                    tool: self.watch.last_tool(),
                });
            }
        }
        if self.session.resume_identity.is_none() {
            // Known only once a line has been read, so this follows the poll rather than preceding
            // it.
            self.session.resume_identity = self.watch.conversation_id();
            if self.session.resume_identity.is_some() {
                crate::save_session(&self.session)?;
                changed = true;
            }
        }
        Ok(changed)
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
        self.mobile.write_desktop(&mut self.master, bytes)
    }

    /// The browser host forwards the exact PTY stream to xterm.js. The TUI does not enable this
    /// second, volatile copy, so ordinary terminal sessions retain their existing memory bound.
    pub fn capture_web_output(&mut self) {
        self.web_output = Some(Vec::new());
    }

    pub fn take_web_output(&mut self) -> Vec<Vec<u8>> {
        self.web_output
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }

    /// Returns input control to the desktop when a paired phone holds it.
    pub fn take_input_back(&self) -> Result<(), String> {
        self.mobile.desktop_take_control()
    }

    pub fn phone_control_status(&self) -> Result<(crate::mobile::Controller, bool), String> {
        self.mobile.control_status()
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.parser.screen_mut().set_size(rows, cols);
        pty::set_window_size(&self.master, rows, cols);
        if self.mobile.wants_screen().unwrap_or(false) {
            let _ = self
                .mobile
                .publish_screen(self.parser.screen().contents().into_bytes());
        }
    }

    pub fn signal(&self, signal: i32) -> Result<(), String> {
        pty::signal_group(self.pid, signal)
    }

    /// Closes the record out exactly as the CLI does: process ended, agent ended, state resolved
    /// from the agent's own evidence, session saved.
    pub fn finish(mut self, exit_code: i32) -> Result<Session, String> {
        if let Some(sink) = &self.sink {
            sink.broadcast_control(serde_json::json!({
                "type": "exit",
                "id": self.session.id,
                "code": exit_code,
            }));
        }
        let _ = self.mobile.end();
        crate::finish_session_quietly(&mut self.session, exit_code)?;
        self.closed = true;
        Ok(self.session.clone())
    }

    /// Stops a still-running hosted program before closing its durable record.
    pub fn stop(mut self) -> Result<Session, String> {
        let _ = self.mobile.end();
        let exit_code = match self.exit_code {
            Some(code) => code,
            None => pty::terminate(self.pid)?,
        };
        if let Some(sink) = &self.sink {
            sink.broadcast_control(serde_json::json!({
                "type": "exit",
                "id": self.session.id,
                "code": exit_code,
            }));
        }
        crate::finish_session_quietly(&mut self.session, exit_code)?;
        self.closed = true;
        Ok(self.session.clone())
    }
}

impl Drop for Hosted {
    fn drop(&mut self) {
        if self.closed {
            return;
        }
        // Error unwinding must not strand either a process or a durable LIVE record. There is no
        // useful error channel from Drop, so the explicit quit/switch paths still call `stop` and
        // report failures; this is the last-resort integrity guard.
        let exit_code = self
            .exit_code
            .or_else(|| pty::terminate(self.pid).ok())
            .unwrap_or(1);
        let _ = self.mobile.end();
        let _ = crate::finish_session_quietly(&mut self.session, exit_code);
        self.closed = true;
    }
}

const SCROLLBACK_LINES: usize = 1_000;
