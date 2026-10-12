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

Before public promotion: document the host/CLI compatibility matrix, validate backup/restore of desktop
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

## 2026-10-12 closeout scope and recovery evidence

Verb is a native terminal that launches installed CLI agents and binds their work to common projects
and durable sessions. PocketFabric is a general application-hosting node, with Verb as its first
release-testing application. A provider CLI's own sandbox incompatibility on one host is an explicit
compatibility note, not a reason to weaken safeguards or restrict Verb to one provider. Native
terminal launch and Verb's automatic recovery adapter are separate capabilities.

`desktop/scripts/workstate.py` provides private credential-free ledger snapshots and SHA256-checked
restoration into a fresh directory. It includes only projects/session/event/work metadata, excludes
session locks, tokens, launch logs and provider homes, rejects unsafe archive members, and refuses
an existing restore destination. Stop writers before `snapshot --quiesced`. It is not a backup of
project source, native conversation histories, imported continuity stores or authentication.

Seven offline restore regressions passed on October 12, including retained session identity/tasks,
private permissions, corruption, traversal, symlink denial and refusal to overwrite live state.
The idle isolated node candidate was stopped, its 37 ledger files (22,834 bytes) snapshotted and
restored into a separate private destination, then restarted. Integrity was verified before loading;
the actual Verb CLI loaded all 15 original session identities from the restored ledger. Verb normally
reconciles runtime status when loading a restored record; post-load byte equality is not the contract.
The public server retained PID 6613 and its previous running executable. Two public terminals were
active during preflight, so no public interruption, state migration or reboot was performed.

Public promotion subsequently passed: host UID995, terminal/tool UID994, all 27 original session IDs
retained after project mapping, old root bearer rejected, native foreground Ctrl-C and continued
shell input verified. The original root project/state remain intact. The public URL now serves the
reviewed clean Desktop artifact; the Host page was verified in Chrome after promotion. Full prior
service/config plus executable rollback was rehearsed and returned to isolated mode without state
overwrite. Operations reject pre-isolation binary-only rollback under the new host account.

The first final boot test exposed an Android PATH inherited by an early Debian helper, preventing
the shared supervisor from starting. USB recovery restored the node; bootstrap helpers now receive
a sanitized Debian environment. Optional isolation-view failures cannot block SSH/unrelated apps,
while isolated launches retain their fail-closed resource/privilege checks. Two actual-script boot
regressions passed. The repaired reboot recovered public Verb, SSH and all required node services
before first unlock, with Android reporting RUNNING_LOCKED. Finite memory and private sudo views
were recreated. No APK, kernel or global mount changes were made.

Subsequent October 12 recovery closeout: the owner approved the exact encrypted Mac backup scope,
including saved CLI logins/private application configuration. AES-256 image roundtrip and isolated
selected Desktop project/ledger restoration passed with all 30 current session identities. Private
credential/configuration files were verified by filename only, not inspected or installed into
another account. SSH keys are excluded. Node operations own the private receipt and runbook.
The earlier approval block is resolved. An unexpected battery shutdown was confirmed by the owner;
after changing from Mac ADB power to a wall charger, observed capacity rose from 1–3% to 18%.

Owner signed in again on October 12. Authenticated public Host/build verification and a real
browser terminal check passed after the operational corrections below. Remaining closeout:
stable power and the agreed normal-use observation window. Code checks cannot substitute for
that observation. Archive
integrity does not certify external databases or another application's functional restore.
Commercial/multi-tenant isolation is separate work.


October 12 authenticated-browser follow-up exposed two operational gaps. The shared test project
retained two broken PRoot `.l2s` Git-object links inherited from the original root project. Both
objects were recovered from old storage, verified against their Git object hashes and atomically
restored in the shared workspace; refs/history and the original root project were preserved.
`git fsck` and clean `master` status passed. Android's memory-cgroup directory was mode0777; the
launcher correctly refused execution. PocketFabric provisioning now explicitly sets root ownership,
mode0755 on the leaf and mode0644 on its limit every start, with only membership delegated.
The browser then launched a connected PTY as `verb-agent`, ran identity/path/Git checks and exited.
Host confirmed source2543cdd and battery42% charging. No Desktop runtime redeployment was required.
Maintenance now distinguishes exited replay buffers from live terminals; ambiguous status remains
a refusal. The approved encrypted backup was refreshed after these repairs.
