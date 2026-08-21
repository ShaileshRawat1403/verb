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

- `verb status` reports the project root, branch, changed-file count, and last session.
- `verb claude`, `verb codex`, `verb opencode`, and `verb dsh` launch the installed agent in the
  current project through a Unix PTY, preserving interactive TUI behavior. Windows currently uses
  inherited terminal input/output as a compatibility fallback.
- Every launch gets `VERB_SESSION_ID` and `VERB_PROJECT_ROOT` environment context.
- Unix launches record structural JSONL events under `~/.verb/events/<project>/<session>.jsonl`:
  session/process lifecycle, state transitions, and exit status. Raw terminal input and output are
  ephemeral and are never persisted by default.
- Claude exits are recorded as interrupted and can be resumed with Claude's `--continue` contract.
- Other agents are tracked, but Verb does not invent resume flags until their adapters are defined.

This is deliberately not the final desktop UI, PTY supervisor, remote sync layer, or credential
broker. Those are the next product layers after the command boundary is proven.
