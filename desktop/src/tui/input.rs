//! Keyboard and mouse input for the TUI: the workbench keys, Verb-owned surfaces (palette,
//! launchers, pickers), and every click. Split out of `mod.rs` unchanged; the state these handlers
//! act on still lives on `App` there.

use super::*;

impl App {
    pub(super) fn on_workbench_key(&mut self, key: KeyEvent) -> Result<(), String> {
        if self.work.new_task.is_none()
            && self.work.memory_editor.is_none()
            && self.work.composer.is_none()
            && self.work.actor_picker.is_none()
            && self.work.search.is_none()
        {
            if let KeyCode::Char(character) = key.code {
                let chord = self.leader.chord();
                if key.modifiers.contains(KeyModifiers::CONTROL) == chord.ctrl
                    && (character.to_ascii_lowercase() == chord.key
                        || (chord.key == '@' && character == ' '))
                {
                    return self.run_command(Command::Palette);
                }
            }
        }
        // Normalize Return after checking the configurable leader, so Ctrl+J can still be used
        // as a leader when explicitly configured.
        let key = if key.code == KeyCode::Char('j') && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
        } else {
            key
        };
        if let Some(action) = self.work.actor_picker.as_ref().map(|picker| picker.action) {
            let count = self.actor_candidates(action).len();
            match key.code {
                KeyCode::Esc => self.work.actor_picker = None,
                KeyCode::Up | KeyCode::Char('k') => {
                    if let Some(picker) = &mut self.work.actor_picker {
                        picker.selected = picker.selected.saturating_sub(1);
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if let Some(picker) = &mut self.work.actor_picker {
                        picker.selected = (picker.selected + 1).min(count.saturating_sub(1));
                    }
                }
                KeyCode::Enter => {
                    let selected = self
                        .work
                        .actor_picker
                        .as_ref()
                        .map_or(0, |picker| picker.selected);
                    self.work.actor_picker = None;
                    if let Some(index) = self.actor_candidates(action).get(selected).copied() {
                        let id = self.sessions[index].id.clone();
                        if let Some(position) = self
                            .project_session_indices()
                            .iter()
                            .position(|candidate| *candidate == index)
                        {
                            self.work.session = position;
                        }
                        self.start_task_action(action, id)?;
                    } else {
                        self.message = Some(
                            "That session is no longer available. Choose another agent.".to_owned(),
                        );
                    }
                }
                _ => {}
            }
            return Ok(());
        }
        if let Some(draft) = &mut self.work.new_task {
            match key.code {
                KeyCode::Esc => self.work.new_task = None,
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    let draft = self.work.new_task.take().expect("draft exists");
                    match crate::workbench::create_ui_task(
                        &self.project,
                        &draft.title,
                        &draft.brief,
                    ) {
                        Ok(id) => {
                            self.message = Some(format!("Created task: {}", draft.title.trim()));
                            self.refresh_workbench()?;
                            self.open_task_by_id(&id);
                        }
                        Err(error) => {
                            self.message = Some(error);
                            self.work.new_task = Some(draft);
                        }
                    }
                }
                KeyCode::Tab => draft.editing_brief = !draft.editing_brief,
                KeyCode::Enter if !draft.editing_brief => draft.editing_brief = true,
                KeyCode::Enter => draft.brief.push('\n'),
                KeyCode::Backspace if draft.editing_brief => {
                    draft.brief.pop();
                }
                KeyCode::Backspace => {
                    draft.title.pop();
                }
                KeyCode::Char(character) if draft.editing_brief => draft.brief.push(character),
                KeyCode::Char(character) => draft.title.push(character),
                _ => {}
            }
            return Ok(());
        }
        if let Some(editor) = &mut self.work.memory_editor {
            match key.code {
                KeyCode::Esc => self.work.memory_editor = None,
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    let note = self
                        .work
                        .memory_editor
                        .take()
                        .expect("memory editor exists");
                    match crate::workbench::append_ui_memory(&self.project, &note) {
                        Ok(message) => {
                            self.message = Some(message);
                            self.refresh_workbench()?;
                        }
                        Err(error) => {
                            self.message = Some(error);
                            self.work.memory_editor = Some(note);
                        }
                    }
                }
                KeyCode::Enter => editor.push('\n'),
                KeyCode::Backspace => {
                    editor.pop();
                }
                KeyCode::Char(character) => editor.push(character),
                _ => {}
            }
            return Ok(());
        }
        if let Some(composer) = &mut self.work.composer {
            match key.code {
                KeyCode::Esc => self.work.composer = None,
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    let composer = self.work.composer.take().expect("composer exists");
                    match crate::workbench::apply_ui_action(
                        &self.project,
                        &composer.task_id,
                        &composer.actor,
                        composer.action,
                        Some(composer.text.clone()),
                        composer.expected_revision.as_deref(),
                    ) {
                        Ok(message) => {
                            self.message = Some(message);
                            self.refresh_workbench()?;
                        }
                        Err(error) => {
                            self.message = Some(error);
                            self.work.composer = Some(composer);
                        }
                    }
                }
                KeyCode::Enter => composer.text.push('\n'),
                KeyCode::Backspace => {
                    composer.text.pop();
                }
                KeyCode::Char(character) => composer.text.push(character),
                _ => {}
            }
            return Ok(());
        }
        if let Some(search) = &mut self.work.search {
            match key.code {
                KeyCode::Esc => {
                    self.work.search = None;
                    self.work.filter.clear();
                }
                KeyCode::Enter => self.work.search = None,
                KeyCode::Backspace => {
                    search.pop();
                    self.work.filter = search.clone();
                    self.work.task = 0;
                }
                KeyCode::Char(character) => {
                    search.push(character);
                    self.work.filter = search.clone();
                    self.work.task = 0;
                }
                _ => {}
            }
            return Ok(());
        }
        if self.mode == Mode::Workbench && self.work.inbox.is_some() && !self.work.detail {
            let count = self
                .work
                .inbox
                .as_ref()
                .map_or(0, |inbox| inbox.items.len());
            match key.code {
                KeyCode::Esc | KeyCode::Char('b') => self.work.inbox = None,
                KeyCode::Up | KeyCode::Char('k') => {
                    self.work.inbox_index = self.work.inbox_index.saturating_sub(1)
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.work.inbox_index = (self.work.inbox_index + 1).min(count.saturating_sub(1))
                }
                KeyCode::Enter => self.open_inbox_task(self.work.inbox_index),
                KeyCode::Char('r') => self.refresh_workbench()?,
                KeyCode::Char('s') => {
                    if let Some(index) = self
                        .project_session_indices()
                        .get(self.work.session)
                        .copied()
                    {
                        self.resume_selected(index)?;
                    }
                }
                KeyCode::Char('1') => self.enter_terminal()?,
                KeyCode::Char('3') => {
                    self.mode = Mode::Activity;
                    self.work.inbox = None;
                }
                KeyCode::Char('4') => {
                    self.mode = Mode::Memory;
                    self.work.inbox = None;
                }
                _ => {}
            }
            return Ok(());
        }
        match key.code {
            KeyCode::Char('1') => self.enter_terminal()?,
            KeyCode::Char('2') => self.enter_workbench()?,
            KeyCode::Char('3') => {
                self.mode = Mode::Activity;
                self.work.detail = false;
                self.work.scroll = 0;
            }
            KeyCode::Char('4') => {
                self.mode = Mode::Memory;
                self.work.detail = false;
                self.work.scroll = 0;
            }
            KeyCode::Char('?') => self.run_command(Command::Help)?,
            KeyCode::Char('n') if self.mode == Mode::Workbench && !self.work.detail => {
                self.surface = Surface::Palette {
                    filter: "New".to_owned(),
                    selected: 0,
                }
            }
            KeyCode::Char('N') if self.mode == Mode::Workbench && !self.work.detail => {
                self.surface = Surface::Palette {
                    filter: "isolated".to_owned(),
                    selected: 0,
                }
            }
            KeyCode::Char('t') if self.mode == Mode::Workbench && !self.work.detail => {
                self.work.new_task = Some(NewTask::default())
            }
            KeyCode::Char('m') if self.mode == Mode::Workbench && !self.work.detail => {
                self.mode = Mode::Memory;
                self.work.scroll = 0;
            }
            KeyCode::Char('g') if self.mode == Mode::Workbench && !self.work.detail => {
                self.surface = Surface::Changes;
            }
            KeyCode::Char('x') if self.mode == Mode::Workbench && !self.work.detail => {
                self.work.filter.clear();
                self.work.task = 0;
            }
            KeyCode::Char('r') if self.mode == Mode::Workbench && !self.work.detail => {
                self.refresh_workbench()?;
                self.message = Some("Workbench refreshed from local records.".to_owned());
            }
            KeyCode::Char('i')
                if self.mode == Mode::Workbench
                    && !self.work.detail
                    && self.work.focus == WorkFocus::Sessions =>
            {
                self.open_session_inbox()?
            }
            KeyCode::Char('a') if self.mode == Mode::Memory => {
                self.work.memory_editor = Some(String::new())
            }
            KeyCode::Char('/') if self.mode == Mode::Workbench && !self.work.detail => {
                self.work.search = Some(self.work.filter.clone());
                self.work.focus = WorkFocus::Tasks;
            }
            KeyCode::Esc if self.mode == Mode::Activity || self.mode == Mode::Memory => {
                self.enter_workbench()?
            }
            KeyCode::Esc if self.work.detail => {
                self.work.detail = false;
                self.work.scroll = 0;
            }
            KeyCode::Esc if self.mode == Mode::Workbench && !self.work.filter.is_empty() => {
                self.work.filter.clear();
                self.work.task = 0;
            }
            KeyCode::Tab if !self.work.detail => {
                self.work.focus = if self.work.focus == WorkFocus::Tasks {
                    WorkFocus::Sessions
                } else {
                    WorkFocus::Tasks
                }
            }
            KeyCode::Up | KeyCode::Char('k')
                if self.mode == Mode::Activity || self.mode == Mode::Memory || self.work.detail =>
            {
                self.work.scroll = self.work.scroll.saturating_sub(1)
            }
            KeyCode::Down | KeyCode::Char('j')
                if self.mode == Mode::Activity || self.mode == Mode::Memory || self.work.detail =>
            {
                self.work.scroll = self.work.scroll.saturating_add(1)
            }
            KeyCode::Up | KeyCode::Char('k') => match self.work.focus {
                WorkFocus::Sessions => self.work.session = self.work.session.saturating_sub(1),
                WorkFocus::Tasks => self.work.task = self.work.task.saturating_sub(1),
            },
            KeyCode::Down | KeyCode::Char('j') => match self.work.focus {
                WorkFocus::Sessions => {
                    self.work.session = (self.work.session + 1)
                        .min(self.project_session_indices().len().saturating_sub(1))
                }
                WorkFocus::Tasks => {
                    self.work.task = (self.work.task + 1)
                        .min(self.visible_task_indices().len().saturating_sub(1))
                }
            },
            KeyCode::Enter
                if self.mode == Mode::Workbench
                    && !self.work.detail
                    && self.work.focus == WorkFocus::Sessions =>
            {
                if let Some(index) = self
                    .project_session_indices()
                    .get(self.work.session)
                    .copied()
                {
                    self.open_workbench_session(index)?;
                }
            }
            KeyCode::Enter
                if self.mode == Mode::Workbench
                    && !self.work.detail
                    && self.selected_task().is_some() =>
            {
                self.work.detail = true;
                self.work.scroll = 0;
            }
            KeyCode::Char('c') if self.mode == Mode::Workbench && self.work.detail => {
                self.task_action(crate::workbench::TaskAction::Claim)?
            }
            KeyCode::Char('a') if self.mode == Mode::Workbench && self.work.detail => {
                self.task_action(crate::workbench::TaskAction::RequestHelp)?
            }
            KeyCode::Char('p') if self.mode == Mode::Workbench && self.work.detail => {
                self.task_action(crate::workbench::TaskAction::Reply)?
            }
            KeyCode::Char('h') if self.mode == Mode::Workbench && self.work.detail => {
                self.task_action(crate::workbench::TaskAction::Handoff)?
            }
            KeyCode::Char('r') if self.mode == Mode::Workbench && self.work.detail => {
                self.task_action(crate::workbench::TaskAction::Reassign)?
            }
            KeyCode::Char('d') if self.mode == Mode::Workbench && self.work.detail => {
                self.task_action(crate::workbench::TaskAction::Done)?
            }
            KeyCode::Char('q') if !self.work.detail => self.run_action(Action::Quit)?,
            _ => {}
        }
        Ok(())
    }

    /// Which Verb command a function key means right now, if any.
    pub(super) fn accelerator(&self, key: KeyEvent) -> Option<Command> {
        if !self.accelerators_active() {
            return None;
        }
        match key.code {
            KeyCode::F(1) => Some(Command::Help),
            KeyCode::F(2) => Some(Command::Workbench),
            KeyCode::F(3) => Some(Command::Contextual),
            KeyCode::F(4) => Some(Command::Palette),
            _ => None,
        }
    }

    /// Accelerators are live at a shell prompt, and off while a full-screen application owns the
    /// screen or the user has turned them off entirely.
    pub(crate) fn accelerators_active(&self) -> bool {
        if std::env::var("VERB_FKEYS").is_ok_and(|value| value == "off") {
            return false;
        }
        !self
            .hosted
            .as_ref()
            .is_some_and(|hosted| hosted.full_screen_app())
    }

    pub(super) fn on_surface_key(&mut self, key: KeyEvent) -> Result<(), String> {
        // A search prompt owns every key until it closes, or typing "n" would jump instead of
        // typing an n.
        if let Surface::Scrollback {
            search: Some(term),
            last_search,
            ..
        } = &mut self.surface
        {
            match key.code {
                KeyCode::Esc => {
                    if let Surface::Scrollback { search, .. } = &mut self.surface {
                        *search = None;
                    }
                }
                KeyCode::Enter => {
                    let term = term.clone();
                    *last_search = Some(term.clone());
                    if let Surface::Scrollback { search, .. } = &mut self.surface {
                        *search = None;
                    }
                    self.search(&term, 1)?;
                }
                KeyCode::Backspace => {
                    term.pop();
                }
                KeyCode::Char(character) => term.push(character),
                _ => {}
            }
            return Ok(());
        }

        match (&mut self.surface, key.code) {
            (Surface::Scrollback { .. }, KeyCode::Esc) => {
                // Back to the live end of the session, or the next output would arrive somewhere
                // the user cannot see.
                self.scroll_to_end()?;
                self.surface = Surface::None;
            }
            (_, KeyCode::Esc) => self.surface = Surface::None,
            (Surface::Help, _) | (Surface::Evidence, _) | (Surface::Changes, _) => {
                self.surface = Surface::None;
            }

            (Surface::Scrollback { .. }, KeyCode::Up | KeyCode::Char('k')) => self.scroll_by(1)?,
            (Surface::Scrollback { .. }, KeyCode::Down | KeyCode::Char('j')) => {
                self.scroll_by(-1)?
            }
            (Surface::Scrollback { .. }, KeyCode::PageUp) => self.scroll_by(10)?,
            (Surface::Scrollback { .. }, KeyCode::PageDown) => self.scroll_by(-10)?,
            (Surface::Scrollback { .. }, KeyCode::Char('g')) => self.scroll_to_end()?,
            (Surface::Scrollback { search, .. }, KeyCode::Char('/')) => {
                *search = Some(String::new());
            }
            (Surface::Scrollback { last_search, .. }, KeyCode::Char('n')) => {
                if let Some(term) = last_search.clone() {
                    self.search(&term, 1)?;
                }
            }
            (Surface::Scrollback { last_search, .. }, KeyCode::Char('N')) => {
                if let Some(term) = last_search.clone() {
                    self.search(&term, -1)?;
                }
            }
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
            (Surface::Sessions { selected }, KeyCode::Char('x')) => {
                let index = *selected;
                self.forget_selected(index)?;
            }
            (Surface::ExternalAgent { command, .. }, KeyCode::Char(character))
                if !character.is_whitespace() && !character.is_control() && command.len() < 256 =>
            {
                command.push(character);
            }
            (Surface::ExternalAgent { command, .. }, KeyCode::Backspace) => {
                command.pop();
            }
            (Surface::ExternalAgent { command, isolated }, KeyCode::Enter) => {
                let command = command.clone();
                let isolated = *isolated;
                if command.is_empty() {
                    self.message =
                        Some("Enter an executable name, such as agy or hermes.".to_owned());
                } else {
                    self.surface = Surface::None;
                    let (rows, cols) = self.last_size();
                    if isolated {
                        let workspace = crate::project::create_isolated_checkout(&self.project)?;
                        self.start(
                            crate::begin_external_session(&workspace, command, Vec::new())?,
                            rows,
                            cols,
                        )
                        .map_err(|error| {
                            format!(
                                "{error}; isolated workspace kept at {}",
                                workspace.display()
                            )
                        })?;
                    } else {
                        self.start(
                            crate::begin_external_session(&self.project, command, Vec::new())?,
                            rows,
                            cols,
                        )?;
                    }
                }
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

    pub(super) fn sync_mouse_capture(&mut self) -> Result<(), String> {
        let wanted = self.mouse_enabled;
        if wanted == self.mouse_captured {
            return Ok(());
        }
        let mut stdout = io::stdout();
        let result = if wanted {
            ratatui::crossterm::execute!(stdout, event::EnableMouseCapture)
        } else {
            ratatui::crossterm::execute!(stdout, event::DisableMouseCapture)
        };
        result.map_err(|error| format!("could not change mouse handling: {error}"))?;
        self.mouse_captured = wanted;
        Ok(())
    }

    /// Verb holds the mouse whenever [`App::mouse_enabled`] is set, which is what lets the action
    /// bar be clicked rather than only read. The cost is that a plain drag no longer selects text;
    /// Option-drag still does in the terminals that support it, and `leader m` hands the mouse back
    /// in the ones that do not.
    pub(super) fn on_mouse(&mut self, mouse: MouseEvent) -> Result<(), String> {
        if self.mode != Mode::Terminal && self.surface == Surface::None {
            return self.on_workbench_mouse(mouse);
        }
        if matches!(self.surface, Surface::None) {
            return self.on_terminal_mouse(mouse);
        }

        if matches!(self.surface, Surface::ExternalAgent { .. }) {
            if mouse.kind == MouseEventKind::Down(MouseButton::Left) {
                let rect = render::external_agent_rect(ratatui::layout::Rect::new(
                    0,
                    0,
                    self.frame_width,
                    self.frame_height,
                ));
                let inside = mouse.column >= rect.x
                    && mouse.column < rect.x.saturating_add(rect.width)
                    && mouse.row >= rect.y
                    && mouse.row < rect.y.saturating_add(rect.height);
                if !inside {
                    self.surface = Surface::None;
                } else if mouse.row == rect.y + 4
                    && (rect.x + 2..rect.x + 9).contains(&mouse.column)
                {
                    self.on_surface_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
                }
            }
            return Ok(());
        }

        match (&mut self.surface, mouse.kind) {
            (Surface::Scrollback { .. }, MouseEventKind::ScrollUp) => self.scroll_by(3)?,
            (Surface::Scrollback { .. }, MouseEventKind::ScrollDown) => self.scroll_by(-3)?,
            (Surface::Sessions { selected }, MouseEventKind::ScrollUp) => {
                *selected = selected.saturating_sub(1);
            }
            (Surface::Sessions { selected }, MouseEventKind::ScrollDown) => {
                *selected = (*selected + 1).min(self.sessions.len().saturating_sub(1));
            }
            (Surface::Palette { selected, filter }, MouseEventKind::ScrollUp) => {
                let _ = filter;
                *selected = selected.saturating_sub(1);
            }
            (Surface::Palette { selected, filter }, MouseEventKind::ScrollDown) => {
                let count = render::palette_entries(filter).len();
                *selected = (*selected + 1).min(count.saturating_sub(1));
            }
            // Palette and session rows are actions, so a single click activates the row.
            (_, MouseEventKind::Down(MouseButton::Left)) => {
                let choice = render::overlay_row_at(
                    &self.surface,
                    self.sessions.len(),
                    self.imported_sessions.len(),
                    ratatui::layout::Rect::new(0, 0, self.frame_width, self.frame_height),
                    mouse.column,
                    mouse.row,
                );
                match choice {
                    Some(index) => {
                        match &mut self.surface {
                            Surface::Sessions { selected } => *selected = index,
                            Surface::Palette { selected, .. } => *selected = index,
                            _ => {}
                        }
                        self.on_surface_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
                    }
                    None => self.surface = Surface::None,
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn on_terminal_mouse(&mut self, mouse: MouseEvent) -> Result<(), String> {
        if matches!(mouse.kind, MouseEventKind::Down(_)) && mouse.row == self.bar_row() {
            if let Some(command) = render::bar_command_at(self, mouse.column) {
                return self.run_command(command);
            }
            return Ok(());
        }
        let pane = self
            .pane_areas
            .iter()
            .find(|(_, content)| {
                mouse.column >= content.x.saturating_sub(1)
                    && mouse.column <= content.x.saturating_add(content.width)
                    && mouse.row >= content.y.saturating_sub(1)
                    && mouse.row <= content.y.saturating_add(content.height)
            })
            .cloned();
        let Some((id, content)) = pane else {
            return Ok(());
        };
        if matches!(
            mouse.kind,
            MouseEventKind::Down(_) | MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
        ) {
            self.focus_pane(&id)?;
        } else if self
            .hosted
            .as_ref()
            .is_none_or(|hosted| hosted.session.id != id)
        {
            return Ok(());
        }
        let Some(hosted) = self.hosted.as_mut() else {
            return Ok(());
        };
        if let Some(bytes) = mouse::encode(hosted.screen(), mouse, content) {
            hosted.write(&bytes)?;
        } else {
            match mouse.kind {
                MouseEventKind::ScrollUp => {
                    let offset = hosted.screen().scrollback();
                    hosted.scroll_to(offset.saturating_add(3));
                }
                MouseEventKind::ScrollDown => {
                    let offset = hosted.screen().scrollback();
                    hosted.scroll_to(offset.saturating_sub(3));
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub(super) fn on_workbench_mouse(&mut self, mouse: MouseEvent) -> Result<(), String> {
        if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            if let Some(action) = workbench_view::modal_action_at(self, mouse.column, mouse.row) {
                return self.on_workbench_click(action);
            }
            if let Some(action) = workbench_view::footer_action_at(self, mouse.column, mouse.row) {
                return self.on_workbench_click(action);
            }
        }
        if let Some(picker) = &self.work.actor_picker {
            let candidates = self.actor_candidates(picker.action);
            match mouse.kind {
                MouseEventKind::ScrollUp => {
                    return self.on_workbench_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE))
                }
                MouseEventKind::ScrollDown => {
                    return self.on_workbench_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
                }
                MouseEventKind::Down(_) => {
                    let panel = workbench_view::actor_picker_rect(
                        ratatui::layout::Rect::new(0, 0, self.frame_width, self.frame_height),
                        candidates.len(),
                    );
                    let first = panel.y + 3;
                    let capacity = panel.height.saturating_sub(5) as usize;
                    let start = picker.selected.saturating_add(1).saturating_sub(capacity);
                    if mouse.column >= panel.x
                        && mouse.column < panel.x + panel.width
                        && mouse.row >= first
                        && mouse.row < first + capacity as u16
                    {
                        let index = start + (mouse.row - first) as usize;
                        if index < candidates.len() {
                            if let Some(picker) = &mut self.work.actor_picker {
                                picker.selected = index;
                            }
                            return self.on_workbench_key(KeyEvent::new(
                                KeyCode::Enter,
                                KeyModifiers::NONE,
                            ));
                        }
                    } else if mouse.column < panel.x
                        || mouse.column >= panel.x + panel.width
                        || mouse.row < panel.y
                        || mouse.row >= panel.y + panel.height
                    {
                        self.work.actor_picker = None;
                    }
                    return Ok(());
                }
                _ => return Ok(()),
            }
        }
        if let Some(draft) = &mut self.work.new_task {
            if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                let area = ratatui::layout::Rect::new(0, 0, self.frame_width, self.frame_height);
                let width = area.width.saturating_sub(8).min(72);
                let height = area.height.saturating_sub(4).min(14);
                let x = (area.width - width) / 2;
                let y = (area.height - height) / 2;
                if mouse.column >= x + 2 && mouse.column < x + width.saturating_sub(2) {
                    if mouse.row == y + 1 || mouse.row == y + 2 {
                        draft.editing_brief = false;
                    } else if mouse.row >= y + 4 && mouse.row < y + height.saturating_sub(1) {
                        draft.editing_brief = true;
                    }
                }
            }
            return Ok(());
        }
        if self.work.memory_editor.is_some() || self.work.composer.is_some() {
            return Ok(());
        }
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                return self.on_workbench_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE))
            }
            MouseEventKind::ScrollDown => {
                return self.on_workbench_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
            }
            MouseEventKind::Down(MouseButton::Left) => {}
            _ => return Ok(()),
        }
        if mouse.row == 1 {
            return match workbench_view::header_action_at(self, mouse.column) {
                Some(workbench_view::HeaderAction::Mode(Mode::Terminal)) => self.enter_terminal(),
                Some(workbench_view::HeaderAction::Mode(Mode::Workbench)) => self.enter_workbench(),
                Some(workbench_view::HeaderAction::Mode(mode)) => {
                    self.mode = mode;
                    self.work.scroll = 0;
                    Ok(())
                }
                Some(workbench_view::HeaderAction::Help) => self.run_command(Command::Help),
                None => Ok(()),
            };
        }
        if self.mode == Mode::Workbench && self.frame_width < 100 && mouse.row == 3 {
            if let Some(focus) = workbench_view::narrow_focus_at(self, mouse.column) {
                self.work.focus = focus;
            }
            return Ok(());
        }
        if let Some(action) = workbench_view::panel_action_at(self, mouse.column, mouse.row) {
            return self.on_workbench_click(action);
        }
        if self.work.inbox.is_some() {
            return Ok(());
        }
        let narrow = self.frame_width < 100;
        let list_first = if narrow { 7 } else { 6 };
        if self.mode != Mode::Workbench
            || self.work.detail
            || self.work.new_task.is_some()
            || mouse.row < list_first
        {
            return Ok(());
        }
        let clicked_focus = if narrow {
            self.work.focus
        } else if mouse.column < 36 {
            WorkFocus::Sessions
        } else {
            WorkFocus::Tasks
        };
        let row_index = ((mouse.row - list_first) / 3) as usize;
        let inner_height = self.frame_height.saturating_sub(if narrow { 8 } else { 7 });
        match clicked_focus {
            WorkFocus::Sessions => {
                let count = self.project_session_indices().len();
                let capacity = (inner_height.saturating_sub(7) / 3).max(1) as usize;
                let start = self.work.session.saturating_add(1).saturating_sub(capacity);
                let index = start + row_index;
                if row_index < capacity && index < count {
                    self.work.focus = WorkFocus::Sessions;
                    self.work.session = index;
                    let session_index = self.project_session_indices()[index];
                    self.open_workbench_session(session_index)?;
                }
            }
            WorkFocus::Tasks => {
                let count = self.visible_task_indices().len();
                let capacity = (inner_height.saturating_sub(10) / 3).max(1) as usize;
                let start = self.work.task.saturating_add(1).saturating_sub(capacity);
                let index = start + row_index;
                if row_index < capacity && index < count {
                    self.work.focus = WorkFocus::Tasks;
                    self.work.task = index;
                    self.work.detail = true;
                    self.work.scroll = 0;
                }
            }
        }
        Ok(())
    }

    pub(super) fn on_workbench_click(
        &mut self,
        action: workbench_view::ClickAction,
    ) -> Result<(), String> {
        use workbench_view::ClickAction;
        match action {
            ClickAction::Back | ClickAction::Cancel => {
                self.on_workbench_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            }
            ClickAction::NewAgent => {
                self.on_workbench_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE))
            }
            ClickAction::IsolatedAgent => {
                self.surface = Surface::Palette {
                    filter: "isolated".to_owned(),
                    selected: 0,
                };
                Ok(())
            }
            ClickAction::NewTask => {
                self.on_workbench_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE))
            }
            ClickAction::Search => {
                self.on_workbench_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE))
            }
            ClickAction::ApplySearch => {
                self.on_workbench_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            }
            ClickAction::ClearSearch => {
                self.work.search = None;
                self.work.filter.clear();
                self.work.task = 0;
                Ok(())
            }
            ClickAction::Memory => {
                self.on_workbench_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE))
            }
            ClickAction::AddMemory => {
                self.on_workbench_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE))
            }
            ClickAction::Changes => {
                self.on_workbench_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE))
            }
            ClickAction::Refresh => {
                self.on_workbench_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE))
            }
            ClickAction::Help => self.run_command(Command::Help),
            ClickAction::Quit => self.run_action(Action::Quit),
            ClickAction::OpenSelected | ClickAction::ChooseAgent => {
                self.on_workbench_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            }
            ClickAction::OpenTask => {
                self.work.focus = WorkFocus::Tasks;
                self.on_workbench_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            }
            ClickAction::Inbox => self.open_session_inbox(),
            ClickAction::InboxTask(index) => {
                self.open_inbox_task(index);
                Ok(())
            }
            ClickAction::OpenSession => {
                if let Some(index) = self
                    .project_session_indices()
                    .get(self.work.session)
                    .copied()
                {
                    self.resume_selected(index)?;
                }
                Ok(())
            }
            ClickAction::Save => {
                self.on_workbench_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL))
            }
            ClickAction::Task(action) => self.task_action(action),
        }
    }
}
