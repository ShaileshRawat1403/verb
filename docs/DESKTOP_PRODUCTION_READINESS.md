# Verb Desktop production readiness

Scope: `desktop/` Rust CLI/TUI, browser workbench, and its deployment operations. Android APK
flavors, Kotlin UI, runtime imports and phone-kernel changes are outside this pass. The OnePlus
currently hosts the desktop web service; it is an execution host, not the Android product.

This document records release evidence, not a blanket claim that every deployment is ready.

## Release contracts

- Terminals are real PTYs with foreground job control, native signals, Unicode, resizing,
  alternate-screen applications and xterm rendering. Speculative local echo is disabled.
- Private native conversation records are checked under the agent UID. Only recovery metadata
  returns; file permissions and credential boundaries stay unchanged. Ambiguous identity remains
  Unknown. Identity is persisted before interruption.
- The Host page hashes the running executable and reports embedded source revision/dirty status.
  `verb --build-info` exposes artifact provenance without loading project state.
- `/healthz` reports minimal HTTP readiness after host/origin checks. It does not grant API access
  or claim all installed agents can execute tools.
- HTTP body deadlines, header limits, parser/worker/terminal-connection budgets and bounded host
  probes prevent individual clients or health checks from creating unlimited work.
- New isolated Linux processes inherit a hard process/thread budget of at most 256, alongside
  their finite memory cgroup and no-new-privileges. Existing processes need a deliberate retirement
  before these inherited limits can be claimed for the whole deployment.
- Desktop deployment validates artifact source identity, rejects dirty/unknown builds, uses unique
  private staging and immutable backups, serializes changes, and restores the previous executable
  on restart/readiness failure. Rollback verifies backup integrity and the running executable.

## Evidence required for promotion

Run `desktop/scripts/check.sh`, build a Linux release and run
`desktop/scripts/test-agent-isolation.sh`. Operations CI exercises artifact rejection and injected
restart/readiness failure without using a live node or credentials. Record source revision,
artifact SHA256, test totals and target platform for each release.

The existing authenticated candidate proved Claude/Codex PTY responses, private-record recovery,
same-session resume, restart durability and read-token boundaries. Claude editing/tests passed.
AGY authentication/native exact resume passed; automatic Verb AGY recovery remains unconfirmed.
Codex tool execution on the current phone is blocked by its kernel's missing user namespaces.
No native sandbox bypass or privilege increase is an acceptable automatic compatibility fallback.
Local desktop or supported Linux execution must be assessed separately from that host limitation.

Before public promotion: settle the supported execution target, validate backup/restore of desktop
work state in an isolated destination, preserve existing work during migration, review public
access configuration, and prove recovery of the final service configuration. Existing user state
and credentials must not be deleted or silently copied into another account. The public service
continues to run its previous artifact until a verified release is promoted.

## 2026-10-11 desktop hardening checkpoint

The complete Desktop check script passed: 335 Rust tests, 5 existing ignored cases, both Clippy
checks, formatting, 41 browser unit tests, current bundled assets and nine browser journeys.
The Linux release and disposable account-isolation fixture passed, including private record
recovery and an attempted increase of the inherited process budget. Seven offline operations tests
passed, including restart and readiness failure injection restoring the original executable.
No Android APK source was changed. No public routing or production service was changed in this pass.
