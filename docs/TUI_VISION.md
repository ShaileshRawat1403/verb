# Verb TUI Vision

How the desktop experience works, as one implementation of `docs/UX_FOUNDATION.md`. The foundation
holds the philosophy — product feel, the one-surface-one-question rule, the utility moments, the
visual language, progressive disclosure, language and input rules, and the test a new surface must
pass. This document is how that lands in Ratatui, and would be replaced wholesale if the host
changed.

The Ratatui implementation is built against this document rather than designed while coding.

## Four rules

> **Terminal first. Context second. Verb appears when useful. Power stays reachable.**

And the rule that keeps the UI from becoming a second product:

> **Every TUI action must map to an underlying Verb capability or CLI command.**

The current `verb ui` already holds that line — it computes no session state and calls the same
functions `verb resume` and `verb claude` call — and Ratatui must not relax it. Anything the UI can
do, a script can do.

## The quiet state

Most of the time nothing important is happening, and the screen says so by getting out of the way:

```text
┌─ Verb ──────────────────────────────────────────────────────────────────────┐
│ ~/projects/verb   main clean   Node 24   Codex   ● LIVE                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│                                                                             │
│                           TERMINAL                                          │
│                                                                             │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│ Ask Verb…                                    available in M2   ⌘ leader ?   │
└─────────────────────────────────────────────────────────────────────────────┘
```

The Ask line is drawn dim and is **not focusable in M1**. The region is reserved so the M1 layout is
the real layout, but nothing accepts a question until M2 can answer one. A user who types into
something that looks functional and gets nothing back has been misled by the product whose whole
purpose is removing that kind of ambiguity.

No permanent Git pane. No permanent file explorer. No permanent chat sidebar. Three regions: one
status line, the terminal, one input line.

## The contextual state

When something happens that the user will have questions about, the answer surface appears next to
it — and only then:

```text
┌─ Verb ──────────────────────────────────────────────────────────────────────┐
│ ~/projects/verb   main ↑2   4 changed   Node 24   Claude   ● LIVE          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  $ npm test                                                                 │
│                                                                             │
│  PASS  terminal/session.test.ts                                             │
│  PASS  git/state.test.ts                                                    │
│  FAIL  runtime/agent.test.ts                                                │
│                                                                             │
│  Error: expected Node 22, found Node 24                                     │
│                                                                             │
│                                                                             │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│ ✕ Command failed · exit 1 · 3.8s                                            │
│                                                                             │
│ [e] Explain   [c] Changes   [d] Diagnose   [r] Retry   [x] Raw             │
├─────────────────────────────────────────────────────────────────────────────┤
│ Ask Verb…                                                  available in M2  │
└─────────────────────────────────────────────────────────────────────────────┘
```

The terminal still owns most of the screen. The contextual band is a few lines, it is driven by an
observed fact (a command finished with a non-zero exit code), and it disappears when the situation
does.

Triggers, and the actions they justify:

```text
a test or command fails      Explain · Compare · Fix with agent · View raw
a Git operation gets risky   Explain impact · Safer option · Continue
an agent stops unexpectedly  Resume · Inspect session · What happened?
runtime mismatch detected    Use compatible runtime · Explain · Advanced
```

Each trigger is an observed fact Verb already records — exit code, session state transition, runtime
version against a declared requirement. **No band appears on inference.**

## The command palette

`Leader p` opens the full-power surface, and it is the only place capability needs to be memorised:

```text
╭─ Command Palette ─────────────────────────────────────────╮
│ > git                                                    │
│                                                          │
│   Git status                                             │
│   Show changed files                                     │
│   Explain current branch                                 │
│   Compare with last successful state                     │
│   Create commit                                          │
│   Advanced Git controls                                  │
╰──────────────────────────────────────────────────────────╯
```

## Ask Verb

Natural language reaches the same capabilities, with the context supplied rather than typed:

```text
╭─ Ask Verb ───────────────────────────────────────────────╮
│ Why did this break?                                      │
├──────────────────────────────────────────────────────────┤
│ Verb has evidence of one relevant change:                │
│                                                          │
│ Node changed from 22 → 24 before the failed test run.    │
│ package.json still declares Node 22.                     │
│                                                          │
│ [Use Node 22]   [Show evidence]   [Ask agent to inspect] │
╰──────────────────────────────────────────────────────────╯
```

`Show evidence` is not decoration. Anything Verb asserts must be traceable to the observed fact it
came from, which is also what keeps an answer honest when the model behind it is wrong.

*(M2. Documented here so the layout reserves room for it; not built in M1.)*

## The debug view

Probably where Verb differentiates most:

```text
╭─ What happened? ────────────────────────────────────────────────────────────╮
│                                                                           │
│  21:03  ✓ Codex session started                                           │
│  21:05  ~ 3 files changed                                                 │
│  21:06  ✓ npm install                                                     │
│  21:07  ✕ npm test                                             exit 1     │
│                                                                           │
│  Relevant change                                                          │
│  runtime: Node 22 → Node 24                                                │
│                                                                           │
│  Last known successful test                                               │
│  20:41 · Node 22 · commit 91ca73                                          │
│                                                                           │
│  [Compare]   [Explain]   [Recover]   [Raw events]                          │
╰───────────────────────────────────────────────────────────────────────────╯
```

The timeline is the structural event log that already exists — `SESSION_STARTED`, `COMMAND_STARTED`,
`COMMAND_FINISHED` with exit codes, `CWD_CHANGED`, session state transitions. Correlation ("relevant
change", "last known successful") is M3.

## Progressive disclosure

Four levels, each entered deliberately:

```text
1  quiet        status line + terminal + Ask
2  contextual   a band appears because an observed fact justifies it
3  palette      Leader p: everything, searchable (subsequence, so "ncs" finds New Claude session)
4  overlay      a full panel — sessions, evidence, help, welcome
```

A fifth thing sits beside rather than above this list: **scrollback** (`Leader [`) is a bar along the
bottom of the terminal rather than a panel in front of it, because what is being looked at *is* the
terminal. `↑↓`, the wheel, `/` to search, `n` to repeat, `g` for the latest output, `Esc` to return
to live — and returning to live is automatic on exit, or new output would arrive somewhere the user
cannot see.

Level 2 is the only one Verb enters on its own, and only from observed evidence. Levels 3 and 4 are
always the user's move, without exception.

This is what keeps Verb from becoming an assistant that interrupts. A tool that volunteers a panel
because it *suspects* something is a tool people learn to fight; one that surfaces exactly when a
command actually failed is one they learn to trust.

## Keyboard model

The hard constraint: **the terminal owns the keyboard.** Claude, Codex and OpenCode are full-screen
TUIs that use nearly every key, and a shell underneath them uses most of the control keys as
readline bindings. Verb cannot claim a global key without breaking the thing it exists to host.

An earlier draft of this document reserved `Ctrl+K` globally. That was wrong on its own terms:
`Ctrl+K` is readline's *kill to end of line*, a key people use constantly, and reserving it
contradicts the sentence directly above it.

### The mouse

The same rule as the keyboard, for the same reason. Verb captures the mouse **only while one of its
surfaces is open**; the rest of the time the terminal keeps it, and with it native selection and
copy, which are not Verb's to take. Inside a surface: the wheel scrolls, a click selects the row
under the pointer, and a click outside the panel closes it.

### Verb Leader

Verb takes a **leader chord**, in the tmux sense: one key that begins a Verb command, followed by a
key that names it.

```text
F1          help                     Leader p    commands
F2          sessions                 Leader s    sessions
F3          what Verb has observed   Leader v    what Verb has observed
F4          commands                 Leader [    look back through earlier output
                                     Leader ?    help
```

Two ways in, on purpose. The **action bar** along the bottom names the four most useful actions and
their keys, so nothing has to be memorised or discovered. The **leader** reaches everything and stays
the interface for anyone who prefers a chord; pressing it opens a menu that **stays open until it is
answered** — no timer, because a menu that vanishes while being read has to be learned rather than
used.

The function keys are **accelerators, not reservations**. They are Verb's only at a shell prompt: the
moment a full-screen application takes the alternate screen — vim, less, Claude, Codex, OpenCode —
they belong to that application, and the bar stops showing them rather than advertising a key that
would not work. `VERB_FKEYS=off` disables them entirely.

Tested the same way the leader was, before being bound (see the collision method below):

```text
             bash        zsh         vim          less        Claude
F1–F4        unbound     unbound     unmapped     unbound     no reaction
F5–F6        leaks "5~"  leaks "7~"  unmapped     unbound     no reaction
```

Nothing loses a function to F1–F4, which is why they are the four. F5 and F6 leak characters into a
readline buffer, so they are not used. Codex and OpenCode still need the same test on the Android
device; the alternate-screen rule means they keep their own keys regardless.

The rules that make a leader safe to live with:

* **Nothing else is reserved.** Every other keystroke goes to the terminal, untouched, including
  every readline binding and every key an agent TUI wants.
* **The leader is configurable, and remapping is a first-class feature, not an escape hatch.** Verb
  is not constitutionally bound to any particular chord.
* **`Leader Leader` sends the leader key itself to the terminal**, so a bound key remains reachable
  even when it is the leader.
* **A leader followed by an unbound key forwards both** to the terminal rather than swallowing them.
  Verb never eats a keystroke it has no meaning for.
* **A leader with no follow-up within a short timeout forwards the leader** and returns to normal.
  A half-typed Verb command must not leave the terminal in a state the user did not ask for.
* **`Esc` closes the topmost Verb surface** and returns the keyboard to the terminal. It is never
  swallowed when no Verb surface is open.
* Contextual `[e] Explain` style hints are shortcuts *within* a focused Verb surface, never global
  captures. Everything reachable by a shortcut is also reachable by name in the palette.
* Any surface that hands the whole terminal to an agent gives the keyboard over completely and takes
  it back afterwards, as the current UI already does.

### Choosing the default

**The default is deliberately not chosen in this document.** It is chosen after collision testing,
and the result is recorded here.

Candidates worth testing, with what is already known against each:

```text
Ctrl+Space   often unbound; sends NUL, which readline treats as set-mark
Ctrl+O       readline operate-and-get-next; rarely used interactively
Ctrl+G       readline abort; used to cancel an incremental search
Ctrl+B       tmux's own default prefix — collides for tmux users
Ctrl+A       screen's prefix, and readline beginning-of-line
Ctrl+K       readline kill-to-end-of-line — rejected above
```

### Results so far

Run with the method below. "reacted" means the program emitted something in response — a bell, a
cursor move, a redraw — with text already typed on the line, because a binding that edits text shows
nothing on an empty one.

```text
                bash      zsh       Claude    Codex     OpenCode
Ctrl+Space      clean     clean     clean     pending   pending
Ctrl+]          clean     reacted   clean     pending   pending
Ctrl+^          reacted   reacted   clean     pending   pending
Ctrl+O          clean     reacted   reacted   pending   pending
Ctrl+G          reacted   reacted   reacted   pending   pending
Ctrl+B          reacted   reacted   reacted   pending   pending
Ctrl+A          reacted   reacted   reacted   pending   pending
```

`Ctrl+Space` leads on the evidence available. Two things are known about it that the table does not
show: it is the NUL byte, so a terminal reports it as either `Ctrl+Space` or `Ctrl+@` and Verb
normalises the two; and some terminal emulators do not transmit it at all, which would make it
useless *on that terminal* rather than wrong in general. That is an argument for remapping being
first-class, which it is.

Codex and OpenCode are pending because they live on the Android validation device. The default
settles when their rows are filled in, and the implementation ships a clearly-marked provisional
binding until then — never a silent one.

The test is the same for each candidate, and it is an observation, not an opinion:

```text
For each of: bash, zsh, Claude Code, Codex, OpenCode
  1. start it inside a Verb PTY session
  2. send the candidate chord
  3. record whether anything observably happens — text edited, mode changed,
     screen redrawn, command cancelled, process signalled
Accept only a candidate with no observable effect in all five.
```

If no candidate is clean everywhere, the default goes to the one that is clean in the shells and
collides only with a documented agent binding, and the collision is written down next to it. Guessing
is not an option here: the whole point is that Verb does not claim facts — or keys — it has not
checked.

Until that test is run and its result recorded here, the implementation treats the leader as
configuration with a **provisional** default rather than a settled one: `VERB_LEADER` selects it,
`Leader ?` shows the current binding, and the status line carries the hint, so a user never has to
remember which chord this install uses and never discovers the binding by having it fire. Provisional
means the doc says so and the release notes say so — not that it is hidden. A configuration file
replaces the environment variable when Verb has one; the abstraction does not change when it does.

The leader is one chord and the follow-up keys are ordinary letters, deliberately. A user who
remaps the leader keeps every Verb command they already know.

## Ratatui, and where the dependency line sits

The crate is dependency-free today, and that has been worth defending: the desktop host builds
anywhere, audits quickly, and has no supply chain to speak of. Ratatui and its input backend are the
first dependencies worth taking, because layout, overlays and resize handling are exactly the code
that is tedious to hand-roll and boring to own.

The line is drawn by layer, not by preference:

```text
session records · agent adapters · PTY host · event log · CLI    dependency-free
TUI rendering and input                                          may depend on Ratatui
```

Consequences that keep this honest:

* The UI layer may not hold state the core does not, so removing Ratatui would cost a renderer, not
  a product.
* `verb status`, `verb sessions`, `verb resume` and `--json` keep working with no UI compiled in.
* A dependency is added when it removes work Verb should not be doing, never for convenience alone.

## Mapping: every action to a capability

The rule is not decorative, so the mapping is written down:

```text
UI action                     underlying capability
──────────────────────────────────────────────────────────────
Resume selected session       resume_session()      = verb resume
Start new session             launch_session()      = verb claude | codex | opencode
Session list / switch         read session records  = verb sessions
Re-check recovery             reconcile_session()   = verb status
Show evidence / raw events    the JSONL event log   = (M2/M3 surface over existing files)
Explain / Diagnose            M2 assistant over the same evidence
Ask Verb                      M2 — inactive in M1, so it maps to nothing and does nothing
```

An action with no capability behind it does not ship. If a surface needs something new, the
capability lands first, in the core, reachable from the CLI.

## The M1 slice

**In:**

* the quiet state: status line, terminal region, and the Ask region drawn dim, inactive and
  non-focusable — reserved space, no placeholder behaviour
* a welcome shown once, on a first run: the keys, and the promise that everything else belongs to
  the terminal
* plain words for session state on screen — running, recoverable, checking, ended — with the
  contract's own terms (`LIVE`, `RECOVERABLE`, `INTERRUPTED`, `ENDED`) still what `verb status`,
  `--json` and the durable record say, and both spellings shown together in help
* scrollback with search, and the mouse policy above
* the evidence overlay (`Leader v`), rendering the same assembly `verb context` prints
* the Verb Leader abstraction, configurable, plus the collision test above run against bash, zsh,
  Claude, Codex and OpenCode, with the chosen default recorded in this document
* session list and switching as an overlay
* resume and start-new from the UI
* command palette (`Leader p`) over the capabilities that exist today
* contextual band for the triggers Verb can already observe: command failed, agent session state
  changed
* resize handling, `NO_COLOR`, SSH-friendly behaviour

**Out:**

* the assistant (M2) — including any input that accepts a question, correlation and the debug view's
  "relevant change" (M3)
* Git panes, file trees, dashboards
* CI, dependency and remote environment understanding (M4)
* any plugin surface (M5)

**Done means:** a person can see the state of their work, act on it, and reach full power without
remembering command names — and every action in the UI maps to a capability that a script can call.

And once a leader is chosen, one more test, which is the real one:

> Can I spend an hour inside Claude, Codex or OpenCode through Verb and forget Verb is intercepting
> the keyboard until I deliberately invoke it?

If yes, the boundary is right.

## Related documents

* `docs/PRODUCT_VISION.md` — why Verb exists.
* `docs/PRD.md` — the problem, the pillars, the non-goals.
* `docs/ROADMAP.md` — M1 is the current scope freeze.
* `docs/DESKTOP_MVP.md` — what the desktop backend already provides.
