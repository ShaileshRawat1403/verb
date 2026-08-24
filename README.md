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

> **Release status:** Verb is preparing for its first public developer preview. Desktop and Android
> automated gates pass. The terminal-first Android workspace, Working World restore, Claude/Codex
> recovery, and OpenCode launch have been exercised on the primary physical device. The full
> Android↔desktop continuity round-trip and OpenCode recovery remain unverified and are not
> advertised as proven.

## What works today

* **Sessions that survive process death.** A session keeps its identity across an agent exiting, the
  app being force-stopped, and the machine losing the process entirely — then resumes by the agent's
  *own* conversation id. Proven end to end on a physical Android device for Claude and Codex.
* **One session lifecycle, three agents.** `LIVE → INTERRUPTED → RECOVERABLE → ENDED`, shared by
  every agent. Each agent contributes only an adapter that reads its own evidence.
* **Two hosts, one contract.** Android (proot + PTY) and desktop (native Unix PTY) implement the same
  session semantics and the same durable record shape.
* **Manual, evidence-only continuity.** A checksummed `.vcont` file moves structural session history
  between Android and desktop. Imported state is dated, read-only evidence—not a live-process or
  cross-device resume claim. Physical phone↔desktop acceptance remains pending.
* **Structural memory, not surveillance.** Durable records hold identity, context and state. Never a
  PID, process handle, command text, terminal bytes, prompts, transcripts or credentials. Optional
  AI explanation receives structural evidence rather than terminal output.

Working World archives protect allowlisted agent state and Verb metadata. They do **not** contain
project source trees; keep projects in Git or another independent backup.

## Repository layout

```text
app/       Android application (Kotlin, Compose)
desktop/   Desktop host, CLI and Ratatui workspace (Rust)
runtime/   Termux-derived userland components for Android
docs/      Product and implementation documentation
```

## Building

Android:

```bash
./gradlew :app:assembleFullCliDebug
./gradlew :app:testFullCliDebugUnitTest
./gradlew :app:lintFullCliDebug
```

Desktop:

```bash
cargo build --release --manifest-path desktop/Cargo.toml
cargo test --manifest-path desktop/Cargo.toml
cargo install --path desktop              # install from source
./desktop/target/release/verb          # the session UI, on a terminal
./desktop/target/release/verb help
```

The developer preview does not yet publish prebuilt desktop binaries.

## Documentation

Start at [`docs/README.md`](docs/README.md). The four canonical product documents are
[`PRODUCT_VISION`](docs/PRODUCT_VISION.md), [`PRD`](docs/PRD.md), [`ROADMAP`](docs/ROADMAP.md) and
[`TUI_VISION`](docs/TUI_VISION.md); everything else records what was measured and built.

For implementation boundaries, see [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md). For cross-host
semantics and the exact wire allowlist, see
[`docs/VERB_CONTINUITY_ENVELOPE.md`](docs/VERB_CONTINUITY_ENVELOPE.md).

Three rules run through all of it:

```text
Unknown ≠ No
Inference ≠ Fact
Agent claim ≠ Verified execution
```

## Security and license

Please report vulnerabilities through [`SECURITY.md`](SECURITY.md) without posting credentials,
terminal output, transcripts, private paths or archive contents publicly.

Verb-authored code is licensed under the [Apache License 2.0](LICENSE). Bundled and derived runtime
components remain under their respective licenses; see
[`THIRD_PARTY_LICENSES.md`](THIRD_PARTY_LICENSES.md).
