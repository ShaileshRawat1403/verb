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
Codex command execution through its native sandbox is blocked by the phone kernel's missing user namespaces.
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


## October 12 maintainability and release-candidate pass

Session/provider records now live in `desktop/src/session.rs`; background workspace snapshots in
`desktop/src/web/state.rs`. Browser socket/protocol ownership and stream rendering are separate
modules, with session-view capability rules centralized. Existing durable record formats and
legacy API fields are preserved. Additive terminal.running and terminal.replayAvailable explicitly
distinguish a live local PTY from retained output. Exact native resume remains independent.
See `DESKTOP_CODE_OWNERSHIP.md` for boundaries and contracts.

A fresh project opens in Sessions; terminal and agent launch require no spec. Sessions is the first
navigation item, with existing keyboard shortcuts preserved. Specs remain an optional workflow.
Completed Git reads with no branch show Git status unavailable rather than permanent Loading.
Exited replay rows open terminal output instead of promising a running process.

Validation:336 Rust tests passed,5 existing ignored; both Clippy toolchains and formatting passed;
44 browser unit tests,10 browser journeys and7 offline ledger restore regressions passed. The real
disposable Linux isolation fixture passed. The fresh-state native release fixture verified Mac
version, workspace startup, PTY commands and exit/replay capability. Generated bundle is current.

Release CI now validates the selected release ref through the full reusable Desktop checks before
creating a draft. Each native release package must be unpacked and pass the fresh-state fixture
before upload. The local checks include that fixture too. GitHub workflow execution remains to be
verified after pushing; local tests do not establish a green remote run.

This pass is local candidate work, not public promotion or completed platform certification.
Remaining release gates: normal-use observation on stable power, supported-platform package
acceptance (especially the cross-built Intel Mac artifact), and a signed/notarized macOS
distribution if advertised as a general production Mac release. Broader AGY/OpenCode automatic
recovery claims require their own evidence. No release tag or public publication was created.

## Hosted candidate promotion and remote validation — October 12

Existing no-merge integration PR12 received the tested commits. GitHub Desktop CI run38264068980
passed both macOS and Linux jobs for97e4767e809fbced7da6bedad64a9d108d1a71bf, including the fresh-state
native release check. Intel Mac x86_64 release built and passed the fresh-state fixture under Rosetta
on the owner's Apple Silicon Mac; native Intel hardware acceptance remains distinct.

Idle public promotion installed the clean ARM64 artifact for97e4767, SHA256
93de63caf506789284ab66d3251c5f649ed17d69d0e28e314bfa4813a39a1fd9. The previous isolated artifact and
manifest are preserved in the node deployment backup. Health reported the intended revision,
UID995, and healthy required services. The2GiB cgroup remained root-controlled (leaf0755/limit0644).
Authenticated Chrome reloaded into Sessions, verified Host provenance, and ran identity/path/clean
Git status through a connected real verb-agent terminal. Owner-facing URL is unchanged.

Only Apple Development signing identity was found on the Mac, not Developer ID Application. This
is insufficient evidence for a signed/notarized general Mac distribution. No release tag, merge or
final public package publication occurred. Stable-use observation and the focused usability tasks
in DESKTOP_UX_SPRINT.md remain acceptance work; the hosted candidate is ready for that observation.


## Desktop polish and continued observation — October 13

The October 12 polish rollout now serves clean source `3961722aee978f9cedeeb9120825a15489ac7c56`,
artifact SHA256 `d1f7aee35b7f68358bc47a7fee6ceaffe4ca4ccb450c620f82d932cc1b115931`.
[Desktop CI 38288595434](https://github.com/ShaileshRawat1403/verb/actions/runs/38288595434)
passed macOS/Linux. Local checks passed 336 Rust tests (5 existing ignored), 45 web unit tests,
13 browser journeys, 9 ledger recovery checks and the fresh-install native terminal contract.

Current workbench surfaces and motion are separated into named stylesheets. Chrome motion is
brief and respects reduced motion; terminal views remain still. Tab refresh keeps keyboard focus.
Compatible xterm add-ons repair GPU disposal. Stopped output remains available across lightweight
refreshes until explicitly closed; closing a buffer preserves history. Streaming owns rendering
and connection status once attached, so late polling responses cannot overwrite them. Ledger
snapshot checks ignore read access-time changes while retaining concurrent-write rejection.

The public browser ran real commands as verb-agent in the shared clean Git checkout, exited
normally, retained output across refresh and closed it without deleting the session record.
October 13 pinned-SSH health observation still identifies the expected clean build and host UID995,
with all six required services responding and approximately 13.5 hours of node uptime. This is a
point-in-time operational observation, not seven days of real-use acceptance or a new reboot proof.
No further deployment or interruption was needed for this follow-up.

The owner approved PocketFabric's separate Mac-local read-only operations console. It uses the
node health contract, backup-file metadata and runbooks; it does not own Verb sessions or change
application privileges. Remaining release gates above are unchanged.

## Desktop continuation — October 13

Host service presentation is extracted into `host-services.js`. Unreadable supervisor status now
explains possible account restrictions and explicitly avoids treating missing observations as a
service outage. Missing supervisor data also avoids claiming an unsupervised ordinary process.
Running, stopping and down observations retain distinct states; no permissions or runtime protocol
were changed. Three unit regressions and a browser journey cover this observation boundary.

The complete Desktop check script passed:336 Rust tests,5 existing ignored, both Clippy checks,
formatting,48 web unit checks,14 browser journeys,9 ledger restore checks and the fresh-state native
release contract. The rebuilt browser asset is synchronized. This is a tested source candidate;
the live hosted artifact remains the previously recorded3961722 build until deliberate promotion.
Stable-use observation and signed/notarized general Mac distribution remain release gates.

## Git diagnosis and multi-session continuation — October 13

Guarded Git snapshots now derive branch/unborn/detached states from porcelain-v2 headers,
distinguish ordinary non-Git folders from unavailable/broken checkouts, and preserve unknown
change counts as null. Browser, CLI and TUI no longer advertise zero changes after a failed read.
Browser workspace polling retains a completed verdict instead of returning it to loading.
Git probing/presentation have named owners; repository filter/fsmonitor safeguards remain intact.

The complete Desktop check script passed:340 Rust tests,5 existing ignored, both Clippy checks,
formatting,52 web unit tests,17 browser journeys,9 ledger restore checks and fresh-state native
release acceptance. A disposable two-PTY journey proved shared project files, distinct process
IDs, same-process reattachment after reload and exit/replay preservation while another shell
remained running. Local opt-in acceptance additionally executed the installed Claude and Codex
version commands successfully under the fixture HOME; no sign-in or model request was performed.
This is startup/hosting evidence, not authenticated editing or exact native conversation resume.

The hosted service still runs the previously recorded3961722 artifact; these source changes have
not been promoted. Stable-use, native Intel hardware and general Mac signing/notarization remain
separate release gates. No tag, merge or public release was created.

## Hosted promotion — October 13

The clean source revision `74a337536e52ce82e753b8feb540fc1e97767a08` passed Desktop CI
on macOS and Linux (run38397416347), then was built with the pinned ARM64 Linux toolchain.
The installed and running artifact SHA256 is
`cf727111cf1af8e8b283972dbd7e8b88fb093d0717e81322adc67e527e04f502`.
The previous3961722 artifact is preserved in the node's owner-only deployment rollback directory.
Installation left the running process untouched; the existing PocketFabric fixed restart utility
then rechecked active work under the shared maintenance lock before restarting. Both the initial
observation and guarded restart found no active terminals. Health identifies the expected clean
revision, host UID995 and all six required services responding.

Live API acceptance created only two synthetic shell sessions. Both ran as agent UID994, had
distinct process IDs, preserved process/output across fresh attachment requests, and retained
exited output while the other session continued running. The test closed only its own terminal
buffers; durable history was retained. It did not inspect project files or call model providers.
This proves hosted PTY behavior; it is not a browser reload or authenticated model-editing proof.

After owner sign-in, the protected public browser loaded the promoted workbench. A new terminal
printed shell PID32666 and UID994; a full browser reload preserved its output and the same PID.
Normal exit retained output with an explicit finished state. Only this test buffer was closed;
its durable history was preserved. The Host page displays the clean74a3375 revision and matching
artifact fingerprint, and accurately describes unreadable supervisor states under host UID995.

Automatic approval review initially blocked provider testing in the existing private project.
The owner then explicitly approved small authenticated requests in new empty test projects.
Separate loopback-only hosts used the promoted binary, fresh state directories and synthetic Git
repositories; the public workbench and its project files remained untouched. Claude successfully
created the requested synthetic file owned by UID994. Codex returned an authenticated no-tool
response. Both produced recoverable native conversation identities and resumed using the same
Verb session ID; agent-owned native processes carried the expected exact conversation identity.
The fixture hosts and their terminals were stopped, and the synthetic edit was removed. Owner-only
fixture state/history remain separate from public state. Provider output and login contents were
not copied to these repositories. Early fixture failures were corrected: Claude's variadic tool
flag consumed a trailing prompt, and Codex requires a normal Git repository for exec mode.

This proves authenticated Claude editing, Codex chat, and exact resume launch. It does not prove
post-resume model continuity or Codex native tool execution on the phone's unsupported kernel.
No sandbox bypass or privilege fallback was introduced. Seven-day stable use, native Intel
hardware and general Mac signing/notarization remain open.


## 2026-10-14 external-isolation checkpoint

Both Claude and Codex completed fresh authentication and verified synthetic
shell execution inside a separately constructed external boundary on the
existing OnePlus kernel. Codex used an explicit per-process external-boundary
mode; its native sandbox remains unsupported. The live Verb launcher and normal
saved sandbox settings were not changed. This is a disposable runtime proof,
not evidence that integrated production sessions use the new boundary.

PocketFabric's candidate request/identity control layer passed 23 controller
and 17 egress regressions on the OnePlus. A subsequent process-supervision,
bounded-DNS and shared-Linux-setup checkpoint passed 46 component regressions and
two disposable namespace/PTY/HTTPS checks. No live launcher changed. Assembling
those components into an execution service, aggregate resource/storage admission,
production recovery lifecycle, Verb transport integration,
persistent authentication and candidate resume/reconnect/stop/rollback still
require implementation and acceptance before promotion. See
`AGENT_USER_ISOLATION.md` for the precise scope. Existing release gates remain.

Durable lease/controller ownership components subsequently passed 51 checks and
a forced-controller-crash proof on the OnePlus. Recovery stopped its recorded
detached test process; stale stop protection preserved a replacement launch.
These are component proofs. The assembled service, bounded project resources and
Verb adapter still need integration before candidate activation. Live Verb was
unchanged and remained healthy; no phone reboot or saved login was used.


## 2026-10-15 private native candidate

The external execution service and opt-in Verb adapter are installed as separate
private candidate services on the OnePlus. Native browser keyboard/close and
API input/resize/file-owner/persistent-home/cleanup proofs passed, alongside
assembled controller crash recovery and sampled storage-stop proofs. Ninety
boundary checks pass on ARM Linux. This does not replace the public deployment,
which remains on its existing launcher and route.

The owner chose monitored storage explicitly; hard quotas remain unavailable.
The clean private artifact is built from
`5e9884193bf4de3be629092eb67586c1e5af9e8b` (`sourceDirty=false`), binary SHA256
`e3a4c72429af44440b9b15adf458d5b24337aa0b0cc1de24d460218723dd3f6e`.
It is not a promoted public release. Fresh owner-completed sign-ins passed integrated Claude/Codex shell
tools and model continuity in exact resumed native conversations. Verb Resume
kept the same session ID and exact native identity. Browser reload preserved a
live shell. API close killed a detached child; forced failure of the installed
controller recovered its recorded child and lease before accepting another shell.
Switching to the preserved old package and back passed shell/home checks; the
clean package then passed exact provider Resume and installed controller crash
recovery. Desktop CI run `38670697911` passed. Real-project migration, new-boot
acceptance for these services, normal-use observation and public promotion
remain gates. The only registered candidate project is synthetic.
See `AGENT_USER_ISOLATION.md` and PocketFabric's dated candidate evidence.

The clean candidate's final phone-backed browser proof passed real keystrokes,
reload with the same shell PID and UI close, with zero page errors. The measured
new-terminal HTTP response took 16.4 seconds during this proof. Earlier 15-second
fixture waits expired before launch completed and left their synthetic terminal
behind; cleanup now waits for the UI request to finish. The observed launch delay
was traced to synchronous `/api/git` work on the PTY/request loop and is resolved
by the responsiveness follow-up below.

## Terminal responsiveness follow-up, 2026-10-15

Clean revision `5c3661ede775549de9017b07f0009e17d661a7c6` moves read-only Git
summary and file-diff requests to the existing bounded worker pool. Authentication,
request limits and error replies are preserved. Profiling showed the blocking
Git route took 16.5 s while actual terminal creation took 27 ms. Temporary profiling
was removed from source and the installed artifact.

A real blocked-Git regression failed before the fix. Both summary and diff cases
now open a terminal and verify native keyboard echo while Git remains blocked.
All 19 browser journeys and the 341-test Rust suite passed (five intentional
ignores). Desktop CI `38676399426` passed macOS and Linux, including repeated
immediate resume/close across sudo's UID transition. EPERM delegates only to
the existing isolated-session signal authority, and final reaping is bounded.

On the private OnePlus candidate, the launch response fell to 154 ms in the
measured flow. With the final bounded priority policy, a cold-start browser opened
two distinct native shells and received their shell PID markers in 666 ms and
1,012 ms respectively. Reload retained both processes and UI close removed both
without page errors. These are observed synthetic runs, not a latency guarantee.

The node still admits at most two sessions. At most one pipe client is active;
a waiting interactive launch briefly takes priority over subsequent probes. Only
explicit capacity rejection before lease issuance can be retried, for at most
three seconds. Transport failures and accepted streams are never resubmitted.
No memory/network/storage guard was relaxed. The running private binary SHA256 is
`ab3e939ccbc21e051bc9c3f98e0ecb89552185c88588586264a1b52831e63be1`.
Public port 3005 remains on its previous clean deployment.

### Private cold-state follow-up, 2026-10-16

Clean revision `d57bec0666a9ee2ea5cee07fa365250ad7caf2dc` runs as the private
host and read-only guest helper. Binary SHA256:
`0523a00e1bd1def4b93c316bbb12590a871716adf88274ea0d1c9a28d544dbf4`.
It groups guarded Git snapshot reads into one bounded project session and
reuses the workspace identity for task and memory reads. Git failures remain
unknown. The controller and public deployment were not upgraded.

On the same phone, uncached `/api/state` improved from 21.395 seconds to
2.802 seconds on the first read. Three later reads after cache expiry took
1.879, 1.870 and 1.838 seconds. `/api/workspace` took 0.001 seconds; a cached
state took 0.004 seconds. These are device observations, not guarantees.
The observed branch and zero changes remained correct. Existing ended history
is retained and does not count as active work.

Library tests, all 34 HTTP integration tests, the repository-command execution
regression and strict Clippy passed. The slow-state regression now delays Git
itself, rather than depending on redundant project-registry locking. Synthetic
phone typing, resize, clean exit, HTTPS/private-network policy, scoped rule
renewal and lease cleanup passed without provider inference. The first upgrade
attempt rolled back on an overly strict empty-history assertion; the corrected
gate distinguishes ended history from active work and the accepted switch keeps
the old immutable package available.

PocketFabric records the full private artifact and measurement evidence in
`docs/evidence/boundary-state-20261016.json`. Natural idle-transition observation,
daily-use soak and public promotion remain open. This binary has not repeated
the earlier build's locked-phone reboot test. The latest encrypted data backup
predates this code-only upgrade.
