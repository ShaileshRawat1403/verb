# Verb TUI Vision

How the desktop experience works. The Ratatui implementation is built against this document rather
than designed while coding.

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
│ Ask Verb…                                                        Ctrl+K     │
└─────────────────────────────────────────────────────────────────────────────┘
```

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
│ Ask Verb…                                                                  │
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

`Ctrl+K` is the full-power surface, and the only place capability needs to be memorised is here:

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
3  palette      Ctrl+K: everything, searchable
4  overlay      a full panel — sessions, debug view, advanced controls
```

Level 2 is the only one Verb enters on its own, and only from evidence. Levels 3 and 4 are always
the user's move.

## Keyboard model

The hard constraint: **the terminal owns the keyboard.** Claude, Codex and OpenCode are full-screen
TUIs that use nearly every key, so Verb cannot claim single keys globally without breaking the
agents it exists to host.

So:

* **One reserved chord: `Ctrl+K`.** It opens the palette and is the entry to everything.
* Contextual `[e] Explain` style hints are live only while the contextual band has focus, which
  `Ctrl+K` (or `Esc` when no agent is running) gives it. They are shortcuts *within* a surface, never
  global captures.
* `Esc` closes the topmost Verb surface and returns the keyboard to the terminal. It is never
  swallowed when no Verb surface is open.
* Everything reachable by a shortcut is also reachable by name in the palette. Shortcuts are speed,
  never the only route.
* Any surface that takes the whole terminal (launching an agent) hands the keyboard over completely
  and takes it back afterwards, as the current UI already does.

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
```

An action with no capability behind it does not ship. If a surface needs something new, the
capability lands first, in the core, reachable from the CLI.

## The M1 slice

**In:**

* the quiet state: status line, terminal region, Ask line (input captured, no assistant behind it)
* session list and switching as an overlay
* resume and start-new from the UI
* `Ctrl+K` command palette over the capabilities that exist today
* contextual band for the triggers Verb can already observe: command failed, agent session state
  changed
* resize handling, `NO_COLOR`, SSH-friendly behaviour

**Out:**

* the assistant (M2), correlation and the debug view's "relevant change" (M3)
* Git panes, file trees, dashboards
* CI, dependency and remote environment understanding (M4)
* any plugin surface (M5)

**Done means:** a person can see the state of their work, act on it, and reach full power without
remembering command names — and every action in the UI maps to a capability that a script can call.

## Related documents

* `docs/PRODUCT_VISION.md` — why Verb exists.
* `docs/PRD.md` — the problem, the pillars, the non-goals.
* `docs/ROADMAP.md` — M1 is the current scope freeze.
* `docs/DESKTOP_MVP.md` — what the desktop backend already provides.
