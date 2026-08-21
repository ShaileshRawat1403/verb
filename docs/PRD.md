# Verb PRD

This is the document that rejects scope creep. When a proposal cannot be traced to a problem, a job
or a pillar below — or lands in the non-goals — it does not get built, however good it is.

## Problem

AI has dramatically increased the speed at which software can be produced, and with it the
operational and cognitive load of keeping up with what was produced.

Users regularly lose track of:

* what an agent changed
* what commands actually ran
* which environment is active
* why something failed
* what Git state means
* how local and CI environments differ
* whether a process is still running
* what is safe to undo
* how to resume after interruption
* where to find relevant documentation or context

Existing coding agents solve substantial parts of **creation**. Verb focuses on **understanding and
controlling the development process around creation**.

## Initial user

> A developer or AI-assisted builder who works in Git repositories and regularly uses commercial
> coding agents such as Claude or Codex.

Deliberately narrow. Other roles get documented when actual usage justifies them, not before.

## Core jobs

```text
Run my preferred development tools and agents.

Know the state of my project and environment.

Understand what just happened.

Understand why something failed.

See what changed.

Ask questions without restating all my context.

Take safe corrective action.

Resume or recover interrupted work.

Access deeper technical controls when I want them.
```

## Product pillars

**Access**
Bring existing shells, projects, tools and agents into one working environment.

**Understanding**
Translate development state into something a human can understand.

**Control**
Make running agents, processes, permissions and changes inspectable and interruptible.

**Debugging**
Connect failures to the execution, environment and changes that produced them.

**Context & Memory**
Remember factual working state and relevant history so conversation starts with context rather than
from zero.

## UX principle

> **Low configuration by default. Full power when requested. Natural-language guidance at every
> layer.**

> **Capability can be large while the visible interface remains small.**

## Non-goals

* not another VS Code
* not a general-purpose IDE
* not a replacement coding agent
* not a new model provider
* not a universal agent harness
* not autonomous orchestration by default
* not a Git GUI for its own sake
* not dashboards everywhere
* not hiding uncertainty
* not vendor-specific architecture
* not cloud sync until there is a demonstrated need

## The architectural rule beneath everything

```text
OBSERVED FACT
    ↓
STRUCTURED VERB STATE
    ↓
MEMORY / CONTEXT
    ↓
USER INTERFACE
    ↓
AI INTERPRETATION
    ↓
USER-APPROVED ACTION
```

AI sits **after** evidence, not before it. An interpretation that cannot point at the observed fact
it came from is a guess wearing a confident voice, and Verb's whole value is being the part of the
stack that does not do that.

The identity rules follow from the same place:

```text
Unknown ≠ No
Inference ≠ Fact
Agent claim ≠ Verified execution
```

## What exists today

Recorded here so the PRD can be read against reality rather than intention. Every claim below is
implemented and tested; the ones proven on physical hardware say so.

**Access.** Android hosts agents in a proot userland through a real PTY; desktop hosts them through
a native Unix PTY (`desktop/src/pty.rs`). Claude, Codex and OpenCode are installable and launchable
on Android; `dsh` is not installable there and the card says why rather than offering a button that
always fails.

**Understanding (structural).** Sessions carry project, runtime, agent, working directory and state.
Desktop reads the shell's own OSC 7 / OSC 633 / OSC 133 markers into structural events — working
directory changes and command boundaries with exit codes — and never records the command text.

**Control.** One session lifecycle for every agent (`AgentSessionCoordinator` on Android, the same
contract in Rust on desktop): `LIVE → INTERRUPTED → RECOVERABLE → ENDED`, with resume driven by the
agent's own conversation identity. Verified end to end on a Vivo I2202 for Claude and Codex,
including force-stop and recovery into the same session id.

**Debugging.** A structural JSONL event log per session, and `--json` output shaped by
`docs/VERB_SESSION_SCHEMA.md`. This is the substrate the debugging pillar will build on; the
connecting and explaining is not built.

**Context & Memory.** Durable session records only: identity, execution context, remembered
observations, agent resume identity, state. No PID, no process handle, no terminal bytes, no
transcripts, no credentials.

**Not built:** natural-language assistance over that evidence, failure-to-change correlation,
recovery suggestions, CI or dependency understanding, and any plugin surface. Those are milestones
in `docs/ROADMAP.md`, deliberately not started.

## Related documents

* `docs/PRODUCT_VISION.md` — why Verb exists.
* `docs/ROADMAP.md` — the order capability arrives in.
* `docs/TUI_VISION.md` — the desktop experience.
* `docs/VERB_SESSION_CONTRACT.md`, `docs/VERB_SESSION_SCHEMA.md` — the session semantics both hosts
  implement.
