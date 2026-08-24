# Verb return handoff — 2026-08-24

This is a verified source-only release candidate produced from transfer snapshot `bf9fd84`. It is
intended to be reviewed and integrated into the primary Verb checkout that owns the real Git
history. It is not a substitute repository and makes no claim of physical Android verification on
this laptop.

## 1. Baseline

### Tooling

- macOS Apple Silicon, Asia/Kolkata.
- Rust: `rustc 1.98.0`, `cargo 1.98.0`.
- Java: Homebrew OpenJDK `21.0.12.1`.
- Android: platform `android-36.1`, build tools `36.0.0` and `36.1.0`, platform tools/ADB `37.0.1`, NDK `28.2.13676358`.
- Gradle wrapper: `9.3.1`.
- Claude Code: `2.1.241`.
- Codex CLI: `0.149.1`.
- OpenCode: `1.17.11`; it was already installed and was not configured.

Java, the Android command-line tools and required SDK packages were initially absent or undiscoverable
and were installed/configured locally for verification. No project `local.properties` was created.

### Commands and initial failures

The initial desktop gate was:

```text
cargo fmt --manifest-path desktop/Cargo.toml -- --check
cargo clippy --manifest-path desktop/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path desktop/Cargo.toml --all-targets
cargo build --release --manifest-path desktop/Cargo.toml
```

After installing Rust and allowing the initial dependency fetch, all passed. The first download
attempt failed only because the managed sandbox blocked DNS.

The Android gate was:

```text
./gradlew :app:testFullCliDebugUnitTest :app:testPlayDebugUnitTest
./gradlew :app:lintFullCliDebug :app:lintPlayDebug
./gradlew :app:assembleFullCliDebug :app:assemblePlayDebug
```

The first Android compile exposed five stale Claude-directory fixture expectations; the installed
CLI and matching Rust implementation showed that `/`, `.` and `_` all normalize to `-`. Those
fixtures were corrected. Later lint correctly rejected Java NIO calls requiring API 26 while Verb's
minimum is API 24; the implementation was changed to canonical `File` path logic and lint passed.

### Initial product defects reproduced

1. Normal desktop Quit could leave the hosted process or durable session `LIVE` without end events.
2. Desktop event JSON used a desktop-only timestamp/key shape rather than the shared schema.
3. Claude Android recovery used a different project-directory normalization from the installed CLI.
4. Codex tool parsing could capture a neighboring JSON `name`.
5. Failed agent-tool timing treated an epoch timestamp as a duration.
6. Direct `verb resume` could create a zero-sized PTY and render one character per line.
7. Android durably stored command text, PTY output, prompts and assistant responses despite the
   constitutional prohibition; several stores were write-only.
8. Restored `resumeIdentity` reached Android shell command construction without validation.
9. Desktop session IDs embedded timestamp and PID-shaped material.
10. Working World import trusted archive members too broadly, and restored provider ciphertext could
    be shown as a usable key after its device Keystore key was gone.
11. `verb run <command>` persisted the supplied command as `runtimeId`, discovered during continuity
    dogfooding.
12. Android's AI terminal helper sent raw PTY output to a provider after heuristic redaction.
13. Android organised the product as five permanent subsystem tabs and exposed several competing
    natural-language surfaces instead of one terminal-first workspace.
14. The replacement overlay boundary existed in `TerminalScreen` but the Activity did not pass the
    open-surface state, leaving terminal focus/back ownership logically active underneath Verb.
15. Searchable System task names all landed at the top provider card rather than the named section.

A temporary local Git repository records checkpoints `20e9e48`, `96bad15`, `add1abc`, `ec59e48`
and `e93cd52`. It has no remote and must not be included in the return archive.

## 2. Changes

### Desktop reliability and evidence

- `desktop/src/pty.rs`, `desktop/src/tui/term.rs`, `desktop/src/tui/mod.rs`,
  `desktop/src/tui/render.rs`: correct PTY sizing, bounded process-group shutdown/reap, truthful Quit
  transitions, observation timing, minimum-size throttling and terminal ownership.
- `desktop/src/agents.rs`, `desktop/src/observe.rs`: central Claude path mapping and correctly scoped
  Codex tool evidence.
- `desktop/src/main.rs`, `desktop/src/context.rs`: shared event schema, legacy timestamp read support,
  opaque random session IDs, safe resume identities, event sequence, tool names in JSON context, and
  the rule that custom launch commands are volatile input—not durable runtime labels.

Architectural effect: no new lifecycle exists. TUI and CLI still use the same resolver, and no
process binding enters durable state.

### Android privacy and recovery

- Deleted `app/src/main/java/com/example/verb/db/` and
  `app/src/main/java/com/example/verb/model/AgentMemoryStore.kt`.
- `app/build.gradle.kts`, `gradle/libs.versions.toml`, `VerbViewModel.kt`, `MainActivity.kt`: removed
  Room and all durable prompt, response, command-text and terminal-output writes.
- `VerbApplication.kt`, `privacy/LegacyPrivateDataPurge.kt`, manifest and backup XML: one-shot,
  idempotent deletion of the legacy database/preferences and removal of obsolete backup entries.
- `privacy/DurableStoreBoundaryTest.kt`, `LegacyPrivateDataPurgeTest.kt`: regression guards against
  restoring the prohibited store.
- `session/ResumeIdentity.kt`, `ClaudeProjectDirectory.kt`, all three agent adapters/coordinators and
  their tests: validate resume references before use and share Claude's installed mapping.
- `ai/AiProviderSettingsStore.kt`: establishes key usability by decrypting; unreadable imported
  ciphertext no longer masquerades as a present key.
- `assets/verb/world.sh`: schema v2, narrow preference allowlist, exact archive-member validation,
  traversal/symlink/special-file rejection and staged restore.

Architectural effect: Android now honors the same structural-memory boundary advertised by the
desktop host. Existing prohibited local history is explicitly purged rather than migrated.

### Manual mobile ↔ desktop continuity

- `docs/VERB_CONTINUITY_ENVELOPE.md`: the complete v1 allowlist, provenance, identity, validation,
  ordering, privacy and threat boundary.
- `desktop/src/continuity.rs`, `desktop/Cargo.toml`, `Cargo.lock`, `main.rs`, TUI context/session files:
  `verb continuity export PATH`, preview-first `import`, explicit `--apply`, strict checksums and
  limits, atomic owner-only files, replay no-op, separate read-only imported namespace, and honest
  imported-session rendering in both CLI and TUI.
- `app/.../session/ContinuityArchive.kt`, `MainActivity.kt`, `VerbViewModel.kt`, `SystemScreen.kt`:
  Android export to Downloads, picker preview, explicit Apply, read-only imported storage and count.
- `app/.../session/VerbEventLog.kt`, `AgentSessionCoordinator.kt` and agent factories: Android now
  records only allowlisted lifecycle/recovery events with monotonic per-session sequence.
- `ContinuityArchiveTest.kt`, `VerbEventLogTest.kt` and desktop continuity/unit tests: checksum,
  traversal, identity and planted-secret/command/transcript leak coverage.
- `VERB_SESSION_CONTRACT.md`, `VERB_SESSION_SCHEMA.md`: evidence may travel; state and process
  authority never do.

Architectural effect: this fulfills cross-host **knowledge continuity**, not impossible runtime
continuity. `(origin hostId, sessionId)` stays stable, imported `recordedState` is dated history, and
Resume is never offered solely because another host said a session was recoverable. Agent transcript
content and credentials do not sync.

### Evidence-bound AI and product hygiene

- `TerminalAiHelper.kt`, `GuestCommandRunner.kt`, `VerbViewModel.kt`, `TerminalScreen.kt` and tests:
  provider egress accepts only lifecycle state, exit code, duration and whether cwd was observed.
  Raw output, command text and absolute paths are absent from the API.
- `AgentsScreen.kt` and tests: only implemented Claude, Codex and OpenCode integrations are admitted
  to the product surface. Unverified runtime profiles remain in the runtime layer but no longer form
  a visible catalogue.
- `.env.example`: removed unrelated Gemini placeholder-key boilerplate.
- `README.md`, `docs/README.md`, `docs/BACKLOG.md`, `docs/ARCHITECTURE.md`, `CONTRIBUTING.md`: current
  scope, contributor guardrails, continuity boundary and open-source architecture.

No model provider, fifth session state, cloud service, daemon, transcript transport, automatic sync
or autonomous action layer was added.

### Android terminal-first workspace and public-preview readiness

- `MainActivity.kt`, `VerbViewModel.kt`, `viewmodel/VerbSurface.kt`: removed the permanent five-tab
  information architecture. The terminal stays mounted as the workspace; one searchable Verb sheet
  opens named tasks as deliberate overlays, with a tested task → sheet → terminal back chain.
- `VerbSheet.kt`, `AskVerbScreen.kt`, `VerbFirstAction.kt`, `VerbStatusVocabulary.kt` and tests:
  replaced competing sentence surfaces with a deterministic-actions-first Ask Verb surface, added
  one truthful first action, and made every state readable as a shared glyph plus plain words.
- `TerminalScreen.kt`, `MobileTerminalKeyboard.kt`, `MainActivity.kt`: wired the actual overlay state
  into terminal input ownership. Opening Verb now clears terminal focus, hides the IME and disables
  its input field until the visible surface gives control back.
- `SystemScreen.kt`, `MainActivity.kt`: kept the existing System implementation but made Provider,
  Working World, continuity, runtime and agent-runtime task names bring their matching sections into
  view. A Compose regression test covers the real scroll behavior.
- Root `LICENSE`, `SECURITY.md`, `docs/RELEASE_CHECKLIST.md`, README/contributor/license documents:
  applied Apache-2.0 to Verb-authored code, preserved third-party licenses, documented private
  security reporting and made public-preview evidence gates explicit. Misleading unused
  server-side-Gemini `metadata.json` was removed.

Architectural effect: no session, event, continuity or persistence schema changed. Terminal input
still belongs to the hosted shell/agent at rest and explicitly stops belonging to it while a Verb
surface is visible. Optional provider interpretation remains read-only and separate from the
deterministic action/approval path; M2 was not implemented.

## 3. Verification

### Final automated gates

```text
cargo fmt --manifest-path desktop/Cargo.toml -- --check
# PASS

cargo clippy --manifest-path desktop/Cargo.toml --all-targets -- -D warnings
# PASS

cargo test --manifest-path desktop/Cargo.toml --all-targets
# PASS: 107 passed, 0 failed, 2 ignored opt-in real-agent-record probes
# PASS: 6 shell integration tests

cargo build --release --manifest-path desktop/Cargo.toml
# PASS

./gradlew :app:testFullCliDebugUnitTest :app:testPlayDebugUnitTest \
  :app:lintFullCliDebug :app:lintPlayDebug \
  :app:assembleFullCliDebug :app:assemblePlayDebug
# PASS: all six tasks, both flavors; 504 tests per flavor, 0 failures; BUILD SUCCESSFUL
# Lint: 0 errors and 86 warnings per flavor. Remaining warnings are pre-existing dependency,
# embedded-runtime/permission and upstream Termux compatibility warnings; none points at this UI diff.

bash -n app/src/main/assets/verb/world.sh
# PASS
```

The Android run used explicit `JAVA_HOME`, `ANDROID_HOME` and `ANDROID_SDK_ROOT`. Gradle/SDK caches
remain machine-local and are not packaged; no project `local.properties` was written.

### Manual desktop scenarios

All state used temporary `VERB_STATE_DIR` locations under `/private/tmp`. Real `~/.verb` was not
opened for content.

- Verb repository: project root, branch and dirty counts matched Git.
- Temporary clean Git repository: zero changes; dirty repository: one untracked change.
- Non-Git directory: explicitly reported as not a Git repository.
- Redirected bare `verb`: printed non-interactive help, launched no TUI and created no state.
- Ordinary zsh: terminal input belonged to the shell; palette Quit restored the terminal and wrote
  process/session end evidence immediately.
- Narrow `30×8` terminal with `NO_COLOR=1`: readable 30×12 minimum message, throttled rather than
  flooding redraws. Leader → palette → Quit restored the alternate screen, cursor and mouse modes.
- Palette, sessions, resume/new/forget, context/evidence, scrollback/search/live return and mouse
  handoff were exercised manually or through dedicated tests.
- `status --json`, `sessions --json` and `context --json` parsed successfully.

### Continuity dogfood

Using an isolated temporary Git project and two isolated state roots:

- Export wrote one session and five structural events to `.vcont` with mode `0600`.
- A planted custom command marker exposed the initial `runtimeId` leak; the durable label was changed
  to `custom`, a regression test was added, and the marker disappeared from session/event/export.
- Searches found no planted marker, absolute project path, executable path, command text, terminal
  output, `processPresent` or API-key field in the envelope.
- Preview created no destination state. Apply wrote only to `imported/<hostId>/`; local sessions
  remained untouched. Replay was a no-op.
- Imported `sessions --json` was valid and reported local `INTERRUPTED`/unknown-here while preserving
  origin `ENDED` only under `recordedElsewhere`.
- Newest origin export wins displayed summary fields; clocks from separate hosts are not merged into
  invented causality.

Desktop-format round-trip is verified. A physical Android file-picker/export/import round-trip is
not verified on this laptop.

### Claude and Codex dogfooding

- Claude ran a substantial authenticated, read-only Rust review through Verb. Its tool activity
  helped expose stale-LIVE exit, event-shape drift, Claude-directory mismatch, Codex tool-name scope,
  failed-duration display and command-label lifetime risks.
- Codex ran a substantial authenticated architecture/diff review through the TUI with roughly 50
  structural tool events. Verb retained tool names/outcomes only—no prompts, arguments, output or
  transcript content.
- A Codex session was deliberately interrupted. Verb claimed `RECOVERABLE` only after positive local
  agent evidence, then resumed the same Verb and Codex identities in a correctly sized PTY.
- The user-provided Claude Opus 5 xhigh audit was independently checked against source. Its concrete
  privacy, injection, archive, identity, provider-key, AI-egress and catalogue findings drove the
  bounded corrections above. Its recommendation to omit continuity was not adopted because the
  product requirement is one user-owned world; instead continuity was constrained to truthful,
  evidence-only manual transport.

No Claude/Codex/OpenCode authentication material or transcript content was copied, printed,
committed or packaged.

The final UI/release pass did not invoke Claude again, honoring the user's instruction. It relied on
the substantial Claude/Codex dogfood sessions above because the desktop implementation did not
change in that pass; only the release binary's isolated shell/JSON/narrow-terminal smoke was rerun.

## 4. Friction findings

### Raw observations

1. **TASK** Exit a hosted shell. **VERB KNEW** It owned the process. **FRICTION** durable state stayed
   `LIVE`. **I LEFT VERB FOR** status/source. **I WISH I COULD ASK** “What evidence closed this
   session?” **SEVERITY** blocked.
2. **TASK** Interpret recent evidence. **VERB KNEW** event type/time. **FRICTION** the desktop event
   envelope diverged from the shared schema. **I LEFT VERB FOR** docs/source. **I WISH I COULD ASK**
   “Is this current or legacy evidence?” **SEVERITY** annoying.
3. **TASK** Recover Claude. **VERB KNEW** project and agent evidence. **FRICTION** path normalization
   mismatch produced `ENDED`. **I LEFT VERB FOR** Claude's own resume output. **I WISH I COULD ASK**
   “Why do Claude and Verb disagree?” **SEVERITY** blocked.
4. **TASK** Run authenticated Claude. **VERB KNEW** only that the process exited. **FRICTION** a host
   sandbox problem looked like an agent failure. **I LEFT VERB FOR** environment diagnostics. **I
   WISH I COULD ASK** “Which boundary failed?” **SEVERITY** annoying.
5. **TASK** Start Codex after welcome. **VERB KNEW** which surface owned keys. **FRICTION** an
   artificially coalesced key burst leaked characters to zsh; human-speed input did not. **I LEFT
   VERB FOR** nowhere. **I WISH I COULD ASK** “Which keys reached the terminal?” **SEVERITY** annoying.
6. **TASK** Check a long Codex review. **VERB KNEW** turns, tool names/outcomes and times. **FRICTION**
   understanding still required reading Codex output. **I LEFT VERB FOR** the agent screen. **I WISH
   I COULD ASK** “What has it done, with evidence?” **SEVERITY** annoying.
7. **TASK** Resume interrupted Codex. **VERB KNEW** positive identity. **FRICTION** the direct PTY was
   zero-sized. **I LEFT VERB FOR** a PTY probe. **I WISH I COULD ASK** “Did recovery fail or only
   rendering?” **SEVERITY** blocked.
8. **TASK** Understand stopped work. **VERB KNEW** activity stopped and recovery was possible.
   **FRICTION** it could not distinguish provider quota from other waits. **I LEFT VERB FOR** provider
   UI. **I WISH I COULD ASK** “What is observed versus provider-reported?” **SEVERITY** annoying.
9. **TASK** Move a session between hosts. **VERB KNEW** local paths and session history. **FRICTION**
   Android private paths and desktop Git roots are not shared identity, and runtime cannot travel.
   **I LEFT VERB FOR** architecture/source. **I WISH I COULD ASK** “What can truthfully continue on
   this device?” **SEVERITY** blocked.
10. **TASK** Audit a continuity export. **VERB KNEW** a custom command was launch input. **FRICTION**
    it appeared as durable `runtimeId`. **I LEFT VERB FOR** an envelope search. **I WISH I COULD ASK**
    “Exactly which fields can leave this host?” **SEVERITY** blocked.

### Clusters

- **Explanation/research dominates:** 1–4, 6, 8–10 repeatedly ask Verb to explain the boundary among
  event-time fact, current observation, agent report, foreign-host history and inference.
- **Guided corrective action is secondary:** 1, 5 and 7 needed fixes, but the safe action depended on
  first understanding evidence.
- **Before/after and causal context is narrower:** interruption/resume and event-schema migration need
  comparison, but evidence does not yet justify a general causal engine.
- **Unexpected—host boundary diagnostics:** permissions, provider quota, terminal geometry and
  cross-host identity feel like one failed agent flow even though they belong to different owners.

### Proposed smallest M2 direction — approval required

The evidence supports a read-only, provider-neutral **“What happened?”** slice. It should consume the
existing evidence bundle and return claims whose non-empty references identify session/event/Git
evidence and whose strength is carried from the source: observed now, recorded at event time,
agent-reported, or unknown. A deterministic offline explainer should establish the contract before
an optional model adapter. Proposed actions remain empty in v1; later actions require explicit user
approval.

No broad assistant layer was implemented because the original mission requires this product choice
to be reviewed first.

## 5. Remaining work

### Release/desktop

- Integrate this source into the primary checkout with full history, inspect the resulting diff,
  confirm the Apache-2.0 choice, choose the release version/release notes and publish from that
  system. This transfer must not be pushed as its temporary repository.
- Distribution beyond `cargo install`, Windows TUI and a dedicated OS-signal terminal-restoration
  harness remain backlog work. Normal Quit/restoration is verified.
- Dynamic resizing of direct non-TUI `verb resume` after launch was not separately verified; initial
  geometry and TUI resize are covered.
- The Kotlin namespace remains `com.example` while the application ID is
  `com.aistudio.verb.app`. Renaming packages is a disruptive upgrade-sensitive decision and was not
  guessed in a source transfer; decide before a final public Android identity is frozen.

### Android/device

- No phone was connected. No new physical Android claim is made.
- On the primary device, re-run Working World v2 upgrade/export/import, legacy-private-data purge,
  Claude/Codex recovery, continuity export→desktop import and desktop export→Android preview/apply.
- OpenCode physical-device recovery proof remains pending on the primary device/provider setup.
- CI defines an emulator `StartupTruthTest`; no local emulator acceptance run was performed here.
- The new workspace has JVM/Compose coverage for reachability, back-chain, input ownership, status,
  first action and System section routing. Touch, IME, TalkBack and real-PTY acceptance still require
  the primary physical device; automated coverage is not described as that proof.

### Continuity/product boundary

- The beta synchronizes structural session knowledge manually. It deliberately does not copy agent
  transcripts or promise that the same Claude/Codex conversation can resume on another device.
  Cross-host Resume is allowed only if that host independently finds positive agent evidence.
- `.vcont` is inspectable plaintext metadata written owner-only. It can reveal project labels,
  agents and work times. Optional encryption is post-beta; credentials remain structurally forbidden.
- Automatic/background/cloud/LAN sync and merge arbitration remain rejected until separately
  threat-modeled and approved.

## 6. Privacy audit

- Android's prohibited Room database, command/PTY history and assistant-memory stores were removed;
  upgrade code purges their previous local artifacts once.
- No source API added here can persist raw PTY input/output, full command text, prompts, assistant
  responses, transcript contents, credentials, PIDs, handles or `processPresent`.
- Continuity writers are allowlist-only. Strict import rejects unknown keys, malformed identities,
  traversal, oversized input and checksum mismatch. Imported evidence cannot occupy local sessions.
- AI terminal explanation receives structural lifecycle fields only; planted command, secret and path
  tests prove they do not reach the provider request.
- No credentials, auth files, transcripts, real local agent state, keystores or terminal streams were
  added to source or Git. Real `~/.verb` data was not inspected for content.
- A repository scan found no private-key headers or common live-token shapes. The removed
  `metadata.json` was stale product metadata, not user or agent state.
- Generated build output and all temporary dogfood state stay local and are excluded from the ZIP.

## 7. Return packaging

The return ZIP must contain only source, tests, documentation and this handoff. Exclude:

- `.git`, `.claude`, `.codex`, `.agents`, `.gradle`, `.kotlin`;
- `local.properties`, `desktop/target`, `app/build`, root/nested `build` directories;
- `*.keystore`, `.env` and `.env.*` except the safe `.env.example`;
- `~/.verb`, all `VERB_STATE_DIR` data and temporary test repositories/directories;
- Claude, Codex and OpenCode transcript/session stores;
- credentials, tokens, caches, APKs, object files, `.DS_Store` and editor metadata.

Before release on the primary machine: extract/copy the source over a clean full-history checkout,
leave its `.git` untouched, inspect `git diff`, run the gates above, perform the pending physical
device acceptance, commit through the primary history, then publish from that system.

The return archive is `Verb_Return_2026-08-24.zip` beside the transfer directory. It is produced with
`git archive` from the final local-only checkpoint, so ignored/untracked caches, build output and the
temporary `.git` directory cannot enter it.
