<img src="assets/brand/verb-wordmark.svg" alt="Verb" width="240">

**Verb makes AI-assisted development easier to understand, control, debug and recover.**

Whether you are an engineer coordinating autonomous agents across workspaces, or a creator who wants to build real software on your phone using the Claude or ChatGPT subscription you already pay for: Verb gives you a complete, crash-proof development studio in your pocket.

You bring the agents you already use (Claude Code, Codex, OpenCode) and they keep doing the AI work. Verb owns the layer around them: what environment you are in, what actually ran, what changed, why it failed, and what is safe to do next.

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

Verb is terminal-first, but designed for human hands. No laptop required, no fragile command-line configuration, and no fear of losing your work when the phone sleeps. It is not an IDE, not a coding agent, and not a model provider. See [`docs/PRODUCT_VISION.md`](docs/PRODUCT_VISION.md) for why that boundary is the point.

> **Release status:** Verb is available as a public Android developer preview. Desktop and Android
> automated gates pass. The terminal-first Android workspace, Working World export *and* restore,
> the Android/desktop continuity round-trip, Claude/Codex recovery, and OpenCode launch have all
> been exercised on the primary physical device. **OpenCode recovery remains unverified** and is not
> advertised as proven.
>
> Several defects in this cycle were found only by running the product on a device while the test
> suite was green - including a Working World export that produced archives its own importer
> refused. Where a capability is listed here, a dated acceptance run is recorded in
> [`docs/BACKLOG.md`](docs/BACKLOG.md).

## What works today

* **Sessions that survive process death.** A session keeps its identity across an agent exiting, the
  app being force-stopped, and the machine losing the process entirely - then resumes by the agent's
  *own* conversation id. Proven end to end on a physical Android device for Claude and Codex.
* **Session-bound lifecycle across multi-terminal workspaces.** Agent processes and lifecycle
  observers are bound strictly to their originating concrete PTY session. Switching active UI
  selection, running commands in concurrent shell sessions, and Activity/ViewModel recreation produce
  zero false lifecycle transitions or misrouted commands.
* **One session lifecycle, three recovery-capable agents.** `LIVE -> INTERRUPTED -> RECOVERABLE ->
  ENDED` is implemented for Claude Code, Codex CLI and OpenCode. Each contributes only an adapter
  that reads its own evidence; Hermes and Antigravity currently have verified launch support.
* **Two hosts, one contract.** Android (proot + PTY) and desktop (native Unix PTY) implement the same
  session semantics and the same durable record shape.
* **Manual, evidence-only continuity.** A checksummed `.vcont` file moves structural session history
  between Android and desktop. Imported state is dated, read-only evidence - not a live-process or
  cross-device resume claim. Physically accepted in both directions on 26 August.
* **Structural memory, not surveillance.** Durable records hold identity, context and state. Never a
  PID, process handle, command text, terminal bytes, prompts, transcripts or credentials.
* **Ask about your own work, without explaining it.** One assistant, reachable from Ask Verb and
  from the terminal. It answers from the evidence Verb observed itself - session lifecycle, the
  shell's command boundaries, your agent sessions, and what the working tree did across the last
  command - and every answer renders beside the same facts, in plain words, so it can be checked.
  What it cannot receive: command text, terminal output, file contents, transcripts, credentials,
  absolute paths, or even a branch name. A model provider is optional and replaceable; the context
  is the product.

Working World archives protect allowlisted agent state and Verb metadata. They do **not** contain
project source trees; keep projects in Git or another independent backup. If you made an archive
with `0.1.0-beta.1` or `0.1.0-beta.2`, see the restore notice in
[`RELEASE_NOTES.md`](RELEASE_NOTES.md) — some of those archives cannot be restored and should be
re-exported.

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

Start at [`docs/README.md`](docs/README.md), or explore the step-by-step **[Understanding Verb Guide](docs/learn/README.md)** for a concept-first explanation of the architecture. The four canonical product documents are
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
