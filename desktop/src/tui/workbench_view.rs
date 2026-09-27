//! The task and session surface. It reads the same durable records as the CLI; it never infers
//! progress from terminal text.

use super::context_view::{EvidenceLines, Kind};
use super::theme::{self, glyph, no_colour};
use super::{App, Mode, WorkFocus};
use crate::workbench::TaskAction;
use crate::SessionState;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

const INBOX_LIST_ROW: u16 = 6;

fn chrome() -> Style {
    if no_colour() {
        Style::default()
    } else {
        Style::default().bg(Color::Rgb(29, 37, 49))
    }
}

fn selected() -> Style {
    if no_colour() {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
            .bg(Color::Rgb(48, 58, 80))
            .add_modifier(Modifier::BOLD)
    }
}

fn attention() -> Style {
    theme::attention()
}

fn state_style(state: &SessionState) -> Style {
    match state {
        SessionState::Live => theme::success(),
        SessionState::Recoverable => theme::attention(),
        SessionState::Interrupted | SessionState::Ended => theme::unconfirmed(),
    }
}

fn task_style(task: &crate::workbench::TaskSnapshot) -> Style {
    if task.needs_help {
        theme::danger()
    } else if task.status == "needs review" {
        attention()
    } else if task.status == "active" {
        theme::success()
    } else {
        theme::unconfirmed()
    }
}

fn task_label(task: &crate::workbench::TaskSnapshot) -> &'static str {
    if task.needs_help {
        "HELP REQUEST"
    } else {
        match task.status {
            "needs review" => "NEEDS REVIEW",
            "active" => "IN PROGRESS",
            "done" => "DONE",
            _ => "OPEN",
        }
    }
}

fn owner_label(app: &App, task: &crate::workbench::TaskSnapshot) -> String {
    let Some(owner) = task.owner.as_deref() else {
        return "Unassigned".to_owned();
    };
    actor_label(app, owner)
}

fn actor_label(app: &App, id: &str) -> String {
    let prefix = &id[..id.len().min(8)];
    match app.sessions.iter().find(|session| session.id == id) {
        Some(session) => format!("{} · {prefix}", session.display_agent()),
        None => format!("session {prefix}"),
    }
}

fn handoff_source(task: &crate::workbench::TaskSnapshot) -> Option<&str> {
    task.events
        .iter()
        .rev()
        .find(|event| event.kind == crate::workbench::EventKind::HandedOff)
        .map(|event| event.session_id.as_str())
}

fn task_context_label(app: &App, task: &crate::workbench::TaskSnapshot) -> String {
    if task.status == "needs review" {
        return match handoff_source(task) {
            Some(source) => format!(
                "Reviewer needed · from session {}",
                actor_label(app, source)
            ),
            None => "Reviewer needed".to_owned(),
        };
    }
    if task.status == "done" {
        return match task.events.last() {
            Some(event) if event.kind == crate::workbench::EventKind::Completed => {
                format!(
                    "Done recorded for session {}",
                    actor_label(app, &event.session_id)
                )
            }
            _ => "Marked done".to_owned(),
        };
    }
    owner_label(app, task)
}

fn next_task_action(task: &crate::workbench::TaskSnapshot) -> &'static str {
    if task.status == "needs review" {
        "Next: choose an agent session to take the review."
    } else if task.status == "open" {
        "Next: choose an agent session to claim this task."
    } else if task.needs_help {
        "Next: choose a different session to reply to the help request."
    } else if task.status == "active" {
        "Next: ask for help, hand off for review, or mark done."
    } else {
        "This task is closed. Its notes remain in the history below."
    }
}

fn session_action(app: &App, session: &crate::Session) -> &'static str {
    if app
        .hosted
        .iter()
        .chain(app.parked.iter())
        .any(|hosted| hosted.session.id == session.id)
    {
        return "Open terminal";
    }
    match session.state {
        SessionState::Live => "View inbox · Running elsewhere",
        SessionState::Recoverable if session.project_id.is_dir() => "Resume exact conversation",
        SessionState::Recoverable => "View inbox · Checkout missing",
        SessionState::Interrupted => "View inbox · Recovery unconfirmed",
        SessionState::Ended => "View inbox · Conversation ended",
    }
}

fn session_card_status(app: &App, session: &crate::Session, width: u16) -> String {
    let action = session_action(app, session);
    let Some(inbox) = app.inboxes.get(&session.id) else {
        return action.to_owned();
    };
    let has_alerts = !inbox.items.is_empty();
    let context_hint = match inbox.context_state {
        "fetched current revision" => None,
        "never fetched" => Some("Context not fetched"),
        _ => Some("New context"),
    };
    if !has_alerts && context_hint.is_none() {
        return action.to_owned();
    }
    if width < 40 {
        let short_action =
            if session.state == SessionState::Recoverable && session.project_id.is_dir() {
                "Resume"
            } else if app.session_can_open(session) {
                "Open terminal"
            } else {
                "View inbox"
            };
        if has_alerts {
            format!("{short_action} · {} alert(s)", inbox.items.len())
        } else {
            format!("{short_action} · {}", context_hint.unwrap_or_default())
        }
    } else if has_alerts {
        format!("{action} · {} alert(s)", inbox.items.len())
    } else {
        format!("{action} · {}", context_hint.unwrap_or_default())
    }
}

fn inset(area: Rect, x: u16, y: u16) -> Rect {
    Rect {
        x: area.x.saturating_add(x),
        y: area.y.saturating_add(y),
        width: area.width.saturating_sub(x * 2),
        height: area.height.saturating_sub(y * 2),
    }
}

fn put(frame: &mut Frame, area: Rect, row: u16, line: Line<'static>) {
    if row >= area.height {
        return;
    }
    frame.render_widget(
        Paragraph::new(line),
        Rect {
            x: area.x,
            y: area.y + row,
            width: area.width,
            height: 1,
        },
    );
}

fn plain(frame: &mut Frame, area: Rect, row: u16, text: impl Into<String>, style: Style) {
    put(
        frame,
        area,
        row,
        Line::from(Span::styled(text.into(), style)),
    );
}

fn short(text: &str, width: u16) -> String {
    super::render::truncate(text, width as usize)
}

pub(super) fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    if area.width < 40 || area.height < 12 {
        frame.render_widget(
            Paragraph::new("Verb needs at least 40×12 for Workbench"),
            area,
        );
        return;
    }
    let regions = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(area);
    header(frame, app, regions[0]);
    match app.mode {
        Mode::Activity => activity(frame, app, regions[1]),
        Mode::Memory => memory(frame, app, regions[1]),
        Mode::Workbench if app.work.detail => detail(frame, app, regions[1]),
        Mode::Workbench if app.work.inbox.is_some() => inbox(frame, app, regions[1]),
        Mode::Workbench => overview(frame, app, regions[1]),
        Mode::Terminal => {}
    }
    footer(frame, app, regions[2]);
    if let Some(picker) = &app.work.actor_picker {
        actor_picker(frame, app, picker);
    }
    if let Some(draft) = &app.work.new_task {
        let width = area.width.saturating_sub(8).min(72);
        let height = area.height.saturating_sub(4).min(14);
        let panel = Rect {
            x: area.x + (area.width - width) / 2,
            y: area.y + (area.height - height) / 2,
            width,
            height,
        };
        frame.render_widget(Clear, panel);
        frame.render_widget(
            Block::default()
                .title(" New task ")
                .borders(Borders::ALL)
                .style(chrome()),
            panel,
        );
        let inner = inset(panel, 2, 1);
        plain(
            frame,
            inner,
            0,
            if draft.editing_brief {
                "TITLE"
            } else {
                "TITLE  ← editing"
            },
            theme::emphasis(),
        );
        plain(
            frame,
            inner,
            1,
            format!(
                "{}{}",
                draft.title,
                if draft.editing_brief { "" } else { "▌" }
            ),
            Style::default(),
        );
        plain(
            frame,
            inner,
            3,
            if draft.editing_brief {
                "BRIEF  ← editing"
            } else {
                "BRIEF"
            },
            theme::emphasis(),
        );
        frame.render_widget(
            Paragraph::new(format!(
                "{}{}",
                draft.brief,
                if draft.editing_brief { "▌" } else { "" }
            ))
            .wrap(Wrap { trim: false }),
            Rect {
                y: inner.y + 4,
                height: inner.height.saturating_sub(6),
                ..inner
            },
        );
        plain(
            frame,
            inner,
            inner.height.saturating_sub(1),
            "[Save task]  [Cancel]  ·  Tab fields",
            theme::secondary(),
        );
    }
}

pub(super) fn actor_picker_rect(area: Rect, count: usize) -> Rect {
    let width = area.width.saturating_sub(6).min(70);
    let height = (count as u16)
        .saturating_add(5)
        .min(16)
        .min(area.height.saturating_sub(4));
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

fn actor_picker(frame: &mut Frame, app: &App, picker: &super::ActorPicker) {
    let candidates = app.actor_candidates(picker.action);
    let panel = actor_picker_rect(frame.area(), candidates.len());
    frame.render_widget(Clear, panel);
    frame.render_widget(
        Block::default()
            .title(" Choose agent session ")
            .borders(Borders::ALL)
            .style(chrome()),
        panel,
    );
    let inner = inset(panel, 2, 1);
    let purpose = match picker.action {
        TaskAction::Claim
            if app
                .selected_task()
                .is_some_and(|task| task.status == "needs review") =>
        {
            "Take this review as"
        }
        TaskAction::Claim => "Claim this task as",
        TaskAction::Reply => "Choose a session to reply to the help request",
        TaskAction::Reassign => "Reassign this task to",
        _ => "Choose agent",
    };
    plain(frame, inner, 0, purpose, theme::emphasis());
    let capacity = inner.height.saturating_sub(3) as usize;
    let start = picker.selected.saturating_add(1).saturating_sub(capacity);
    for (ordinal, index) in candidates.iter().enumerate().skip(start).take(capacity) {
        let session = &app.sessions[*index];
        let active = picker.selected == ordinal;
        let marker = if active { glyph::CURSOR } else { " " };
        let label = format!(
            "{marker} {} · {}    {} {}",
            session.display_agent(),
            &session.id[..session.id.len().min(8)],
            state_glyph(&session.state),
            super::render::plain_state(&session.state)
        );
        plain(
            frame,
            inner,
            2 + (ordinal - start) as u16,
            short(&label, inner.width),
            if active {
                selected()
            } else {
                state_style(&session.state)
            },
        );
    }
    plain(
        frame,
        inner,
        inner.height.saturating_sub(1),
        "[Choose agent]  [Cancel]  ·  ↑↓ selects",
        theme::secondary(),
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HeaderAction {
    Mode(Mode),
    Help,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ClickAction {
    Back,
    NewAgent,
    IsolatedAgent,
    NewTask,
    Search,
    ApplySearch,
    ClearSearch,
    Memory,
    AddMemory,
    Changes,
    Refresh,
    Help,
    Quit,
    OpenSelected,
    OpenTask,
    Inbox,
    InboxTask(usize),
    OpenSession,
    Save,
    Cancel,
    ChooseAgent,
    Task(TaskAction),
}

#[derive(Clone, Copy)]
struct Button {
    label: &'static str,
    action: ClickAction,
}

fn button(label: &'static str, action: ClickAction) -> Button {
    Button { label, action }
}

fn session_panel_buttons(app: &App) -> Vec<Button> {
    let mut buttons = vec![button("New agent", ClickAction::NewAgent)];
    if app.git.root.is_some() {
        buttons.push(button("Isolated", ClickAction::IsolatedAgent));
    } else {
        buttons.push(button("Memory", ClickAction::Memory));
    }
    buttons
}

fn task_panel_buttons(app: &App) -> Vec<Button> {
    let mut buttons = Vec::new();
    if app.selected_task().is_some() {
        buttons.push(button("Open task", ClickAction::OpenTask));
    }
    buttons.push(button("New task", ClickAction::NewTask));
    buttons.push(button("Search", ClickAction::Search));
    buttons
}

fn footer_buttons(app: &App, row: u16) -> Vec<Button> {
    if row == 1 {
        return if app.mode == Mode::Workbench
            && !app.work.detail
            && app.work.inbox.is_none()
            && app.work.new_task.is_none()
            && app.work.actor_picker.is_none()
            && app.work.search.is_none()
        {
            let mut buttons = vec![
                button("Changed files", ClickAction::Changes),
                button("Refresh", ClickAction::Refresh),
                button("Help", ClickAction::Help),
                button("Quit", ClickAction::Quit),
            ];
            if !app.work.filter.is_empty() {
                buttons.insert(0, button("Clear search", ClickAction::ClearSearch));
            }
            buttons
        } else {
            vec![]
        };
    }
    if app.work.actor_picker.is_some() {
        return vec![
            button("Choose agent", ClickAction::ChooseAgent),
            button("Cancel", ClickAction::Cancel),
        ];
    }
    if app.work.new_task.is_some()
        || app.work.memory_editor.is_some()
        || app.work.composer.is_some()
    {
        return vec![
            button("Save", ClickAction::Save),
            button("Cancel", ClickAction::Cancel),
        ];
    }
    if app.work.search.is_some() {
        return vec![
            button("Apply search", ClickAction::ApplySearch),
            button("Clear", ClickAction::ClearSearch),
        ];
    }
    match app.mode {
        Mode::Memory => vec![
            button("Back", ClickAction::Back),
            button("Add note", ClickAction::AddMemory),
        ],
        Mode::Activity => vec![
            button("Back", ClickAction::Back),
            button("Help", ClickAction::Help),
        ],
        Mode::Workbench if app.work.detail => {
            let mut buttons = vec![button("Back", ClickAction::Back)];
            if let Some(task) = app.selected_task() {
                if task.status != "done" {
                    if matches!(task.status, "open" | "needs review") {
                        buttons.push(button(
                            if task.status == "needs review" {
                                "Take review"
                            } else {
                                "Claim"
                            },
                            ClickAction::Task(TaskAction::Claim),
                        ));
                    } else if task.needs_help {
                        buttons.push(button("Reply", ClickAction::Task(TaskAction::Reply)));
                    } else {
                        buttons.push(button(
                            "Ask for help",
                            ClickAction::Task(TaskAction::RequestHelp),
                        ));
                    }
                    if task.status == "active" {
                        buttons.push(button("Handoff", ClickAction::Task(TaskAction::Handoff)));
                        buttons.push(button("Mark done", ClickAction::Task(TaskAction::Done)));
                    }
                    buttons.push(button("Reassign", ClickAction::Task(TaskAction::Reassign)));
                }
            }
            buttons
        }
        Mode::Workbench if app.work.inbox.is_some() => {
            let mut buttons = vec![button("Back", ClickAction::Back)];
            if app
                .work
                .inbox
                .as_ref()
                .is_some_and(|inbox| !inbox.items.is_empty())
            {
                buttons.push(button(
                    "Open task",
                    ClickAction::InboxTask(app.work.inbox_index),
                ));
            }
            if app
                .project_session_indices()
                .get(app.work.session)
                .and_then(|index| app.sessions.get(*index))
                .is_some_and(|session| app.session_can_open(session))
            {
                buttons.push(button("Open session", ClickAction::OpenSession));
            }
            buttons.push(button("Refresh", ClickAction::Refresh));
            buttons
        }
        Mode::Workbench => {
            let mut buttons = if app.work.focus == super::WorkFocus::Sessions {
                let mut buttons = vec![button("New agent", ClickAction::NewAgent)];
                if app.git.root.is_some() {
                    buttons.push(button("Isolated", ClickAction::IsolatedAgent));
                }
                if !app.project_session_indices().is_empty() {
                    let can_open = app
                        .project_session_indices()
                        .get(app.work.session)
                        .and_then(|index| app.sessions.get(*index))
                        .is_some_and(|session| app.session_can_open(session));
                    if can_open {
                        buttons.push(button("Open session", ClickAction::OpenSelected));
                        buttons.push(button("Inbox", ClickAction::Inbox));
                    } else {
                        buttons.push(button("View inbox", ClickAction::Inbox));
                    }
                }
                buttons
            } else {
                let mut buttons = vec![button("New task", ClickAction::NewTask)];
                if app.selected_task().is_some() {
                    buttons.push(button("Open task", ClickAction::OpenSelected));
                }
                buttons.push(button("Search", ClickAction::Search));
                buttons
            };
            buttons.push(button("Memory", ClickAction::Memory));
            buttons
        }
        Mode::Terminal => vec![],
    }
}

fn buttons_line(buttons: &[Button]) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for item in buttons {
        spans.push(Span::styled(format!("[{}]", item.label), theme::emphasis()));
        spans.push(Span::raw("  "));
    }
    Line::from(spans)
}

fn button_at(buttons: &[Button], column: u16, width: u16) -> Option<ClickAction> {
    let mut x = 1_u16;
    for item in buttons {
        let end = x.saturating_add(item.label.chars().count() as u16 + 2);
        if (x..end.min(width)).contains(&column) {
            return Some(item.action);
        }
        x = end.saturating_add(2);
    }
    None
}

pub(super) fn footer_action_at(app: &App, column: u16, row: u16) -> Option<ClickAction> {
    let footer_start = app.frame_height.saturating_sub(2);
    if row < footer_start || row >= app.frame_height {
        return None;
    }
    button_at(
        &footer_buttons(app, row - footer_start),
        column,
        app.frame_width,
    )
}

pub(super) fn panel_action_at(app: &App, column: u16, row: u16) -> Option<ClickAction> {
    if app.frame_width < 40 || app.frame_height < 12 {
        return None;
    }
    let body = Rect::new(0, 3, app.frame_width, app.frame_height.saturating_sub(5));
    if app.mode == Mode::Memory {
        let inner = inset(body, 2, 1);
        if app.memory.is_empty() && row == inner.y + 2 {
            let prefix = "No shared memory yet. ".len() as u16;
            if (inner.x + prefix..inner.x + prefix + 10).contains(&column) {
                return Some(ClickAction::AddMemory);
            }
        }
        return None;
    }
    if app.mode != Mode::Workbench || app.work.detail {
        return None;
    }
    if let Some(inbox) = &app.work.inbox {
        let inner = inset(body, 2, 1);
        if column < inner.x
            || column >= inner.x.saturating_add(inner.width)
            || row < inner.y + INBOX_LIST_ROW
        {
            return None;
        }
        let capacity = (inner.height.saturating_sub(INBOX_LIST_ROW + 1) / 2).max(1) as usize;
        let start = app
            .work
            .inbox_index
            .saturating_add(1)
            .saturating_sub(capacity);
        let index = start + ((row - inner.y - INBOX_LIST_ROW) / 2) as usize;
        if index < inbox.items.len() && index < start + capacity {
            return Some(ClickAction::InboxTask(index));
        }
        return None;
    }
    let narrow = body.width < 100;
    let pane = if narrow {
        Rect {
            y: body.y + 1,
            height: body.height.saturating_sub(1),
            ..body
        }
    } else if column < 36 {
        Rect { width: 36, ..body }
    } else {
        Rect {
            x: 36,
            width: body.width.saturating_sub(36),
            ..body
        }
    };
    let focus = if narrow {
        app.work.focus
    } else if column < 36 {
        super::WorkFocus::Sessions
    } else {
        super::WorkFocus::Tasks
    };
    let inner = inset(pane, 2, 1);
    if column < inner.x || column >= inner.x.saturating_add(inner.width) {
        return None;
    }
    match focus {
        super::WorkFocus::Sessions => {
            if app.project_session_indices().is_empty()
                && row == inner.y + 3
                && column < inner.x + 11
            {
                return Some(ClickAction::NewAgent);
            }
            if row == inner.y + inner.height.saturating_sub(1) {
                return button_at(&session_panel_buttons(app), column - inner.x, inner.width);
            }
        }
        super::WorkFocus::Tasks => {
            if !app.work.filter.is_empty() && row == inner.y + 1 {
                let prefix = format!(
                    "Filter: {} · {} shown · ",
                    app.work.filter,
                    app.visible_task_indices().len()
                );
                let start = inner.x.saturating_add(prefix.chars().count() as u16);
                if (start..start.saturating_add(7).min(inner.x + inner.width)).contains(&column) {
                    return Some(ClickAction::ClearSearch);
                }
            }
            if app.visible_task_indices().is_empty() && row == inner.y + 2 && column < inner.x + 10
            {
                return Some(if app.work.filter.is_empty() {
                    ClickAction::NewTask
                } else {
                    ClickAction::ClearSearch
                });
            }
            if row == inner.y + inner.height.saturating_sub(1) {
                return button_at(&task_panel_buttons(app), column - inner.x, inner.width);
            }
        }
    }
    None
}

pub(super) fn modal_action_at(app: &App, column: u16, row: u16) -> Option<ClickAction> {
    let area = Rect::new(0, 0, app.frame_width, app.frame_height);
    if let Some(picker) = &app.work.actor_picker {
        let panel = actor_picker_rect(area, app.actor_candidates(picker.action).len());
        let inner = inset(panel, 2, 1);
        if row == inner.y + inner.height.saturating_sub(1)
            && column >= inner.x
            && column < inner.x + inner.width
        {
            return button_at(
                &[
                    button("Choose agent", ClickAction::ChooseAgent),
                    button("Cancel", ClickAction::Cancel),
                ],
                column - inner.x + 1,
                inner.width + 1,
            );
        }
    }
    if app.work.new_task.is_some() {
        let width = area.width.saturating_sub(8).min(72);
        let height = area.height.saturating_sub(4).min(14);
        let panel = Rect::new(
            (area.width - width) / 2,
            (area.height - height) / 2,
            width,
            height,
        );
        let inner = inset(panel, 2, 1);
        if row == inner.y + inner.height.saturating_sub(1)
            && column >= inner.x
            && column < inner.x + inner.width
        {
            return button_at(
                &[
                    button("Save task", ClickAction::Save),
                    button("Cancel", ClickAction::Cancel),
                ],
                column - inner.x + 1,
                inner.width + 1,
            );
        }
    }
    None
}

fn tabs(_app: &App) -> [(Mode, String); 4] {
    [
        (Mode::Terminal, "[1 Terminal]".to_owned()),
        (Mode::Workbench, "[2 Workbench]".to_owned()),
        (Mode::Activity, "[3 Activity]".to_owned()),
        (Mode::Memory, "[4 Memory]".to_owned()),
    ]
}

pub(super) fn header_action_at(app: &App, column: u16) -> Option<HeaderAction> {
    let mut x = 1_u16;
    for (mode, label) in tabs(app) {
        let end = x + label.chars().count() as u16;
        if (x..end).contains(&column) {
            return Some(HeaderAction::Mode(mode));
        }
        x = end + 2;
    }
    if (x..x + 6).contains(&column) {
        Some(HeaderAction::Help)
    } else {
        None
    }
}

fn header(frame: &mut Frame, app: &App, area: Rect) {
    frame.render_widget(Block::default().style(chrome()), area);
    let git = &app.git;
    let project = app
        .project_anchor
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| crate::display_path(&app.project_anchor));
    let workspace = if app.project == app.project_anchor {
        ""
    } else {
        " · isolated"
    };
    let summary = format!(
        "verb  ›  {}{} / {}  ·  {} changed",
        project,
        workspace,
        git.branch.as_deref().unwrap_or("detached"),
        git.changed_files
    );
    plain(
        frame,
        area,
        0,
        short(&summary, area.width.saturating_sub(2)),
        theme::emphasis(),
    );
    let mut spans = vec![Span::raw(" ")];
    for (index, (mode, label)) in tabs(app).into_iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(
            label,
            if mode == app.mode {
                selected()
            } else {
                theme::secondary()
            },
        ));
    }
    spans.push(Span::raw("  "));
    spans.push(Span::styled("[Help]", theme::secondary()));
    put(frame, area, 1, Line::from(spans));
    plain(
        frame,
        area,
        2,
        "─".repeat(area.width as usize),
        theme::secondary(),
    );
}

fn overview(frame: &mut Frame, app: &App, area: Rect) {
    if area.width < 100 {
        narrow_focus(frame, app, area);
        let pane = Rect {
            y: area.y + 1,
            height: area.height.saturating_sub(1),
            ..area
        };
        if app.work.focus == WorkFocus::Sessions {
            sessions(frame, app, pane);
        } else {
            tasks(frame, app, pane);
        }
        return;
    }
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(36), Constraint::Min(1)])
        .split(area);
    sessions(frame, app, split[0]);
    for row in 0..split[0].height {
        plain(
            frame,
            Rect {
                x: split[0].x + split[0].width - 1,
                width: 1,
                ..split[0]
            },
            row,
            "│",
            theme::secondary(),
        );
    }
    tasks(frame, app, split[1]);
}

fn focus_labels(app: &App) -> (String, String) {
    let tasks = format!(
        "{} Tasks ({})",
        if app.work.focus == WorkFocus::Tasks {
            "["
        } else {
            " "
        },
        app.tasks.len()
    );
    let tasks = if app.work.focus == WorkFocus::Tasks {
        format!("{tasks}]")
    } else {
        tasks
    };
    let sessions = format!(
        "{} Sessions ({})",
        if app.work.focus == WorkFocus::Sessions {
            "["
        } else {
            " "
        },
        app.project_session_indices().len()
    );
    let sessions = if app.work.focus == WorkFocus::Sessions {
        format!("{sessions}]")
    } else {
        sessions
    };
    (tasks, sessions)
}

pub(super) fn narrow_focus_at(app: &App, column: u16) -> Option<WorkFocus> {
    let (tasks, sessions) = focus_labels(app);
    let tasks_end = 2 + tasks.chars().count() as u16;
    let sessions_start = tasks_end + 3;
    if (2..tasks_end).contains(&column) {
        Some(WorkFocus::Tasks)
    } else if (sessions_start..sessions_start + sessions.chars().count() as u16).contains(&column) {
        Some(WorkFocus::Sessions)
    } else {
        None
    }
}

fn narrow_focus(frame: &mut Frame, app: &App, area: Rect) {
    let (tasks, sessions) = focus_labels(app);
    put(
        frame,
        area,
        0,
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                tasks,
                if app.work.focus == WorkFocus::Tasks {
                    theme::emphasis()
                } else {
                    theme::secondary()
                },
            ),
            Span::raw("   "),
            Span::styled(
                sessions,
                if app.work.focus == WorkFocus::Sessions {
                    theme::emphasis()
                } else {
                    theme::secondary()
                },
            ),
            Span::styled("    Tab switches", theme::secondary()),
        ]),
    );
}

fn sessions(frame: &mut Frame, app: &App, area: Rect) {
    let inner = inset(area, 2, 1);
    plain(
        frame,
        inner,
        0,
        format!(
            "SESSIONS  ·  {} in project",
            app.project_session_indices().len()
        ),
        theme::emphasis(),
    );
    let indices = app.project_session_indices();
    if indices.is_empty() {
        plain(
            frame,
            inner,
            2,
            "No agent sessions here yet.",
            theme::secondary(),
        );
        plain(
            frame,
            inner,
            3,
            "[New agent] Start a session",
            theme::secondary(),
        );
    }
    let memory_row = inner.height.saturating_sub(5);
    let capacity = (memory_row.saturating_sub(2) / 3).max(1) as usize;
    let start = app.work.session.saturating_add(1).saturating_sub(capacity);
    for (offset, index) in indices.iter().enumerate().skip(start).take(capacity) {
        let row = 2 + (offset - start) as u16 * 3;
        let session = &app.sessions[*index];
        let active = app.work.focus == WorkFocus::Sessions && app.work.session == offset;
        let marker = if active { glyph::CURSOR } else { " " };
        let label = format!(
            "{marker} {} · {}  {} {}",
            session.display_agent(),
            &session.id[..session.id.len().min(6)],
            state_glyph(&session.state),
            super::render::plain_state(&session.state)
        );
        plain(
            frame,
            inner,
            row,
            short(&label, inner.width),
            if active {
                selected()
            } else {
                state_style(&session.state)
            },
        );
        let assignments = app
            .tasks
            .iter()
            .filter(|task| task.owner.as_deref() == Some(session.id.as_str()))
            .collect::<Vec<_>>();
        let assignment = match assignments.as_slice() {
            [] => "No task assigned".to_owned(),
            [task] => task.title.clone(),
            [task, ..] => format!("{} (+{} more)", task.title, assignments.len() - 1),
        };
        plain(
            frame,
            inner,
            row + 1,
            short(
                &format!(
                    "  {}{assignment}",
                    if session.project_id != app.project_anchor {
                        "Isolated · "
                    } else {
                        ""
                    }
                ),
                inner.width,
            ),
            theme::secondary(),
        );
        let status = session_card_status(app, session, inner.width);
        plain(
            frame,
            inner,
            row + 2,
            short(&format!("  {status}"), inner.width),
            state_style(&session.state),
        );
    }
    plain(
        frame,
        inner,
        memory_row,
        "PROJECT MEMORY",
        theme::emphasis(),
    );
    let preview = app.memory.lines().take(2).collect::<Vec<_>>();
    if preview.is_empty() {
        plain(
            frame,
            inner,
            memory_row + 1,
            "No shared notes yet.",
            theme::secondary(),
        );
    } else {
        for (line, text) in preview.iter().enumerate() {
            plain(
                frame,
                inner,
                memory_row + 1 + line as u16,
                short(text, inner.width),
                Style::default(),
            );
        }
    }
    put(
        frame,
        inner,
        inner.height.saturating_sub(1),
        buttons_line(&session_panel_buttons(app)),
    );
}

fn state_glyph(state: &SessionState) -> &'static str {
    match state {
        SessionState::Live => glyph::RUNNING,
        SessionState::Recoverable => glyph::RECOVERABLE,
        SessionState::Interrupted => glyph::CHECKING,
        SessionState::Ended => glyph::ENDED,
    }
}

fn tasks(frame: &mut Frame, app: &App, area: Rect) {
    let inner = inset(area, 2, 1);
    let attention_count = app
        .tasks
        .iter()
        .filter(|task| task.needs_help || task.status == "needs review")
        .count();
    put(
        frame,
        inner,
        0,
        Line::from(vec![
            Span::styled("PRIORITY WORK", theme::emphasis()),
            Span::styled(
                format!(
                    "  ·  {} tasks  ·  {attention_count} need attention",
                    app.tasks.len()
                ),
                attention(),
            ),
        ]),
    );
    let visible = app.visible_task_indices();
    if !app.work.filter.is_empty() {
        plain(
            frame,
            inner,
            1,
            short(
                &format!(
                    "Filter: {} · {} shown · [Clear]",
                    app.work.filter,
                    visible.len()
                ),
                inner.width,
            ),
            theme::attention(),
        );
    } else {
        plain(
            frame,
            inner,
            1,
            "Help → review → active → open",
            theme::secondary(),
        );
    }
    if visible.is_empty() {
        plain(
            frame,
            inner,
            2,
            if app.work.filter.is_empty() {
                "[New task] Create the first task"
            } else {
                "[Clear] No tasks match this search."
            },
            theme::secondary(),
        );
    }
    let max_rows = (inner.height.saturating_sub(10) / 3).max(1) as usize;
    let start = app.work.task.saturating_add(1).saturating_sub(max_rows);
    for (ordinal, index) in visible.iter().enumerate().skip(start).take(max_rows) {
        let row = 2 + (ordinal - start) as u16 * 3;
        let task = &app.tasks[*index];
        let active = app.work.focus == WorkFocus::Tasks && app.work.task == ordinal;
        let marker = if active { glyph::CURSOR } else { " " };
        let title_width = inner.width.saturating_sub(21) as usize;
        let title = super::render::truncate(&task.title, title_width);
        let label = format!("{marker} {title:<title_width$} {}", task_label(task));
        plain(
            frame,
            inner,
            row,
            short(&label, inner.width),
            if active { selected() } else { Style::default() },
        );
        let owner = task_context_label(app, task);
        plain(
            frame,
            inner,
            row + 1,
            short(
                &format!("  {owner} · {}", &task.id[..task.id.len().min(8)]),
                inner.width,
            ),
            theme::secondary(),
        );
    }
    let preview_row = inner.height.saturating_sub(7);
    plain(
        frame,
        inner,
        preview_row,
        "SELECTED TASK",
        theme::emphasis(),
    );
    if let Some(task) = app.selected_task() {
        plain(
            frame,
            inner,
            preview_row + 1,
            short(&task.title, inner.width),
            theme::emphasis(),
        );
        plain(
            frame,
            inner,
            preview_row + 2,
            task_label(task),
            task_style(task),
        );
        let note = task
            .events
            .last()
            .and_then(|event| event.note.as_deref())
            .unwrap_or(&task.brief);
        plain(
            frame,
            inner,
            preview_row + 4,
            short(note.lines().next().unwrap_or(""), inner.width),
            theme::secondary(),
        );
    }
    put(
        frame,
        inner,
        inner.height.saturating_sub(1),
        buttons_line(&task_panel_buttons(app)),
    );
}

fn memory(frame: &mut Frame, app: &App, area: Rect) {
    let inner = inset(area, 2, 1);
    plain(frame, inner, 0, "SHARED PROJECT MEMORY", theme::emphasis());
    if app.memory.is_empty() {
        plain(
            frame,
            inner,
            2,
            "No shared memory yet. [Add note]",
            theme::secondary(),
        );
    } else {
        frame.render_widget(
            Paragraph::new(app.memory.as_str())
                .wrap(Wrap { trim: false })
                .scroll((app.work.scroll, 0)),
            Rect {
                y: inner.y + 2,
                height: inner.height.saturating_sub(3),
                ..inner
            },
        );
    }
    if let Some(editor) = &app.work.memory_editor {
        let box_area = Rect {
            y: inner.y + inner.height.saturating_sub(6),
            height: 6.min(inner.height),
            ..inner
        };
        frame.render_widget(Block::default().style(chrome()), box_area);
        plain(
            frame,
            box_area,
            0,
            "APPEND NOTE · Ctrl+D save · Esc cancel",
            theme::emphasis(),
        );
        frame.render_widget(
            Paragraph::new(editor.as_str()).wrap(Wrap { trim: false }),
            Rect {
                y: box_area.y + 1,
                height: box_area.height.saturating_sub(1),
                ..box_area
            },
        );
    }
}

fn inbox(frame: &mut Frame, app: &App, area: Rect) {
    let Some(inbox) = &app.work.inbox else {
        return;
    };
    let inner = inset(area, 2, 1);
    let label = actor_label(app, &inbox.session_id);
    plain(
        frame,
        inner,
        0,
        short(&format!("SESSION INBOX  ·  {label}"), inner.width),
        theme::emphasis(),
    );
    let freshness = match inbox.context_state {
        "fetched current revision" => "Shared context current",
        "never fetched" => "Shared context never fetched",
        _ => "New shared context available",
    };
    plain(
        frame,
        inner,
        1,
        short(
            &format!("{freshness}  ·  {} attention items", inbox.items.len()),
            inner.width,
        ),
        if inbox.context_state == "fetched current revision" {
            theme::success()
        } else {
            attention()
        },
    );
    if let Some(session) = app
        .sessions
        .iter()
        .find(|session| session.id == inbox.session_id)
    {
        if let Some(git) = app.workspace_status.get(&session.project_id) {
            let workspace = if git.root.is_some() {
                format!(
                    "Workspace: {} · {} changed",
                    git.branch.as_deref().unwrap_or("detached"),
                    git.changed_files
                )
            } else {
                "Workspace: checkout missing or unavailable".to_owned()
            };
            plain(
                frame,
                inner,
                2,
                short(&workspace, inner.width),
                theme::emphasis(),
            );
        }
        plain(
            frame,
            inner,
            3,
            short(&session.project_id.to_string_lossy(), inner.width),
            theme::secondary(),
        );
    }
    plain(
        frame,
        inner,
        4,
        "Recorded attention. Opening this list does not start an agent.",
        theme::secondary(),
    );
    if inbox.items.is_empty() {
        plain(
            frame,
            inner,
            INBOX_LIST_ROW,
            "Nothing needs this session's attention right now.",
            theme::secondary(),
        );
    }
    let capacity = (inner.height.saturating_sub(INBOX_LIST_ROW + 1) / 2).max(1) as usize;
    let start = app
        .work
        .inbox_index
        .saturating_add(1)
        .saturating_sub(capacity);
    for (index, item) in inbox.items.iter().enumerate().skip(start).take(capacity) {
        let row = INBOX_LIST_ROW + (index - start) as u16 * 2;
        let marker = if index == app.work.inbox_index {
            glyph::CURSOR
        } else {
            " "
        };
        let fresh = match item.new_since_fetch {
            Some(true) => " · new since fetch",
            Some(false) => "",
            None => " · freshness unknown",
        };
        plain(
            frame,
            inner,
            row,
            short(
                &format!("{marker} {}{fresh}", item.kind.label()),
                inner.width,
            ),
            if index == app.work.inbox_index {
                selected()
            } else {
                attention()
            },
        );
        plain(
            frame,
            inner,
            row + 1,
            short(
                &format!(
                    "  {}  ·  {}",
                    item.title,
                    &item.task_id[..item.task_id.len().min(8)]
                ),
                inner.width,
            ),
            theme::secondary(),
        );
    }
    plain(
        frame,
        inner,
        inner.height.saturating_sub(1),
        "Enter opens task · s opens session · r refreshes",
        theme::secondary(),
    );
}

fn detail(frame: &mut Frame, app: &App, area: Rect) {
    let inner = inset(area, 2, 1);
    let Some(task) = app.selected_task() else {
        plain(
            frame,
            inner,
            0,
            "Task no longer available. Esc returns to Workbench.",
            theme::secondary(),
        );
        return;
    };
    let mut lines = vec![
        Line::from(Span::styled(
            format!("← WORKBENCH / TASK     {}", task.id),
            theme::secondary(),
        )),
        Line::from(""),
        Line::from(Span::styled(task.title.clone(), theme::emphasis())),
        Line::from(Span::styled(task_label(task), task_style(task))),
        Line::from(Span::styled(
            task_context_label(app, task),
            theme::secondary(),
        )),
        Line::from(Span::styled(next_task_action(task), theme::attention())),
        Line::from(""),
        Line::from(Span::styled("BRIEF", theme::emphasis())),
        Line::from(task.brief.clone()),
        Line::from(""),
        Line::from(Span::styled("ACTIVITY · newest first", theme::emphasis())),
        Line::from(Span::styled(
            "Recorded notes do not verify the agent's work.",
            theme::secondary(),
        )),
    ];
    for event in task.events.iter().rev() {
        lines.push(Line::from(format!(
            "{}  {} · session {}",
            crate::iso8601(event.at),
            event.kind.label(),
            actor_label(app, &event.session_id)
        )));
        if let Some(note) = &event.note {
            lines.push(Line::from(Span::styled(
                format!("  {note}"),
                theme::secondary(),
            )));
        }
    }
    if task.events.is_empty() {
        lines.push(Line::from(Span::styled(
            "No actions recorded yet.",
            theme::secondary(),
        )));
    }
    let content_height =
        inner
            .height
            .saturating_sub(if app.work.composer.is_some() { 7 } else { 1 });
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((app.work.scroll, 0)),
        Rect {
            height: content_height,
            ..inner
        },
    );
    if let Some(composer) = &app.work.composer {
        let box_area = Rect {
            y: inner.y + content_height,
            height: 6.min(inner.height.saturating_sub(content_height)),
            ..inner
        };
        plain(
            frame,
            box_area,
            0,
            format!("NOTE · {:?} · Ctrl+D save · Esc cancel", composer.action),
            theme::emphasis(),
        );
        frame.render_widget(
            Paragraph::new(composer.text.as_str()).wrap(Wrap { trim: false }),
            Rect {
                y: box_area.y + 1,
                height: box_area.height.saturating_sub(1),
                ..box_area
            },
        );
    }
}

fn activity(frame: &mut Frame, app: &App, area: Rect) {
    let inner = inset(area, 2, 1);
    let context = crate::context::assemble_for(
        &app.project,
        app.hosted.as_ref().map(|hosted| &hosted.session),
    );
    match context {
        Ok(context) => {
            let evidence = EvidenceLines::build_with(
                &context,
                crate::now_millis(),
                app.hosted
                    .as_ref()
                    .and_then(|hosted| hosted.observed_if_read()),
            );
            let lines: Vec<Line> = evidence
                .lines
                .into_iter()
                .map(|(kind, text)| {
                    let style = match kind {
                        Kind::Heading => theme::emphasis(),
                        Kind::Caveat => theme::secondary(),
                        Kind::Fact | Kind::Empty => Style::default(),
                    };
                    Line::from(Span::styled(text, style))
                })
                .collect();
            frame.render_widget(
                Paragraph::new(lines)
                    .wrap(Wrap { trim: false })
                    .scroll((app.work.scroll, 0)),
                inner,
            );
        }
        Err(error) => plain(
            frame,
            inner,
            0,
            format!("Could not load activity: {error}"),
            theme::danger(),
        ),
    }
}

fn footer(frame: &mut Frame, app: &App, area: Rect) {
    frame.render_widget(Block::default().style(chrome()), area);
    put(frame, area, 0, buttons_line(&footer_buttons(app, 0)));
    if let Some(search) = &app.work.search {
        plain(
            frame,
            area,
            1,
            format!(" Search: {search}▌"),
            theme::emphasis(),
        );
    } else if !footer_buttons(app, 1).is_empty() {
        let buttons = footer_buttons(app, 1);
        put(frame, area, 1, buttons_line(&buttons));
        if let Some(message) = &app.message {
            let offset = 1 + buttons
                .iter()
                .map(|item| item.label.chars().count() + 4)
                .sum::<usize>() as u16;
            if offset < area.width {
                plain(
                    frame,
                    Rect {
                        x: area.x + offset,
                        y: area.y + 1,
                        width: area.width - offset,
                        height: 1,
                    },
                    0,
                    short(message, area.width - offset),
                    theme::attention(),
                );
            }
        }
    } else if let Some(message) = &app.message {
        plain(
            frame,
            area,
            1,
            short(message, area.width),
            theme::attention(),
        );
    } else {
        plain(
            frame,
            area,
            1,
            " Tab switches lists · ↑↓ moves selection · Enter opens",
            theme::secondary(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn sample_task(index: u128) -> crate::workbench::TaskSnapshot {
        crate::workbench::TaskSnapshot {
            id: format!("{index:032x}"),
            title: format!("Review agent handoff {index}"),
            brief: "Verify the proposed implementation and record a decision.".to_owned(),
            status: "needs review",
            owner: None,
            needs_help: false,
            created_at: index,
            events: Vec::new(),
        }
    }

    fn screen(app: &App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| {
                super::super::render::workspace(frame, app);
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol().to_owned())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn workbench_shows_priority_and_session_columns_at_120_by_30() {
        let mut app = App::for_tests();
        app.mode = Mode::Workbench;
        app.tasks = (1..=6).map(sample_task).collect();
        app.sessions.push(crate::Session::new(
            app.project.clone(),
            crate::Agent::Claude,
        ));
        app.memory = "Use short-lived branches and record handoffs.".to_owned();
        let first = screen(&app, 120, 30);
        assert!(first.contains("SESSIONS"));
        assert!(first.contains("PRIORITY WORK"));
        assert!(first.contains("Review agent handoff 6"));
        assert!(first.contains("PROJECT MEMORY"));
        app.work.task = 5;
        let scrolled = screen(&app, 120, 30);
        assert!(scrolled.contains("Review agent handoff 1"));
    }

    #[test]
    fn inbox_is_readable_and_clickable_at_eighty_columns() {
        let mut app = App::for_tests();
        app.mode = Mode::Workbench;
        app.frame_width = 80;
        app.frame_height = 24;
        let session = crate::Session::new(app.project.clone(), crate::Agent::Claude);
        let id = session.id.clone();
        app.sessions.push(session);
        app.work.focus = WorkFocus::Sessions;
        app.workspace_status.insert(
            app.project.clone(),
            crate::GitSnapshot {
                root: Some(app.project.clone()),
                branch: Some("verb/example".to_owned()),
                changed_files: 2,
            },
        );
        let inbox = crate::workbench::InboxSnapshot {
            session_id: id.clone(),
            revision: "sha256:example".to_owned(),
            context_state: "new revision available",
            fetched_at: Some(1),
            items: vec![crate::workbench::InboxItem {
                task_id: "abcdef0123456789".to_owned(),
                title: "Review the storage boundary".to_owned(),
                kind: crate::workbench::InboxKind::ReviewAvailable,
                linked_session_id: "other".to_owned(),
                new_since_fetch: Some(true),
                at: 2,
            }],
        };
        app.inboxes.insert(id, inbox.clone());
        let overview = screen(&app, 80, 24);
        assert!(overview.contains("View inbox · Running elsewhere · 1 alert(s)"));
        assert!(overview.contains("[View inbox]"));
        app.work.inbox = Some(inbox);
        let detail = screen(&app, 80, 24);
        assert!(detail.contains("SESSION INBOX"));
        assert!(detail.contains("New shared context available"));
        assert!(detail.contains("Workspace: verb/example · 2 changed"));
        assert!(detail.contains("Review available · new since fetch"));
        assert!(detail.contains("Review the storage boundary"));
        assert!(!detail.contains("[Open session]"));
        assert_eq!(
            panel_action_at(&app, 4, 10),
            Some(ClickAction::InboxTask(0))
        );
        assert_eq!(footer_action_at(&app, 3, 22), Some(ClickAction::Back));
        let mut task = sample_task(1);
        task.id = "abcdef0123456789".to_owned();
        app.tasks.push(task);
        app.open_inbox_task(0);
        assert!(app.work.detail);
        assert!(app.work.inbox.is_some());
        app.on_workbench_key(ratatui::crossterm::event::KeyEvent::new(
            ratatui::crossterm::event::KeyCode::Esc,
            ratatui::crossterm::event::KeyModifiers::NONE,
        ))
        .unwrap();
        assert!(!app.work.detail);
        assert!(app.work.inbox.is_some());
    }

    #[test]
    fn narrow_workbench_uses_focus_to_switch_between_tasks_and_sessions() {
        let mut app = App::for_tests();
        app.mode = Mode::Workbench;
        app.tasks.push(sample_task(1));
        app.sessions.push(crate::Session::new(
            app.project.clone(),
            crate::Agent::Codex,
        ));
        let tasks = screen(&app, 80, 24);
        assert!(tasks.contains("PRIORITY WORK"));
        assert!(!tasks.contains("SESSIONS"));
        app.work.focus = WorkFocus::Sessions;
        let sessions = screen(&app, 80, 24);
        assert!(sessions.contains("SESSIONS"));
        assert!(!sessions.contains("PRIORITY WORK"));
    }

    #[test]
    fn header_and_narrow_focus_hit_targets_match_visible_controls() {
        let mut app = App::for_tests();
        app.mode = Mode::Workbench;
        assert_eq!(
            header_action_at(&app, 1),
            Some(HeaderAction::Mode(Mode::Terminal))
        );
        assert_eq!(header_action_at(&app, 80), None);
        assert_eq!(narrow_focus_at(&app, 3), Some(WorkFocus::Tasks));
        let sessions_column = (0..80)
            .find(|column| narrow_focus_at(&app, *column) == Some(WorkFocus::Sessions))
            .unwrap();
        assert_eq!(
            narrow_focus_at(&app, sessions_column),
            Some(WorkFocus::Sessions)
        );
    }

    #[test]
    fn visible_buttons_have_matching_hit_targets_at_wide_and_narrow_sizes() {
        let mut app = App::for_tests();
        app.mode = Mode::Workbench;
        app.frame_width = 120;
        app.frame_height = 30;
        let rendered = screen(&app, 120, 30);
        assert!(rendered.contains("[New agent]"));
        assert!(rendered.contains("[New task]"));
        assert_eq!(footer_action_at(&app, 2, 28), Some(ClickAction::NewTask));
        assert_eq!(footer_action_at(&app, 16, 28), Some(ClickAction::Search));
        assert_eq!(panel_action_at(&app, 3, 7), Some(ClickAction::NewAgent));
        assert_eq!(panel_action_at(&app, 39, 6), Some(ClickAction::NewTask));
        assert_eq!(footer_action_at(&app, 2, 29), Some(ClickAction::Changes));

        app.frame_width = 80;
        app.frame_height = 24;
        assert_eq!(panel_action_at(&app, 3, 7), Some(ClickAction::NewTask));
        assert_eq!(narrow_focus_at(&app, 3), Some(WorkFocus::Tasks));
        assert_eq!(footer_action_at(&app, 16, 22), Some(ClickAction::Search));
        app.work.focus = WorkFocus::Sessions;
        assert_eq!(footer_action_at(&app, 2, 22), Some(ClickAction::NewAgent));
        app.git.root = Some(app.project.clone());
        assert!(screen(&app, 80, 24).contains("[Isolated]"));
        assert_eq!(
            footer_action_at(&app, 16, 22),
            Some(ClickAction::IsolatedAgent)
        );
    }

    #[test]
    fn modal_buttons_share_their_rendered_positions_with_hit_testing() {
        let mut app = App::for_tests();
        app.mode = Mode::Workbench;
        app.frame_width = 120;
        app.frame_height = 30;
        app.work.new_task = Some(super::super::NewTask::default());
        let rendered = screen(&app, 120, 30);
        assert!(rendered.contains("[Save task]  [Cancel]"));
        let save = (0..30)
            .flat_map(|row| (0..120).map(move |column| (column, row)))
            .find(|(column, row)| modal_action_at(&app, *column, *row) == Some(ClickAction::Save))
            .unwrap();
        assert_eq!(
            modal_action_at(&app, save.0 + 13, save.1),
            Some(ClickAction::Cancel)
        );
    }

    #[test]
    fn review_handoff_names_the_submitter_and_the_next_action() {
        let mut app = App::for_tests();
        app.mode = Mode::Workbench;
        let session = crate::Session::new(app.project.clone(), crate::Agent::Claude);
        let id = session.id.clone();
        app.sessions.push(session);
        let mut task = sample_task(1);
        task.events.push(crate::workbench::TaskHistory {
            at: 1,
            kind: crate::workbench::EventKind::HandedOff,
            session_id: id,
            note: Some("Please check the parser error path.".to_owned()),
        });
        app.tasks.push(task);
        let overview = screen(&app, 80, 24);
        assert!(
            overview.contains("Reviewer needed · from session claude"),
            "{overview}"
        );
        app.work.detail = true;
        let detail = screen(&app, 80, 24);
        assert!(detail.contains("choose an agent session to take the review"));
        assert!(detail.contains("[Take review]"));
        assert!(detail.contains("handed off for review · session claude"));
    }

    #[test]
    fn session_cards_make_recovery_boundary_visible() {
        let mut app = App::for_tests();
        app.mode = Mode::Workbench;
        app.work.focus = WorkFocus::Sessions;
        let mut session = crate::Session::new(app.project.clone(), crate::Agent::Codex);
        session.state = SessionState::Interrupted;
        app.sessions.push(session);
        let view = screen(&app, 80, 24);
        assert!(view.contains("View inbox · Recovery unconfirmed"));
        assert!(view.contains("[View inbox]"));
        app.sessions[0].state = SessionState::Recoverable;
        let missing = std::env::temp_dir().join(format!("verb-ui-missing-{}", std::process::id()));
        app.project = missing.clone();
        app.project_anchor = missing.clone();
        app.sessions[0].project_id = missing;
        let view = screen(&app, 80, 24);
        assert!(view.contains("View inbox · Checkout missing"));
        let existing = std::env::current_dir().unwrap();
        app.project = existing.clone();
        app.project_anchor = existing.clone();
        app.sessions[0].project_id = existing;
        let view = screen(&app, 80, 24);
        assert!(view.contains("Resume exact conversation"));
        assert!(view.contains("[Open session]"));
        app.sessions[0].state = SessionState::Live;
        let view = screen(&app, 80, 24);
        assert!(view.contains("View inbox · Running elsewhere"));
    }

    #[test]
    fn isolated_workspace_header_keeps_the_project_name() {
        let mut app = App::for_tests();
        app.mode = Mode::Workbench;
        app.project_anchor = "/tmp/my-project".into();
        app.project = "/tmp/verb/worktrees/abc123".into();
        app.git.branch = Some("verb/feature".to_owned());
        let view = screen(&app, 80, 24);
        assert!(
            view.contains("my-project · isolated / verb/feature"),
            "{view}"
        );
    }
}
