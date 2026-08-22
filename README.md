# Verb

**Verb makes AI-assisted development easier to understand, control, debug and recover.**

You bring the agents you already use — Claude Code, Codex, OpenCode — and they keep doing the AI
work. Verb owns the layer around them: what environment you are in, what actually ran, what changed,
why it failed, and what is safe to do next.

```text
Claude / Codex / OpenCode / future agents
                    ↓
             do the AI work
                    ↓
                  Verb
                    ↓
     understands the environment,
     remembers what happened,
     explains what is happening,
     helps the user control it
```

Verb is terminal-first. It is not an IDE, not a coding agent, and not a model provider. See
[`docs/PRODUCT_VISION.md`](docs/PRODUCT_VISION.md) for why that boundary is the point.

## What works today

* **Sessions that survive process death.** A session keeps its identity across an agent exiting, the
  app being force-stopped, and the machine losing the process entirely — then resumes by the agent's
  *own* conversation id. Proven end to end on a physical Android device for Claude and Codex.
* **One session lifecycle, three agents.** `LIVE → INTERRUPTED → RECOVERABLE → ENDED`, shared by
  every agent. Each agent contributes only an adapter that reads its own evidence.
* **Two hosts, one contract.** Android (proot + PTY) and desktop (native Unix PTY) implement the same
  session semantics and the same durable record shape.
* **Structural memory, not surveillance.** Durable records hold identity, context and state. Never a
  PID, a process handle, terminal bytes, transcripts or credentials.

## Repository layout

```text
app/       Android application (Kotlin, Compose)
desktop/   Desktop host and CLI (Rust, dependency-free)
runtime/   Termux-derived userland components for Android
docs/      Product and implementation documentation
```

## Building

Android:

```bash
./gradlew :app:assembleFullCliDebug
./gradlew :app:testFullCliDebugUnitTest
```

Desktop:

```bash
cargo build --release --manifest-path desktop/Cargo.toml
cargo test --manifest-path desktop/Cargo.toml
./desktop/target/release/verb          # the session UI, on a terminal
./desktop/target/release/verb help
```

## Documentation

Start at [`docs/README.md`](docs/README.md). The four canonical product documents are
[`PRODUCT_VISION`](docs/PRODUCT_VISION.md), [`PRD`](docs/PRD.md), [`ROADMAP`](docs/ROADMAP.md) and
[`TUI_VISION`](docs/TUI_VISION.md); everything else records what was measured and built.

Three rules run through all of it:

```text
Unknown ≠ No
Inference ≠ Fact
Agent claim ≠ Verified execution
```
