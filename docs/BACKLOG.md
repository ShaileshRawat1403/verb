# Verb backlog

Everything known to be undone, and what it costs to say yes. The current sprint is at the top; the
rest is a menu, not a plan.

Milestone definitions live in `docs/ROADMAP.md`. M2 remains deliberately undecided.

---

## Current sprint — "Nothing to memorise, evidence on demand"

**Theme:** someone who has never read our documentation should be able to open Verb, understand what
they are looking at, and find everything without knowing a key.

**Audience decision (2026-08-22):** approachable, still terminal-first. The PRD's initial user is
unchanged — this is quality, not scope. Verb does not become a tool for people who do not use a
terminal; it becomes a tool that does not punish people who are not fluent in one.

| # | Item | Status |
| --- | --- | --- |
| 1 | Desktop CI — fmt, clippy, tests, release build on every PR | done |
| 2 | Mouse support — wheel, click to select, click away to close; captured only while a Verb surface is open | done |
| 3 | Scrollback and search in the terminal pane (`Leader [`) | done |
| 4 | Discoverability — leader menu, first-run welcome, fuller help, empty states that name the next action | done |
| 5 | Plain-language state, with the technical term still visible | done |
| 6 | `Leader v` evidence overlay — `verb context` rendered in the workspace | done |
| 7 | Palette subsequence matching, minimum-size guard, forget a session record | done |

Then a short dogfood, which decides M2's direction.

**Sequence:** 1 → 2 → 3 → 4 → 5 → 6. CI first because everything after it is verified by it; the
evidence overlay last so it lands on a workspace that is already comfortable to sit in.

**Not in this sprint:** M2 itself, deeper observation (see C below), phone work, Windows TUI,
distribution, splitting PR #2.

---

## A. Quality and approachability

| # | Item | Size |
| --- | --- | --- |
| A1 | ~~Desktop CI~~ | done |
| A2 | ~~Scrollback + search~~ | done |
| A3 | ~~Palette subsequence matching~~ (recency ordering still open) | partly |
| A4 | ~~Minimum-size guard~~ | done |
| A5 | ~~Forget a session record~~ | done |
| A6 | ~~Mouse support~~ | done |
| A7 | Windows support for the TUI (CLI already falls back to inherited stdio) | L |

## B. Blocked on the Android device

| # | Item | Blocker |
| --- | --- | --- |
| B1 | Codex and OpenCode leader-collision rows, so the leader default stops being provisional | device |
| B2 | OpenCode Android recovery proof | needs a signed-in provider |
| B3 | Decide the fate of `cmake`/`clang`/`make` left in the guest during the dsh attempt | your call |
| B4 | `dsh` | upstream: `koffi` has no Android build. Recorded as unavailable with the reason |

## C. Observation gaps — possibly M0 rather than M2

The mockups in `docs/TUI_VISION.md` promise more than Verb currently observes. Whether that matters
is exactly what dogfooding answers: if seeing the evidence makes the next question obvious, M2 is an
assistant; if the evidence itself is thin, the next milestone is better observation wearing an M2
costume.

| # | Item | Unlocks |
| --- | --- | --- |
| C1 | Agent sessions emit nothing structural — the failure band is silent inside Claude and Codex | the agent half of the product |
| C2 | Git snapshot at command boundaries | "what changed since it last worked" |
| C3 | Last-known-good tracking | comparison and recovery |
| C4 | Runtime version facts (node, python, …) | the runtime-mismatch scenario in the mockups |
| C5 | Richer contextual triggers: risky Git operation, runtime mismatch | two of the four bands in `TUI_VISION.md` |

## D. Repository and release

| # | Item | Size |
| --- | --- | --- |
| D1 | PR #2 is 66 commits across Android, desktop, TUI and docs — merge as-is or split by subsystem | S–M |
| D2 | No release or distribution path: `cargo install` only | M |
| D3 | The Rust crate has no library target, so integration tests drive the binary | S |
| D4 | Dated snapshots (`HANDOFF.md`, `NEXT_SPRINT.md`, V0 validation) are marked, not rewritten | S |
| D5 | No CONTRIBUTING or architecture overview | S |

## E. Android — paused, not abandoned

| # | Item |
| --- | --- |
| E1 | Foreground service for the background-kill gap `docs/DURABLE_SESSION.md` leaves open |
| E2 | More than one concurrent agent session |
| E3 | Codex runs under qemu because proot refuses its static non-PIE binary; needs a patched proot or an out-of-proot spawn |

## F. Frozen by design

M2 — explain, compare, or guided action. Decided by friction notes, not by this list. See
`docs/DOGFOODING.md`.
