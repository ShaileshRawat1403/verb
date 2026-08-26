# Verb documentation

Two kinds of document live here, and they answer to each other in one direction only.

## Canonical product documents

These define what Verb is and what gets built. Implementation documents answer to them; they do not
answer to implementation documents.

| Document | Question it answers |
| --- | --- |
| [`PRODUCT_VISION.md`](PRODUCT_VISION.md) | Why Verb exists, and what it refuses to compete on. |
| [`PRD.md`](PRD.md) | What problem is being solved, for whom, and what is deliberately not built. |
| [`ROADMAP.md`](ROADMAP.md) | Which capability arrives when, and what "finished" means for each. |
| [`UX_FOUNDATION.md`](UX_FOUNDATION.md) | How Verb should feel, and the rules that keep it feeling that way. Host-agnostic. |
| [`TUI_VISION.md`](TUI_VISION.md) | How the desktop experience works, down to the mockups — the current implementation of the foundation. |
| [`BACKLOG.md`](BACKLOG.md) | What is undone, what it costs, and the current sprint. |

**Current scope: the M1 desktop workspace is built and frozen for dogfooding.** Reliability fixes
continue where tests or real use demonstrate defects. Manual cross-host structural continuity is a
bounded beta capability; it transports evidence, never process authority or transcript content.
The Android workspace is terminal-first and exposes named tasks through one searchable Verb sheet;
that workspace and all three admitted agent launch paths have physical-device acceptance. OpenCode
recovery remains an explicit beta gap. M2 is implemented as a reviewed, provider-neutral,
evidence-bound assistant with physical-device acceptance; it is one surface, reachable from Ask Verb
and from the terminal.

## Implementation documents

What was measured, decided and built. Several are dated snapshots and say so; a snapshot that gets
edited to look correct stops being evidence.

| Document | Subject |
| --- | --- |
| [`VERB_SESSION_CONTRACT.md`](VERB_SESSION_CONTRACT.md) | The session states, the adapter boundary, and why `unknown` is not `no`. |
| [`VERB_SESSION_SCHEMA.md`](VERB_SESSION_SCHEMA.md) | The host-neutral durable record and event shapes shared by Android and desktop. |
| [`VERB_CONTINUITY_ENVELOPE.md`](VERB_CONTINUITY_ENVELOPE.md) | The manual, evidence-only Android↔desktop transport and its strict privacy boundary. |
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | A contributor-oriented map of hosts, truth resolution, persistence and AI boundaries. |
| [`DURABLE_SESSION.md`](DURABLE_SESSION.md) | What survives what on Android — the evidence the contract was designed against. |
| [`DESKTOP_MVP.md`](DESKTOP_MVP.md) | The desktop host: PTY, adapters, structural events, command surface. |
| [`WORKING_WORLD.md`](WORKING_WORLD.md) | What an Android install must not lose, how `verb export`/`verb import` protect it, and the packaging rules that keep upgrades in place. |
| [`AGENT_RUNTIME_V1.md`](AGENT_RUNTIME_V1.md) | The two Android execution environments. |
| [`TERMINAL_RUNTIME_ROADMAP.md`](TERMINAL_RUNTIME_ROADMAP.md) | Why Termux-derived components are used, and the package-management boundary. |
| [`P0.5_TERMINAL_CONTEXT.md`](P0.5_TERMINAL_CONTEXT.md) | The terminal context contract (cwd, command tracking). |
| [`AI_PROVIDERS.md`](AI_PROVIDERS.md) | The Assistant's provider boundary. |
| [`V0_DEVICE_VALIDATION.md`](V0_DEVICE_VALIDATION.md), [`V0_DEVICE_VALIDATION_RESULTS.md`](V0_DEVICE_VALIDATION_RESULTS.md) | Physical-device validation checklist and its results. |
| [`DIRECT_BETA.md`](DIRECT_BETA.md) | The direct-distribution Android build. |
| [`RELEASE_CHECKLIST.md`](RELEASE_CHECKLIST.md) | The evidence required before a public preview is tagged. |
| [`HANDOFF.md`](HANDOFF.md), [`NEXT_SPRINT.md`](NEXT_SPRINT.md) | Dated snapshots, superseded in part. |

## The rules that outrank everything in both lists

```text
Unknown ≠ No
Inference ≠ Fact
Agent claim ≠ Verified execution
```

```text
OBSERVED FACT → STRUCTURED VERB STATE → MEMORY / CONTEXT → USER INTERFACE
    → AI INTERPRETATION → USER-APPROVED ACTION
```

AI sits after evidence, not before it.
