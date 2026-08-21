# Verb Roadmap

Capability-based, not date-based. A milestone is finished when its exit condition is true, not when
a date arrives.

```text
M0 — TRUTH
     Know what actually happened.

M1 — WORKSPACE
     Make that truth easy to see and control.

M2 — ASSIST
     Let users ask Verb about the work.

M3 — DEBUG
     Connect failures, changes and recovery.

M4 — OPERATE
     Extend understanding into Git, CI and environments.

M5 — EXTEND
     Let future agents, runtimes and capabilities plug in cleanly.
```

---

## M0 — Truth substrate

**Status: substantially complete on both hosts.**

Sessions, PTY lifecycle, working directory, command boundaries, exit status, Git state, runtime
state, agent identity, durable session records, resume and recovery.

Implemented and verified:

* One session lifecycle shared by every agent — `AgentSessionCoordinator` on Android, the same
  contract in Rust on desktop. Adding an agent adds an adapter, never a state machine.
* Agent adapters that read each agent's own evidence: Claude's transcripts and session metadata,
  Codex's rollout files, OpenCode's SQLite database. Each was written only after the real format was
  observed on an installed build.
* Durable records that hold no PID, no process handle, no terminal bytes, no transcripts.
* Structural event log on desktop, including shell-integration command boundaries and cwd changes.
* Physical-device proof on a Vivo I2202 (Android 14) for Claude and Codex: real conversation,
  force-stop with every process confirmed gone, relaunch into the same `VerbSession.id`, resume by
  the agent's own conversation identity, prior context restored.

**Exit condition:** Verb does not claim facts it cannot prove.

Outstanding against that condition:

* OpenCode's device proof needs a signed-in provider on the validation device. The adapter and its
  tests exist; the end-to-end run does not.
* `dsh` has no observable resume contract because it cannot be installed on Android at all (its
  `koffi` native module has no Android build). Recorded as unavailable with the reason, not guessed.

---

## M1 — Desktop workspace

**Status: the immediate focus, and the only thing being built now.**

Move the current `verb ui` scaffold to Ratatui and design one strong default workspace:

```text
project · git · runtime · agent · session

                 TERMINAL

contextual actions when relevant

              Ask Verb…
```

Needed:

* terminal-first layout
* project / Git / runtime / session awareness
* the Verb Leader abstraction — configurable, with the collision test against bash, zsh, Claude,
  Codex and OpenCode run and its result recorded in `docs/TUI_VISION.md` before a default settles
* command palette
* contextual overlays
* session switching and resume
* full-power controls accessible but unobtrusive
* keyboard-first interaction
* SSH-friendly operation

Not in M1: no file-tree IDE clone, no permanent panes, no assistant. The Ask region is drawn in the
layout but inactive and non-focusable until M2 — reserved space, never placeholder behaviour that
accepts a question nothing can answer.

The interaction model, layout rules and mockups are specified in `docs/TUI_VISION.md`, which the
implementation is built against rather than designed during.

**Exit condition:** a person can see the state of their work, act on it, and reach full power
without remembering command names — and every action in the UI maps to an underlying capability that
is also reachable from the CLI.

---

## M2 — Contextual Verb Assistant

**Status: documented, not started.**

Where natural language becomes important. The user asks *Why did this fail? What changed? What did
Claude do? Can I safely undo this? Why is CI different?* and Verb builds the context itself from
structured evidence.

The architecture separates:

```text
User intent
     +
Verb context
     +
chosen model
     =
assistance
```

The model is replaceable. The context system is Verb. Anything that makes a specific provider
load-bearing violates the constitutional constraint in `docs/PRODUCT_VISION.md`.

**Exit condition:** a user can ask a question about their own work without restating context Verb
already holds, and every answer can name the evidence it came from.

---

## M3 — Debugging and recovery

**Status: documented, not started.** The causal-memory idea belongs here and nowhere earlier.

```text
working state
      ↓
action
      ↓
changes
      ↓
failure
      ↓
diagnosis / recovery
```

Candidate capabilities: execution timeline, last known successful state, relevant file and
dependency changes, agent actions associated with a failure, before/after comparison, safe rollback
suggestions, reproducing important steps.

Not full event-sourcing theatre. Only what a user can see the value of.

**Exit condition:** a failure can be traced to the change that produced it, and the safe next action
is offered rather than researched.

---

## M4 — Operational understanding

**Status: documented, not started.**

Expand carefully into what repeatedly causes pain: Git → CI → runtimes → dependencies → processes →
remote environments. Verb does not replace GitHub Actions or package managers. It explains and
connects them.

**Exit condition:** the common "works locally, fails in CI" class of question is answerable from
evidence Verb already holds.

---

## M5 — Capability ecosystem

**Status: documented, not started.** Only once the product loop works.

Future systems plug into stable interfaces:

```text
AgentAdapter
RuntimeAdapter
ContextProvider
ActionProvider
EvidenceProvider
```

`AgentAdapter` already exists in both hosts and is the proof the shape works: three agents, three
adapters, one lifecycle. The others are named here so the architecture can accommodate them, not so
they can be built early.

**Exit condition:** a new agent or runtime can be supported without changing Verb's core.

---

## Scope freeze

**We build only M1 now.**

No causal memory system. No CI integrations. No plugin SDK. No new harness. No autonomous assistant.

Later milestones are documented so the architecture can accommodate them, and then deliberately not
built.
