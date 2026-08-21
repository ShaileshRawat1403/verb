# Verb desktop MVP

This is the first desktop slice of Verb: a native, dependency-free CLI that sits above the
machine's existing shell, agents, and Git installation.

It intentionally starts as a terminal experience rather than a GUI. The product boundary is:

```text
Verb: project + Git context + session metadata + recovery
  ↓
native shell / agent process
  ↓
macOS, Linux, or Windows host tools
```

## Run

```bash
cargo run --manifest-path desktop/Cargo.toml -- status
cargo run --manifest-path desktop/Cargo.toml -- claude
cargo run --manifest-path desktop/Cargo.toml -- codex
cargo run --manifest-path desktop/Cargo.toml -- resume
```

To install a local `verb` command:

```bash
cargo install --path desktop
verb status
```

Verb uses the current Git repository root as the project context when one exists. Session metadata
is kept under `~/.verb/sessions`; credentials, transcripts, and agent-specific state remain owned
by the agent. `VERB_STATE_DIR` can point tests or a development build at an isolated state folder.

## Current guarantees

- `verb status` reports the project root, branch, changed-file count, and last session, including
  the agent conversation a resume would land on.
- Bare `verb` opens the UI on a terminal, and prints help when it is piped, redirected, or run in
  CI. A bare command should not do something interactive that depends on where its output is going.
  `verb shell`, which used to be the bare default, is unchanged.
- `verb ui` opens the same information as a screen: sessions newest first, arrow keys to move,
  enter to resume, `n` for a new session in that project, `r` to re-check. It hands the terminal
  back to the agent when one starts, and takes it back afterwards.
- `verb sessions` lists every project Verb holds a session for, newest first.
- `verb context` assembles everything Verb currently knows about a project -- Git state read now,
  the session record, and the tail of its structural event log -- into one place, in text or with
  `--json`. It interprets nothing: there is no field for a conclusion, and no model behind it. It is
  the groundwork every M2 direction needs (explanation, comparison and guided action all start by
  gathering the same evidence), built before choosing between them.
- `--json` on `status` and `sessions` emits the durable record exactly as `docs/VERB_SESSION_SCHEMA.md`
  defines it, ISO-8601 timestamps included, so a consumer reading one host's output does not have to
  learn the other's. `sessions --json` on an empty state is `[]`, not a message.
- Exit codes are stable: `0` success, `1` failure, `2` a wrong command line, `3` nothing to do --
  no session, or recovery not confirmed. `3` exists so a caller that retries on failure does not
  retry on a correct "there is nothing recoverable here".
- `NO_COLOR` is honoured in the UI (no-color.org). The selected row is marked with a glyph rather
  than colour alone, so the screen stays readable without it. It is read-only: it
  reconciles nothing and writes nothing, and a recorded `live` session is shown as unconfirmed,
  because nothing durable holds a process handle and another process cannot see one.
- `verb claude`, `verb codex`, `verb opencode`, and `verb dsh` launch the installed agent in the
  current project through a Unix PTY, preserving interactive TUI behavior. Windows currently uses
  inherited terminal input/output as a compatibility fallback.
- Every launch gets `VERB_SESSION_ID` and `VERB_PROJECT_ROOT` environment context.
- Unix launches record structural JSONL events under `~/.verb/events/<project>/<session>.jsonl`:
  session/process lifecycle, state transitions, exit status, and -- from the shell's own OSC 7 and
  OSC 633/133 markers -- working-directory changes and command boundaries with an opaque command id
  and exit code. Raw terminal input and output are ephemeral and are never persisted, and the
  marker that carries the command line (`OSC 633;E`) is recognised only so it can be skipped.
- Claude, Codex, and OpenCode each have an adapter that reads that agent's own evidence and resumes
  by its own conversation id (`desktop/src/agents.rs`). `dsh` is still tracked without a resume
  contract: Verb does not invent resume flags it has not observed.

This is deliberately not the final desktop UI, PTY supervisor, remote sync layer, or credential
broker. Those are the next product layers after the command boundary is proven.
