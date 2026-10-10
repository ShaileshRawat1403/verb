# Verb interaction polish — 1 October 2026

This is a pre-commit validation snapshot. The subsequent user-authorized source-control closeout and remaining work are recorded in [BACKLOG.md](BACKLOG.md#pocketfabric-web-ux-closeout--1-october-2026); references below to uncommitted work describe the measurement/review stage.

The polish is implemented in the working tree and verified on an isolated OnePlus candidate. Production still serves the existing binary. Nothing was committed or deployed to the live service; this is ready for the requested review before committing.

## Product changes

- Removed the large Overview promotional hero and illustration. Overview now starts with a compact project heading, active-work counts, and an Open workspace action.
- Project checks, Active sessions, and Tasks are keyboard-operable disclosure panels. A collapsed card actually contracts independently of its neighboring card.
- Navigation and the Sessions list can be hidden independently. Their restore controls remain outside the hidden panels. These two preferences survive browser reloads, with a safe fallback when browser storage is unavailable.
- Collapsing panels resizes existing terminals rather than creating new sessions or refreshing full application state. The same terminal expanded from 702 to 1,168 pixels in the desktop review viewport.
- Unified line icons, smaller type and controls, restrained surfaces, consistent corners and spacing, and visible selected-session treatment replace the heavy promotional presentation.
- Sessions remains the initial workspace, active sessions remain first, and History remains collapsed. Overview and navigation counts now consistently exclude historical records.
- The previous journey improvements remain: contextual launch actions, Opening feedback, duplicate-launch prevention, explicit Split panes/Focus/End controls, an in-app End confirmation, and immediate restoration of the empty workspace after the last session ends.

The follow-up edits only web HTML, JavaScript, CSS, and generated assets. No architecture, backend computation, dependency, agent type, infrastructure, or Android change was introduced by this polish pass.

## Validation and measured results

Complete Rust tests: **248 passed, 2 existing ignored, 0 failed**. Web tests: **11 passed, 0 failed**. Production web build, desktop debug build, and ARM64 Linux release cross-build passed. The cross-linker reported the existing deprecated optimization warning. Diff whitespace checks passed.

Chrome interaction checks covered panel collapse/restore, preference persistence, retaining the same session across reload, launcher selection/action labels and cancellation, session switching, ten historical sessions, project disclosure panels, and OnePlus terminal commands. No browser warnings or errors were observed in the candidate inspection.

The OnePlus candidate ran in a disposable Debian project, home, and state directory. The Mac was only a browser/build/admin client, connected through the explicitly authorized temporary loopback SSH forward. `pwd`, `git status --short`, and `uname -a` returned the disposable node project, its changed file, and ARM64 Linux output.

| Observation | Earlier node baseline | Polished candidate |
|---|---:|---:|
| Initial workspace/restored pane | 5,389.7 ms | 252.8 ms |
| New Terminal request | 3,376.6 ms | 98.2 ms |
| New Terminal pane visible | 3,550.8 ms | 125.4 ms |
| Active-session switch to next frame | 723.8 ms | 16.9 ms |

These are individual observations, not percentiles. Timing begins at the temporary page instrumentation, not navigation start; pane visibility is a DOM observation, and switching measures the next frame. The path includes SSH, proxy, and LAN overhead and excludes Cloudflare. The fixture had ten ended sessions and one initial live shell; opening another shell and switching kept historical sessions out of the active workspace.

After the final CSS-only adjustment to let disclosure cards contract independently, the rebuilt node candidate was checked again: initial workspace/restored pane was **137.9 ms**, and a collapsed Active sessions card measured **57.5 pixels** high beside an expanded Tasks card. The earlier creation/switch observations above precede only that layout adjustment and were not unnecessarily repeated.

The previous API, input/output, session-scaling, and latency attribution results remain in [the performance report](WEB_UX_PERFORMANCE_REVIEW.md). This pass makes no new claim that idle input or raw output endpoint latency improved. Full state/check computation still happens asynchronously and can take seconds without gating the usable terminal.

An attempted 390 × 844 browser viewport override did not change the observed 1,224-pixel viewport in this session. This pass therefore does not claim a fresh narrow-viewport verification; the earlier journey report documents its prior responsive checks. Temporary overrides were reset.

Final node candidate SHA-256:

`83e9a5a77aa6fb5b82f2da67b498e6202468e07d42c57116d7d7784e8362ccf5`

The temporary node fixture and forwarding were stopped after validation. The local review preview remains available at `http://127.0.0.1:60937/`; it uses disposable Mac state with simulated deployment labels, not production node sessions.

## Diff review and deployment boundary

The complete working diff includes AGY's inherited authentication/remote-host changes and the prior performance and journey passes, as well as this polish. It spans the desktop Rust host, integration tests, web source and built assets, Cargo dependency lock changes, and these review documents. No temporary measurement script was added to the repository. New frontend additions were checked for debug logging and measurement instrumentation; whitespace checks passed. Test RSA fixtures in the integration tests are synthetic test data.

Production acceptance remains pending replacement/restart of only Verb and validation through `https://verb.example.com`. A Verb restart interrupts its live PTYs; session records should remain preserved. Cloudflare, DNS, Access policies, AdGuard, SSH configuration, LanguageOps, the Android APK, and the PocketFabric tunnel were not altered.

The separate read-only SSH inspection found a reachable `sshd` process but `sv status sshd` reported `runsv not running`; no `runsv` process for that service was observed. This establishes a supervision mismatch, not its cause. It was not repaired as part of the Verb patch.
