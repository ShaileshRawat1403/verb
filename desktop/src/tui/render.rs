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
///
/// The line is built to a budget rather than assembled and hoped for. On a narrow terminal the
/// first things to disappear were the session state and the leader hint -- the two things a person
/// most needs -- because they were last in the string. Now the path shortens first, then the
/// optional details drop in reverse order of usefulness, and state and leader survive to the end.
fn status(frame: &mut Frame, app: &App, area: Rect) {
    let git = crate::git_snapshot(app.project());
    let session = app.hosted().map(|hosted| &hosted.session);
    let width = area.width as usize;

    let hint = leader_hint(app, width);
    let state = session.map(|session| status_state_label(&session.state));
    let runtime = session.map(|session| session.runtime_id.as_deref().unwrap_or("shell").to_owned());
    let changes = match git.branch.as_deref() {
        Some(branch) if git.changed_files == 0 => Some(format!("{branch} clean")),
        Some(branch) => Some(format!("{branch}  {} changed", git.changed_files)),
        None => None,
    };

    // Everything except the path, which absorbs whatever is left.
    let reserved: usize = [
        hint.chars().count(),
        state.as_ref().map_or(0, |value| value.chars().count() + SEPARATOR),
        runtime.as_ref().map_or(0, |value| value.chars().count() + SEPARATOR),
        changes.as_ref().map_or(0, |value| value.chars().count() + SEPARATOR),
    ]
    .iter()
    .sum::<usize>()
        + 3;

    let path = crate::display_path(app.project());
    let (path, changes, runtime) = fit_status(path, changes, runtime, reserved, width);

    let mut spans = vec![
        Span::raw(" "),
        Span::styled(path, Style::default().add_modifier(Modifier::BOLD)),
    ];
    for detail in [changes, runtime].into_iter().flatten() {
        spans.push(Span::raw("   "));
        spans.push(Span::raw(detail));
    }
    if let Some(state) = session.map(|session| &session.state) {
        spans.push(Span::raw("   "));
        spans.push(state_span(state));
    }

    let used: usize = spans.iter().map(|span| span.content.chars().count()).sum();
    let padding = width.saturating_sub(used + hint.chars().count() + 1).max(1);
    spans.push(Span::raw(" ".repeat(padding)));
    spans.push(Span::styled(hint, Style::default().add_modifier(Modifier::DIM)));

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

const SEPARATOR: usize = 3;

/// Decides what survives on a narrow status line: shorten the path first, then drop the
/// changed-files detail, then the runtime, keeping the leader hint and session state intact.
pub(super) fn fit_status(
    path: String,
    changes: Option<String>,
    runtime: Option<String>,
    reserved: usize,
    width: usize,
) -> (String, Option<String>, Option<String>) {
    let available = width.saturating_sub(reserved);
    if path.chars().count() <= available {
        return (path, changes, runtime);
    }

    let shortened = shorten_path(&path, available);
    if shortened.chars().count() <= available {
        return (shortened, changes, runtime);
    }
    // Still too tight: give up the details rather than the path, which says where you are.
    let available = width.saturating_sub(reserved - changes.as_ref().map_or(0, |value| value.chars().count() + SEPARATOR));
    let shortened = shorten_path(&path, available);
    if shortened.chars().count() <= available {
        return (shortened, None, runtime);
    }
    (shorten_path(&path, width.saturating_sub(4)), None, None)
}

/// Keeps the end of a path, which is the part that identifies the project.
pub(super) fn shorten_path(path: &str, available: usize) -> String {
    if path.chars().count() <= available || available < 4 {
        return path.to_owned();
    }
    let mut kept = String::new();
    for segment in path.rsplit('/') {
        let candidate = if kept.is_empty() {
            segment.to_owned()
        } else {
            format!("{segment}/{kept}")
        };
        // +2 for the leading ellipsis
        if candidate.chars().count() + 2 > available {
            break;
        }
        kept = candidate;
    }
    if kept.is_empty() {
        let tail: String = path
            .chars()
            .rev()
            .take(available.saturating_sub(1))
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        return format!("…{tail}");
    }
    format!("…/{kept}")
}

fn status_state_label(state: &SessionState) -> String {
    match state {
        SessionState::Live => "● LIVE".to_owned(),
        SessionState::Recoverable => "◐ RECOVERABLE".to_owned(),
        SessionState::Interrupted => "◌ INTERRUPTED".to_owned(),
        SessionState::Ended => "○ ENDED".to_owned(),
    }
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
        let leader = app.leader().chord();
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    "  No session running.".to_owned(),
                    Style::default().add_modifier(Modifier::DIM),
                )),
                Line::from(Span::styled(
                    format!("  {leader} p  to start one"),
                    Style::default().add_modifier(Modifier::DIM),
                )),
            ]),
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

    // Not while a Verb surface is in front: a cursor blinking in the terminal underneath an open
    // palette says the keyboard is going somewhere it is not.
    if !screen.hide_cursor() && matches!(app.surface(), Surface::None) {
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

/// What the band names: the command the shell reported, or the fact alone.
pub(super) fn failure_subject(label: Option<&str>) -> &str {
    label.unwrap_or("Command failed")
}

/// A shell that reported no command line leaves an absence, and the band says so rather than
/// letting the user wonder why Verb is being vague.
pub(super) fn failure_note(label: Option<&str>) -> Option<&'static str> {
    label
        .is_none()
        .then_some("The shell did not report what was running.")
}

/// Cuts to a width, with an ellipsis so the cut is visible rather than mysterious.
pub(super) fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    if width <= 1 {
        return "…".to_owned();
    }
    let kept: String = text.chars().take(width - 1).collect();
    format!("{kept}…")
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
        Context::CommandFailed {
            exit_code,
            millis,
            label,
        } => {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  ✕ {}", failure_subject(label.as_deref())),
                    Style::default().fg(if no_colour() { Color::Reset } else { Color::Red }),
                ),
                Span::raw(format!(" · exit {exit_code} · {}", duration(*millis))),
            ]));
            if let Some(note) = failure_note(label.as_deref()) {
                lines.push(Line::from(Span::styled(
                    format!("  {note}"),
                    Style::default().add_modifier(Modifier::DIM),
                )));
            }
        }
        // The session's own exit status, which is a fact about the session and needs no note about
        // command text -- that caveat belongs to a failed command, and repeating it here was left
        // over from an earlier version.
        Context::SessionEnded { exit_code } => {
            lines.push(Line::from(Span::raw(format!(
                "  ✕ Session ended · exit {exit_code}"
            ))));
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
            // Truncated, not wrapped: an entry that folds onto a second line reads as two entries.
            truncate(&format!("  {}", entry.label), inner.width as usize),
            if index == selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            },
        )));
    }
    frame.render_widget(Paragraph::new(lines), inner);
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
            truncate(&line, inner.width as usize),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_narrow_status_line_keeps_what_matters_and_shortens_the_path() {
        // The leader hint and session state are the two things a person needs; they must not be the
        // first casualties of a small terminal.
        let (path, changes, runtime) = fit_status(
            "~/work/some/deeply/nested/project".to_owned(),
            Some("main  4 changed".to_owned()),
            Some("claude".to_owned()),
            40,
            60,
        );
        assert!(path.chars().count() <= 20, "{path}");
        assert!(path.ends_with("project"), "{path}");
        assert!(changes.is_some() || runtime.is_some());
    }

    #[test]
    fn a_shortened_path_keeps_the_end_because_that_is_the_project() {
        assert_eq!(shorten_path("/a/b/c", 40), "/a/b/c");
        let short = shorten_path("/Users/example/work/deeply/nested/project", 20);
        assert!(short.ends_with("project"), "{short}");
        assert!(short.starts_with('…'), "{short}");
        assert!(short.chars().count() <= 20, "{short}");
    }

    #[test]
    fn overlay_rows_are_cut_rather_than_folded_onto_a_second_line() {
        assert_eq!(truncate("New OpenCode session", 12), "New OpenCod…");
        assert_eq!(truncate("short", 12), "short");
    }

    #[test]
    fn the_band_names_the_command_when_the_shell_reported_one() {
        assert_eq!(failure_subject(Some("npm test")), "npm test");
        assert_eq!(failure_note(Some("npm test")), None);
    }

    #[test]
    fn without_a_reported_command_the_band_states_the_fact_and_the_absence() {
        assert_eq!(failure_subject(None), "Command failed");
        assert!(failure_note(None).is_some());
    }

    #[test]
    fn durations_read_the_way_a_person_would_say_them() {
        assert_eq!(duration(0), "0ms");
        assert_eq!(duration(3_800), "3.8s");
        assert_eq!(duration(125_000), "2m 5s");
    }
}
