//! One answer per terminal question.
//!
//! A program asks its terminal questions by printing them: where is the cursor (`ESC[6n`), what
//! are you (`ESC[c`), what colour is the background (`ESC]11;?`). The terminal answers by typing
//! the reply into the program's input. Verb can show one session in several browser tabs, and every
//! tab's terminal answers. The program consumes the answer it was waiting for and reads the rest as
//! keystrokes: the 0.2.0-rc.1 real-agent test found `32;3R` in Claude Code's prompt this way.
//!
//! The gate counts questions in the output every viewer is about to receive, before any of them can
//! answer, and lets one answer through per question. An answer that arrives with no open question
//! shortly after one was already used is a duplicate and is dropped. Anything else passes unchanged,
//! including a key press that happens to look like an answer (Shift+F3 sends the same bytes as a
//! cursor report), so the only input ever dropped is a second answer to a question already answered.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// How long after a used answer a further answer of the same kind still counts as its duplicate.
const DUPLICATE_WINDOW: Duration = Duration::from_secs(2);
/// Questions nobody answered stop counting after this long, so a tab that never answers cannot
/// leave a debt that lets a later duplicate through.
const QUESTION_TTL: Duration = Duration::from_secs(5);
/// The longest question we recognise, kept between output chunks so a split question still counts.
const TAIL: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Kind {
    CursorPosition,
    Status,
    PrimaryAttributes,
    SecondaryAttributes,
    Colour(u8),
    Mode,
}

#[derive(Default)]
pub struct ReplyGate {
    open: HashMap<Kind, Vec<Instant>>,
    answered: HashMap<Kind, Instant>,
    tail: Vec<u8>,
}

impl ReplyGate {
    /// Records the questions in a chunk of program output that viewers are about to receive.
    pub fn observe_output(&mut self, chunk: &[u8], now: Instant) {
        let mut scan = std::mem::take(&mut self.tail);
        let carried = scan.len();
        scan.extend_from_slice(chunk);
        let mut i = 0;
        while i < scan.len() {
            if scan[i] == 0x1b {
                if let Some((kind, len)) = question_at(&scan[i..]) {
                    // A question wholly inside the carried tail was already counted last time.
                    if i + len > carried {
                        self.open.entry(kind).or_default().push(now);
                    }
                    i += len;
                    continue;
                }
            }
            i += 1;
        }
        let keep = scan.len().min(TAIL);
        self.tail = scan[scan.len() - keep..].to_vec();
    }

    /// Returns the input with duplicate answers removed. Everything else is kept in order.
    pub fn admit_input(&mut self, input: &[u8], now: Instant) -> Vec<u8> {
        for times in self.open.values_mut() {
            times.retain(|asked| now.duration_since(*asked) < QUESTION_TTL);
        }
        let mut out = Vec::with_capacity(input.len());
        let mut i = 0;
        while i < input.len() {
            if input[i] == 0x1b {
                if let Some((kind, len)) = answer_at(&input[i..]) {
                    if self.accept(kind, now) {
                        out.extend_from_slice(&input[i..i + len]);
                    }
                    i += len;
                    continue;
                }
            }
            out.push(input[i]);
            i += 1;
        }
        out
    }

    fn accept(&mut self, kind: Kind, now: Instant) -> bool {
        if let Some(times) = self.open.get_mut(&kind) {
            if !times.is_empty() {
                times.remove(0);
                self.answered.insert(kind, now);
                return true;
            }
        }
        !matches!(self.answered.get(&kind), Some(at) if now.duration_since(*at) < DUPLICATE_WINDOW)
    }
}

fn digits(bytes: &[u8]) -> usize {
    bytes.iter().take_while(|b| b.is_ascii_digit()).count()
}

/// `ESC [` followed by parameter bytes (digits and `;`) and the given final byte, after an
/// optional private marker. Returns the length of the whole sequence.
fn csi(bytes: &[u8], marker: Option<u8>, finals: &[u8]) -> Option<(usize, u8)> {
    if bytes.len() < 3 || bytes[1] != b'[' {
        return None;
    }
    let mut i = 2;
    match marker {
        Some(m) if bytes.get(i) == Some(&m) => i += 1,
        Some(_) => return None,
        None if matches!(bytes.get(i), Some(b'?' | b'>' | b'<' | b'=')) => return None,
        None => {}
    }
    while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b';') {
        i += 1;
    }
    let last = *bytes.get(i)?;
    finals.contains(&last).then_some((i + 1, last))
}

fn osc_colour(bytes: &[u8]) -> Option<(u8, usize, usize)> {
    // ESC ] 1 0|1|2 ; ... (BEL | ESC \)
    if bytes.len() < 6
        || bytes[1] != b']'
        || bytes[2] != b'1'
        || !(b'0'..=b'2').contains(&bytes[3])
        || bytes[4] != b';'
    {
        return None;
    }
    let body = 5;
    let mut i = body;
    while i < bytes.len() && i < 128 {
        if bytes[i] == 0x07 {
            return Some((bytes[3] - b'0', body, i + 1));
        }
        if bytes[i] == 0x1b && bytes.get(i + 1) == Some(&b'\\') {
            return Some((bytes[3] - b'0', body, i + 2));
        }
        i += 1;
    }
    None
}

fn question_at(bytes: &[u8]) -> Option<(Kind, usize)> {
    if let Some((len, _)) = csi(bytes, None, b"n") {
        return match &bytes[2..len - 1] {
            b"6" => Some((Kind::CursorPosition, len)),
            b"5" => Some((Kind::Status, len)),
            _ => None,
        };
    }
    if let Some((len, _)) = csi(bytes, Some(b'?'), b"n") {
        return (&bytes[3..len - 1] == b"6").then_some((Kind::CursorPosition, len));
    }
    if let Some((len, _)) = csi(bytes, None, b"c") {
        return matches!(&bytes[2..len - 1], b"" | b"0").then_some((Kind::PrimaryAttributes, len));
    }
    if let Some((len, _)) = csi(bytes, Some(b'>'), b"c") {
        return matches!(&bytes[3..len - 1], b"" | b"0")
            .then_some((Kind::SecondaryAttributes, len));
    }
    if let Some((which, body, len)) = osc_colour(bytes) {
        return (bytes.get(body) == Some(&b'?')).then_some((Kind::Colour(which), len));
    }
    // DECRQM: ESC [ ? Ps $ p, or ESC [ Ps $ p.
    if bytes.len() >= 5 && bytes[1] == b'[' {
        let start = if bytes[2] == b'?' { 3 } else { 2 };
        let n = digits(&bytes[start..]);
        if n > 0 && bytes.get(start + n) == Some(&b'$') && bytes.get(start + n + 1) == Some(&b'p') {
            return Some((Kind::Mode, start + n + 2));
        }
    }
    None
}

fn answer_at(bytes: &[u8]) -> Option<(Kind, usize)> {
    // Cursor report: ESC [ row ; col R, or ESC [ ? row ; col (; page) R.
    for marker in [None, Some(b'?')] {
        if let Some((len, _)) = csi(bytes, marker, b"R") {
            let params = &bytes[if marker.is_some() { 3 } else { 2 }..len - 1];
            if params.contains(&b';') {
                return Some((Kind::CursorPosition, len));
            }
        }
    }
    if let Some((len, _)) = csi(bytes, None, b"n") {
        return (&bytes[2..len - 1] == b"0" || &bytes[2..len - 1] == b"3")
            .then_some((Kind::Status, len));
    }
    if let Some((len, _)) = csi(bytes, Some(b'?'), b"c") {
        return Some((Kind::PrimaryAttributes, len));
    }
    if let Some((len, _)) = csi(bytes, Some(b'>'), b"c") {
        return Some((Kind::SecondaryAttributes, len));
    }
    if let Some((which, body, len)) = osc_colour(bytes) {
        return (bytes.get(body) != Some(&b'?')).then_some((Kind::Colour(which), len));
    }
    // DECRPM: ESC [ ? Ps ; Pm $ y, or ESC [ Ps ; Pm $ y.
    if bytes.len() >= 6 && bytes[1] == b'[' {
        let start = if bytes[2] == b'?' { 3 } else { 2 };
        let mut i = start;
        while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b';') {
            i += 1;
        }
        if i > start && bytes.get(i) == Some(&b'$') && bytes.get(i + 1) == Some(&b'y') {
            return Some((Kind::Mode, i + 2));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(ms: u64, base: Instant) -> Instant {
        base + Duration::from_millis(ms)
    }

    #[test]
    fn one_question_lets_exactly_one_answer_through() {
        let t = Instant::now();
        let mut gate = ReplyGate::default();
        gate.observe_output(b"draw\x1b[6n", t);
        assert_eq!(gate.admit_input(b"\x1b[32;3R", at(10, t)), b"\x1b[32;3R");
        // The second tab's answer to the same question.
        assert_eq!(gate.admit_input(b"\x1b[32;3R", at(12, t)), b"");
    }

    #[test]
    fn two_questions_let_two_answers_through() {
        let t = Instant::now();
        let mut gate = ReplyGate::default();
        gate.observe_output(b"\x1b[6n", t);
        gate.observe_output(b"\x1b[6n", at(1, t));
        assert_eq!(
            gate.admit_input(b"\x1b[1;1R\x1b[1;1R\x1b[1;1R", at(5, t)),
            b"\x1b[1;1R\x1b[1;1R"
        );
    }

    #[test]
    fn keystrokes_around_a_duplicate_are_kept() {
        let t = Instant::now();
        let mut gate = ReplyGate::default();
        gate.observe_output(b"\x1b[6n", t);
        gate.admit_input(b"\x1b[5;9R", at(1, t));
        assert_eq!(
            gate.admit_input(b"ls\x1b[5;9R -la\r", at(2, t)),
            b"ls -la\r"
        );
    }

    #[test]
    fn a_key_that_looks_like_an_answer_passes_when_no_answer_was_just_used() {
        let t = Instant::now();
        let mut gate = ReplyGate::default();
        // Shift+F3 in xterm.js.
        assert_eq!(gate.admit_input(b"\x1b[1;2R", t), b"\x1b[1;2R");
        gate.observe_output(b"\x1b[6n", t);
        gate.admit_input(b"\x1b[4;4R", at(1, t));
        assert_eq!(gate.admit_input(b"\x1b[1;2R", at(3_000, t)), b"\x1b[1;2R");
    }

    #[test]
    fn a_question_split_across_chunks_is_counted_once() {
        let t = Instant::now();
        let mut gate = ReplyGate::default();
        gate.observe_output(b"text\x1b[", t);
        gate.observe_output(b"6n more", t);
        gate.observe_output(b" output", t);
        assert_eq!(gate.admit_input(b"\x1b[2;2R", at(1, t)), b"\x1b[2;2R");
        assert_eq!(gate.admit_input(b"\x1b[2;2R", at(2, t)), b"");
    }

    #[test]
    fn unanswered_questions_expire() {
        let t = Instant::now();
        let mut gate = ReplyGate::default();
        gate.observe_output(b"\x1b[6n", t);
        gate.observe_output(b"\x1b[6n", t);
        assert_eq!(gate.admit_input(b"\x1b[1;1R", at(10, t)), b"\x1b[1;1R");
        // The second question was never answered by anyone; much later it no longer opens the gate
        // for what is now a duplicate inside a fresh answer window.
        gate.observe_output(b"\x1b[6n", at(9_000, t));
        assert_eq!(gate.admit_input(b"\x1b[1;1R", at(9_001, t)), b"\x1b[1;1R");
        assert_eq!(gate.admit_input(b"\x1b[1;1R", at(9_002, t)), b"");
    }

    #[test]
    fn other_question_kinds_are_answered_once() {
        let t = Instant::now();
        let mut gate = ReplyGate::default();
        gate.observe_output(b"\x1b[c\x1b[>c\x1b]11;?\x07\x1b[?2026$p\x1b[5n", t);
        let answers: &[&[u8]] = &[
            b"\x1b[?1;2c",
            b"\x1b[>0;276;0c",
            b"\x1b]11;rgb:0000/0000/0000\x1b\\",
            b"\x1b[?2026;2$y",
            b"\x1b[0n",
        ];
        for answer in answers {
            assert_eq!(gate.admit_input(answer, at(1, t)), *answer);
            assert_eq!(gate.admit_input(answer, at(2, t)), b"");
        }
    }

    #[test]
    fn ordinary_escape_keys_are_never_touched() {
        let t = Instant::now();
        let mut gate = ReplyGate::default();
        gate.observe_output(b"\x1b[6n", t);
        gate.admit_input(b"\x1b[1;1R", at(1, t));
        let keys: &[u8] = b"\x1b[A\x1b[B\x1b[1;5C\x1b[3~\x1bOP\x1b[200~paste\x1b[201~\x1b";
        assert_eq!(gate.admit_input(keys, at(2, t)), keys);
    }
}
