# Studio: current state, target and remaining effort

30 October 2026. Engineering assessment, not a delivery commitment or measured market validation.
Public baseline:04eacf0. Layout repair and pane controls described below are local review changes.
Android is separate. PocketFabric remains outside this work.

## Product direction

A capable development workstation that people can direct without already knowing development
terminology. One continuous journey: describe → clarify → execute → inspect → review → approve.
Keep professional editing, Git, terminal and debugging available through progressive disclosure.
Keep execution, evidence, human review and delivery as distinct facts. CLI agents and model providers
remain replaceable. A welcoming interface must not substitute optimistic labels for actual state.

The direction is coherent. The current risk is fragmentation: adding capabilities and toolbars
without making the complete journey easier. The differentiator should be reliable control and
understanding of a real project. An editor, chat box and connector catalogue alone do not establish
that advantage. Impact is a hypothesis until people complete meaningful work and recover from failures.

## As-is and target

| Area | Current evidence | Target |
| --- | --- | --- |
| Layout | Desktop navigation collapse can constrain Studio to the former sidebar column; compact rules can hide restore buttons. Local repair addresses both. | Every main pane collapses/restores without lost drafts, app resets or off-screen escape controls. |
| Studio | Work thread, editor/document/app/browser previews, Git review and separate terminal surface exist. Browser controls consume substantial first-screen space. | Conversation and output remain primary; compact controls, consistent focus, clear next action and easy tool access. |
| Apps | Explicit saved single-HTML previews and public HTTPS browsing through the chosen Mac companion passed acceptance. | Real multi-file frontend/backend projects, scoped dev-server URLs, errors, logs and source association. |
| Agent work | Supported conversation paths enforce readiness, content opt-in and uncertain delivery. Agent and isolated-runtime support varies; terminal remains essential. | Real Claude/Codex workflows, interruption/resume and permissions work predictably on supported runtimes; unsupported states are clear. |
| Coding | CodeMirror, syntax/local suggestions, explicit saves, conflict recovery and draft tools exist. | Project-aware definitions, diagnostics, rename/refactor, format and debugger on an initial supported language set. |
| Git/delivery | Selective commit/recovery, reviewed transfers, evidence and approval records exist. | Branch/history/PR/CI/conflict guidance understandable without Git expertise; deployed and healthy remain separately observed. |
| Connections | Reviewed remote HTTPS MCP and explicit result-to-draft handoff passed public acceptance. | Selected useful integrations with safe authentication/revocation; no assumption that CLI agents share Verb connections. |

## Layout repair scope

Navigation and Sessions retain existing hide/show controls; compact headers must retain them.
Studio adds independent Current work and Preview hide/show controls, plus Reset Studio layout.
Restoration controls remain outside hidden panes. At least one Studio surface stays visible.
Collapsing uses presentation changes: preserve file drafts, editor history and running app/browser
state. Files/review actions reveal their destination. Escape from layout controls restores the split;
Escape inside an editor or dialog retains its own meaning. Phone retains one surface at a time.
Git changes, task details, history, editor find/location and browser inspectors already have toggles.
This does not promise collapse controls on every arbitrary nested HTML container.

## Suggested order and effort

Ranges assume one experienced full-time engineer, focused JS/TS and Python support, reuse of current
boundaries and adapters, and access to real agents/hosts. Discovery, review and failure acceptance
are included. External auth/provider changes and target-host constraints can move the estimates.

| Work package | Rough engineering effort | Acceptance |
| --- | --- | --- |
| Layout recovery, pane controls and focused visual polish | 2–4 days | Desktop/laptop/phone, keyboard, drafts, preview state and repeated restore. |
| One complete multi-file app/run/preview workflow | 2–3 weeks | Build and change an actual frontend/backend app; port/auth/process scope, crash and reload recovery. |
| Real-agent daily workflow and continuity gaps | 1–3 weeks | Live provider tests on supported local/public runtimes; unknown delivery, permissions and recovery. |
| Project language tooling for the first two language families | 2–4 weeks | Definitions/references, diagnostics, formatting and rename on real projects; execution authority proven. |
| Tasks/debugger integration | 2–4 weeks | Breakpoints, stepping, variables, output and cancellation/reconnect; source/project attribution. |
| Beginner Git/CI/delivery guidance | 1–2 weeks | Branch and conflict recovery, reviewed history/transfer, observable CI and clear delivery truth. |
| Reliability, backup/restore and accessibility qualification | 2–3 weeks, partly overlapping | Network loss, process death, disk/resource limits, keyboard/screen reader and sustained daily use. |

Allow roughly **10–18 engineer-weeks** for this constrained workstation scope, with overlap and
implementation choices still uncertain. This is not VS Code extension/ecosystem parity. Broader
language, debugger, extension and host support is a longer programme. OAuth/local MCP and scheduled
work should follow demonstrated workflow needs rather than displace the core journey.

## Validate impact

Use 5–8 people, including non-developers, on the same three jobs: build a small app, fix a deliberate
failure, and review/deliver a change. Track completion without coaching, terminal-only detours,
recovery time, lost work and whether users correctly distinguish saved/committed/pushed/deployed.
Choose expansion based on those results. The next highest-value milestone is a complete real-app
workflow, after the layout repair is accepted.
