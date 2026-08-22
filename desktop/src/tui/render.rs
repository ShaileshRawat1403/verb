//! Drawing the workspace.
//!
//! Layout, in order and by weight: one status line, the terminal (everything left over), the
//! contextual band only when there is an observed fact to report, and the Ask region -- drawn, dim
//! and inactive until M2 can answer.

use super::context_view::{EvidenceLines, Kind};
use super::theme::{self, glyph, no_colour, space};
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

/// The complexity budget from `docs/UX_FOUNDATION.md`, in rows.
///
/// One status line, at most two lines of context band, one Ask line: four rows of chrome at the
/// absolute maximum, and two of those only when something actually happened. Everything else on the
/// screen belongs to the work. A change that raises this total is a change to the product's shape,
/// not a layout tweak.
const STATUS_ROWS: u16 = 1;
const ASK_ROWS: u16 = 1;
const BAND_ROWS: u16 = 2;
const MAX_CHROME_ROWS: u16 = STATUS_ROWS + BAND_ROWS + ASK_ROWS;

/// Draws the workspace and returns the rectangle the terminal was actually given.
///
/// The caller resizes the PTY to match. Making the *rendered* rectangle the authority is what keeps
/// a full-screen agent correct when Verb's own chrome changes height -- the contextual band
/// appearing takes two rows away from the terminal, and an agent that still believes it has them
/// draws its bottom rows into space that is no longer there.
/// Below this the layout stops being a layout: the terminal region would be smaller than the chrome
/// around it, which inverts the budget the whole design rests on.
const MINIMUM: (u16, u16) = (30, MAX_CHROME_ROWS * 3);

pub(super) fn workspace(frame: &mut Frame, app: &App) -> Rect {
    let area = frame.area();
    if area.width < MINIMUM.0 || area.height < MINIMUM.1 {
        // Said plainly rather than drawn badly. The session underneath is untouched and comes back
        // as soon as there is room.
        frame.render_widget(
            Paragraph::new(vec![
                Line::from("Terminal too small"),
                Line::from(Span::styled(
                    format!("Verb needs {}×{}", MINIMUM.0, MINIMUM.1),
                    theme::secondary(),
                )),
            ])
            .wrap(Wrap { trim: true }),
            area,
        );
        return Rect {
            x: area.x,
            y: area.y,
            width: area.width.max(1),
            height: area.height.max(1),
        };
    }

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
    if app.leader_pending() {
        // Pressing the leader shows what it can do. Nothing to memorise: the menu is one key away
        // and announces itself.
        leader_menu(frame, app, areas[3]);
    } else {
        ask(frame, areas[3]);
    }

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
        Surface::Evidence => evidence(frame, app),
        Surface::Welcome => welcome(frame, app),
        // The scrollback view is drawn as a bar over the terminal rather than a panel in front of
        // it: what is being looked at is the terminal itself.
        Surface::Scrollback {
            offset,
            search,
            last_search,
        } => scrollback_bar(
            frame,
            areas[1],
            *offset,
            search.as_deref(),
            last_search.as_deref(),
        ),
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
    let runtime =
        session.map(|session| session.runtime_id.as_deref().unwrap_or("shell").to_owned());
    let changes = match git.branch.as_deref() {
        Some(branch) if git.changed_files == 0 => Some(format!("{branch} clean")),
        Some(branch) => Some(format!("{branch}  {} changed", git.changed_files)),
        None => None,
    };

    // Everything except the path, which absorbs whatever is left.
    let reserved: usize = [
        hint.chars().count(),
        state
            .as_ref()
            .map_or(0, |value| value.chars().count() + space::FIELD_GAP),
        runtime
            .as_ref()
            .map_or(0, |value| value.chars().count() + space::FIELD_GAP),
        changes
            .as_ref()
            .map_or(0, |value| value.chars().count() + space::FIELD_GAP),
    ]
    .iter()
    .sum::<usize>()
        + 3;

    let path = crate::display_path(app.project());
    let (path, changes, runtime) = fit_status(path, changes, runtime, reserved, width);

    let mut spans = vec![
        Span::raw(space::MARGIN),
        Span::styled(path, theme::emphasis()),
    ];
    let gap = " ".repeat(space::FIELD_GAP);
    for detail in [changes, runtime].into_iter().flatten() {
        spans.push(Span::raw(gap.clone()));
        spans.push(Span::raw(detail));
    }
    if let Some(state) = session.map(|session| &session.state) {
        spans.push(Span::raw(gap.clone()));
        spans.push(state_span(state));
    }

    let used: usize = spans.iter().map(|span| span.content.chars().count()).sum();
    let padding = width.saturating_sub(used + hint.chars().count() + 1).max(1);
    spans.push(Span::raw(" ".repeat(padding)));
    spans.push(Span::styled(hint, theme::secondary()));

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

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
    let available = width.saturating_sub(
        reserved
            - changes
                .as_ref()
                .map_or(0, |value| value.chars().count() + space::FIELD_GAP),
    );
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
        return format!("{}{tail}", glyph::ELLIPSIS);
    }
    format!("{}/{kept}", glyph::ELLIPSIS)
}

fn status_state_label(state: &SessionState) -> String {
    format!("{} {}", state_glyph(state), plain_state(state))
}

/// The word a person would use. The contract's own term (`LIVE`, `RECOVERABLE`, …) is still what
/// `verb status`, `--json` and the durable record say -- this is the reading, not the vocabulary.
pub(crate) fn plain_state(state: &SessionState) -> &'static str {
    match state {
        SessionState::Live => "running",
        SessionState::Recoverable => "recoverable",
        SessionState::Interrupted => "checking",
        SessionState::Ended => "ended",
    }
}

fn state_glyph(state: &SessionState) -> &'static str {
    match state {
        SessionState::Live => glyph::RUNNING,
        SessionState::Recoverable => glyph::RECOVERABLE,
        SessionState::Interrupted => glyph::CHECKING,
        SessionState::Ended => glyph::ENDED,
    }
}

/// A recorded LIVE is shown with a question mark for the same reason `verb sessions` prints one:
/// while Verb hosts the process it can see it, but the record alone never proves it.
fn state_span(state: &SessionState) -> Span<'static> {
    // Colour repeats what the glyph and the word already say, so the row survives NO_COLOR and a
    // reader who cannot tell the hues apart.
    let style = match state {
        SessionState::Live => theme::success(),
        SessionState::Recoverable => theme::attention(),
        SessionState::Interrupted | SessionState::Ended => theme::unconfirmed(),
    };
    Span::styled(
        format!("{} {}", state_glyph(state), plain_state(state)),
        style,
    )
}

/// The hosted session's screen, cell for cell.
fn terminal(frame: &mut Frame, app: &App, area: Rect) {
    let Some(hosted) = app.hosted() else {
        let leader = app.leader().chord();
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    "  No session running.".to_owned(),
                    theme::secondary(),
                )),
                Line::from(Span::styled(
                    format!("  {leader} p  to start one"),
                    theme::secondary(),
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
            // An empty cell is a space rather than nothing: vt100 reports "" for a cell that has
            // never been written, and a zero-width symbol would leave whatever was drawn there
            // before.
            let symbol: &str = if contents.is_empty() { " " } else { contents };
            target.set_symbol(symbol);
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
        return glyph::ELLIPSIS.to_owned();
    }
    let kept: String = text.chars().take(width - 1).collect();
    format!("{kept}{}", glyph::ELLIPSIS)
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

/// Only from an observed fact. There is no variant here for a suspicion, by construction.
fn context_band(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines = Vec::new();

    if let Some(message) = app.message() {
        lines.push(Line::from(Span::styled(
            format!("  {message}"),
            theme::danger(),
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
                    format!("  {} {}", glyph::FAILED, failure_subject(label.as_deref())),
                    theme::danger(),
                ),
                Span::raw(format!(
                    " {} exit {exit_code} {} {}",
                    glyph::SEPARATOR,
                    glyph::SEPARATOR,
                    duration(*millis)
                )),
            ]));
            if let Some(note) = failure_note(label.as_deref()) {
                lines.push(Line::from(Span::styled(
                    format!("  {note}"),
                    theme::secondary(),
                )));
            }
        }
        // The session's own exit status, which is a fact about the session and needs no note about
        // command text -- that caveat belongs to a failed command, and repeating it here was left
        // over from an earlier version.
        Context::SessionEnded { exit_code } => {
            lines.push(Line::from(Span::raw(format!(
                "  {} Session ended {} exit {exit_code}",
                glyph::FAILED,
                glyph::SEPARATOR
            ))));
        }
        Context::SessionState(state) => {
            let leader = app.leader().chord();
            lines.push(Line::from(vec![
                Span::raw(space::INDENT),
                state_span(state),
            ]));
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
                    theme::secondary(),
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
        Span::styled(" Ask Verb…", theme::secondary()),
        Span::styled("   available in M2", theme::secondary()),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

/// What the leader can do, shown the moment it is pressed.
fn leader_menu(frame: &mut Frame, app: &App, area: Rect) {
    let leader = app.leader().chord();
    let line = Line::from(vec![
        Span::styled(format!(" {leader} "), theme::selected()),
        Span::raw("  p palette   s sessions   v what Verb knows   [ scroll back   ? help"),
        Span::styled("   esc cancel".to_owned(), theme::secondary()),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

/// The scrollback bar, drawn along the bottom of the terminal region.
fn scrollback_bar(
    frame: &mut Frame,
    terminal: Rect,
    offset: usize,
    search: Option<&str>,
    last_search: Option<&str>,
) {
    let area = Rect {
        x: terminal.x,
        y: terminal.y + terminal.height.saturating_sub(1),
        width: terminal.width,
        height: 1,
    };
    let line = match search {
        Some(term) => Line::from(vec![
            Span::styled(" search ".to_owned(), theme::selected()),
            Span::raw(format!(" /{term}▌")),
            Span::styled(
                "   enter to find · esc to cancel".to_owned(),
                theme::secondary(),
            ),
        ]),
        None => {
            let position = if offset == 0 {
                "at the latest output".to_owned()
            } else {
                format!("{offset} lines back")
            };
            let repeat = if last_search.is_some() {
                " · n next match"
            } else {
                ""
            };
            Line::from(vec![
                Span::styled(" scrolling ".to_owned(), theme::selected()),
                Span::raw(format!(" {position}")),
                Span::styled(
                    format!("   ↑↓ or wheel · / search{repeat} · g latest · esc back to live"),
                    theme::secondary(),
                ),
            ])
        }
    };
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(line), area);
}

/// Shown once, on a first run. Everything here is a fact about how Verb behaves, not a tour.
fn welcome(frame: &mut Frame, app: &App) {
    let leader = app.leader().chord();
    let inner = overlay(frame, "Welcome to Verb", 14);
    let lines = vec![
        Line::from("This is your terminal. Verb watches the session around it —"),
        Line::from("what ran, what failed, what can be resumed."),
        Line::from(""),
        Line::from(vec![
            Span::raw("One key opens everything:  "),
            Span::styled(format!("{leader}"), theme::emphasis()),
        ]),
        Line::from(format!("  {leader} p   commands, by name")),
        Line::from(format!("  {leader} v   what Verb has observed")),
        Line::from(format!("  {leader} s   sessions across projects")),
        Line::from(format!("  {leader} [   look back through output")),
        Line::from(format!("  {leader} ?   help")),
        Line::from(""),
        Line::from(Span::styled(
            format!("Every other key goes to the terminal, including {leader} {leader}."),
            theme::secondary(),
        )),
        Line::from(Span::styled(
            "Press any key to start.".to_owned(),
            theme::secondary(),
        )),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

/// What Verb has observed, rendered from the same assembly `verb context` prints.
fn evidence(frame: &mut Frame, app: &App) {
    let built = match crate::context::assemble_for(
        app.project(),
        app.hosted().map(|hosted| &hosted.session),
    ) {
        Ok(context) => EvidenceLines::build(&context, crate::now_millis()),
        Err(error) => {
            let inner = overlay(frame, "What Verb has observed", 5);
            frame.render_widget(
                Paragraph::new(format!("Could not assemble: {error}")).wrap(Wrap { trim: true }),
                inner,
            );
            return;
        }
    };

    let inner = overlay(
        frame,
        "What Verb has observed",
        built.lines.len() as u16 + 4,
    );
    let lines: Vec<Line> = built
        .lines
        .iter()
        .map(|(kind, text)| {
            let style = match kind {
                Kind::Heading => theme::emphasis(),
                Kind::Caveat => theme::secondary(),
                Kind::Fact | Kind::Empty => Style::default(),
            };
            Line::from(Span::styled(truncate(text, inner.width as usize), style))
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

/// Which overlay row the pointer is over, if any -- so a click selects what it is pointing at and a
/// click elsewhere closes the overlay.
pub(super) fn overlay_row_at(surface: &Surface, sessions: usize, row: u16) -> Option<usize> {
    let (count, first_row) = match surface {
        // Rows start after the border and the filter line.
        Surface::Palette { filter, .. } => (palette_entries(filter).len(), 3_u16),
        Surface::Sessions { .. } => (sessions, 2_u16),
        _ => return None,
    };
    if count == 0 || row < first_row {
        return None;
    }
    let index = (row - first_row) as usize;
    (index < count).then_some(index)
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
            label: "What Verb has observed here",
            action: Action::Evidence,
        },
        Entry {
            label: "Look back through earlier output",
            action: Action::Scrollback,
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
    // Subsequence matching, so "nc" finds "New Claude session" -- typing the initials of what you
    // want is how people actually use a palette.
    let mut scored: Vec<(usize, Entry)> = all
        .into_iter()
        .filter_map(|entry| fuzzy_score(entry.label, filter).map(|score| (score, entry)))
        .collect();
    scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    scored.into_iter().map(|(_, entry)| entry).collect()
}

/// `None` when the filter is not a subsequence of the label. Higher is better: consecutive matches
/// and matches at word starts score above scattered ones, so an exact prefix wins.
pub(super) fn fuzzy_score(label: &str, filter: &str) -> Option<usize> {
    let label_lower = label.to_lowercase();
    let mut characters = label_lower.char_indices().peekable();
    let mut score = 0;
    let mut last_index: Option<usize> = None;

    for wanted in filter.to_lowercase().chars() {
        if wanted == ' ' {
            continue;
        }
        loop {
            let (index, character) = characters.next()?;
            if character == wanted {
                score += 1;
                if last_index == Some(index.saturating_sub(1)) {
                    score += 3; // consecutive
                }
                if index == 0 || label_lower.as_bytes().get(index - 1) == Some(&b' ') {
                    score += 2; // start of a word
                }
                last_index = Some(index);
                break;
            }
        }
    }
    Some(score)
}

fn overlay(frame: &mut Frame, title: &str, height: u16) -> Rect {
    let area = frame.area();
    let width = area.width.saturating_sub(8).clamp(24, 72);
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
        Line::from(Span::styled(format!("> {filter}"), theme::emphasis())),
        Line::from(""),
    ];
    for (index, entry) in entries.iter().enumerate() {
        // Truncated, not wrapped: an entry that folds onto a second line reads as two entries. The
        // selection is marked by a glyph as well as by reverse video, so it survives NO_COLOR and a
        // terminal that renders reverse poorly.
        let marker = if index == selected {
            format!("{} ", glyph::CURSOR)
        } else {
            space::INDENT.to_owned()
        };
        lines.push(Line::from(Span::styled(
            truncate(&format!("{marker}{}", entry.label), inner.width as usize),
            if index == selected {
                theme::selected()
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
        let marker = if index == selected {
            format!("{} ", glyph::CURSOR)
        } else {
            space::INDENT.to_owned()
        };
        let line = format!(
            "{marker}{:<13} {:<9} {:>8}  {}",
            // "unconfirmed" everywhere else, because a record alone never proves a process. Here
            // Verb is holding that process, so for this one row the doubt would be false modesty.
            match session.state {
                SessionState::Live if hosted == Some(session.id.as_str()) => "running".to_owned(),
                SessionState::Live => "running?".to_owned(),
                ref other => plain_state(other).to_owned(),
            },
            session.runtime_id.as_deref().unwrap_or("shell"),
            crate::relative_time(now.saturating_sub(session.last_seen_at)),
            crate::display_path(&session.project_id)
        );
        lines.push(Line::from(Span::styled(
            truncate(&line, inner.width as usize),
            if index == selected {
                theme::selected()
            } else {
                Style::default()
            },
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "enter resume · n new session · x forget record · esc close",
        theme::secondary(),
    )));
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn help(frame: &mut Frame, app: &App) {
    let inner = overlay(frame, "Verb keys", 12);
    let leader = app.leader().chord();
    let mut lines = vec![
        Line::from(Span::raw(app.leader().hint())),
        Line::from(""),
        Line::from(Span::raw(format!("{leader} p    command palette"))),
        Line::from(Span::raw(format!("{leader} s    sessions"))),
        Line::from(Span::raw(format!("{leader} v    what Verb has observed"))),
        Line::from(Span::raw(format!("{leader} [    look back through output"))),
        Line::from(Span::raw(format!("{leader} ?    this help"))),
        Line::from(Span::raw(format!(
            "{leader} {leader}    send {leader} to the terminal"
        ))),
        Line::from(""),
        Line::from(Span::styled(
            "Every other key belongs to the terminal.".to_owned(),
            theme::secondary(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "running · recoverable · checking · ended  are Verb's session states;".to_owned(),
            theme::secondary(),
        )),
        Line::from(Span::styled(
            "`verb status` prints the same four as LIVE, RECOVERABLE, INTERRUPTED, ENDED."
                .to_owned(),
            theme::secondary(),
        )),
    ];
    if app.leader().is_provisional() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Provisional (shown as *): collision testing has not settled a default yet. \
             Set VERB_LEADER to change it."
                .to_owned(),
            theme::secondary(),
        )));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chrome_never_costs_more_than_the_budget_allows() {
        // Quiet costs two rows; a moment costs four; nothing costs more.
        assert_eq!(MAX_CHROME_ROWS, 4);
        assert_eq!(STATUS_ROWS + ASK_ROWS, 2, "quiet chrome");

        // 90/10 is the target the usual sizes converge to, not a promise at every size. The
        // guarantee is weaker and always true: the work keeps the majority of the screen, even at
        // the minimum with a contextual moment open.
        for height in [12_u16, 24, 30, 40, 120] {
            let quiet = terminal_rows(height);
            assert_eq!(quiet, height - STATUS_ROWS - ASK_ROWS, "at {height} rows");
            assert!(
                quiet * 10 >= height * 8,
                "quiet should leave at least 80% of {height} rows to the work, got {quiet}"
            );

            let moment = height - MAX_CHROME_ROWS;
            assert!(
                moment * 2 > height,
                "even a contextual moment should leave the majority of {height} rows, got {moment}"
            );
        }

        // And at the sizes people actually work in, it is the target rather than merely a majority.
        assert!(terminal_rows(40) * 10 >= 40 * 9);
    }

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
    fn the_palette_matches_the_way_people_type_at_one() {
        // Initials, not substrings: "ncs" should find "New Claude session".
        let entries = palette_entries("ncs");
        assert_eq!(
            entries.first().map(|entry| entry.label),
            Some("New Claude session"),
            "{:?}",
            entries.iter().map(|entry| entry.label).collect::<Vec<_>>()
        );
        // A filter that is not a subsequence of anything matches nothing.
        assert!(palette_entries("zzz").is_empty());
        // An exact prefix outranks a scattered match.
        assert!(fuzzy_score("New shell session", "new") > fuzzy_score("Quit Verb", "n"));
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
