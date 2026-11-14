# Verb IDE for non-developers: direction and open-source shortlist

Research date: 25 October 2026. Status: proposed architecture; first local full-IDE comparison completed.
Source baseline: deployed clean 52b70727c1e7a2d57a6469893c5ed8450c02fd6d.
This document adds no runtime dependency, starts no server and changes no deployment.
PocketFabric Node repositories, controllers and services remain outside this work.

## Product direction

Verb's target is a complete general-purpose development environment, comparable in
scope to VS Code plus an interactive terminal. A non-developer can use guided tasks,
while a developer can directly edit code, run commands, inspect internals and use
language tooling. Both operate on the same project and actual execution state.

The full capability target includes navigation/refactoring, extensions, integrated
terminals, arbitrary authorized CLI tasks, builds, tests, debugging, Git/conflicts,
previews/ports, environments, remote workspaces and release workflows. Guided UX
must not replace these capabilities with a fixed catalogue of agent actions.

Primary journey: Describe → Plan → Build → Preview → Check → Publish.
Retain the actual six Spec stages and independent execution, verification and delivery
records beneath that journey; user-facing shortcuts must not invent evidence or skip
required review. Preview is an activity, not another durable stage.

Desktop: task/Spec navigation, main preview or file/change view, contextual agent or
terminal panel. Phone: one main surface at a time, explicit navigation, persistent
attention and reconnect actions. Make technical detail progressively available.
A beginner should see outcomes, changed files, check results and recovery choices.
An experienced user should still be able to inspect diffs, branches and raw output.

## Existing integration point

Verb Desktop already has a Rust HTTP host, a plain JavaScript/esbuild web client,
xterm terminals, project-file listing/preview, Git and the Spec/evidence workbench.
The current web package has no Monaco/CodeMirror dependency. Existing file endpoints
provide a read path; this is not evidence of a safe general-purpose write API.

Keep the same browser client for a Mac and phone, and the same host authority for
files, terminals and work records. An embedded editor runs in the client browser;
a language server or extension host, if added later, runs elsewhere and has a separate
resource cost. Do not require a native desktop application to use basic Verb.

## Open-source options

| Option | What it supplies | Fit assessment for Verb | Published license / primary source |
| --- | --- | --- | --- |
| CodeMirror 6 | Embeddable browser editor with configurable extensions | Useful embedded component or mobile editor candidate; insufficient as the entire IDE foundation. Phone input, accessibility and actual bundle cost need measurement. It does not supply a complete IDE, Git service or LSP host. | MIT; [official view repository](https://github.com/codemirror/view), [development repository](https://github.com/codemirror/dev) |
| Monaco | Browser editor derived from VS Code sources | Editor component used by full IDE foundations; alone it does not provide extensions, debuggers or environments. Official FAQ says mobile browsers are unsupported and VS Code extensions do not directly run in Monaco. | MIT; [official repository and FAQ](https://github.com/microsoft/monaco-editor) |
| Zed | Native editor with local UI and SSH remote development | Study responsiveness, project navigation and agent workflow; consider an optional desktop companion later. Current official README lists web as unavailable. A direct fork would mean a native product path as well as license and upstream maintenance decisions. | Primarily GPL-3.0-or-later, marked Apache components; [official README](https://github.com/zed-industries/zed), [remote architecture](https://zed.dev/docs/remote-development) |
| Lapce | Rust/Floem native editor, LSP, remote development and WASI plugins | Another native reference/possible companion. Its architecture is not a drop-in browser editor. Validate current remote UX before choosing it; its docs record a workspace-opening limitation during a UI rewrite. | Apache-2.0; [official repository](https://github.com/lapce/lapce), [remote docs](https://docs.lapce.dev/get-started/remote-development) |
| Eclipse Theia | Framework for browser and desktop IDE products | Primary custom-IDE foundation candidate now that broad language tooling and extensions are core requirements. Evaluate the additional backend/services and customization effort against Verb's small host. | Platform EPL-2.0 with stated secondary license; IDE packaging template MIT. [platform](https://github.com/eclipse-theia/theia), [IDE template](https://github.com/eclipse-theia/theia-ide), [FAQ](https://theia-ide.org/docs/faq/) |
| code-server / OpenVSCode Server | Complete VS Code-derived browser workbench | Primary VS Code-derived foundation/baseline candidates for full engineering capability. Embedding a second workbench may duplicate terminals, auth, Git and state. code-server publishes a 1 GB RAM / 2 vCPU minimum; that is not measured idle use or a guarantee for this phone. | MIT at these project repositories, with dependency notices to check; [code-server](https://github.com/coder/code-server), [OpenVSCode Server](https://github.com/gitpod-io/openvscode-server) |

These are architectural assessments, not measured performance rankings. Native GPU
rendering does not establish lower total RAM than an embedded browser component.
The first local full-IDE trial is now recorded in [IDE lab results](../desktop/experiments/ide-lab/RESULTS.md); it is not a phone or sustained performance benchmark.

CodeMirror's GitHub repositories were archived after moving to the maintainer's
[own forge](https://code.haverbeke.berlin/codemirror/view). That notice is a source
migration, not proof of abandonment. The forge/site returned 403 to this research
client; recheck active releases and package provenance before selecting a version.
Licenses above describe upstream declarations; redistribution needs the actual
selected components and dependency notices checked, rather than relying on this table.

## Full capability contract

| Area | Required capability and proof |
| --- | --- |
| Editing | Multiple files/tabs, project search/replace, syntax, completion, definitions/references, rename/refactor, formatting, diagnostics, explicit saves and conflicting-writer handling. Prove with real language servers and external file edits. |
| Terminal and tasks | Existing persistent PTYs plus direct interactive commands, task definitions, background jobs, exit status, output and reconnect. Preserve authorized CLI access; no dependence on an AI provider for basic development. |
| Build and test | Invoke project tools/package managers, unit/integration tests, watch modes, test navigation, failure output and reproducible environment configuration. Success must identify the source tested. |
| Debugging | Actual debug-adapter sessions with breakpoints, continue/step, variables, stack frames and attach/launch where the environment permits. Debugging is a core milestone. |
| Extensions | Install/run selected compatible extensions from supported sources; verify exact API, native dependency and architecture compatibility. Extension-provided commands participate in the same authorization and execution model. |
| Git | Branches/worktrees, diff/stage/commit, fetch/pull/push, conflict resolution, history and reviewed recovery. Beginner wording accompanies accurate Git semantics; it does not remove direct operations. |
| Preview | Start approved app/dev servers, authenticated port access, isolated preview content, browser errors and source/version association. A port number alone is not preview authorization. |
| Environments | Configure runtimes, dependencies, environment-variable references, remote workspaces and supported container workflows. Choose the execution host explicitly; opening a project must not silently install or execute it. |
| Delivery | CI/PR/release/deployment workflows, exact-revision provenance, status and rollback visibility. Public interruption/publishing retains action-specific authorization. |
| Continuity | Preserve edits and selected workspace through reconnect, offer recovery after process death, and distinguish ended work from unconfirmed cleanup. |

"Every development task" means broad tooling and an extensible execution environment,
not that a phone can perform every OS/hardware-specific build. A macOS/iOS build may
need an authorized Mac worker; other tasks may need a different architecture, GPU,
container runtime or credential. Verb should route to appropriate workspaces and
explain requirements instead of claiming unsupported execution.

## Provisional foundation decision

Evaluate two complete-IDE paths first:

1. Eclipse Theia for a custom Verb IDE: customize the workbench and add the guided
   Spec/evidence experience while retaining language tooling, terminals, debuggers
   and compatible extension support. Its [extension architecture](https://theia-ide.org/docs/extensions/)
   distinguishes product-level Theia extensions from runtime VS Code extensions.
2. code-server / OpenVSCode Server for a VS Code-derived foundation: establish a
   working full-IDE baseline and evaluate a Verb extension/integration. Prefer
   supported extension points over a deep upstream fork. Explicitly test whether
   enough control exists for the desired beginner UI and execution boundary.

CodeMirror and Monaco remain editor components to evaluate within those paths,
rather than the primary foundation decision. Zed/Lapce remain native alternatives
if research proves native desktop distribution is required. Keep browser access as
the current default assumption; a native-only choice needs an explicit product decision.

Published extension compatibility does not mean all Microsoft VS Code extensions
are available or permitted. code-server documents [Open VSX and marketplace differences](https://github.com/coder/code-server/blob/main/docs/FAQ.md).
Record the required extensions and verify each, including native ARM64 dependencies,
instead of promising universal Marketplace parity.

## Integration architecture to prove

- Verb owns Spec intent, execution identity, evidence, approvals and delivery facts.
  The IDE supplies editing, navigation, diagnostics, debugging and extension UI.
- A shared workspace contract identifies project, path, source revision, active
  tools and selected execution host. Do not create competing project/terminal stores
  with inconsistent ownership. Decide which system owns each state before integrating.
- Keep the Rust host where it remains useful, but do not require every upstream IDE
  backend to be rewritten in Rust. Additional IDE/extension/language-server processes
  must have measured budgets and explicit lifecycles.
- IDE filesystem writes, extension tasks and debugger launches must obey the existing
  execution boundary. A second service pointed at the public project with broader
  privileges is not an acceptable integration shortcut.
- Guided and advanced views share operations. For example, “Run checks” invokes a
  visible project task that is also available through the terminal/task UI, while
  evidence still records what actually ran and against which revision.
- Prototype phone editing and debugging navigation explicitly. Monaco's documented
  lack of mobile support is a real selection constraint, not a reason to assume
  Theia or a VS Code-derived shell becomes phone-compatible automatically.

## Ordered milestones and acceptance

| Milestone | Deliverable | Acceptance |
| --- | --- | --- |
| 1. Complete-IDE comparison | Disposable Theia and code-server/OpenVSCode prototypes, same sample projects and tasks | Open/edit files, run a terminal, complete a build/test task, use real completion/refactor, debug a failure, resolve a Git conflict, run a compatible extension and open a preview. Record exact versions/licenses, compatibility gaps, browser assets/heap, host/child RSS, cold start and input/reconnect latency. No live phone or Node changes. |
| 2. Foundation and workspace contract | Choose the base from measured capability, usability, resource and integration evidence; define authority/state ownership | A concrete decision record covers filesystem/terminal ownership, extension-host privileges, selected builders, reconnect, auth and resource limits. Required Node-owned boundary changes are dependencies to defer, never implicit scope. |
| 3. Complete first supported stack | Real editing/language tools, build/tests, debugger, Git/conflicts, terminal and preview in an isolated workspace | A small web app and Python tool can each be built, tested, debugged and changed manually without agent assistance. A broken test and conflicting external edit remain recoverable. Expand to further stacks through verified tooling. |
| 4. Guided non-developer UX | Spec-first navigation, outcome explanations, contextual next actions, change/preview review and task guidance | A beginner completes a visible app change and understands what was saved, checked and published. An advanced user can access underlying files, terminal, tasks, debug tools and Git. Both views preserve one state model. |
| 5. Evidence and delivery integration | Exact-source CI/PR/deployment receipts, draft/reconnect recovery and guarded publishing | Source edits invalidate stale acceptance; an upload is not labelled a deployment. Failed or ambiguous writes are never replayed automatically. Backup/restore and safe rollback are exercised in disposable workspaces. |
| 6. Resource/phone/remote acceptance | Demand-start language/debug/extension tooling, tested mobile interactions and alternate builders | Measure sustained representative use on intended hosts under load. Explicitly test mobile typing/IME, touch selection, keyboard/screen reader, 320–390px layouts and soft-keyboard resizing. Route unsupported builds to an authorized host; label unavailable tools accurately. |

Milestone 1 is the next implementation slice. Its scope is a disposable local
comparison, not replacing the live Verb service. The stopped local preview remains
stopped unless a later authorized development task creates a new isolated fixture.
The first local comparison has now run; see [the actual results and limits](../desktop/experiments/ide-lab/RESULTS.md). The next slice is the workspace/authority adapter contract, using a VS Code-derived baseline first while retaining Theia as a contender.

## Usability principles and scope

- Prefer “Review changes”, “Save checkpoint”, “Upload changes” and “Publish” alongside
  accurate technical detail. Never label a commit as a backup or an upload as a deploy.
- Explain the next action in context; keep Specs/evidence/delivery as a single source
  of truth rather than adding a parallel IDE task store.
- Preserve drafts on network loss and route/navigation changes; explicitly label
  drafts held only in the current browser. Do not assume sensitive drafts belong in
  persistent browser storage without a retention and threat-model decision.
- Keep keyboard navigation, visible focus, screen-reader naming, touch target sizes,
  readable contrast and reduced-motion behaviour as acceptance requirements.
- Avoid automatically creating worktrees, installing dependencies, executing preview
  scripts or starting agents just because a project was opened.
- First non-developer trial: change a visible label in a disposable sample app, inspect
  its preview and diff, understand the check result, save a checkpoint and explain
  what is and is not published. Observe where help is needed before expanding scope.

## Decisions deliberately left open

Exact first supported language/framework versions, primary phone browser, whether
native desktop distribution is needed, IDE/extension versions, resource budgets
and the alternate execution-host contract. Current assumption: browser-first
Verb on the existing personal phone host, with desktop browser access equally valid.

## Approved canvas direction — 26 October 2026

The owner approved the interactive canvas concept after rejecting the dashboard-first
mockups. The workspace should start from the thing being made, with a focused work
thread beside it. Direct selection, outcome descriptions, visible execution, before/after
review and progressively available files/terminal/debugging are the target. Kanban is
an overview, not the mandatory entry point. Phone layouts need a useful decision and
continuation surface rather than a compressed desktop dashboard.

The first local implementation adds an opt-in **Studio** view. It reads real project
files through the existing file-preview API and shows the selected Spec's actual
execution, verification and delivery presentation. New intent prepares the existing
Spec form; only explicit form submission creates work. Draft text stays in the tab
through navigation. Existing terminal sessions and Git/evidence workflows remain the
single source of truth. Existing Sessions remains the default entry point during this
first slice.

This is a document canvas and work-context integration, not the finished live-product
canvas. Project HTML is displayed as text, never executed. There is no new editor,
preview port proxy, extension host, provider invocation or execution authority. The UI
labels read-only files and the missing live-preview/editor connection. The next slice
is the workspace/authority contract for authenticated product previews and the chosen
IDE's editing/debugging bridge. Do not relax CSP or point a second IDE at the public
workspace to make a mockup appear functional.

Local acceptance: build and module/browser checks are recorded in the session handoff.
No public deployment, PocketFabric Node changes, commit or push is part of this slice.

### Local Studio refinement — before public deployment

Canvas tools now precede the document. File-path search preserves the open document,
and unavailable raw-source actions are disabled. At phone/tablet widths, explicit
Canvas/Current work controls expose one surface at a time and retain drafts across
navigation. Desktop retains both surfaces and its visible direction box.

A user can select up to 4,000 characters inside the current document and explicitly
attach that text, with its source path, to a new change request. The quote remains
visible and removable in the draft. Define change includes it in the existing Spec
form for review; cancellation preserves it and successful saving clears only the
matching draft. It is a quoted snapshot, not a claim of current-source evidence.
No agent runs and no permissions change by selecting or attaching text.

### Native-local edit and preview proof

The first workspace adapter now implements actual revision-checked text saves and
interactive saved-file HTML previews in Studio. The capabilities endpoint fails
closed for external/account/invalid execution profiles and remote/Access-configured
hosts. Public/Node editing stays unavailable. See the [workspace contract](STUDIO_WORKSPACE_CONTRACT.md)
for ownership, API, save/recovery semantics, sandboxing and precise limitations.
This is a native-local self-contained-web-app proof, not the full IDE foundation or
an arbitrary development-server integration. Earlier read-only-only descriptions
record previous slices and are superseded only for supported native-local workspaces.

### Monitored-boundary Studio adapter — release candidate

The single-file Studio pilot now has a Verb-only boundary adapter. It carries fixed,
embedded Python source through the existing controller-admitted pipes runtime, using
isolated Python and JSON stdin. It neither installs a guest executable nor changes
Node source, services, registry or policy. The host performs no external-project file
writes directly. Capabilities verify project UID and protocol; saves retain explicit
revision checks and ambiguous-outcome recovery. It stays disabled until explicit
launcher opt-in and installed-boundary acceptance. See STUDIO_WORKSPACE_CONTRACT.md
for release/rollback gates. Full IDE tooling and dev-server previews remain future work.
