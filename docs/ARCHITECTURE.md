# Verb architecture

Verb is the accountability and control layer around coding agents. Claude, Codex, OpenCode and
future adapters do AI work; Verb establishes what environment is active, what evidence exists, what
can be recovered, and which user-approved capability may run next.

The constitutional flow is:

```text
OBSERVED FACT → STRUCTURED VERB STATE → MEMORY / CONTEXT → USER INTERFACE
    → AI INTERPRETATION → USER-APPROVED ACTION
```

The ordering is an authority boundary. A model cannot create an observed fact, a UI cannot promote
an inference to truth, and an agent's record cannot prove that a host action executed.

## Hosts and shared semantics

- `app/` is the Android Compose host. Agents run inside the Termux-derived guest through one PTY.
- `desktop/` is the Rust CLI and Ratatui host. It owns a native Unix PTY while interactive.
- `runtime/` packages the Android userland; it is not a second session model.
- `docs/VERB_SESSION_CONTRACT.md` and `docs/VERB_SESSION_SCHEMA.md` define the lifecycle and durable
  vocabulary both hosts implement.

`VerbSession` is identity and structural history, not a PTY. `ProcessBinding` exists only while the
current host owns the process. Persisted `LIVE` is historical evidence. Without a confirmed local
binding, the host asks the agent adapter and maps `YES → RECOVERABLE`, `UNKNOWN → INTERRUPTED`, and
`NO → ENDED`.

## Observation and persistence

Both hosts may persist session metadata and allowlist-only structural events: lifecycle boundaries,
cwd observations, opaque command boundaries and exit status, plus agent-reported turn/tool/failure
events where an adapter can read them. Event time and observation time stay distinct.

Verb never durably stores raw PTY input/output, command text, prompts, responses, complete terminal
transcripts, agent transcript content, credentials, PIDs, PTY handles or `processPresent`. Android
purges a legacy database that once violated this rule. Source-level and export leak tests hold the
boundary.

## Agent boundary

An agent integration implements an adapter that discovers and validates the agent's own recovery
evidence. It does not modify the four states. Android's visible Agents surface admits only the three
implemented integrations; package availability alone is not product support.

## Cross-host continuity

Continuity is manual and evidence-only in the beta. A strict, checksummed `.vcont` file carries
opaque host/session identity and structural records. Imported files remain read-only outside the
local session store. Origin `recordedState` is displayed as dated history; it never enables Resume.
See `docs/VERB_CONTINUITY_ENVELOPE.md` for the complete allowlist and threat boundary.

Runtime continuity is impossible across devices. Agent conversation continuity is available only
if the destination independently finds the agent's own resume evidence. Project matching uses a
credential-free Git remote identity when available and otherwise remains unresolved.

## AI boundary

Verb is not a model provider. The existing Android explanation path sends only structural command
lifecycle facts—state, exit code, duration and whether cwd was observed—to the configured provider.
It has no raw-terminal or command-text parameter. The M2 assistant remains read-only,
provider-neutral and evidence-linked; its bounded envelope also includes recent command lifecycle
facts, shell-integration state and agent-session facts, never command text, paths or PTY output.

`TerminalEvidence` is the envelope, and it is a structured type on purpose: every field is a fact
Verb observed itself, so no caller can widen the boundary by handing it a formatted string. One
snapshot feeds both directions — `TerminalAiHelper.evidenceLines` renders it for the provider in the
contract's vocabulary, `ui/AssistEvidence` renders the same snapshot for the person in plain
language — so what the user is shown as "what the model saw" cannot drift from what was sent.

There is exactly one surface where a model answers (`ui/AssistPanel`), reachable from Ask Verb and
from the terminal. A second ask box that attached no context existed until 26 August; two boxes
answering the same question differently is the ambiguity `AskVerbScreen` was built to remove.

## Verification

Desktop format, Clippy, unit/integration tests and release build are required. Android's FullCli and
Play unit, lint and assemble tasks are required. Physical Android recovery and cross-host file/UI
acceptance remain separate evidence and must never be inferred from JVM or build success.
