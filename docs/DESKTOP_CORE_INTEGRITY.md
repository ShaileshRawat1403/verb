# Desktop core integrity

This document describes the desktop workbench core as implemented in `desktop/`. It is a local,
headless collaboration layer around installed CLI agents. The UX journey for these capabilities
is tracked separately in `DESKTOP_WORKBENCH_UX_PLAN.md`.

## State and ownership contracts

| Contract | Mechanism | Verification |
| --- | --- | --- |
| Several agents can work in one project | Each launch has its own session ID and file under `~/.verb/sessions` | `two_sessions_in_one_project_survive_restart_and_export_together` |
| Parallel agents can edit without sharing a checkout | `isolated` creates a Git worktree from HEAD; a registry gives every worktree one Verb project ID while each native agent keeps its checkout path | `isolated_agent_edits_its_own_checkout_and_joins_the_same_project_ledger` |
| Older project work survives the ID migration | The registry moves one path-keyed work store under a lock; competing populated stores produce an explicit error | `older_path_keyed_work_is_migrated_without_losing_memory` |
| Resume chooses the intended conversation | Explicit session ID maps to one agent resume identity; missing evidence blocks resume | `explicit_resume_uses_only_the_chosen_conversation_and_migrates_legacy_record` |
| One process owns a live session | Per-session kernel file lock spans hosting; on Unix the agent inherits it across a Verb host crash | `a_live_session_cannot_be_resumed_by_a_second_verb_process`, `an_agent_keeps_the_session_lock_when_the_verb_host_crashes` |
| Shared memory and handoffs survive process restarts | Project `memory.md` and task JSON records use a project lock, atomic replacement, and file/directory sync | `shared_memory_and_handoff_history_survive_separate_agent_processes` |
| Any installed CLI can join the task ledger | `verb agent CMD [ARGS...]` creates an external agent session with the same session bridge and actor checks | `arbitrary_cli_agent_can_fetch_claim_and_handoff_without_a_native_resume_contract` |
| Agents can fetch one consistent shared revision | `shared read` hashes memory and full task history under the project lock, then writes a per-session receipt after successful output | `two_agent_sessions_share_a_revision_and_detect_updates_across_processes`, `hosted_claude_and_codex_can_fetch_through_their_inherited_bridge` |
| A session can find pending collaboration after restart | `inbox` derives help, review, reply, and assignment attention from durable task events and per-session fetch cursors; opening it has no mutation or execution side effect | `session_inbox_tracks_pending_work_and_delivered_events_across_processes`, `inbox_is_readable_and_clickable_at_eighty_columns` |
| Claude and Codex are prompted to fetch on start and exact resume | A positional interactive first turn tells each CLI to run `shared read`; the receipt still requires the agent to execute it | `shared_bootstrap_is_a_positional_first_turn_only_for_verified_clis`, `hosted_claude_and_codex_can_fetch_through_their_inherited_bridge` |
| Concurrent shared notes do not overwrite one another | `shared publish` validates and appends under the same project lock, with session attribution | `concurrent_shared_publications_keep_both_notes_and_reject_foreign_sessions` |
| Replacing memory cannot erase an unseen change | `memory set --base-hash` checks the current memory digest under the project lock; `--force` is explicit | `replacing_shared_memory_requires_the_version_that_was_read` |
| Two agents cannot claim the same task | Claim is validated and written while holding the project lock | `concurrent_claims_have_one_winner` |
| Stale handoffs cannot silently release ownership | Under the project lock, the owner's last fetched memory and target-task revision (or the Workbench form's opening revision) must match; unrelated tasks may change | `handoff_requires_the_owners_current_shared_revision` |
| A lost owner does not strand its task | Explicit reassignment records the new owner and reason without deleting history | `an_unavailable_owner_can_be_reassigned_without_losing_history` |
| A phone cannot become a second session or send input without control | A bridge is bound to one live TUI PTY; one-use pairing, a single input lease, revocation, and idle expiry guard its volatile screen and input | `local_process_protocol_pairs_and_controls_only_the_chosen_live_session`, `the_control_lease_prevents_two_devices_from_typing`, `an_idle_phone_loses_input_control_without_ending_the_agent` |
| Damaged session records cannot silently disappear | Session listing fails with the offending record path | `a_corrupt_session_record_is_reported_instead_of_silently_disappearing` |
| The local web UI uses the same ledger and a real hosted PTY | Loopback-only HTTP, a per-launch API token, and a Rust-owned PTY connect browser panes to the existing sessions and work store | `local_web_ui_shares_the_ledger_and_hosts_a_real_terminal` |

Task history records ownership, reassignment, help requests and replies, review handoffs, and completion. A
handoff releases ownership for another session to claim. The task brief, project memory, and event
history can be read together through `verb task context ID`. Verb provides the path to shared memory
in `VERB_PROJECT_MEMORY_PATH`, the session actor in `VERB_SESSION_ID`, and its own executable in
`VERB_BIN`. `verb shared read` delivers a versioned snapshot through the CLI and records a fetch
receipt. This does not establish that a model read or followed the content. Agent transcripts and
credentials remain in each agent's store.
The project registry under `~/.verb/projects` assigns a local ID to a Git repository's common
directory. Task records and shared snapshots carry that ID; existing task records without it
remain readable through the migrated primary checkout identity. Session records also keep the ID
in a desktop-private field. Their `projectId` in the shared v1 session JSON still names the actual
checkout, preserving exact native resume and the host-neutral schema. A deleted worktree can
therefore leave a visible, non-resumable session and its task history in the parent project.
An isolated session starts from committed HEAD and retains its worktree after exit. Verb makes no
merge decision and never copies uncommitted edits from the source checkout into the new branch.
`verb inbox [SESSION_ID]` and the Workbench session inbox use these records to show pending help
and review work, replies, assignments, and shared-context freshness. Help and review remain
available until a task transition resolves them. A newly delivered reply or assignment leaves
the inbox when the session fetches the updated shared snapshot. Older receipts without event
cursors cannot establish event-level freshness. The inbox is recorded attention only: it does
not dispatch work, infer model comprehension, or add another task owner.
For installed Claude and Codex CLIs, Verb uses their positional prompt support on start and exact
resume to ask for a fresh `shared read` before work. This starts an agent turn; it does not bypass
the agent's native tool approvals. The fetch command itself is what records delivery. OpenCode,
external agents, and launches with caller-supplied arguments retain the manual bridge until their
startup contract is verified. `verb agent CMD [ARGS...]` makes any installed interactive CLI a
tracked task actor. The Workbench offers AGY, Hermes, OpenClaw TUI, and an executable picker; those
presets use this same generic adapter. Native conversation ID capture and exact resume are not
claimed for those presets. Their task and memory history persists after their process exits.

The task owner must fetch the current memory and its task before a CLI review handoff. A concurrent
change to either blocks the handoff until the owner fetches again. Unrelated tasks can advance
independently. Workbench
captures a revision when its handoff composer opens and compares it under the same project lock
when the user submits. A rejected handoff leaves ownership and history unchanged.

Recovery adapters check exact local Claude, Codex, or OpenCode conversation evidence. Unknown
evidence stays `INTERRUPTED`, never an invented `RECOVERABLE` state. OpenCode's read-only immutable
SQLite query cannot see uncheckpointed WAL entries, so an absent ID while a WAL exists is reported
as unknown. Resuming a Claude or Codex conversation starts observation at the end of its existing
record, preserving new events without replaying old ones.

## Verification gate

From the repository root:

```bash
npm ci --prefix desktop/web
npm run build --prefix desktop/web
cargo fmt --all --check --manifest-path desktop/Cargo.toml
cargo clippy --manifest-path desktop/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path desktop/Cargo.toml --all-targets
cargo build --release --manifest-path desktop/Cargo.toml
```

Run this gate after changing the desktop core. The integration suite includes two-agent shared
revision, restart, hosted bridge, generic agent hosting, memory replacement conflicts, scoped
handoff conflicts, concurrent publication, and cross-project rejection checks.
On 26 September 2026, this checkout passed formatting, Clippy with warnings denied, the release
build, 149 unit tests, 17 durable-session integration tests, 6 shell integration tests, and 3 web
integration tests. Two
machine-specific observation tests remain ignored by their existing design.

The integration tests use isolated state directories and fake CLI agents. They exercise separate
Verb processes, real file locks, forced host termination, state reconciliation, concurrent claims,
and actual persisted files. The shell integration tests exercise Bash and Zsh markers without
changing the user's own shell configuration. Adapter unit tests exercise the supported evidence
formats. A passing suite proves those contracts in the test environment; it does not certify every
future CLI release or operating system.

## Boundaries

- Exact resume needs the original CLI and its local conversation store. A source ZIP and `.vcont`
  export do not include those stores. `.vcont` is read-only structural evidence.
- AGY, Hermes, OpenClaw TUI, and other `verb agent` processes are task-capable while live but have
  no verified native identity adapter. Verb marks their sessions ended on exit instead of guessing
  which native conversation to resume. The installed OpenCode adapter retains exact resume.
- Project memory and task records live under `~/.verb/work` or `VERB_STATE_DIR/work`; moving only
  the source tree does not move this work store or the local project registry. There is no
  cross-machine work-store sync yet.
- Shared-context fetches are explicit CLI calls. Verb does not yet inject context into every
  vendor model turn or claim that a model consumed a fetched snapshot.
- The native Unix PTY and inherited process lock have automated coverage on macOS. Windows uses
  inherited console I/O and does not have the Unix crash-inheritance behavior.
- The phone bridge is a local owner-only Unix socket for live TUI-hosted sessions. There is no LAN
  listener, remote transport, Android receiver, or cross-device continuation yet. The local socket
  protocol is a developer preview; it never turns imported `.vcont` evidence into a live session.
- The browser workbench binds to localhost. Its API requires a per-run token; it is a local
  controller, not a remote collaboration server. A browser can control terminal panes only while
  the web host that started those PTYs remains running.
- Each task accepts at most 200 history events, with bounded notes and memory. Verb reports a
  storage-limit error instead of dropping history.
- Task and memory operations are available through the CLI and Workbench. The first-user usability
  gate remains in `DESKTOP_WORKBENCH_UX_PLAN.md`.
