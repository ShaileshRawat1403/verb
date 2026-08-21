//! Drawing the workspace.
//!
//! Layout, in order and by weight: one status line, the terminal (everything left over), the
//! contextual band only when there is an observed fact to report, and the Ask region -- drawn, dim
//! and inactive until M2 can answer.

use super::{Action, App, Context, Surface};
use crate::{Agent, Session, SessionState};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

/// How many rows the terminal region gets for a given terminal height. Kept here so the PTY is
/// always told the size it is actually drawn at.
///
/// Never returns zero. Some pty allocations report no size at all, and a terminal with no rows is
/// not a smaller terminal -- it is an invalid one that takes the emulator down with it.
pub(super) fn terminal_rows(height: u16) -> u16 {
    height.saturating_sub(STATUS_ROWS + ASK_ROWS).max(1)
}

/// The size to host a session at, given what the terminal claims. A terminal reporting nothing gets
/// the conventional 80x24 rather than an impossible zero.
pub(super) fn hosting_size(width: u16, height: u16) -> (u16, u16) {
    if width == 0 || height == 0 {
        return (24, 80);
    }
    (terminal_rows(height), width.max(1))
}

/// Long form when it fits, then the short form, then nothing rather than a half-word.
pub(super) fn leader_hint(app: &App, available: usize) -> String {
    let long = app.leader().hint();
    if long.chars().count() <= available {
        return long;
    }
    // The short form keeps a marker for a provisional binding: a user must never see a leader
    // presented as settled when it is not. `Leader ?` spells the asterisk out.
    let short = if app.leader().is_provisional() {
        format!("leader {}*", app.leader().chord())
    } else {
        format!("leader {}", app.leader().chord())
    };
    if short.chars().count() <= available {
        return short;
    }
    let bare = format!("{}", app.leader().chord());
    if bare.chars().count() <= available {
        return bare;
    }
    String::new()
}

const STATUS_ROWS: u16 = 1;
const ASK_ROWS: u16 = 1;
const BAND_ROWS: u16 = 2;

/// Draws the workspace and returns the rectangle the terminal was actually given.
///
/// The caller resizes the PTY to match. Making the *rendered* rectangle the authority is what keeps
/// a full-screen agent correct when Verb's own chrome changes height -- the contextual band
/// appearing takes two rows away from the terminal, and an agent that still believes it has them
/// draws its bottom rows into space that is no longer there.
pub(super) fn workspace(frame: &mut Frame, app: &App) -> Rect {
    let band = if app.message().is_some() || !matches!(app.context(), Context::None) {
        BAND_ROWS
    } else {
        0
    };

    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(STATUS_ROWS),
            Constraint::Min(1),
            Constraint::Length(band),
            Constraint::Length(ASK_ROWS),
        ])
        .split(frame.area());

    status(frame, app, areas[0]);
    terminal(frame, app, areas[1]);
    if band > 0 {
        context_band(frame, app, areas[2]);
    }
    ask(frame, areas[3]);

    let terminal_area = areas[1];

    match app.surface() {
        Surface::None => {}
        Surface::Palette { filter, selected } => palette(frame, filter, *selected),
        Surface::Sessions { selected } => sessions(
            frame,
            app.sessions(),
            *selected,
            app.hosted().map(|hosted| hosted.session.id.as_str()),
        ),
        Surface::Help => help(frame, app),
    }

    terminal_area
}

/// project · branch · changes · runtime · session state · leader hint.
fn status(frame: &mut Frame, app: &App, area: Rect) {
    let git = crate::git_snapshot(app.project());
    let session = app.hosted().map(|hosted| &hosted.session);

    let mut spans = vec![
        Span::styled(
            crate::display_path(app.project()),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::raw(match git.branch.as_deref() {
            Some(branch) if git.changed_files == 0 => format!("{branch} clean"),
            Some(branch) => format!("{branch}  {} changed", git.changed_files),
            None => "no repository".to_owned(),
        }),
    ];

    if let Some(session) = session {
        spans.push(Span::raw("   "));
        spans.push(Span::raw(
            session.runtime_id.as_deref().unwrap_or("shell").to_owned(),
        ));
        spans.push(Span::raw("   "));
        spans.push(state_span(&session.state));
    }

    // The leader hint is the one thing on this line a user cannot afford to have truncated -- it is
    // how they find Verb at all -- so it is fitted to the space that is left, long form first.
    let used: usize = spans.iter().map(|span| span.content.chars().count()).sum();
    let available = (area.width as usize).saturating_sub(used + 2);
    let hint = leader_hint(app, available);
    let padding = available.saturating_sub(hint.chars().count()).max(1);
    spans.push(Span::raw(" ".repeat(padding)));
    spans.push(Span::styled(hint, Style::default().add_modifier(Modifier::DIM)));

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// A recorded LIVE is shown with a question mark for the same reason `verb sessions` prints one:
/// while Verb hosts the process it can see it, but the record alone never proves it.
fn state_span(state: &SessionState) -> Span<'static> {
    let (glyph, label, colour) = match state {
        SessionState::Live => ("●", "LIVE", Color::Green),
        SessionState::Recoverable => ("◐", "RECOVERABLE", Color::Yellow),
        SessionState::Interrupted => ("◌", "INTERRUPTED", Color::Gray),
        SessionState::Ended => ("○", "ENDED", Color::Gray),
    };
    Span::styled(format!("{glyph} {label}"), Style::default().fg(colour))
}

/// The hosted session's screen, cell for cell.
fn terminal(frame: &mut Frame, app: &App, area: Rect) {
    let Some(hosted) = app.hosted() else {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  No session running.",
                Style::default().add_modifier(Modifier::DIM),
            ))),
            area,
        );
        return;
    };

    let screen = hosted.screen();
    let buffer = frame.buffer_mut();
    for row in 0..area.height {
        for column in 0..area.width {
            let Some(cell) = screen.cell(row, column) else {
                continue;
            };
            let target = &mut buffer[(area.x + column, area.y + row)];
            let contents = cell.contents();
            target.set_symbol(if contents.is_empty() { " " } else { &contents });
            target.set_style(cell_style(cell));
        }
    }

    if !screen.hide_cursor() {
        let (row, column) = screen.cursor_position();
        if row < area.height && column < area.width {
            frame.set_cursor_position((area.x + column, area.y + row));
        }
    }
}

fn cell_style(cell: &vt100::Cell) -> Style {
    let mut style = Style::default()
        .fg(convert(cell.fgcolor(), Color::Reset))
        .bg(convert(cell.bgcolor(), Color::Reset));
    if cell.bold() {
        style = style.add_modifier(Modifier::BOLD);
    }
    if cell.italic() {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if cell.underline() {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    if cell.inverse() {
        style = style.add_modifier(Modifier::REVERSED);
    }
    style
}

fn convert(colour: vt100::Color, default: Color) -> Color {
    if no_colour() {
        return Color::Reset;
    }
    match colour {
        vt100::Color::Default => default,
        vt100::Color::Idx(index) => Color::Indexed(index),
        vt100::Color::Rgb(red, green, blue) => Color::Rgb(red, green, blue),
    }
}

/// Human-sized durations: a command that took 3.8 seconds should not read as 3800ms.
pub(super) fn duration(millis: u128) -> String {
    if millis < 1_000 {
        format!("{millis}ms")
    } else if millis < 60_000 {
        format!("{:.1}s", millis as f64 / 1_000.0)
    } else {
        format!("{}m {}s", millis / 60_000, (millis % 60_000) / 1_000)
    }
}

/// `NO_COLOR` (no-color.org), honoured for the chrome and for the hosted screen alike.
pub(super) fn no_colour() -> bool {
    std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty())
}

/// Only from an observed fact. There is no variant here for a suspicion, by construction.
fn context_band(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines = Vec::new();

    if let Some(message) = app.message() {
        lines.push(Line::from(Span::styled(
            format!("  {message}"),
            Style::default().fg(if no_colour() { Color::Reset } else { Color::Red }),
        )));
    }

    match app.context() {
        Context::None => {}
        Context::CommandFailed { exit_code, millis } => {
            lines.push(Line::from(vec![
                Span::styled(
                    "  ✕ Command failed".to_owned(),
                    Style::default().fg(if no_colour() { Color::Reset } else { Color::Red }),
                ),
                Span::raw(format!(" · exit {exit_code} · {}", duration(*millis))),
            ]));
            lines.push(Line::from(Span::styled(
                "  Verb records that a command failed, never what was typed.".to_owned(),
                Style::default().add_modifier(Modifier::DIM),
            )));
        }
        Context::SessionEnded { exit_code } => {
            lines.push(Line::from(Span::raw(format!(
                "  ✕ Session ended · exit {exit_code}"
            ))));
            lines.push(Line::from(Span::styled(
                "  Verb records that it failed, not what was typed.".to_owned(),
                Style::default().add_modifier(Modifier::DIM),
            )));
        }
        Context::SessionState(state) => {
            let leader = app.leader().chord();
            lines.push(Line::from(vec![Span::raw("  "), state_span(state)]));
            // Only the action this state actually justifies, which is the same rule the CLI follows
            // when it refuses to resume something that is not recoverable.
            let hint = match state {
                SessionState::Recoverable => Some(format!("{leader} s  resume this conversation")),
                SessionState::Ended => Some(format!("{leader} p  start a new session")),
                SessionState::Interrupted => {
                    Some("recovery status unknown; nothing to resume yet".to_owned())
                }
                SessionState::Live => None,
            };
            if let Some(hint) = hint {
                lines.push(Line::from(Span::styled(
                    format!("  {hint}"),
                    Style::default().add_modifier(Modifier::DIM),
                )));
            }
        }
    }

    frame.render_widget(Paragraph::new(lines), area);
}

/// Reserved space, deliberately inactive: M1 has nothing behind it, and an input that swallows
/// questions nothing answers is exactly the ambiguity Verb exists to remove.
fn ask(frame: &mut Frame, area: Rect) {
    let line = Line::from(vec![
        Span::styled(
            " Ask Verb…",
            Style::default().add_modifier(Modifier::DIM),
        ),
        Span::styled(
            "   available in M2",
            Style::default().add_modifier(Modifier::DIM),
        ),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

pub(crate) struct Entry {
    pub label: &'static str,
    pub action: Action,
}

/// Everything Verb can do, by name. Each entry is an existing capability; nothing here is a stub.
pub(crate) fn palette_entries(filter: &str) -> Vec<Entry> {
    let all = vec![
        Entry {
            label: "Resume session in this project",
            action: Action::Resume,
        },
        Entry {
            label: "New shell session",
            action: Action::NewShell,
        },
        Entry {
            label: "New Claude session",
            action: Action::NewAgent(Agent::Claude),
        },
        Entry {
            label: "New Codex session",
            action: Action::NewAgent(Agent::Codex),
        },
        Entry {
            label: "New OpenCode session",
            action: Action::NewAgent(Agent::OpenCode),
        },
        Entry {
            label: "Sessions across projects",
            action: Action::Sessions,
        },
        Entry {
            label: "Re-check recovery from agent evidence",
            action: Action::Reconcile,
        },
        Entry {
            label: "Help — Verb keys",
            action: Action::Help,
        },
        Entry {
            label: "Quit Verb",
            action: Action::Quit,
        },
    ];
    if filter.is_empty() {
        return all;
    }
    let needle = filter.to_lowercase();
    all.into_iter()
        .filter(|entry| entry.label.to_lowercase().contains(&needle))
        .collect()
}

fn overlay(frame: &mut Frame, title: &str, height: u16) -> Rect {
    let area = frame.area();
    let width = area.width.saturating_sub(8).min(72).max(24);
    let height = height.min(area.height.saturating_sub(4)).max(3);
    let rect = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" {title} ")),
        rect,
    );
    Rect {
        x: rect.x + 2,
        y: rect.y + 1,
        width: rect.width.saturating_sub(4),
        height: rect.height.saturating_sub(2),
    }
}

fn palette(frame: &mut Frame, filter: &str, selected: usize) {
    let entries = palette_entries(filter);
    let inner = overlay(frame, "Command Palette", entries.len() as u16 + 4);
    let mut lines = vec![
        Line::from(Span::styled(
            format!("> {filter}"),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    for (index, entry) in entries.iter().enumerate() {
        lines.push(Line::from(Span::styled(
            format!("  {}", entry.label),
            if index == selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            },
        )));
    }
    // trim: false keeps the entries indented under the filter line.
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn sessions(frame: &mut Frame, sessions: &[Session], selected: usize, hosted: Option<&str>) {
    let inner = overlay(frame, "Sessions", sessions.len() as u16 + 4);
    if sessions.is_empty() {
        frame.render_widget(
            Paragraph::new("No sessions yet. Run an agent in a project first."),
            inner,
        );
        return;
    }
    let now = crate::now_millis();
    let mut lines = Vec::new();
    for (index, session) in sessions.iter().enumerate() {
        let line = format!(
            "{:<13} {:<9} {:>8}  {}",
            // "live?" everywhere else, because a record alone never proves a process. Here Verb
            // is holding that process, so for this one row the question mark would be false
            // modesty.
            match session.state {
                SessionState::Live if hosted == Some(session.id.as_str()) => "live".to_owned(),
                SessionState::Live => "live?".to_owned(),
                ref other => other.as_str().to_owned(),
            },
            session.runtime_id.as_deref().unwrap_or("shell"),
            crate::relative_time(now.saturating_sub(session.last_seen_at)),
            crate::display_path(&session.project_id)
        );
        lines.push(Line::from(Span::styled(
            line,
            if index == selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            },
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "enter resume · n new session · esc close",
        Style::default().add_modifier(Modifier::DIM),
    )));
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn help(frame: &mut Frame, app: &App) {
    let inner = overlay(frame, "Verb keys", 12);
    let leader = app.leader().chord();
    let mut lines = vec![
        Line::from(Span::raw(format!("{}", app.leader().hint()))),
        Line::from(""),
        Line::from(Span::raw(format!("{leader} p    command palette"))),
        Line::from(Span::raw(format!("{leader} s    sessions"))),
        Line::from(Span::raw(format!("{leader} v    what Verb has observed"))),
        Line::from(Span::raw(format!("{leader} ?    this help"))),
        Line::from(Span::raw(format!("{leader} {leader}    send {leader} to the terminal"))),
        Line::from(""),
        Line::from(Span::styled(
            "Every other key belongs to the terminal.".to_owned(),
            Style::default().add_modifier(Modifier::DIM),
        )),
    ];
    if app.leader().is_provisional() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Provisional (shown as *): collision testing has not settled a default yet. \
             Set VERB_LEADER to change it."
                .to_owned(),
            Style::default().add_modifier(Modifier::DIM),
        )));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}
