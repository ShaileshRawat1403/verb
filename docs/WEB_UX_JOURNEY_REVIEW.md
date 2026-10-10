# Verb product journey review — 1 October 2026

This is a pre-commit validation snapshot. The subsequent user-authorized source-control closeout and remaining work are recorded in [BACKLOG.md](BACKLOG.md#pocketfabric-web-ux-closeout--1-october-2026); references below to uncommitted work describe the measurement/review stage.

The browser UX refinement is implemented and verified locally and against an isolated candidate on PocketFabric Node 1 (OnePlus 9). No commit or production deployment was made. The live HTTPS URL still serves the earlier binary. Git/CI utilities remain deferred.

## What changed

- Sessions is first in the navigation and remains the starting view. The project card now identifies the project and exposes its path as a tooltip; the inactive dropdown affordance is removed.
- The session launcher names the current project and deployment node. Terminal is selected by default. The action changes with the selection: Open Terminal, Open Claude Code, Open Codex, Open AGY, or Open CLI. The primary launcher types are unchanged.
- Terminal launch shows Opening… and prevents repeated quick-launch clicks while the request is pending. Counts update immediately after mounting a new session, using existing cached state.
- The workspace uses fewer headings and duplicate actions. Each pane shows its session type instead of decorative window controls. Split panes, Focus, and End have explicit labels; split is hidden when there is only one pane. The current session has an accessible current-state attribute and a visual indicator.
- End opens an in-app dialog naming the session, explaining that commands stop and the record remains in History, with Keep working as the initial focus. Closing the browser continues to leave sessions running.
- Ending the final session immediately restores the working empty state and its launch actions. Recovery/history copy no longer incorrectly says that all prior sessions have ended.
- A polling race during intentional session ending is suppressed; failures unrelated to ending still show normally. Failed end requests restore polling so the workspace remains usable.
- At narrow widths, header actions occupy their own row, navigation icons have visible labels, and session selection stays compact and scrollable. The Memory persistence caption is project-aware.

This follow-up changes only the existing web HTML, JavaScript, stylesheet, and their generated assets. It adds no backend endpoint, repository computation, dependency, launcher type, or infrastructure change.

## Verification

Complete Rust suite: **248 passed, 2 existing ignored, 0 failed**. Web suite: **11 passed, 0 failed**. Web production build and ARM64 Linux cross-compilation passed; cross-linking emitted the same harmless deprecated optimization warning as the previous candidate. Diff whitespace checks passed.

Real Chrome local verification covered launcher defaults/context, changing the selected agent and action label, shell launch, split/focus controls, cancelling End, confirming End, the last-session empty state, launching again, and a 390 × 844 viewport check. The temporary viewport override was reset. Intentional ending produced no connection-error toast in the final build.

The final ARM64 binary was copied only to Debian `/tmp/verb-ux-candidate`; matching SHA-256 on Mac and node:

`c1d2fd32148c39e53138d1b6bece3e55f4cd7adab4fbb8367966e8cc959302a8`

A disposable OnePlus project/home/state fixture contained ten ended sessions and one live shell. Through explicitly approved temporary loopback SSH forwarding, real Chrome verified Sessions-first navigation, PocketFabric Node 1 / REMOTE identity, node-aware launcher context, Terminal default and Claude action label, Terminal creation, ARM64 Linux command output, current-session indication, collapsed History, cancellation and confirmation of End, preservation of the other active session, and movement of the ended session into History. Console inspection returned no warnings/errors. Screenshots of the final node workspace and local review UI were displayed during verification.

Final candidate browser observations:

| Path | Time |
|---|---:|
| Initial usable workspace/restored pane | 221.3 ms |
| New Terminal request | 103.2 ms |
| New Terminal pane visible | 127.0 ms |

These are individual observations using the same temporary instrumentation described in [the performance report](WEB_UX_PERFORMANCE_REVIEW.md). The runtime and PTYs ran on the OnePlus; the Mac was only the browser/admin client. SSH/proxy/LAN costs are included, Cloudflare is excluded. These observations check that the UX revision retained quick readiness; they are not new percentile or production performance claims. The previous before/after API and agent-reopen measurements apply to the earlier candidate documented in that report and were not unnecessarily repeated for this web-only revision.

The disposable node host and SSH forwarding were closed afterward. No production binary, `/root/.verb` data, startup script, service configuration, Cloudflare, DNS, AdGuard, SSH configuration, LanguageOps, Android APK, or PocketFabric tunnel was changed. The previously observed SSH supervision issue was not repaired as part of this pass.

## Review

Local review preview: `http://127.0.0.1:60937/`. It uses disposable Mac state and simulated deployment labels; it is not the OnePlus runtime. Production acceptance through `https://verb.example.com` remains pending a separately reviewed replacement and restart of Verb. Such a restart would interrupt its live PTYs; no other service needs restarting.

Rust test log: `/private/tmp/verb-rust-tests-journey.log`. Temporary fixture scripts and timing instrumentation remain outside the repository. Nothing was committed.
