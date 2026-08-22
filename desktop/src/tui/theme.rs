//! The workspace's visual vocabulary, in one place.
//!
//! Every style and symbol Verb draws is named here by *meaning* rather than appearance, so a surface
//! cannot invent its own dialect. See `docs/TUI_DESIGN.md` for the reasoning; the short version:
//!
//! * **Four roles, no palette.** Normal, secondary, attention, danger -- and success only for a
//!   state that is genuinely good. A terminal is not a canvas, and a fifth colour would be a
//!   decision no reader can decode.
//! * **Colour never carries meaning alone.** Anything coloured also has a word or a glyph, because
//!   `NO_COLOR` is honoured, terminals disagree about palettes, and some readers cannot see the
//!   difference.
//! * **Emphasis is structural.** Bold marks a heading or a key; reverse marks a selection or a mode
//!   badge. Italic and underline are not used: terminals treat them inconsistently and underline
//!   collides with hyperlink rendering.

use ratatui::style::{Color, Modifier, Style};

/// `NO_COLOR` (no-color.org). Honoured for the chrome and for the hosted screen alike.
pub(super) fn no_colour() -> bool {
    std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty())
}

fn coloured(colour: Color) -> Style {
    if no_colour() {
        Style::default()
    } else {
        Style::default().fg(colour)
    }
}

/// Secondary content: hints, captions, caveats, anything a reader may skip without losing a fact.
pub(super) fn secondary() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

/// A heading, or a key the reader is meant to press.
pub(super) fn emphasis() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

/// The selected row, or a mode badge. Never used for emphasis in running text.
pub(super) fn selected() -> Style {
    Style::default().add_modifier(Modifier::REVERSED)
}

/// Something needs a decision: a recoverable session, a pending state.
pub(super) fn attention() -> Style {
    coloured(Color::Yellow)
}

/// Something failed. Always accompanied by a word -- "failed", "exit 1" -- never colour alone.
pub(super) fn danger() -> Style {
    coloured(Color::Red)
}

/// Something is running, and Verb can see it.
pub(super) fn success() -> Style {
    coloured(Color::Green)
}

/// Content Verb is reporting but cannot currently confirm.
pub(super) fn unconfirmed() -> Style {
    if no_colour() {
        Style::default()
    } else {
        Style::default().fg(Color::Gray)
    }
}

/// The symbols the workspace uses, and nowhere else invents.
///
/// ASCII-safe by choice of a narrow set: every glyph here is single-width in a monospace terminal,
/// which emoji are not -- a double-width character in a fixed grid breaks every column after it.
pub(super) mod glyph {
    /// Session states, in the order they appear in the contract.
    pub(in crate::tui) const RUNNING: &str = "●";
    pub(in crate::tui) const RECOVERABLE: &str = "◐";
    pub(in crate::tui) const CHECKING: &str = "◌";
    pub(in crate::tui) const ENDED: &str = "○";

    /// A failure.
    pub(in crate::tui) const FAILED: &str = "✕";
    /// The selected row.
    pub(in crate::tui) const CURSOR: &str = "▸";
    /// Content continues past the edge.
    pub(in crate::tui) const ELLIPSIS: &str = "…";
    /// Between facts on one line. Prose uses an em dash; data uses this.
    pub(in crate::tui) const SEPARATOR: &str = "·";
    // There is deliberately no rule glyph. Only overlays draw borders; the workspace separates its
    // regions by position and emphasis, and a drawn line would cost a row of the terminal to say
    // something the layout already says.
}

/// Spacing, so indentation means the same thing on every surface.
pub(super) mod space {
    /// Left margin for a line of content in a region or an overlay.
    pub(in crate::tui) const MARGIN: &str = " ";
    /// A fact indented under a heading.
    pub(in crate::tui) const INDENT: &str = "  ";
    /// Between fields on the status line: wide enough to read as a gap, not a gutter.
    pub(in crate::tui) const FIELD_GAP: usize = 3;
}
