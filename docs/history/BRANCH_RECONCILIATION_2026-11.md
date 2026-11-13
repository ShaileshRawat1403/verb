# Verb Desktop backup branch reconciliation

Base `origin/codex/reconcile-local-work` at `4294d1013eae5af9a78436ca749ded485c578b4f`. Compared each backup tip's change from its own parent against the base tree and history. Excluded `desktop/web/dist`. These are static findings. Runtime parity and test execution are unverified.

**Landed** means the change is present in the base. **Superseded** means the base uses a different implementation for the same job. **Unlanded** means a material part of the backup change is absent or could not be confirmed. For mixed files, the evidence column names retained behavior where applicable. An unlanded row cites the backup commit containing the change and the base commit checked for its absence.

## codex/backup-2026-11-07-verb-security at `5a109cc`

| Changed file | Status | Commit and source evidence |
| --- | --- | --- |
| `desktop/src/specs.rs` | Unlanded | `5a109cc` against `4294d10`. Assignment roles and reviewed revision handoff fields are absent from the base spec model. |
| `desktop/src/web.rs` | Unlanded | `5a109cc` against `4294d10`. Base records confirmed admission but lacks the reviewed-revision handoff reservation and structured successor flow. |
| `desktop/tests/synthetic_boundary.rs` | Unlanded | `5a109cc` against `4294d10`. Backup binary override and approved handoff retry journey are absent. |
| `desktop/tests/web_integration.rs` | Unlanded | `5a109cc` against `4294d10`. Approved handoff interruption journey is absent. |
| `desktop/web/e2e/spec-workbench.spec.js` | Superseded | `917569c`. Busy reads are handled through shared retry logic. The backup two-response fixture differs. |
| `desktop/web/e2e/workbench.spec.js` | Unlanded | `5a109cc` against `4294d10`. Structured handoff, duplicate submit and stale review journey is absent. |
| `desktop/web/index.html` | Unlanded | `5a109cc` against `4294d10`. Assignment roles, structured handoff controls and idle-session disclosure are absent. |
| `desktop/web/src/api-response.js` | Unlanded | `5a109cc` against `4294d10`. Manual redirect and non-JSON handling module is absent. Base app calls response.json directly. |
| `desktop/web/src/app.js` | Unlanded | `5a109cc` against `4294d10`. Backup redirect handling and idle-session disclosure are absent. |
| `desktop/web/src/specs-ui.js` | Unlanded | `5a109cc` against `4294d10`. Structured handoff UI is absent. The busy retry part is superseded by 917569c. |
| `desktop/web/src/style.css` | Unlanded | `5a109cc` against `4294d10`. Idle-session disclosure styles are absent. |
| `desktop/web/test/api-response.test.js` | Unlanded | `5a109cc` against `4294d10`. Parser regression coverage is absent. |

## codex/backup-2026-11-07-verb-integration at `2db0ce5`

| Changed file | Status | Commit and source evidence |
| --- | --- | --- |
| `app/src/main/java/com/example/verb/mobile/DesktopBridgeClient.kt` | Unlanded | `2db0ce5` against `4294d10`. Reusable pinned TLS connection is absent. |
| `app/src/main/java/com/example/verb/viewmodel/DesktopPhoneViewModel.kt` | Unlanded | `2db0ce5` against `4294d10`. Backup cleanup for the retained connection is absent. |
| `app/src/test/java/com/example/verb/mobile/DesktopBridgeClientTest.kt` | Unlanded | `2db0ce5` against `4294d10`. Persistent connection compatibility coverage is absent. |
| `desktop/Cargo.lock` | Unlanded | `2db0ce5` against `4294d10`. The tip adds time as a direct dependency of Verb. Base resolves time transitively. TLS updates belong to 4294d10 and are separate. |
| `desktop/Cargo.toml` | Unlanded | `2db0ce5` against `4294d10`. Direct time dependency with parsing is absent. |
| `desktop/src/agent_chat.rs` | Superseded | `77e69ec`. The old chat module was replaced by shared conversation machinery. Exact parity with backup reconciliation is unverified. |
| `desktop/src/ask.rs` | Unlanded | `2db0ce5` against `4294d10`. Git helper still uses Command::output without the backup deadline wrapper. |
| `desktop/src/continuity.rs` | Superseded | `dab9a44`. Remote origin read is bounded through the base helper with different limits. |
| `desktop/src/diff.rs` | Unlanded | `2db0ce5` against `4294d10`. Read is bounded by f303c43, but backup NUL-delimited rename parsing differs from base old-to-new text handling. |
| `desktop/src/exec.rs` | Unlanded | `2db0ce5` against `4294d10`. Base has bounded execution through 08b5664 and f303c43. Backup streaming hash support and its additional runner coverage are not established in base. |
| `desktop/src/gitstate.rs` | Superseded | `dab9a44`. Base bounds Git state probes with its own helpers. Exact failure mapping parity is unverified. |
| `desktop/src/gitview.rs` | Superseded | `08b5664`. Uses the base bounded reader for checkout and isolated Git observations with different limits. |
| `desktop/src/good.rs` | Unlanded | `2db0ce5` against `4294d10`. Base already has bounded hash logic from fd2fb59, but plain Git reads still use Command::output. |
| `desktop/src/host.rs` | Superseded | `f303c43`. Bounded host probe has a different timeout. |
| `desktop/src/hub.rs` | Unlanded | `2db0ce5` against `4294d10`. Project file listing Git helper still uses Command::output. |
| `desktop/src/phone.rs` | Unlanded | `2db0ce5` against `4294d10`. Conversation flow was reworked by 77e69ec. Phone TLS handler still reads one request per connection. |
| `desktop/src/project.rs` | Unlanded | `2db0ce5` against `4294d10`. Project Git identity helper still uses Command::output. |
| `desktop/src/runtime.rs` | Superseded | `f303c43`. Uses the base bounded runtime probe. Exact environment and failure mapping parity is unverified. |
| `desktop/src/specs.rs` | Superseded | `dd1086e`, `2e50a90`. Selected-file commits are implemented through reviewed selection and recovery. Git read behavior is bounded in the base, while backup previous_path representation differs. |
| `desktop/src/talk.rs` | Unlanded | `2db0ce5` against `4294d10`. Backup transcript ID and timestamp propagation is absent. |
| `desktop/src/transcript.rs` | Unlanded | `2db0ce5` against `4294d10`. Shared conversation parsing exists, but backup Item id and timestamp_ms representation is absent. |
| `desktop/src/web.rs` | Unlanded | `2db0ce5` against `4294d10`. Selected-file commits use dd1086e and stalled-read capacity uses ed180b5. The backup's unified background response deadline is not established in base. |
| `desktop/src/web/state.rs` | Unlanded | `2db0ce5` against `4294d10`. Base StateJob::run locks and shares cache work. Backup worker returns busy via try_lock. |
| `desktop/tests/web_integration.rs` | Unlanded | `2db0ce5` against `4294d10`. Reusable phone connection test helpers and journeys are absent. |
| `desktop/web/e2e/workbench.spec.js` | Superseded | `dd1086e`. Reviewed selected-file commit has a replacement journey and request contract. |
| `desktop/web/index.html` | Superseded | `dd1086e`. The backup selected-path checklist is replaced by reviewed-file commit controls. |
| `desktop/web/src/specs-ui.js` | Superseded | `dd1086e`. Selected-file commit sends reviewed inputs through a different contract. |
| `docs/CAPABILITY_ARCHITECTURE_MAP.md` | Unlanded | `2db0ce5` against `4294d10`. Document absent at base. |
| `docs/PRODUCTION_HARDENING_PLAN.md` | Unlanded | `2db0ce5` against `4294d10`. Document absent at base. |
| `docs/REVIEW_HARDENING_2026-11-02.md` | Unlanded | `2db0ce5` against `4294d10`. Document absent at base. |
| `docs/SECURITY_MODEL.md` | Unlanded | `2db0ce5` against `4294d10`. Backup mobile privacy paragraph is absent. The related conversation implementation evolved in 77e69ec. |

## codex/backup-2026-11-07-verb-studio at `219656a`

| Changed file | Status | Commit and source evidence |
| --- | --- | --- |
| `desktop/build.rs` | Landed | `45d1f25`. Same blob. Obsolete wire build path removed. |
| `desktop/scripts/studio-workspace.py` | Landed | `45d1f25`. Same blob. Bounded raster image reads. |
| `desktop/scripts/studio-workspace.wire` | Landed | `45d1f25`. Absent at both tips. |
| `desktop/scripts/test-studio-workspace.py` | Landed | `45d1f25`. Same blob. Adapter framing and image tests. |
| `desktop/src/exec.rs` | Landed | `45d1f25`. Studio exchange retains the 4 MiB cap after f303c43. |
| `desktop/src/studio.rs` | Landed | `45d1f25`. Native image handling retained alongside preview changes in 32a1cf7. |
| `desktop/src/studio_bridge.rs` | Landed | `45d1f25`. Same blob. Embedded adapter and image operation. |
| `desktop/src/tokens.rs` | Landed | `45d1f25`. Studio image route remains in read policy after 0aa643e. |
| `desktop/src/web.rs` | Landed | `45d1f25`. Studio image endpoint retained in native and isolated paths. |
| `desktop/tests/synthetic_boundary.rs` | Landed | `45d1f25`. Same blob. Isolated adapter image coverage. |
| `desktop/web/e2e/studio-native.spec.js` | Landed | `45d1f25`. Image and console coverage carried forward in Studio journeys with startup changes in 917569c. |
| `desktop/web/index.html` | Superseded | `77e69ec`, `c467e79`. Console pane structure and controls evolved with conversation and restored collapsible layouts. |
| `desktop/web/src/studio-editor.js` | Superseded | `32a1cf7`, `59e8ac4`, `917569c`. Preview recovery, editor and startup handling evolved. Draft protection remains part of Studio. |
| `desktop/web/src/studio-previews.js` | Landed | `45d1f25`. Typed previews and image handling retained through c467e79. |
| `desktop/web/src/studio-ui.js` | Superseded | `77e69ec`, `54704db`, `c467e79`. Console wiring evolved with shared conversation, guided checks and pane controls. |
| `desktop/web/src/studio.css` | Superseded | `c467e79`. Console layout evolved with restored collapsible panes. |
| `docs/history/STUDIO_CONSOLE_DESIGN.md` | Landed | `45d1f25`. Same blob. |
| `docs/STUDIO_WORKSPACE_CONTRACT.md` | Landed | `45d1f25`. Same blob. |

## Cargo audit

`cargo audit` in `desktop/` exited 1 before scanning. The tool could not obtain an exclusive lock on the read-only advisory database lock path `~/.cargo/advisory-db..lock`. Advisory results are unverified.

### Audit rerun

With a temporary writable advisory database and network access, Cargo Audit loaded 1,446 advisories and scanned 293 locked dependencies. It exited 1 with one medium advisory (6.5), RUSTSEC-2026-0462, against rmcp 3.8.0. The audit reports 3.9.0 or later as the fixed version. Dependency files were untouched.

## Owner decision list

Security and boundary items come first. File names refer to the full paths in the branch tables above. Some files participate in several items. Tests listed here were inspected in the backup and were not executed.

| ID | Behavior and source | Sensitive surfaces | Backup tests | Recommendation |
| --- | --- | --- | --- | --- |
| D1 | Bind a structured spec handoff to the reviewed revision, serialize competing launches and support retries after refused admission. Security: specs.rs, web.rs, index.html, specs-ui.js, synthetic_boundary.rs, web_integration.rs, workbench.spec.js. | Execution boundary and spec audit. No Git commit or token change identified. | Yes. Stale review, duplicate submission, interruption and refusal coverage. | Needs owner review. Changes how successor agents are authorized and launched. |
| D2 | Add implement, review and investigate role instructions for spec launches. Security: specs.rs, web.rs, index.html, specs-ui.js. | Execution boundary. | Standalone role coverage unverified. | Needs owner review. Defines the instructions passed to the agent. |
| D3 | Surface sign-in redirects and malformed or non-JSON responses without trying to parse them as normal API data. Security: api-response.js, app.js, api-response.test.js. | Authentication UX. No token policy change identified. | Yes, parser unit tests. | Land. Gives the operator a distinct recovery message for access interruptions. |
| D4 | Bound remaining automatic Git observations and hash reads. Integration: ask.rs, exec.rs, good.rs, hub.rs, project.rs. | Execution boundary. | Yes, runner deadline and output-limit tests. Call-site coverage unverified. | Land. The base still has direct process output calls on these paths. |
| D5 | Bound background HTTP response waits and return busy for contended workspace cache refreshes. Integration: web.rs, web/state.rs. | Execution boundary and worker admission. | Yes for response timeout and slot retention. Cache contention coverage unverified. | Needs owner review. The base already separates read capacity and shares refresh work. |
| D6 | Read rename and copy paths from NUL-delimited Git output. Integration: diff.rs, specs.rs. | Spec commit path selection and Git reads. | Yes, selected-file rename and unusual filename coverage in specs.rs. | Needs owner review. Compare against the current reviewed commit path before carrying over any parser changes. |
| D7 | Allow synthetic tests to use an explicitly selected sibling Verb binary. Security: synthetic_boundary.rs. | Test execution boundary only. | The harness contains the override. Separate coverage is unverified. | Skip. No cross-built test run is part of the approved phase. |
| D8 | Reuse a pinned TLS phone connection with one-shot compatibility and lifecycle cleanup. Integration: DesktopBridgeClient.kt, DesktopPhoneViewModel.kt, DesktopBridgeClientTest.kt, phone.rs, web_integration.rs. | Phone authentication transport. No token policy change identified. | Yes, persistent and one-shot connection journeys. | Needs owner review. Changes connection lifetime and server concurrency. |
| D9 | Carry transcript item identity and timestamps into Talk and chat reconciliation. Integration: transcript.rs, talk.rs, agent_chat.rs, Cargo.toml, Cargo.lock. | None of the four surfaces identified. | Standalone coverage unverified. | Skip. The base replaced the old chat path, and carrying the old model would introduce a direct dependency. |
| D10 | Show a dedicated idle-session disclosure when the list has only inactive sessions. Security: index.html, app.js, style.css. | None identified. | Dedicated coverage unverified. | Skip. Session placement is already part of the Bench design. |
| D11 | Record capability ownership, hardening priorities and review findings. Integration: CAPABILITY_ARCHITECTURE_MAP.md, PRODUCTION_HARDENING_PLAN.md, REVIEW_HARDENING_2026-11-02.md. | Documents authentication and boundary behavior. | No executable tests in these artifacts. | Skip. The documents describe the backup baseline. |
| D12 | Document phone transcript opt-in, redaction and prompt eligibility. Integration: SECURITY_MODEL.md. | Documents privacy and prompt eligibility. | Documentation-only change. Related behavioral parity is unverified. | Needs owner review. Validate the claims against the current shared conversation path before adopting the wording. |

### Superseded behavior with a weaker base guarantee

- Integration exec limits are tracked in D4. In the superseded entries for continuity.rs, gitstate.rs and gitview.rs, the base general Git read helper permits 15 seconds and 8 MiB, versus the backup's usual 3 seconds and 2 MiB. Those are weaker bounds on the individual reads, even where the base adds other controls. The isolated Git view backup also supplies an explicit 12-second cap.
- Integration specs.rs replaces the backup's explicit previous_path field. The base still supports reviewed selected-file commits, but preservation of separate rename source metadata is weaker in the spec summary. See D6 for the related parser difference.
- No other weaker guarantee was confirmed in the superseded entries. Exact parity for replaced chat, runtime probe behavior and Studio journeys remains unverified.
