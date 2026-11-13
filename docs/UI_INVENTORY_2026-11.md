# Verb Desktop web interface inventory

Source snapshot `origin/codex/reconcile-local-work` at `4294d1013eae5af9a78436ca749ded485c578b4f`. Static source review. Runtime behavior is unverified. Paths in source references are repository-relative.

## 1. Views

Distinct `data-view` values in `desktop/web/index.html`. The sidebar, Overview shortcuts and Studio links repeat some values. JS modules are under `desktop/web/src/`. Routes below are relative to `/api` unless stated otherwise. Braces denote path variables. Dialog-specific calls are listed separately. The app shell also loads `GET /whoami`, `/agents`, `/state`, `/workspace` and `/talk`.

| View | Purpose | JS | API routes | Goal |
| --- | --- | --- | --- | --- |
| `studio` | Define or pick spec work, read an agent conversation and inspect project files, changes and previews in one workspace. | `studio-ui.js`, `studio-editor.js`, `studio-previews.js`, `studio-browser.js`, `specs-ui.js`, `sdlc-ui.js`, `conversation-ui.js` | GET `/files`, `/files/preview?path=…`, `/studio/workspace`, `/studio/file?path=…`, `/studio/image?path=…`, `/git`, `/git/diff?path=…`, `/specs`, `/specs/{id}/sdlc`; POST `/studio/file`, `/studio/preview` (loads returned `/studio-preview/{id}` outside API); GET/POST `/browser`, `/browser/pairing`; spec creation and check actions delegate to the spec/evidence dialogs; conversation routes shared with Sessions. | spec, board |
| `sessions` | Launch, continue, control and inspect live terminals and session history. | `app.js`, `terminal-ui.js`, `terminal-transport.js`, `conversation-ui.js`, `talk-ui.js` | GET `/state`, `/workspace`, `/agents`, `/terminals/{id}/output?after=…`; WebSocket `/terminals/ws`; POST `/terminals`, `/terminals/{id}/input`, `/control` and `/resize` under that terminal, `/sessions/clear-ended`; DELETE `/terminals/{id}`, `/sessions/{id}`; GET `/terminals/{id}/conversation`; POST `/terminals/{id}/chat`, `/terminals/{id}/chat/{messageId}/check` and `/discard` under that message; GET/POST `/stream`, POST `/stream/ack`; GET/DELETE `/talk/{id}`, POST its `/message`, `/stop`, `/terminal`. See also phone and inbox dialogs. | none |
| `project` | Browse files and manage the project brief and generated agent context. | `project-ui.js` | GET `/files`, `/files/preview?path=…`, `/hub`; POST `/hub/brief`, `/hub/sync`. | memory |
| `specs` | Show spec list and stage board, inspect acceptance criteria and evidence, and coordinate Git and agent work. | `specs-ui.js`, `sdlc-ui.js`, `spec-summary.js`, `workbench.js` | GET/POST `/specs`; GET `/git`, `/git/diff?path=…`, `/meters`, `/specs/{id}/stage-check?to=…`; POST `/specs/{id}/branch`, `/agent`, `/talk` under that spec; stage, evidence, Git and handoff routes below. | spec, board |
| `host` | Show machine health, storage and resource usage. | `host-ui.js` | GET `/host`. | none |
| `overview` | Summarize sessions, open tasks, attention and project/runtime checks. | `app.js` | GET `/state`, `/workspace`, `/checks`; POST `/good/mark` from the workspace action. | none |
| `tasks` | Manage the standalone task queue, assignments, help and handoffs. | `app.js` | GET `/state`; POST `/tasks`; GET `/tasks/{id}/handoff-revision`; POST `/tasks/{id}/actions`. | none |
| `memory` | Read and append project Shared notes. | `app.js` | GET `/state`; POST `/memory`. | memory |
| `connections` | List and manage project MCP connections and their tools. | `connections-ui.js` | GET/POST `/connections`. | none |

The goal labels describe direct support for the three stated goals. The standalone task queue and terminal lifecycle have their own records.

## 2. Dialogs

All `<dialog>` elements in `desktop/web/index.html`. Shared state refreshes are covered in section 1. Palette and Observer actions can delegate to another UI.

| Dialog ID | Purpose | JS | API routes | Goal | Opened from |
| --- | --- | --- | --- | --- | --- |
| `phone-dialog` | Display and manage a phone continuation offer. | `app.js` | GET/POST/DELETE `/terminals/{id}/phone`; POST `/terminals/{id}/phone/renew`. | none | Session terminal phone action. |
| `end-session-dialog` | Confirm stopping a running terminal. | `app.js` | DELETE `/terminals/{id}`. | none | Session terminal End action. |
| `session-dialog` | Configure and launch an agent or shell terminal. | `app.js` | POST `/terminals`; agent availability from GET `/agents`. | none | New session actions in the workspace and Sessions. |
| `spec-dialog` | Enter a problem, scope and acceptance criteria for a spec. | `specs-ui.js` | POST `/specs`. | spec, board | Specs New spec, Studio change draft and palette. |
| `stage-dialog` | Review warnings and confirm a workflow stage change. | `specs-ui.js` | GET `/specs/{id}/stage-check?to=…`; POST `/specs/{id}/stage`. | board | Spec stage actions and board card drops. |
| `evidence-dialog` | Record a criterion checkmark and supporting text. | `specs-ui.js` | POST `/specs/{id}/criteria/{index}`. | spec, board | Spec criterion controls, including a control to amend evidence. |
| `git-transfer-dialog` | Review and execute a Git transfer action. | `specs-ui.js` | GET/POST `/git/transfer`. | board | Specs Git transfer action. |
| `sdlc-dialog` | Review or record SDLC evidence and workflow actions, including running a check. | `sdlc-ui.js` | GET/POST `/specs/{id}/sdlc`; related SDLC diff and export controls call GET `/specs/{id}/sdlc-diff`, `/specs/{id}/sdlc-export`. | spec, board | Spec SDLC workflow; Studio Run project check delegates to it. |
| `commit-dialog` | Review file selection and diffs, commit or recover an interrupted commit. | `specs-ui.js` | GET `/git/commit-review`, `/git/diff?path=…`; POST `/git/commit`, `/git/commit-recovery`. | spec, board | Specs commit action and palette. |
| `palette-dialog` | Search and dispatch workbench commands. | `specs-ui.js`, `workbench.js` | No direct route; selected commands use their own handlers. | spec, board, memory | Header command control and keyboard shortcut. |
| `shortcuts-dialog` | List keyboard shortcuts. | `specs-ui.js` | None. | none | Palette and `?` shortcut. |
| `brief-dialog` | Create the project brief from five structured fields. | `project-ui.js` | POST `/hub/brief`. | memory | Project hub Create brief action and palette. |
| `ask-dialog` | Ask a question and show evidence-linked answers. | `ask-ui.js` | POST `/ask`. | none | Header Ask control and workbench command. |
| `handoff-dialog` | Record a spec handoff and start the receiving agent. | `specs-ui.js` | POST `/specs/{id}/handoff`. | spec, board | Spec handoff action, including the context meter prompt. |
| `observer-panel` | Read signals and manage observer settings. Opens as a nonmodal dialog. | `observer-ui.js` | GET/POST `/observer`. | board | Header Observer control. |
| `task-dialog` | Create a standalone task with a title and brief. | `app.js` | POST `/tasks`. | none | Tasks New task action. |
| `note-dialog` | Append a shared note. | `app.js` | POST `/memory`. | memory | Shared notes Add note action and empty state. |
| `action-dialog` | Apply a task action such as claim, reassign, request help, reply, hand off or complete. | `app.js` | GET `/tasks/{id}/handoff-revision`; POST `/tasks/{id}/actions`. | none | Selected task action buttons. |
| `inbox-dialog` | Show what an ended session left and offer a fresh or continued agent launch. | `app.js` | GET `/inbox/{id}`; POST `/terminals`. | none | Session history via an ended session. |
| `paste-dialog` | Preview and confirm multiline terminal paste. | `app.js` | Confirmation sends terminal input via the WebSocket or POST `/terminals/{id}/input`. | none | Terminal multiline paste guard. |

## 3. Units of work

- **Spec and board.** The spec record has `id`, `file`, `stage`, `branch`, `criteria` and `audit` among its fields (`desktop/src/specs.rs:37`). The board groups those records by `stage` (`desktop/web/src/workbench.js:75`). The spec model has no task ID. Its Git association uses the spec branch and the `spec:NNN` commit-message convention (`desktop/web/src/workbench.js:135`).
- **Spec and Studio work thread.** The Studio picker uses spec IDs as option values and renders the selected spec (`desktop/web/src/studio-ui.js:378`). Its change handler calls `workbench.selectSpec(value, { navigate: false })` (`desktop/web/src/studio-ui.js:530`). Specs holds `selectedId`, resolves it against `spec.id`, and stores the selection under browser `localStorage` key `verb.selectedSpec` (`desktop/web/src/specs-ui.js:43`, `:63`, `:77`, `:500`). The Studio work thread has no separate durable ID or task record. Its session conversation picker holds a separate in-memory `conversation.id` and admits live hosted agent sessions, prioritizing those associated with the spec (`desktop/web/src/studio-ui.js:255`, `:277`, `:302`).
- **Task and session.** `Task.id` keys the standalone task. `Task.events[].sessionId` (Rust `TaskEvent.session_id`) holds full session IDs. Ownership and status are derived from the events and surfaced in `TaskSnapshot.owner` and `status` (`desktop/src/workbench.rs:26`, `:40`, `:85`). The task action route keys the task by URL `id` and accepts a session ID in its action payload (`desktop/src/web.rs:2697`). Task and session both carry a checkout/project path and optional stable Verb project ID (`Task.project_id` / `verb_project_id`, `desktop/src/workbench.rs:28`; `Session.project_id` / `verb_project_id`, `desktop/src/session.rs:199`). Project identity gives them shared scope. There is no stored task-to-spec or task-to-Studio-thread field.
- **Spec and session.** The spec agent, handoff and talk-to-terminal launch routes return a full `sessionId` (`desktop/src/web.rs:2646`, `:2661`, `:2669`). The confirmed start audit text stores only the first eight characters of that ID, alongside the agent (`desktop/src/web.rs:147`). The UI parses `spec.audit[].action`, matches the prefix against session `id` and exposes a live link only for a unique match in state `live` (`desktop/web/src/workbench.js:235`). The durable `Session` has no `spec_id` or `task_id` field (`desktop/src/session.rs:199`).
- **Studio work thread and Antigravity Talk.** Talk is a separate planning conversation with `spec_id` and `conversation_id` in its in-memory model (`desktop/src/talk.rs:40`, `:54`, `:70`). The spec Talk route associates it with `spec.id` and records the Talk ID in the spec audit (`desktop/src/web.rs:2594`). Continuing it in a terminal passes the agent conversation identity and spec ID to the launcher (`desktop/src/web.rs:2646`). The Studio conversation picker uses session IDs.

## 4. Shared context

- `desktop/src/hub.rs:19` and `:309` generate the same block for root `AGENTS.md` and `CLAUDE.md`, bounded by `<!-- verb:context:start -->` and `<!-- verb:context:end -->`. It has the heading **Project context (maintained by Verb)** and a notice that the brief and specs are the editable sources.
- If `docs/project/BRIEF.md` is readable, the block names its path and includes the first paragraph of each filled section among Problem, Who it is for, Scope, Constraints and Glossary. Otherwise it says “No project brief yet.” The optional **Active specs** list includes every spec with `stage != "ship"`. Each row contains ID, title, file, stage, completed criterion count out of total and the text of unchecked criteria. There is no browser-selected active spec marker. Full spec text, audit, task records and Shared notes are omitted.
- The **Working agreements** tell agents to read the relevant spec before changing code and stay within its Out of scope section, start relevant commit messages with `spec:NNN`, and leave the Audit trail for Verb to append. The generated block ends with a newline.
- The production write entry point is explicit `POST /api/hub/sync` (`desktop/src/web.rs:2356`, `desktop/web/src/project-ui.js:177`). Reads can detect missing or outdated blocks. Brief creation has its own route. No automatic sync call on brief creation, spec edits or agent start was found. Sync replaces the first recognized block, skips matching blocks, or appends after trimming trailing whitespace when there is no recognized block. A blank or missing file gets an `# AGENTS` or `# CLAUDE` heading. When files changed and a brief exists, it appends a sync audit entry to the brief (`desktop/src/hub.rs:359`, `:398`).
- From the project root an agent can read the synced block with `sed -n '/<!-- verb:context:start -->/,/<!-- verb:context:end -->/p' AGENTS.md` or the same command on `CLAUDE.md`. The listed brief and spec files are also directly readable. Separately, `verb shared read [--json]` reads the current versioned project memory, tasks and handoffs and records a fetch receipt when called by a validated hosted session (`desktop/src/lib.rs:168`, `desktop/src/workbench.rs:528`). `VERB_PROJECT_MEMORY_PATH` exposes the note file to the launched process (`desktop/src/lib.rs:771`, `:882`). Claude and Codex get an explicit bootstrap prompt for the CLI read in the ordinary account configuration, with a different prompt for a separate agent account (`desktop/src/lib.rs:722`). Automatic reading by every supported CLI is unverified.

## 5. Stored formats

The state root is `VERB_STATE_DIR`, defaulting to `~/.verb` (`desktop/src/lib.rs:1412`). “Git” below means the project repository.

| Record | Format and location | Git |
| --- | --- | --- |
| Specs | `specs/<id>-<slug>.md` in project root. Markdown metadata, criteria checkboxes, handoff notes and audit. See `desktop/src/specs.rs:82`, `:392`, `:822`. | Project files can be staged and committed. Creation writes the file. |
| Tasks | Versioned JSON in `<state>/work/p-<project-id>/tasks/<id>.json`. See `desktop/src/project.rs:157`, `desktop/src/workbench.rs:1323`. | Host state, outside the Git workflow. |
| Shared notes | Markdown in `<state>/work/p-<project-id>/memory.md`. See `desktop/src/workbench.rs:171`, `:1330`. | Host state, outside the Git workflow. |
| Brief and agent context | Project Markdown at `docs/project/BRIEF.md`, `AGENTS.md`, `CLAUDE.md`. | Project files can be staged and committed. The hub writes files and records an audit entry. |
| SDLC evidence | Versioned JSON ledger at `<state>/sdlc/v1/<digest(project-id)>/<spec-id>.json`. Records include criterion, evidence ID, result and binding to tree, intent, commit and branch (`desktop/src/sdlc.rs:51`, `:233`, `:864`). The legacy criterion/audit text belongs to the spec Markdown. | Ledger is host state. Spec Markdown and the Git objects referenced by evidence have their own Git lifecycle. |
| Sessions | Key/value schema-v1 records at `<state>/sessions/s-<encoded-id>.session`, with legacy project-keyed records. Structural events are JSONL at `<state>/events/<hex-project-path>/<session-id>.jsonl` (`desktop/src/session.rs:247`, `desktop/src/lib.rs:1335`, `:1376`, `:1680`). | Host state, outside the Git workflow. |

## 6. Docs

Classifications refer to this source snapshot. Current means an indexed guide, contract, reference or live plan. Historical includes snapshots, research and mockups. Supersession is listed only where a repository document identifies a successor. “Unverified” means a successor could not be confirmed.

| File under `docs/` | Classification | Superseding document |
| --- | --- | --- |
| `AGENT_RUNTIME_V1.md` | Current | — |
| `AGENT_USER_ISOLATION.md` | Current | — |
| `AGY_LAUNCH_DEBUG.md` | Historical | Unverified |
| `AI_ASSIST_BRIEF.md` | Current | — |
| `AI_PROVIDERS.md` | Current | — |
| `ARCHITECTURE.md` | Current | — |
| `ARCHITECTURE_HOSTED.md` | Historical | Unverified |
| `AREAS.md` | Current | — |
| `BACKLOG.md` | Current | — |
| `BETA8_HANDOFF.md` | Historical | Unverified |
| `BROWSER_ARCHITECTURE_RESEARCH.md` | Historical | Unverified |
| `BROWSER_COMPANION_CONTRACT.md` | Current | — |
| `BROWSER_RUNTIME_CONTRACT.md` | Current | — |
| `COMPATIBILITY_MATRIX.md` | Historical | Unverified |
| `CONVERSATION_FLOW_CONTRACT.md` | Current | — |
| `DESIGN_DIRECTION_2026-10-28.md` | Historical | Unverified |
| `DESKTOP_CODE_OWNERSHIP.md` | Current | — |
| `DESKTOP_CORE_INTEGRITY.md` | Current | — |
| `DESKTOP_MOBILE_BRIDGE_PROTOCOL.md` | Current | — |
| `DESKTOP_MOBILE_CONTINUATION_PRODUCT.md` | Current | — |
| `DESKTOP_MVP.md` | Historical | Unverified |
| `DESKTOP_PRODUCTION_READINESS.md` | Current | — |
| `DESKTOP_TERMINALS_BRIEF.md` | Current | — |
| `DESKTOP_TUI_REDESIGN_PROPOSAL.md` | Current | — |
| `DESKTOP_UX_SPRINT.md` | Historical | Unverified |
| `DESKTOP_WORKBENCH_UX_PLAN.md` | Historical | Unverified |
| `DIRECT_BETA.md` | Current | — |
| `DOGFOODING.md` | Current | — |
| `DURABLE_SESSION.md` | Historical | Unverified |
| `GIT_READ_HARDENING.md` | Historical | Unverified |
| `GIT_RECOVERY_CONTRACT.md` | Current | — |
| `HANDOFF.md` | Historical | `PRODUCT_VISION.md`, `PRD.md`, `ROADMAP.md`, `TUI_VISION.md` for the product layer. |
| `HANDOFF_MOBILE_UX_SLICE.md` | Historical | Unverified |
| `IN_APP_BROWSER_PROPOSAL.md` | Superseded | `BROWSER_ARCHITECTURE_RESEARCH.md` |
| `LOCAL_REVIEW_CHECKPOINT_2026-10-28.md` | Historical | Unverified |
| `MCP_CONNECTIONS_CONTRACT.md` | Current | — |
| `NEXT_SPRINT.md` | Historical | `ROADMAP.md` for scope and `PRODUCT_VISION.md`, `PRD.md`, `TUI_VISION.md` for the product layer. |
| `P0.5_TERMINAL_CONTEXT.md` | Current | — |
| `POSITIONING.md` | Current | — |
| `PRD.md` | Current | — |
| `PREVIEW_LOADING_CONTRACT.md` | Current | — |
| `PRIMARY_WORKSTATION_RELEASE.md` | Current | — |
| `PRODUCT_VISION.md` | Current | — |
| `README.md` | Current | — |
| `RELEASE_CHECKLIST.md` | Current | — |
| `ROADMAP.md` | Current | — |
| `SDLC_EVIDENCE_CONTRACT.md` | Current | — |
| `SDLC_RELEASE_READINESS.md` | Historical | Unverified |
| `SDLC_WORKBENCH_BRIEF.md` | Current | — |
| `SECURITY_MODEL.md` | Current | — |
| `SELECTIVE_COMMIT_CONTRACT.md` | Current | — |
| `SPEC_WORKBENCH.md` | Current | — |
| `STUDIO_CODE_EDITOR.md` | Current | — |
| `STUDIO_CONSOLE_DESIGN.md` | Historical | Unverified |
| `STUDIO_WORKSPACE_CONTRACT.md` | Current | — |
| `STUDIO_WORKSTATION_ASSESSMENT.md` | Historical | Unverified |
| `TERMINAL_RUNTIME_ROADMAP.md` | Current | — |
| `TUI_VISION.md` | Current | `DESKTOP_TUI_REDESIGN_PROPOSAL.md` for the landing screen. Terminal guidance remains current. |
| `UI_INVENTORY_2026-11.md` | Current | — |
| `UTILITY_SCOUT_2026-10-28.md` | Historical | Unverified |
| `UX_FOUNDATION.md` | Current | — |
| `UX_WALKTHROUGH_2026-10-28.md` | Historical | Unverified |
| `V0_DEVICE_VALIDATION.md` | Historical | Unverified |
| `V0_DEVICE_VALIDATION_RESULTS.md` | Historical | Unverified |
| `VERB_CONTINUITY_ENVELOPE.md` | Current | — |
| `VERB_NON_DEVELOPER_IDE_PLAN.md` | Current | — |
| `VERB_SESSION_CONTRACT.md` | Current | — |
| `VERB_SESSION_SCHEMA.md` | Current | — |
| `WEB_UX_DEPLOYMENT.md` | Historical | Unverified |
| `WEB_UX_JOURNEY_REVIEW.md` | Historical | Unverified |
| `WEB_UX_PERFORMANCE_REVIEW.md` | Historical | Unverified |
| `WEB_UX_POLISH_REVIEW.md` | Historical | Unverified |
| `WORKING_WORLD.md` | Current | — |
| `learn/01_the_verb_philosophy.md` | Current | — |
| `learn/02_session_lifecycle_and_truth.md` | Current | — |
| `learn/03_multi_terminal_and_pty_engine.md` | Current | — |
| `learn/04_agent_adapters_and_evidence.md` | Current | — |
| `learn/05_privacy_continuity_and_context.md` | Current | — |
| `learn/README.md` | Current | — |
| `mockups/desktop-phone-continuation.html` | Historical | Unverified |
| `mockups/desktop-workbench.html` | Historical | Unverified |
| `mockups/verb-tui-task-detail-v2.png` | Historical | Unverified |
| `mockups/verb-tui-task-detail-v2.txt` | Historical | Unverified |
| `mockups/verb-tui-terminal-v2.png` | Historical | Unverified |
| `mockups/verb-tui-terminal-v2.txt` | Historical | Unverified |
| `mockups/verb-tui-workbench-v2.png` | Historical | Unverified |
| `mockups/verb-tui-workbench-v2.txt` | Historical | Unverified |
| `perf/HANDOFF.md` | Historical | Unverified |
| `perf/README.md` | Current | — |
| `perf/results/2026-10-02-0.1.0-beta.12-vivo-I2202-manual.json` | Historical | Unverified |

## 7. Overlaps

| Job | Surfaces and shared behavior |
| --- | --- |
| Define or resume spec work | Studio’s change draft and Specs New spec use the same spec dialog and endpoint. Studio’s thread picker and the board/list share the selected spec. |
| Inspect evidence or run checks | Studio presents a check summary and delegates Run project check to the spec SDLC flow. Specs also has legacy criterion checkmarks with supporting text, while the SDLC flow stores separate evidence records and acceptance. |
| Browse or preview files | Studio and Project both use the file tree and preview endpoints. Studio adds editing and saved HTML previews. |
| Inspect Git changes | Studio’s Review Git changes and Specs’ changed-files UI use the Git summary and per-path diff endpoints. The commit dialog also reads per-path diffs. |
| Read an agent conversation | Studio and Sessions mount the same conversation component by terminal ID. Antigravity Talk is a separate spec-linked planning interface with its own routes and a continue-in-terminal action. |
| Track work and hand off | Specs and Tasks both show work status and support a handoff. Specs records a note in spec Markdown and launches an agent. Tasks records a session-linked event in the task ledger. |
| Read project context | Project’s synced agent files and Shared notes both expose project information to agents. The generated files contain brief/spec summaries. The Shared notes view and `verb shared read` read the host-side memory, with tasks and handoffs also available through the CLI read. |
| Scan session and task status | Overview summarizes the same session and task state displayed in the dedicated views and links to them. |
