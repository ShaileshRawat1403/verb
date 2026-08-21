//! Shell-integration parsing for the desktop PTY host.
//!
//! The host proxies raw bytes between the user's terminal and the agent's PTY. Those bytes are
//! transport, not memory: they are written straight through and dropped. This module is the one
//! place they are *looked at*, and only for the small, fixed set of structural markers a
//! shell-integrated shell emits -- where the working directory changed, and where a command started
//! and finished.
//!
//! Deliberately the same vocabulary Android parses (`ShellIntegrationEvent`), so a session means
//! the same thing on both hosts:
//!
//! - **OSC 7** — the shell's current working directory.
//! - **OSC 633;A/B/C/D** — VS Code's shell integration: prompt start, prompt end, command start,
//!   command finished (with exit code).
//! - **OSC 133;A/B/C/D** — the FinalTerm/iTerm2 spelling of the same four, which most shells on a
//!   desktop emit when integration is enabled at all.
//!
//! `OSC 633;E` carries the command *line* the user typed. It is recognised only so it can be
//! skipped: nothing here ever returns, stores, or logs command text, and the event vocabulary below
//! has no variant that could carry it.

/// A structural fact read out of the terminal stream. No variant carries command text, terminal
/// output, or any other payload the user typed -- by construction, not by convention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellEvent {
    CurrentDirectory(String),
    PromptStart,
    PromptEnd,
    CommandStart,
    CommandEnd(i32),
}

/// An incremental scanner over the PTY output stream.
///
/// PTY reads split wherever the kernel happens to split them, so a marker routinely arrives across
/// two or three chunks. The scanner therefore holds only the bytes of a marker currently being
/// accumulated, never the stream itself, and gives that up too if a sequence grows past
/// [`MAX_OSC_BYTES`] -- a terminal writing an unterminated OSC must not be able to grow Verb's
/// memory without bound.
#[derive(Default)]
pub struct ShellScanner {
    state: State,
    pending: Vec<u8>,
}

#[derive(Default, PartialEq, Eq)]
enum State {
    #[default]
    Text,
    /// Saw ESC; the next byte decides whether this is an OSC.
    Escape,
    /// Inside an OSC payload, accumulating into `pending`.
    Osc,
    /// Saw ESC inside an OSC; a following `\` terminates it (ST).
    OscEscape,
}

const MAX_OSC_BYTES: usize = 4096;
const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;

impl ShellScanner {
    /// Feeds one chunk of PTY output and returns whatever structural markers completed inside it.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<ShellEvent> {
        let mut events = Vec::new();
        for &byte in bytes {
            match self.state {
                State::Text => {
                    if byte == ESC {
                        self.state = State::Escape;
                    }
                }
                State::Escape => {
                    self.state = if byte == b']' {
                        self.pending.clear();
                        State::Osc
                    } else {
                        // Any other escape sequence (colours, cursor moves) is display, not
                        // structure, and is passed over without being looked at further.
                        State::Text
                    };
                }
                State::Osc => match byte {
                    BEL => {
                        events.extend(self.finish_osc());
                    }
                    ESC => self.state = State::OscEscape,
                    _ => {
                        self.pending.push(byte);
                        if self.pending.len() > MAX_OSC_BYTES {
                            self.reset();
                        }
                    }
                },
                State::OscEscape => {
                    if byte == b'\\' {
                        events.extend(self.finish_osc());
                    } else {
                        // An ESC that was not a string terminator means this was never a
                        // well-formed OSC. Drop it rather than guessing at its meaning.
                        self.reset();
                    }
                }
            }
        }
        events
    }

    fn finish_osc(&mut self) -> Option<ShellEvent> {
        let payload = std::mem::take(&mut self.pending);
        self.state = State::Text;
        parse_osc(&String::from_utf8_lossy(&payload))
    }

    fn reset(&mut self) {
        self.pending.clear();
        self.state = State::Text;
    }
}

fn parse_osc(payload: &str) -> Option<ShellEvent> {
    let (code, rest) = payload.split_once(';')?;
    match code {
        "7" => parse_cwd(rest).map(ShellEvent::CurrentDirectory),
        "633" | "133" => parse_lifecycle(rest),
        _ => None,
    }
}

/// OSC 7's payload is `file://<host><path>`; the host part is usually empty (`file:///path`).
/// Anything else is another terminal's private use of the same code and is ignored.
fn parse_cwd(payload: &str) -> Option<String> {
    let without_scheme = payload.strip_prefix("file://")?;
    let path = match without_scheme.find('/') {
        Some(index) => &without_scheme[index..],
        None => return None,
    };
    let decoded = percent_decode(path);
    if decoded.is_empty() {
        None
    } else {
        Some(decoded)
    }
}

fn parse_lifecycle(payload: &str) -> Option<ShellEvent> {
    let mut parts = payload.split(';');
    match parts.next()? {
        "A" => Some(ShellEvent::PromptStart),
        "B" => Some(ShellEvent::PromptEnd),
        "C" => Some(ShellEvent::CommandStart),
        // An absent or unparseable exit code is reported as 0 rather than dropped: the command
        // genuinely finished, which is the structural fact, and inventing a failure would be worse
        // than recording an unknown status as success.
        "D" => Some(ShellEvent::CommandEnd(
            parts.next().and_then(|code| code.parse().ok()).unwrap_or(0),
        )),
        // "E" is the command line the user typed, and "P" is a property bag. Both are recognised
        // here only so they are explicitly skipped rather than falling through to something that
        // might later be tempted to keep them.
        _ => None,
    }
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let high = (bytes[index + 1] as char).to_digit(16);
            let low = (bytes[index + 2] as char).to_digit(16);
            if let (Some(high), Some(low)) = (high, low) {
                decoded.push((high * 16 + low) as u8);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(chunks: &[&str]) -> Vec<ShellEvent> {
        let mut scanner = ShellScanner::default();
        let mut events = Vec::new();
        for chunk in chunks {
            events.extend(scanner.feed(chunk.as_bytes()));
        }
        events
    }

    #[test]
    fn reads_the_working_directory_from_osc_7() {
        assert_eq!(
            scan(&["\x1b]7;file://host/Users/example/my%20project\x07"]),
            vec![ShellEvent::CurrentDirectory(
                "/Users/example/my project".to_owned()
            )]
        );
    }

    #[test]
    fn reads_command_boundaries_from_both_integration_spellings() {
        assert_eq!(
            scan(&["\x1b]633;C\x07output\x1b]633;D;3\x07"]),
            vec![ShellEvent::CommandStart, ShellEvent::CommandEnd(3)]
        );
        assert_eq!(
            scan(&["\x1b]133;C\x07\x1b]133;D;0\x07"]),
            vec![ShellEvent::CommandStart, ShellEvent::CommandEnd(0)]
        );
    }

    #[test]
    fn accepts_the_string_terminator_as_well_as_bel() {
        assert_eq!(
            scan(&["\x1b]633;A\x1b\\"]),
            vec![ShellEvent::PromptStart]
        );
    }

    #[test]
    fn a_marker_split_across_reads_is_still_read() {
        // PTY reads split wherever the kernel splits them, including mid-sequence.
        assert_eq!(
            scan(&["\x1b]6", "33;D", ";7\x07"]),
            vec![ShellEvent::CommandEnd(7)]
        );
    }

    #[test]
    fn command_text_is_never_turned_into_an_event() {
        // OSC 633;E carries what the user typed. It must produce nothing at all.
        let events = scan(&["\x1b]633;E;rm -rf /secret\x07\x1b]633;C\x07"]);
        assert_eq!(events, vec![ShellEvent::CommandStart]);
    }

    #[test]
    fn ordinary_output_and_colour_escapes_produce_nothing() {
        assert!(scan(&["\x1b[31mred\x1b[0m plain text\n"]).is_empty());
    }

    #[test]
    fn an_unterminated_sequence_cannot_grow_memory_without_bound() {
        let mut scanner = ShellScanner::default();
        scanner.feed(b"\x1b]633;");
        scanner.feed(&vec![b'x'; MAX_OSC_BYTES * 2]);
        assert!(scanner.pending.len() <= MAX_OSC_BYTES);
        // And the scanner still works afterwards.
        assert_eq!(
            scanner.feed(b"\x1b]633;C\x07"),
            vec![ShellEvent::CommandStart]
        );
    }

    #[test]
    fn a_malformed_directory_marker_is_ignored_rather_than_guessed() {
        assert!(scan(&["\x1b]7;not-a-url\x07"]).is_empty());
        assert!(scan(&["\x1b]7;file://host\x07"]).is_empty());
    }
}
