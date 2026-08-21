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

verb sessions
  every project with a session, newest first; read-only, and a recorded LIVE
  session is reported as unconfirmed rather than as fact

verb ui
  the same thing as a screen: move, resume, start a new session; the UI computes
  no state of its own and hands the terminal to the agent when one starts

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

## The shell (`desktop/src/ui.rs`)

Shell-first, as the thesis says: a terminal UI in the same dependency-free crate, not a window.
Raw mode through `stty` (as the PTY host already does), drawing through ANSI escapes, input read a
byte at a time. It works over SSH, which a GUI would not.

It is presentation only. It computes no session state and defines no semantics: recoverability comes
from the same resolver `verb status` uses, and resuming or launching calls the same functions
`verb resume` and `verb claude` call. Two consequences worth keeping:

- A recorded `LIVE` row reads `live?` with "this process cannot confirm it is running", the same
  honesty `verb sessions` prints.
- The footer offers only the action the selected row's state actually justifies -- `enter` on a
  recoverable session, `n` on an ended one, and neither on one whose status is still unknown.

When a session starts, the UI gives the whole terminal back before the agent runs and reclaims it
afterwards, rather than drawing underneath an interactive TUI it would fight for the cursor.

## The command surface

```text
verb            the UI on a terminal; help when piped, redirected, or in CI
verb shell      the work-context shell (what bare `verb` used to do)
verb status     one project
verb sessions   every project, as text
verb ui         the same, as a screen
verb resume     resume the tracked session
verb claude|codex|opencode|dsh|run CMD
```

```text
--json          on status and sessions: the durable record, schema-shaped
NO_COLOR        honoured by the UI
exit 0/1/2/3    success / failure / bad command line / nothing to do
```

Three rules hold this together as more is added:

1. **Everything the UI can do is also a subcommand.** The UI is a client of the same functions, so
   nothing becomes reachable only by hand -- scripts, CI and remote sessions keep working, and the
   UI can never drift into being the only place some behaviour exists.
2. **The bare command adapts to the terminal, never to guesswork.** Interactive when there is a
   person, help when there is not. Anything with side effects keeps an explicit name.
3. **Read commands are machine-consumable and say nothing they cannot prove.** `--json` emits the
   schema's field names and ISO timestamps, and never a PID or a `processPresent` -- a reader that
   needs to know whether a process exists has to ask the host that owns it, which is the whole point
   of the contract.

## Next desktop increments
2. `dsh` is on hold rather than pending: it cannot be installed on Verb's Android userland at all
   (its `koffi` native module has no Android build), so there is no runtime to observe a resume
   contract from. If it ever runs on either host, it gets an adapter on the same terms as the rest.
