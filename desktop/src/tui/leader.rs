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

    /// The bytes a terminal would have received for *this chord*.
    ///
    /// Only ever called for the leader itself, which is a control chord and therefore one ASCII
    /// byte. Ordinary keys are encoded by [`super::keys::encode`], which handles multi-byte
    /// characters -- casting a `char` to `u8` here would truncate anything outside ASCII.
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
    /// Looking back through output that has scrolled away. `[` follows tmux's copy-mode key, which
    /// is the closest thing to a convention terminal users already have.
    Scrollback,
}

fn command_for(key: char) -> Option<Command> {
    match key {
        'p' => Some(Command::Palette),
        's' => Some(Command::Sessions),
        '?' | 'h' => Some(Command::Help),
        'v' => Some(Command::Contextual),
        '[' => Some(Command::Scrollback),
        _ => None,
    }
}

/// What the caller should do with a keystroke.
///
/// Verb never re-encodes a key it is not claiming: [`Outcome::Passthrough`] tells the caller to send
/// the key exactly as the terminal encodes it, rather than the leader trying to reconstruct bytes it
/// only half understands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Not Verb's key. Send it to the terminal as pressed.
    Passthrough,
    /// Swallow it: the leader was pressed and Verb is waiting for the command key.
    Pending,
    /// Send exactly these bytes -- the leader itself, given back to the terminal.
    SendLeader,
    /// Send the leader's bytes, then the key as pressed: an unbound follow-up eats neither.
    SendLeaderThen,
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
        match std::env::var("VERB_LEADER")
            .ok()
            .as_deref()
            .and_then(Chord::parse)
        {
            Some(chord) => Self::configured(chord),
            None => Self::provisional(),
        }
    }

    pub fn configured(chord: Chord) -> Self {
        Self {
            chord: chord.normalised(),
            provisional: false,
            pending: false,
        }
    }

    /// `Ctrl+O` is readline's `operate-and-get-next`, which is rarely used interactively -- the
    /// least-bad chord *before* evidence, which is exactly why it is marked provisional rather than
    /// chosen. The collision test decides the real default.
    pub fn provisional() -> Self {
        Self {
            chord: Chord::ctrl('o').normalised(),
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
        // Normalisation decides only whether this *is* the leader. What gets forwarded is always
        // the key as it arrived: normalising the forwarded bytes would quietly turn every capital
        // letter into a lowercase one.
        let chord = Chord { ctrl, key }.normalised();
        if !self.pending {
            if chord == self.chord {
                self.pending = true;
                return Outcome::Pending;
            }
            return Outcome::Passthrough;
        }

        self.pending = false;
        if chord == self.chord {
            // Leader Leader: the terminal gets the leader key itself, so binding a key never costs
            // the user that key.
            return Outcome::SendLeader;
        }
        match command_for(chord.key).filter(|_| !ctrl) {
            Some(command) => Outcome::Run(command),
            // Unbound follow-up: both keys go through. Verb never eats a keystroke it has no
            // meaning for.
            None => Outcome::SendLeaderThen,
        }
    }

    /// The menu was open and the user changed their mind. Nothing is sent to the terminal: they
    /// pressed Escape to close a menu, not to send an escape.
    pub fn cancel(&mut self) {
        self.pending = false;
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
        assert_eq!(leader.key(false, 'p'), Outcome::Passthrough);
        // Including the control keys readline binds, which is the whole point.
        assert_eq!(leader.key(true, 'k'), Outcome::Passthrough);
        assert_eq!(leader.key(true, 'a'), Outcome::Passthrough);
        // And characters the leader could not encode itself without damaging them.
        assert_eq!(leader.key(false, '日'), Outcome::Passthrough);
        assert_eq!(leader.key(false, '🚀'), Outcome::Passthrough);
    }

    #[test]
    fn a_key_verb_does_not_claim_is_encoded_by_the_terminal_layer_not_here() {
        // Two regressions live in this test. Matching the leader is case-insensitive, but the key
        // sent on must not be -- capitals arrived lowercase. And a `char` cast to `u8` truncates
        // anything outside ASCII, which silently mangled every accented and non-Latin character
        // typed into the workspace. The leader now says "not mine" and encodes nothing.
        let mut leader = leader();
        assert_eq!(leader.key(false, 'P'), Outcome::Passthrough);
        assert_eq!(leader.key(false, 'é'), Outcome::Passthrough);

        leader.key(true, 'o');
        assert_eq!(leader.key(false, 'Z'), Outcome::SendLeaderThen);
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
        assert_eq!(leader.key(true, 'o'), Outcome::SendLeader);
        assert_eq!(leader.chord().bytes(), vec![0x0f]);
    }

    #[test]
    fn an_unbound_follow_up_forwards_both_keys_rather_than_eating_them() {
        let mut leader = leader();
        leader.key(true, 'o');
        assert_eq!(leader.key(false, 'z'), Outcome::SendLeaderThen);
    }

    #[test]
    fn the_menu_stays_open_until_it_is_answered() {
        // Sticky rather than timed: a menu that vanishes while being read is a menu that has to be
        // learned instead of used.
        let mut leader = leader();
        leader.key(true, 'o');
        assert!(leader.is_pending());

        leader.cancel();
        assert!(!leader.is_pending());
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
        assert!(!Leader::configured(Chord::ctrl('g'))
            .hint()
            .contains("provisional"));
        assert_eq!(Leader::configured(Chord::ctrl('g')).hint(), "leader ^G");
    }
}
