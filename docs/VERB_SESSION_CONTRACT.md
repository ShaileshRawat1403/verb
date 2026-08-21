# VerbSession — the platform-agnostic contract

Status: **the data shape, the shared session lifecycle, and three `AgentAdapter`s are implemented
and wired into the app.** `VerbSession`, `VerbSessionState`, `ProcessBinding`, and `AgentRef` exist in
`com.example.verb.session` (commit `432251b`); `VerbTerminalSessionHolder` gives session lifetime an
owner above the ViewModel (commit `241b739`); `ClaudeAgentAdapter` proves the full
`LIVE -> INTERRUPTED -> RECOVERABLE -> LIVE` cycle against real Claude transcript detection (see
`ClaudeResumeCycleTest`), verified end-to-end on a physical Android device across force-stop.

`AgentSessionCoordinator` now owns that lifecycle for *every* agent: there is one state machine, and
an agent joins it by supplying an adapter (`ClaudeSessionCoordinator`, `CodexSessionCoordinator`,
`OpenCodeSessionCoordinator`) and
its own durable store (`SharedPreferencesVerbSessionStore`, one preferences file per agent, so one
agent's launch can never erase another's recovery record). `CodexAgentAdapter` reads Codex's rollout files and
`OpenCodeAgentAdapter` reads OpenCode's SQLite session database; `dsh` still needs its own adapter.

The desktop host in `desktop/` implements the same contract in Rust against the same three agents
(`desktop/src/agents.rs`), including the agent resume identity the schema defines -- one contract,
two hosts, no second state machine.

This is the shape from `docs/HANDOFF.md`'s "Durable Session, step 2", made precise enough to type
and to test against. It adds nothing conceptually new — it resolves the ambiguities that would
otherwise get decided differently by whoever writes the Android code versus whoever writes the
desktop code.

## One conceptual distinction to preserve

`VerbTerminalSessionHolder` is **process-scoped runtime ownership** -- it answers "does a
`TerminalRuntime` already exist, so a new `VerbViewModel` can reattach instead of spawning a
duplicate." `VerbSession.id` is **product-level session identity** -- it answers "is this the same
piece of work the user was doing, independent of whether any process currently backs it."

They are related -- a live `VerbSession` will typically point at whatever `TerminalRuntime` the
holder is holding -- but they must not become the same abstraction. The holder has no concept of
`INTERRUPTED` or `RECOVERABLE`; it only ever knows "a runtime exists" or "it doesn't." `VerbSession`
is what makes "the process is gone, but the work is not" a meaningful sentence at all.

## The one rule everything else follows

**`VerbSession` is not the PTY.** The PTY (or on desktop, whatever native process backs the shell)
is a temporary execution resource a session may or may not currently hold. A session's identity —
its `id` — is stable across that resource being created, dying, and being replaced. This is what
makes "process died, but you can still resume" a coherent statement instead of a contradiction.

## Shape

```
VerbSession
  id                String   -- opaque, generated once at creation, stable for the session's life,
                                 never reused, never recomputed from other fields
  projectId         String?  -- the project this session actually launched under; fixed at creation
  runtime           String?  -- the RuntimeProfile/agent context this session actually launched
                                 with; fixed at creation, null for bare shell
  lastKnownCwd      String?  -- see "Known vs remembered", below
  lastObservedAt    Instant? -- when lastKnownCwd was last confirmed true, not merely recorded
  createdAt         Instant
  lastSeenAt        Instant  -- updated whenever the session itself is observed live (distinct from
                                 lastObservedAt, which is specifically about cwd freshness)
  state             LIVE | INTERRUPTED | RECOVERABLE | ENDED
  process           ProcessBinding?   -- see "What ProcessBinding is, deliberately", below
  agent             AgentRef?
    agentType         String   -- "claude" | "codex" | "opencode" | "dsh" | ...
    resumeIdentity    String?  -- opaque to VerbSession; only the owning AgentAdapter interprets it
                                  (e.g. Claude's own session uuid for --resume). Can outlive the
                                  VerbSession itself: the transcript is on disk independent of any
                                  session bookkeeping, per docs/DURABLE_SESSION.md.
```

### Known vs remembered

`lastKnownCwd` is never called "current" and never called "authoritative" — Verb cannot claim to
know the working directory of a process that no longer exists. The two fields together say exactly
how stale the claim is:

- **While `LIVE`** and shell integration (OSC 7) is active, `lastKnownCwd` is live observed truth and
  `lastObservedAt` tracks essentially in real time.
- **While `INTERRUPTED` or `RECOVERABLE`**, `lastKnownCwd` is simply the last thing observed before
  the process went away — still useful (it's what a resume should default to), but presented as
  remembered, not current.

This is the same distinction the rest of Verb already holds to: known facts and remembered facts are
different claims, and the UI must not blur them into one string.

### `projectId` and `runtime` are fixed at creation, not live-editable

**`selectedProject` (the UI's current picker state) is not `session.projectId`.** If Session A was
created under Project A and the user then selects Project B in the UI, Session A's `projectId`
does not change — the UI is now pointed at a different session (or no session), and Session A keeps
existing, attached to Project A, in the background. The same holds for `runtime`: switching the
desired runtime in the UI does not rewrite a running session's `runtime` field. A session's
`projectId`/`runtime` describe the execution context that actually launched it; wanting a different
one means deliberately creating or resuming a session with that context, never mutating the current
one in place.

### What `ProcessBinding` is, deliberately

`ProcessBinding` is the **only** field that differs in shape between hosts, and shared logic never
looks inside it — only at whether it is present or absent.

It is runtime-only state. It is not part of the durable/session interchange schema and must never be
reconstructed from a persisted `LIVE` value. On startup, a host must re-establish the binding from
its own process owner; if no binding is confirmed, it resolves the session through the existing
`INTERRUPTED` / `RECOVERABLE` / `ENDED` mapping below. Persisted history is evidence of what was true,
not proof of what is true now.

- Android: today's PTY/proot process handle (whatever `TerminalRuntime` already holds).
- Desktop: a native PTY handle (whatever the Rust process host holds).

Android's existing `TerminalSessionState` (`STARTING`, `RUNNING`, `EXITED`, `FAILED`, `STOPPING`)
is **not replaced** by this contract — it continues to describe the `ProcessBinding`'s own lifecycle.
`VerbSession.state` is one level up: it's what you get after asking "given the current
`ProcessBinding` state, plus what the owning `AgentAdapter` says about resumability, what should the
user see?" The mapping:

| ProcessBinding / TerminalSessionState | `AgentAdapter.canResume(agent)` | VerbSession.state |
| --- | --- | --- |
| present, `STARTING` or `RUNNING` | n/a | `LIVE` |
| absent, or `EXITED`/`FAILED` | `yes` | `RECOVERABLE` |
| absent, or `EXITED`/`FAILED` | `unknown` | `INTERRUPTED` |
| absent, or `EXITED`/`FAILED` | `no`, or no `agent` at all (bare shell) | `ENDED` |
| `STOPPING` | n/a | still `LIVE` until the transition above resolves |

`unknown` is **not** treated as recoverable. `RECOVERABLE` is a positive claim -- Verb has evidence
recovery will work -- and `unknown` is not evidence, it's the absence of an answer. Collapsing
`unknown` into `RECOVERABLE` would tell the user "Resume" when Verb does not actually know that it
can; collapsing it into `ENDED` would claim impossibility Verb hasn't established either. `INTERRUPTED`
says neither thing: process gone, recovery status not yet known.

### Resumability is agent-specific knowledge; `VerbSession` only consumes it

`VerbSession` does not decide whether a transcript is resumable — that requires knowing the shape of
Claude's session files versus Codex's versus OpenCode's, which is exactly the kind of per-agent
knowledge this contract is not supposed to hold. That decision lives behind an adapter:

```
AgentAdapter
  canResume(agentRef: AgentRef) -> yes | no | unknown
  suspend resume(agentRef: AgentRef) -> ProcessBinding?
```

Claude's adapter knows how to check Claude's transcripts; Codex's adapter knows Codex's. `VerbSession`
only ever sees the `yes | no | unknown` result, the same way it only ever sees "present or absent"
for `ProcessBinding`. `resume` returns `null` on failure -- no separate failure signal, deliberately:
a caller branching only on null-vs-non-null cannot accidentally treat a failure as success.
`ClaudeAgentAdapter` (`docs/HANDOFF.md`'s Claude-first slice) reads `canResume` from transcript
presence under `~/.claude/projects/<cwd, "/" replaced by "-">/` and from Claude's own session
metadata under `~/.claude/sessions/`, whose filename is a PID and is therefore never used as
identity. `CodexAgentAdapter` reads Codex's rollout files under
`~/.codex/sessions/<yyyy>/<mm>/<dd>/rollout-<timestamp>-<session id>.jsonl`, matching the header
record's `cwd` and taking its `id` as the resume identity; it resumes with `codex resume <id>`
(`--last` when no id is known, never the bare `codex resume`, which opens an interactive picker).
`OpenCodeAgentAdapter` has no transcripts to read at all: OpenCode keeps its sessions in the SQLite
database `~/.local/share/opencode/opencode.db`, so the adapter copies that database (never opening a
live one in place) and reads `session`/`message` rows, resuming with `opencode --session <id>`
(`--continue` when no id is known).

Both detect `resume` failure the same way, through the shared `AgentResumeLauncher`: from the
agent's own command-history settling before an interactive session would ever return control -- not
from a transcript scan, since a resumed agent keeps running rather than writing anything new to
check.

### Opened is not used: what counts as recovery evidence

Codex writes a rollout file the moment it starts, and injects its own `<environment_context>` as a
*user-role* message. Either one, taken as evidence, would make Verb offer "Resume" for a session the
user only ever opened and then resume an empty conversation. `CodexAgentAdapter` therefore requires a
user-role record that is not one of Codex's own injected blocks (read from a real rollout written by
codex-cli 0.149.0 on the validation device, not assumed). This is the general rule for every future
adapter: **the evidence must prove a conversation happened, not that a process started.**

No adapter ever persists what it reads. Rollouts and transcripts are scanned for a marker; the lines
themselves are never returned, stored, or logged.

## Transitions

```
create()              -> LIVE          (process present)
LIVE -> RECOVERABLE       process reference lost; AgentAdapter.canResume() already known yes
LIVE -> INTERRUPTED       process reference lost; AgentAdapter.canResume() is unknown
LIVE -> ENDED             explicit close with nothing resumable, or process ended with nothing to
                          resume (no agent, or AgentAdapter.canResume() already known no)
INTERRUPTED -> RECOVERABLE   AgentAdapter later establishes canResume() == yes
INTERRUPTED -> ENDED         AgentAdapter later establishes canResume() == no
RECOVERABLE -> LIVE       user resumes/attaches; a NEW ProcessBinding is created; id is unchanged
RECOVERABLE -> ENDED      user discards
```

`INTERRUPTED` is expected to resolve, not to sit: whoever owns the transition (today the ViewModel,
later a service) is responsible for re-invoking `AgentAdapter.canResume()` -- on a retry, on
reconnect, whatever the adapter needs -- until it lands on `RECOVERABLE` or `ENDED`. It is a waiting
state, not a resting one.

`RECOVERABLE -> LIVE` resumes the same `VerbSession` identity with a new process. `ENDED` is
terminal and cannot resume: starting again creates a **new** `VerbSession`, with a new `id` — it is
only the "start over from `ENDED`" path that mints a fresh identity, never the resume path.

## Invariants

1. **`id` never changes** across `LIVE -> INTERRUPTED -> RECOVERABLE -> LIVE` (or any subpath of it).
   That persistence is the entire point of the contract; anything that regenerates `id` on resume has
   broken it.
2. **State is written by whoever owns the process**, never inferred by a screen from "is something
   drawn." Today that owner is the Activity-scoped ViewModel; later it may be a service. The UI
   reads `state`, it does not compute it from side signals (this is the mistake `docs/DURABLE_SESSION.md`
   already found once, where the UI's own lifetime got treated as the session's).
3. **`lastKnownCwd` and `agent.resumeIdentity` remain valid with no live `process`.** They are what
   `RECOVERABLE` is built from, not values that go stale the moment the process dies — they just stop
   being "current" and become "remembered" (see "Known vs remembered", above).
4. **A session change is not a session destroy, and never mutates the session's recorded context.**
   Project switch, runtime switch, and any future "reconnect the screen to a session" action operate
   on which `VerbSession` the UI is pointed at, and never rewrite an existing session's `projectId`
   or `runtime` (see "`projectId` and `runtime` are fixed at creation", above). (This is
   `docs/DURABLE_SESSION.md`'s row 2, already diagnosed as self-inflicted, not fixed by this contract
   but no longer excusable once it exists.)

## Explicitly out of scope here

Kept out on purpose, per the steer against over-generalizing for hypothetical needs:

- **Multiple agents per session.** One optional `agent` per session matches today's reality (one PTY,
  one shell, at most one foreground agent). If that stops being true, it's a new field then, not a
  speculative array now.
- **Cross-device sync.** P4's "work context" is a different, looser object built from multiple
  sessions' history — not a field on this one.
- **Persistence format.** Whether this is stored as a Room entity, a flat file, or in-memory only is
  an implementation choice for step 2, not part of the contract.
- **Process supervision** (foreground service, `tmux`/`dtach`). Those are *consumers* of `state`
  transitions, not definitions of them. `docs/DURABLE_SESSION.md` already argues for deciding the
  multiplexer question last; this contract doesn't need it decided at all.

## The resulting mental model

```
VerbSession
    │
    ├── stable identity
    ├── execution context that actually launched it (projectId, runtime -- fixed)
    ├── last observed facts (lastKnownCwd, lastObservedAt -- known vs remembered)
    ├── optional agent resume reference (consumed via AgentAdapter, not interpreted here)
    │
    └── ProcessBinding?
             │
             └── disposable, host-specific PTY/process
```

```
LIVE                INTERRUPTED              RECOVERABLE              LIVE
same VerbSession     process dies,    canResume    same VerbSession    resume    same VerbSession
process 123        resumability      == yes      no ProcessBinding   -------->   process 456
                    unknown yet
```

The UI reflects each step honestly rather than jumping straight to an optimistic "Resume":

```
Claude
Session interrupted
Checking recovery options…

OpenCode
Session interrupted
Recovery status unknown

Claude
Session recoverable
[Resume]
```

## Next step, once this is agreed

Implement against Android only, replacing `onCleared() -> terminalRuntime.destroy()` and the
project-switch destroy-and-restart with code that operates on `VerbSession.state` instead. Desktop
is not touched until Android has proven the contract holds under the seven rows in
`docs/DURABLE_SESSION.md`'s survivability table.
