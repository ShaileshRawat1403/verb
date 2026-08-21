//! Turning terminal key events into the bytes a PTY expects.
//!
//! This is the part that decides whether an agent running inside Verb feels like an agent running
//! in a terminal. Everything Verb does not claim has to arrive at the child process byte-identical,
//! including the escape sequences full-screen agents read for arrows, Home/End and the rest.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// The bytes a terminal emulator would have sent for this key press, or `None` for a key with no
/// terminal representation (a bare modifier, say), which is dropped rather than guessed at.
pub fn encode(event: KeyEvent) -> Option<Vec<u8>> {
    let control = event.modifiers.contains(KeyModifiers::CONTROL);
    let alt = event.modifiers.contains(KeyModifiers::ALT);

    let mut bytes = match event.code {
        KeyCode::Char(character) => {
            if control {
                let upper = character.to_ascii_uppercase() as u8;
                match upper {
                    b'@'..=b'_' => vec![upper - 0x40],
                    b'?' => vec![0x7f],
                    _ => vec![character as u8],
                }
            } else {
                let mut buffer = [0_u8; 4];
                character.encode_utf8(&mut buffer).as_bytes().to_vec()
            }
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        // 0x7f, not 0x08: this is what a terminal sends, and what readline and every full-screen
        // agent expects for backspace.
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::F(number @ 1..=4) => {
            vec![0x1b, b'O', b'P' + (number - 1)]
        }
        KeyCode::F(number @ 5..=12) => {
            let code = match number {
                5 => "15",
                6 => "17",
                7 => "18",
                8 => "19",
                9 => "20",
                10 => "21",
                11 => "23",
                _ => "24",
            };
            format!("\x1b[{code}~").into_bytes()
        }
        _ => return None,
    };

    if alt {
        // Alt is transmitted as ESC before the key, which is what terminals do and what readline's
        // meta bindings read.
        let mut prefixed = vec![0x1b];
        prefixed.append(&mut bytes);
        bytes = prefixed;
    }

    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyEvent;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn ordinary_characters_pass_through_as_themselves() {
        assert_eq!(encode(key(KeyCode::Char('a'))), Some(b"a".to_vec()));
        assert_eq!(encode(key(KeyCode::Char('é'))), Some("é".as_bytes().to_vec()));
    }

    #[test]
    fn control_keys_become_control_characters() {
        let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(encode(event), Some(vec![0x03]));
        let event = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL);
        assert_eq!(encode(event), Some(vec![0x0b]));
    }

    #[test]
    fn backspace_sends_delete_which_is_what_readline_expects() {
        assert_eq!(encode(key(KeyCode::Backspace)), Some(vec![0x7f]));
    }

    #[test]
    fn arrows_and_function_keys_keep_their_escape_sequences() {
        assert_eq!(encode(key(KeyCode::Up)), Some(b"\x1b[A".to_vec()));
        assert_eq!(encode(key(KeyCode::F(1))), Some(b"\x1bOP".to_vec()));
        assert_eq!(encode(key(KeyCode::F(5))), Some(b"\x1b[15~".to_vec()));
    }

    #[test]
    fn alt_is_sent_as_an_escape_prefix() {
        let event = KeyEvent::new(KeyCode::Char('b'), KeyModifiers::ALT);
        assert_eq!(encode(event), Some(vec![0x1b, b'b']));
    }
}
