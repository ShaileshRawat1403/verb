# VerbSession host-neutral schema

Status: **schema v1, 2026-08-21**

This document is the language-neutral interchange shape for Android and desktop. It mirrors
`docs/VERB_SESSION_CONTRACT.md`; it does not introduce a second session model.

## Durable session record

The durable record contains identity, execution context, remembered observations, agent resume
identity, and the product-level state:

```json
{
  "schemaVersion": 1,
  "sessionId": "opaque-session-id",
  "projectId": "project-id",
  "runtimeId": "claude",
  "lastKnownCwd": "/project",
  "lastObservedAt": "2026-08-21T15:00:00Z",
  "createdAt": "2026-08-21T14:55:00Z",
  "lastSeenAt": "2026-08-21T15:00:00Z",
  "state": "LIVE",
  "agent": {
    "agentType": "claude",
    "resumeIdentity": null
  }
}
```

`projectId`, `runtimeId`, and `agent` describe the context that actually launched the session.
They are not rewritten when the UI selects another project or runtime.

Allowed `state` values are exactly:

```text
LIVE | INTERRUPTED | RECOVERABLE | ENDED
```

`lastKnownCwd` is remembered truth when the session is not live. `lastObservedAt` says when that
working directory was actually observed, not when it was merely written down.

## Runtime-only host state

This is deliberately absent from the durable record:

```text
ProcessBinding?
TerminalSessionState
pid
PTY handle
native process object
```

`ProcessBinding` is Android/PTY or desktop/native-process machinery. A persisted `LIVE` session
does not prove that a process still exists. On host startup or attachment:

```text
persisted LIVE
    ↓
host checks for an actual ProcessBinding
    ↓
binding confirmed → LIVE
no binding       → AgentAdapter.canResume()
                     YES     → RECOVERABLE
                     UNKNOWN → INTERRUPTED
                     NO      → ENDED
```

The host must never serialize `processPresent` as part of this schema.

## Durable structural events

PTY bytes are transport data, not Verb memory. Input and output are rendered or held in an optional,
bounded in-memory diagnostic buffer and then discarded. They are not written to the durable event
log by default.

The durable event log contains only structural facts:

```text
SESSION_STARTED
SESSION_STATE_CHANGED
PROCESS_STARTED
PROCESS_ENDED
AGENT_STARTED
AGENT_ENDED
COMMAND_STARTED
COMMAND_FINISHED
CWD_CHANGED
RUNTIME_CHANGED
RECOVERY_CHECKED
SESSION_ENDED
```

Example:

```json
{
  "schemaVersion": 1,
  "type": "PROCESS_ENDED",
  "timestamp": "2026-08-21T15:00:02Z",
  "sessionId": "opaque-session-id",
  "exitCode": 0
}
```

Structural events must not contain raw command text, terminal input, terminal output, prompts,
passwords, API keys, or stderr. A command event may carry a stable opaque `commandId`, source,
cwd, duration, and exit code. A future diagnostic mode may retain a bounded, explicitly opt-in
buffer, but that is separate from durable execution memory.

## Host mapping

```text
VerbSession schema / contract
              ↓
    Android host adapter
      PTY/process binding
      AgentAdapter

    Desktop host adapter
      Unix PTY/process binding
      AgentAdapter
```

The adapters implement the existing contract. They do not define new states or reinterpret
`LIVE`, `INTERRUPTED`, `RECOVERABLE`, or `ENDED`.
