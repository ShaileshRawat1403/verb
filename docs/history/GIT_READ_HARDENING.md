# Bounded Git observation candidate

2026-10-21. Target: reliable daily use by one owner on a phone-hosted service.
Base: accepted pilot source `f352aa2368247248e22389783a89489dac8cf1a9`.
Local source candidate only; not deployed or phone-accepted.

## Behavior

Repository-local filter discovery now has a five-second timeout and a 64 KiB
output limit. Failed discovery refuses the observation instead of allowing a
repository-configured filter to run without inspection. Git's ordinary no-match
and not-a-repository results remain supported.

Git presentation reads and the external snapshot client now have a fifteen-second
host timeout and an 8 MiB stdout limit (64 KiB stderr). Config discovery runs
before the read timeout, so one local read can take up to twenty seconds across
those stages. These are per-command limits, not one deadline for an entire page
request or a multi-command snapshot.

Output is collected from nonblocking pipes on the requesting worker, with bounded
drain turns. No additional pipe-reader threads are created. A child that exits
while a descendant retains a pipe still times out. On failure, the still-unreaped
leader's process group is killed and the direct child reaped; group IDs are never
signalled after reaping the leader. This bounds ordinary userspace stalls, not an
unkillable kernel task or filesystem operation during spawn/reaping.

The native boundary controller retains responsibility for processes across its
privilege boundary. Killing the local client does not itself prove guest cleanup;
recovery receipts remain untouched by the collector. The installed-boundary
disconnect proof below verifies cleanup and acknowledged receipt retirement.
This change does not promise cancellation of detached processes
that escaped the local process group.

## Scope and remaining work

This first slice covers filter discovery and `gitview` reads. Other automatic
Git callers, the overall request budget, worker-permit recovery under an installed
boundary failure and background-read coalescing still need separate work. Existing
bounded project-read helpers are unchanged. User-requested Git mutation and hook
semantics are unchanged.

Regressions cover both output streams, nonzero exits, stalled children, inherited
pipes, descendant cancellation, output floods, invalid local config, valid filter
discovery, ordinary non-repository folders and a real five-second config-probe
deadline. Run the Rust, browser and offline restore gates before review. Linux
boundary fixtures and device acceptance remain separate release gates; a successful
Mac run is insufficient to authorize deployment.

## Local validation results

Candidate branch: `codex/production-git-read-deadlines`, uncommitted at this checkpoint.
No phone services, launchers, ledgers or user sessions were changed.

| Check | Result |
| --- | --- |
| Mac Rust suite | 386 passed, 5 intentionally ignored; after the portable fixture edit, all 6 shell integration tests passed again. |
| ARM64 GNU/Linux, Rust 1.99.0, disposable Ubuntu container | All test groups passed after correcting fixtures: 403 passed, 5 intentionally ignored, including 15 synthetic-boundary and 36 HTTP tests. The initial combined invocation failed two fixture groups; both affected groups passed on targeted rerun. |
| Real GNU/Linux account-separation fixture | Passed PTY/Talk/Git execution, private-record metadata and exact resume, secret denial, unchanged private-file permissions, no-new-privileges, zero capabilities and inherited 512 MiB memory limit. |
| Clippy | Warnings denied on the local and CI-pinned toolchains; pinned synthetic-boundary feature check also passed. |
| Browser unit tests | 57 passed. |
| Browser journeys on Mac | 22 passed; 1 Linux-only stalled-cleanup journey skipped on Mac and covered separately below. |
| Linux stalled-cleanup browser journey | Passed with no skips in a disposable Chromium container; confirmed cleanup stays quiet, stalled cleanup produces a persistent notice, and dismissal works. |
| Installed native controller, targeted disconnect proof | Passed: the production collector timed out after 15.099 s; an admitted detached synthetic child, its lease and cgroup were removed; the client receipt survived until exact acknowledgement and was then retired by the installed client. Required service PIDs, registry, executable/launcher and original receipts were unchanged. |
| Offline work-state restore | 9 passed. |
| Native fresh-state binary smoke fixture | Passed version, workspace, PTY and exit/replay checks against the local debug binary; not an optimized release-package certification. |
| Formatting, whitespace and browser bundle consistency | Passed. |

The shell-history fixture now explicitly configures saving history in its synthetic
user's `.zshrc` before checking preservation. It previously assumed a nonzero
zsh `SAVEHIST` default. Product history settings are unchanged.

The disposable Linux setup needed an init process to reap orphaned children, a
network interface for the same-network pairing fixture and the full Vim package.
These are test-environment requirements, not phone configuration changes.

An exploratory musl build failed conversation-discovery's creation-time test; the
GNU build passed it. Do not infer musl support or a deployed recovery defect from
that experiment. The source's creation-time fallback deserves separate review
before advertising that target.

The device proof used an existing reserved synthetic project, one background slot
and a bounded sleep command. It started no additional Verb host, performed no
provider calls and restarted no service. No project files or logins were read or
modified. The operator harness and detailed receipts are retained in owner-only,
Git-ignored storage. This proves client-disconnect handling by the installed
controller; it does not claim a new controller-crash or full deployment rehearsal.

## Repeating the Linux browser check

With browser dependencies installed and a Linux debug binary built with
`synthetic-boundary`, run from the repository root:

```sh
VERB_LINUX_BIN="$PWD/desktop/target/aarch64-unknown-linux-gnu/debug/verb" \
  bash desktop/scripts/test-linux-browser.sh e2e/stalled-cleanup.spec.js
```

The runner mounts source read-only, pins the built container image ID and retains
browser failure traces in the ignored `desktop/web/test-results/linux-browser`
directory. It needs no privileged container, phone, credentials or provider.
The optional `VERB_TEST_CHROMIUM` executable override is limited to the test
configuration; normal CI continues using Playwright's pinned browser.

`desktop/scripts/boundary-read-proof.rs` is the small Linux-only manual device
driver. It imports the production collector directly. Do not run it against owner
work or treat it as a service or delegated operator endpoint; the private operator
harness supplies the reserved identity and verifies continuity and exact recovery.

Remaining promotion work: exact-source remote CI and the separately reviewed
candidate/deployment process. Broader Git
callers and overall request budgets remain engineering follow-ups.

## Source cleanup checkpoint

Removed the unused terminal `stream_sink` accessor and browser `ICON_NAMES`
export after checking repository-wide references. Consolidated identical Git
stdout/stderr and exit-status decoding from diff, hub and spec operations into
`exec::git_text`. Each caller retains its original command builder, flags, error
messages and execution policy. Regenerated the embedded browser bundle.

Post-cleanup checks passed: 386 Mac Rust tests (5 intentionally ignored), 57 browser
unit tests, 22 Mac browser journeys (1 Linux-only journey skipped), both local and
CI-pinned Clippy checks, formatting, 9 offline restore tests and the fresh-state
binary smoke fixture. No phone operation or deployment was part of this cleanup.
The earlier targeted device acceptance covers the unchanged bounded collector;
exact-source remote CI remains a release gate for the complete final candidate.

## Git usability checkpoint

The Git card now distinguishes uncommitted files from local history and explains
outgoing and incoming commits relative to the tracked branch. It labels these
comparisons as local records: refreshing status does not contact the remote.
An unlinked branch is explained without claiming the repository has no remotes.

The commit dialog explains that a commit is local and includes all project changes,
including work outside the selected spec and changes made before submission. It
lists every observed file instead of truncating at 40. Loading or failed status
disables submission and explains how to obtain a reviewable list. This does not
introduce file selection, lock the working tree, or change backend commit scope.

Validation: 58 browser unit tests and 23 Mac browser journeys passed; the Linux-only
journey remains skipped on Mac. The browser tests cover a real audited commit,
a 41-file review, remote comparison copy and disabled submission after a failed
Git read. The embedded development binary was rebuilt. No deployment or phone
service restart was performed. Guided remote upload/download and conflict recovery
remain future work.

## Guided Git transfers

This is a focused Git workflow, not Verb session or cloud synchronization. Open
**Upload / download…** in Project or the spec's Git card (also available through
the command palette). Opening the review reads local metadata only. **Check
remote** explicitly fetches the linked branch, then enables an eligible transfer.
The dialog shows the branch, remote name, server, counts and latest 20 commit
subjects in each direction. It fits a phone viewport.

Upload sends the reviewed commit ID to the explicit upstream branch, without
force, additional branches, tags or submodules. Download requires a clean tree
and uses `merge --ff-only --no-autostash`: diverged history needs a person or
coding agent to review it in a terminal. Neither path resets, stashes, rebases or
automatically resolves a conflict. Dirty files and unfinished Git operations
block both transfers. Checking the remote remains available while blocked.

The backend re-derives the entire review before acting, fetches only the linked
remote branch into its direct remote-tracking ref, and requires a new review if
the state or destination changed. Symbolic tracking refs, unlinked branches,
separate push URLs, multiple endpoints and custom transports require terminal
review. URLs are not returned; only a sanitized server label and destination
fingerprint are included. Git failure output is replaced with fixed guidance
because transport diagnostics and hooks can echo credentials.

Reads share a 30-second review deadline (config preparation can consume up to
five additional seconds). Each explicit transfer subprocess has a 60-second
deadline and output cap. A timeout reports an unconfirmed outcome; it does not
claim rollback. Concurrent Verb transfers are refused. Git's own locks and
fast-forward checks still apply; this does not lock out other terminal tools,
and users should finish other Git operations before transferring. Existing Git
credentials and user-configured hooks remain in use through the existing
execution boundary. The request loop stays responsive through deferred workers.

Regression fixtures use local bare repositories and planted credential markers.
They cover exact uploads, fast-forward downloads, stale reviews, destination
changes, indirect refs, dirty files, unfinished operations, real merge conflicts,
divergence, private tags/branches and secret-safe failed uploads. Browser coverage
exercises the complete check/review/upload/download flow, preserves divergent
work, confirms that opening the dialog makes no transfer request, and checks
phone-sized controls. No production remote is used by these tests.

Reference semantics: [Git fetch](https://git-scm.com/docs/git-fetch),
[Git push](https://git-scm.com/docs/git-push) and
[Git merge](https://git-scm.com/docs/git-merge). New remote setup, authentication
onboarding and assisted conflict resolution remain follow-ups. Exact-source CI
and a separate deployment review remain required; no phone service was changed.

Validation checkpoint: the full Mac Rust suite passed 391 tests with five
intentional ignores. After the final review deadline and destination guards,
all five focused transfer regressions passed again on Mac and ARM64 GNU Linux
(Rust 1.99, existing Docker fixture). Clippy and formatting passed. All 58 browser
unit tests and 24 browser journeys passed; the existing Linux-only browser
journey was skipped on Mac. The complete transfer browser journey was repeated
against the final rebuilt development binary, including a 390-pixel phone view.
Changes remain local and uncommitted on `codex/production-git-read-deadlines`.
