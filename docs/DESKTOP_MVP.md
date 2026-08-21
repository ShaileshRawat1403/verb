# Desktop MVP cut line

Status: **started 2026-08-21**

Android's first product slice is now the stopping point for this phase: the runtime truth work is
done, the platform-agnostic `VerbSession` contract exists, and Claude's four session states are
visible in the existing Agents surface. The remaining Android work is supervision/persistence and
device-authenticated long-session validation. It is not a prerequisite for proving the desktop
product thesis.

The desktop MVP is the native CLI in `desktop/`.

## First vertical slice

```text
verb status
  project root + Git branch + changed files + last session

verb claude / codex / opencode / dsh
  launch the existing agent in the current Git project
  proxy the interactive process through a Unix PTY
  attach Verb session identity through environment variables
  persist session state and structured JSONL execution events

verb resume
  resumes the tracked session by the agent's own stable conversation id
    claude --resume <id>      (falls back to --continue)
    codex resume <id>         (falls back to resume --last)
    opencode --session <id>   (falls back to --continue)
```

This is intentionally a shell-first desktop product. The host already supplies the shell, process
runtime, Git, and agent binaries; Verb owns the work context and the honest session boundary above
them.

## Deliberate non-goals

- no desktop UI yet — the native PTY host is the backend proof, not the product surface;
- no cross-device sync;
- no credential storage or provider proxy;
- no guessed resume behavior for Codex, OpenCode, or DSH;
- no second Rust session state machine;
- no raw PTY input/output in durable storage.

## Agent adapters (`desktop/src/agents.rs`)

Added once each agent's real resume contract had been observed on an installed build, never guessed:

- **Claude** — transcripts under `~/.claude/projects/<cwd with "/" as "-">/*.jsonl`, plus
  `~/.claude/sessions/*.json` for the stable conversation id. The metadata filename is a PID and is
  never used as identity.
- **Codex** — rollout files under `~/.codex/sessions/<yyyy>/<mm>/<dd>/rollout-<ts>-<id>.jsonl`;
  header `payload.id` is the identity, `payload.cwd` the project match.
- **OpenCode** — no transcripts at all: sessions live in the SQLite database
  `~/.local/share/opencode/opencode.db`. The desktop host queries it through the `sqlite3` it
  already provides, read-only and `immutable=1`, so a running OpenCode is never locked or disturbed.
  No `sqlite3` on the host means no answer, which is `unknown`, not `no`.
- **`dsh`** — deliberately still `unknown`. Its resume contract has not been observed.

Two rules the adapters share with Android's, for the same reason:

- **Opened is not used.** An agent writing a session record at startup proves it was launched, not
  that there is a conversation to recover. Codex additionally injects its own `<environment_context>`
  as a user-role message, which is not a user turn either.
- **No transcript ever enters Verb's own storage.** Files are scanned for a marker; content is never
  returned, stored, or logged.

## Shell integration (`desktop/src/shell.rs`)

The host proxies PTY bytes straight through and drops them. `shell.rs` is the one place they are
*looked at*, and only for the markers a shell-integrated shell emits: **OSC 7** (working directory)
and **OSC 633/133 A/B/C/D** (prompt and command boundaries). Both spellings are read, because
Android's integration emits 633 and most desktop shells emit 133.

What the durable log gets from that is `CWD_CHANGED`, `COMMAND_STARTED`, and `COMMAND_FINISHED`,
carrying an opaque per-session `commandId`, the cwd, and an exit code -- never the command line.
`OSC 633;E` *is* the command line; it is recognised only so it can be skipped, and the event type
has no variant that could carry it.

Two properties worth keeping: the scanner holds only the bytes of a marker currently in flight
(never the stream) and gives that up past a fixed bound, so an unterminated sequence cannot grow
Verb's memory; and a shell that emits nothing produces no events at all, because silence means
"unknown", not a fabricated boundary.

## Next desktop increments

1. Observe `dsh`'s real resume contract, then give it an adapter on the same terms.
2. Add a project/session list and then put a polished desktop shell around the proven backend.
