//! Translate a click inside a Verb pane into the coordinates and protocol its hosted TUI asked for.
//! Verb keeps clicks for pane focus when the child has not enabled mouse tracking.

use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use vt100::{MouseProtocolEncoding, MouseProtocolMode, Screen};

fn button_code(button: MouseButton) -> u16 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
    }
}

pub(super) fn encode(screen: &Screen, event: MouseEvent, content: Rect) -> Option<Vec<u8>> {
    let mode = screen.mouse_protocol_mode();
    if mode == MouseProtocolMode::None
        || event.column < content.x
        || event.row < content.y
        || event.column >= content.x + content.width
        || event.row >= content.y + content.height
    {
        return None;
    }

    let (base, release) = match event.kind {
        MouseEventKind::Down(button) => (button_code(button), false),
        MouseEventKind::Up(button) if mode != MouseProtocolMode::Press => {
            (button_code(button), true)
        }
        MouseEventKind::Drag(button)
            if matches!(
                mode,
                MouseProtocolMode::ButtonMotion | MouseProtocolMode::AnyMotion
            ) =>
        {
            (32 + button_code(button), false)
        }
        MouseEventKind::Moved if mode == MouseProtocolMode::AnyMotion => (35, false),
        MouseEventKind::ScrollUp => (64, false),
        MouseEventKind::ScrollDown => (65, false),
        MouseEventKind::ScrollLeft => (66, false),
        MouseEventKind::ScrollRight => (67, false),
        _ => return None,
    };
    let mut code = base;
    if event.modifiers.contains(KeyModifiers::SHIFT) {
        code += 4;
    }
    if event.modifiers.contains(KeyModifiers::ALT) {
        code += 8;
    }
    if event.modifiers.contains(KeyModifiers::CONTROL) {
        code += 16;
    }
    let x = event.column - content.x + 1;
    let y = event.row - content.y + 1;

    match screen.mouse_protocol_encoding() {
        MouseProtocolEncoding::Sgr => {
            Some(format!("\x1b[<{code};{x};{y}{}", if release { 'm' } else { 'M' }).into_bytes())
        }
        MouseProtocolEncoding::Default => {
            let code = if release { 3 + (code & 28) } else { code };
            if code > 223 || x > 223 || y > 223 {
                return None;
            }
            Some(vec![
                0x1b,
                b'[',
                b'M',
                (code + 32) as u8,
                (x + 32) as u8,
                (y + 32) as u8,
            ])
        }
        MouseProtocolEncoding::Utf8 => {
            let code = if release { 3 + (code & 28) } else { code };
            let mut bytes = b"\x1b[M".to_vec();
            for value in [code + 32, x + 32, y + 32] {
                bytes.extend(char::from_u32(value as u32)?.to_string().as_bytes());
            }
            Some(bytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn click(column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn sgr_mouse_coordinates_are_relative_to_the_pane() {
        let mut parser = vt100::Parser::new(10, 20, 0);
        parser.process(b"\x1b[?1000h\x1b[?1006h");
        let content = Rect::new(40, 5, 20, 10);
        assert_eq!(
            encode(parser.screen(), click(42, 7), content),
            Some(b"\x1b[<0;3;3M".to_vec())
        );
        assert_eq!(encode(parser.screen(), click(39, 7), content), None);
    }

    #[test]
    fn a_child_that_did_not_request_mouse_tracking_gets_no_click() {
        let parser = vt100::Parser::new(10, 20, 0);
        assert_eq!(
            encode(parser.screen(), click(2, 2), Rect::new(0, 0, 20, 10)),
            None
        );
    }
}
