# Contributing to Verb

Verb welcomes focused changes that strengthen practical, user-controlled AI-assisted development.
Read `docs/PRODUCT_VISION.md`, `docs/PRD.md`, `docs/UX_FOUNDATION.md`,
`docs/VERB_SESSION_CONTRACT.md`, and `docs/VERB_SESSION_SCHEMA.md` before changing product or session
behavior.

## Non-negotiable boundaries

- Unknown is not no; inference is not fact; an agent claim is not verified execution.
- Do not persist or export terminal bytes, command text, prompts, transcripts, credentials, PIDs,
  process handles or `processPresent`.
- Add an agent through an adapter. Do not add lifecycle states or vendor branches to session core.
- UI and CLI must consume the same resolver; a screen must not calculate its own truth.
- AI consumes bounded structural evidence. Actions require explicit user approval.

Never include local `.verb`, `.claude`, `.codex`, authentication, transcript, keystore or build-state
files in an issue, patch or archive. Use synthetic fixtures with planted markers for privacy tests.
Security-sensitive findings follow [`SECURITY.md`](SECURITY.md), not a public issue.

## Verification

Desktop:

```bash
cargo fmt --manifest-path desktop/Cargo.toml -- --check
cargo clippy --manifest-path desktop/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path desktop/Cargo.toml --all-targets
cargo build --release --manifest-path desktop/Cargo.toml
```

Android, with Java and the Android SDK configured:

```bash
./gradlew :app:testFullCliDebugUnitTest :app:testPlayDebugUnitTest
./gradlew :app:lintFullCliDebug :app:lintPlayDebug
./gradlew :app:assembleFullCliDebug :app:assemblePlayDebug
```

Debug builds use the disposable package `com.aistudio.verb.app.debug`. Android instrumentation may
uninstall that package when it cleans up; it must never target the canonical release package
`com.aistudio.verb.app`. Connected Gradle tests also refuse to start while a physical device is
attached; use a disposable emulator.

Add the smallest regression test that reproduces each corrected defect. Clearly separate automated,
emulator and physical-device evidence in change descriptions.

## Change shape

Keep changes narrow and evidence-backed. Explain the observed failure, the product or architecture
rule it violates, the correction, and exact verification. Product-wide assistant, sync, cloud,
agent-catalogue or durable-schema changes require an explicit reviewed proposal before code.

Unless explicitly marked otherwise, contributions intentionally submitted to Verb are accepted
under the repository's Apache-2.0 license. Third-party runtime sources retain their own licenses.
