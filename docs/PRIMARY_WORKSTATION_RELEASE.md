# Primary development workstation release

29 October 2026. Target: a person's primary development workspace, welcoming to
non-developers while retaining complete underlying development tools. Android is a
separate product; PocketFabric Node implementation remains outside this work.
Keep this release local until separately approved for promotion.

## Current truth and foundation

Verb has durable PTYs, guarded text saves, single-file HTML previews, selective Git
commit/recovery, Specs/evidence/review and partial conversation adapters. A local
browser runtime exists with explicit limits. Studio now has a CodeMirror editing surface with a plain-text fallback.
Project language servers, debugger and general IDE extensions are not connected. Real-provider conversation
edge cases need live acceptance. Green unit tests do not establish workstation readiness.

Do not repeat the completed code-server/Theia trial: see
`desktop/experiments/ide-lab/RESULTS.md`. Continue with the VS Code-compatible adapter
baseline, retaining the final shell choice until authority, resource and beginner UX
evidence exists. Studio remains the canvas/work-thread experience; adding capability
must not force everyone into generic IDE chrome.

## Ordered release gates

| Slice | Required outcome | Evidence |
| --- | --- | --- |
| Workspace adapter | Single project identity, explicit authority | Reject stale/wrong project; no competing terminal/file authority |
| Editor/files | Creation/rename, tabs, syntax, search/replace, completion, definitions/references, refactor, format, diagnostics | Real web/JS and Python projects; external-write conflict; keyboard/IME/screen reader; draft recovery |
| Tasks/debugger | Build/test/watch, exit/output, breakpoint/step/variables/stack | Broken test and real debug session; cancel/reconnect; project/source attribution |
| Conversation | Live provider read/send/stream and permission handoff | Real Claude/Codex, uncertain delivery, interrupt/resume, no automatic resend, terminal fallback and content opt-out |
| Product preview | Multi-file app/dev server, browser, errors, source association | Actual app/API interaction, scoped ports/auth, crash/reload recovery; public boundary separately accepted |
| Git/delivery | Diff/stage/commit/history/branches/conflicts, CI/PR/status | Beginner understands saved/committed/uploaded/deployed; recovery; stale review invalidation |
| Daily reliability | Session/workspace/draft restoration, backup/restore and resource bounds | Sustained real use, crashes/network loss/disk limits; supported host matrix and measured budgets |
| Workstation UX | Canvas/work thread, progressive tools, coherent utilities | Build/change/debug/review an app inside Verb; desktop/phone walkthrough; accessible focus/contrast/layout |

Each slice requires working behavior, failure-state coverage, actual UI acceptance and
honest limits. Tie checks to source identity. Start language/debug/browser processes
deliberately; measure target-host feasibility. Unsupported builds route to an explicitly
authorized host rather than weakening isolation or claiming universal phone compatibility.

## First implementation

`desktop/integrations/vscode/` contains the local read-only adapter. It binds Verb's
workspace identity, reads capabilities and opens exact-revision text snapshots using
the IDE's read-only document provider. No server routes or execution authority are
added. Sixteen regression checks include the actual Rust host in a disposable fixture.

Real code-server UI acceptance passed for discovery, read-only JavaScript snapshot,
edit refusal and disconnect clearing. Theia/mobile acceptance is not established.

Next gate: project/authority mapping and revision-
checked editing through Verb's existing save boundary. Snapshots are not editable
project mounts. IDE-native language/debug processes require a separately proved
workspace execution contract before they can operate on a public project.

The owner approved commit and distribution on 29 October. This slice is packaged
as a local-installation VSIX. It adds no public server runtime or UI changes, so
public service replacement or restart is unnecessary. Public IDE enablement remains
a future release gate.

## Studio editing release candidate

The next public-facing slice adds code editing inside Studio while retaining the
existing file/save/preview boundary. See [the editor contract](STUDIO_CODE_EDITOR.md)
for behavior, dependencies, failure states and acceptance. This does not enable an
IDE backend or claim the remaining workstation gates are complete. The owner has
approved public Verb-only rollout after validation and a fresh idle check.
