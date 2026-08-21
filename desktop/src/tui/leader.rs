//! The Verb Leader: one configurable chord that begins a Verb command.
//!
//! See `docs/TUI_VISION.md`. The constraint this exists to satisfy is that the terminal owns the
//! keyboard: Claude, Codex and OpenCode use nearly every key, and the shell underneath them uses
//! most control keys as readline bindings. So Verb claims exactly one chord, and even that one is
//! configurable and forwardable.
//!
//! No production default is bound yet. The default here is **provisional** and says so wherever it
//! is shown, pending the collision test in `docs/TUI_VISION.md` against bash, zsh, Claude, Codex and
//! OpenCode.

use std::fmt;

/// A single chord, kept deliberately small: a control character or a function-style named key is
/// all a leader ever needs to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chord {
    pub ctrl: bool,
    pub key: char,
}

impl Chord {
    pub const fn ctrl(key: char) -> Self {
        Self { ctrl: true, key }
    }

    /// Terminals disagree about how to name the same chord: Ctrl+Space and Ctrl+@ are both the NUL
    /// byte, and a terminal library may report either. Normalising here means a leader configured
    /// one way still matches the key that actually arrives.
    fn normalised(mut self) -> Self {
        if self.ctrl && self.key == ' ' {
            self.key = '@';
        }
        self.key = self.key.to_ascii_lowercase();
        self
    }

    /// The bytes a terminal would have received had Verb not intercepted this chord. Forwarding
    /// these is what makes `Leader Leader` and an unbound follow-up honest rather than approximate.
    pub fn bytes(&self) -> Vec<u8> {
        if self.ctrl {
            let upper = self.key.to_ascii_uppercase() as u8;
            // Control characters are the letter's position in the alphabet: Ctrl+A is 0x01.
            match upper {
                b'@'..=b'_' => vec![upper - 0x40],
                _ => vec![self.key as u8],
            }
        } else {
            vec![self.key as u8]
        }
    }

    /// Parses `VERB_LEADER` values such as `ctrl-o`, `C-g`, `^k`. Returns `None` for anything it
    /// does not understand, so a typo falls back to the provisional default rather than silently
    /// binding something else.
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim().to_ascii_lowercase();
        let key = value
            .strip_prefix("ctrl-")
            .or_else(|| value.strip_prefix("ctrl+"))
            .or_else(|| value.strip_prefix("c-"))
            .or_else(|| value.strip_prefix('^'))?;
        if key == "space" {
            return Some(Self::ctrl('@').normalised());
        }
        let mut characters = key.chars();
        let key = characters.next()?;
        if characters.next().is_some() || !key.is_ascii() {
            return None;
        }
        Some(Self::ctrl(key).normalised())
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl && self.key == '@' {
            // "^@" is the byte; "^Space" is the key people press.
            write!(formatter, "^Space")
        } else if self.ctrl {
            write!(formatter, "^{}", self.key.to_ascii_uppercase())
        } else {
            write!(formatter, "{}", self.key)
        }
    }
}

/// What a Verb command asks for once the leader has been pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Palette,
    Sessions,
    Help,
    Contextual,
}

fn command_for(key: char) -> Option<Command> {
    match key {
        'p' => Some(Command::Palette),
        's' => Some(Command::Sessions),
        '?' | 'h' => Some(Command::Help),
        'v' => Some(Command::Contextual),
        _ => None,
    }
}

/// What the caller should do with a keystroke.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Send these bytes to the terminal, untouched.
    Forward(Vec<u8>),
    /// Swallow it: the leader was pressed and Verb is waiting for the command key.
    Pending,
    /// Run a Verb command.
    Run(Command),
}

/// The leader state machine.
///
/// Every rule in `docs/TUI_VISION.md` is expressed here rather than scattered through the event
/// loop: nothing but the leader is reserved, `Leader Leader` forwards the leader itself, an unbound
/// follow-up forwards *both* keys rather than being swallowed, and a leader left hanging past the
/// timeout forwards itself and returns to normal.
pub struct Leader {
    chord: Chord,
    provisional: bool,
    pending: bool,
}

impl Leader {
    /// Reads `VERB_LEADER`, falling back to the provisional default.
    pub fn from_environment() -> Self {
        match std::env::var("VERB_LEADER").ok().as_deref().and_then(Chord::parse) {
            Some(chord) => Self::configured(chord),
            None => Self::provisional(),
        }
    }

    pub fn configured(chord: Chord) -> Self {
        Self {
            chord,
            provisional: false,
            pending: false,
        }
    }

    /// `Ctrl+O` is readline's `operate-and-get-next`, which is rarely used interactively -- the
    /// least-bad chord *before* evidence, which is exactly why it is marked provisional rather than
    /// chosen. The collision test decides the real default.
    pub fn provisional() -> Self {
        Self {
            chord: Chord::ctrl('o'),
            provisional: true,
            pending: false,
        }
    }

    pub fn chord(&self) -> Chord {
        self.chord
    }

    pub fn is_provisional(&self) -> bool {
        self.provisional
    }

    pub fn is_pending(&self) -> bool {
        self.pending
    }

    /// How the leader should be described on screen. A user must never discover the binding by
    /// having it fire.
    pub fn hint(&self) -> String {
        if self.provisional {
            format!("leader {} (provisional)", self.chord)
        } else {
            format!("leader {}", self.chord)
        }
    }

    pub fn key(&mut self, ctrl: bool, key: char) -> Outcome {
        let chord = Chord { ctrl, key }.normalised();
        if !self.pending {
            if chord == self.chord {
                self.pending = true;
                return Outcome::Pending;
            }
            return Outcome::Forward(chord.bytes());
        }

        self.pending = false;
        if chord == self.chord {
            // Leader Leader: the terminal gets the leader key itself, so binding a key never costs
            // the user that key.
            return Outcome::Forward(self.chord.bytes());
        }
        match command_for(chord.key).filter(|_| !ctrl) {
            Some(command) => Outcome::Run(command),
            // Unbound follow-up: both keys go through. Verb never eats a keystroke it has no
            // meaning for.
            None => {
                let mut bytes = self.chord.bytes();
                bytes.extend(chord.bytes());
                Outcome::Forward(bytes)
            }
        }
    }

    /// The leader was pressed and nothing followed within the timeout: forward it and forget it.
    pub fn timeout(&mut self) -> Option<Vec<u8>> {
        if self.pending {
            self.pending = false;
            Some(self.chord.bytes())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leader() -> Leader {
        Leader::configured(Chord::ctrl('o'))
    }

    #[test]
    fn an_ordinary_key_is_never_intercepted() {
        let mut leader = leader();
        assert_eq!(leader.key(false, 'p'), Outcome::Forward(vec![b'p']));
        // Including the control keys readline binds, which is the whole point.
        assert_eq!(leader.key(true, 'k'), Outcome::Forward(vec![0x0b]));
        assert_eq!(leader.key(true, 'a'), Outcome::Forward(vec![0x01]));
    }

    #[test]
    fn the_leader_opens_a_verb_command() {
        let mut leader = leader();
        assert_eq!(leader.key(true, 'o'), Outcome::Pending);
        assert_eq!(leader.key(false, 'p'), Outcome::Run(Command::Palette));
        assert!(!leader.is_pending());
    }

    #[test]
    fn leader_leader_gives_the_key_back_to_the_terminal() {
        let mut leader = leader();
        leader.key(true, 'o');
        assert_eq!(leader.key(true, 'o'), Outcome::Forward(vec![0x0f]));
    }

    #[test]
    fn an_unbound_follow_up_forwards_both_keys_rather_than_eating_them() {
        let mut leader = leader();
        leader.key(true, 'o');
        assert_eq!(leader.key(false, 'z'), Outcome::Forward(vec![0x0f, b'z']));
    }

    #[test]
    fn a_hanging_leader_is_forwarded_and_forgotten() {
        let mut leader = leader();
        leader.key(true, 'o');
        assert_eq!(leader.timeout(), Some(vec![0x0f]));
        assert!(!leader.is_pending());
        // And nothing is forwarded when no leader was pending.
        assert_eq!(leader.timeout(), None);
    }

    #[test]
    fn ctrl_space_and_ctrl_at_are_the_same_chord_however_the_terminal_names_it() {
        // Both are the NUL byte, and terminal libraries report either one.
        assert_eq!(Chord::parse("ctrl-space"), Chord::parse("ctrl-@"));
        assert_eq!(Chord::parse("ctrl-space").unwrap().bytes(), vec![0x00]);
        assert_eq!(format!("{}", Chord::parse("ctrl-space").unwrap()), "^Space");

        let mut leader = Leader::configured(Chord::parse("ctrl-@").unwrap());
        assert_eq!(leader.key(true, ' '), Outcome::Pending);
        assert_eq!(leader.key(false, 'p'), Outcome::Run(Command::Palette));
    }

    #[test]
    fn the_leader_is_configurable_and_a_typo_does_not_bind_something_else() {
        assert_eq!(Chord::parse("ctrl-g"), Some(Chord::ctrl('g')));
        // "C-Space" now parses, since Ctrl+Space is a chord a terminal actually sends.
        assert_eq!(Chord::parse("C-Space"), Some(Chord::ctrl('@')));
        assert_eq!(Chord::parse("ctrl-escape"), None);
        assert_eq!(Chord::parse("^k"), Some(Chord::ctrl('k')));
        assert_eq!(Chord::parse("meta-x"), None);
        assert_eq!(Chord::parse(""), None);
    }

    #[test]
    fn a_provisional_default_says_so_wherever_it_is_shown() {
        // A user must never discover the binding by having it fire.
        assert!(Leader::provisional().hint().contains("provisional"));
        assert!(!Leader::configured(Chord::ctrl('g')).hint().contains("provisional"));
        assert_eq!(Leader::configured(Chord::ctrl('g')).hint(), "leader ^G");
    }
}
