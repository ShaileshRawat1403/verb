# Verb PocketFabric UX and performance review — 1 October 2026

This is a pre-commit validation snapshot. The subsequent user-authorized source-control closeout and remaining work are recorded in [BACKLOG.md](BACKLOG.md#pocketfabric-web-ux-closeout--1-october-2026); references below to uncommitted work describe the measurement/review stage.

Status: implementation, local verification, ARM64 cross-compilation, and isolated OnePlus API/browser verification complete. No commit or production replacement was made. The user subsequently supplied the SSH administration route; existing SSH access was used without changing its configuration. Production acceptance remains pending review of a live-host replacement, which would interrupt its active PTYs. Git/CI utility additions are deferred under the latest instruction to focus only on UX and terminal latency.

## Baseline and ownership

The baseline was AGY's dirty working tree, preserved before edits in `/private/tmp/verb-agy-baseline.diff` and reconstructed into an isolated temporary tree for comparison. Existing authentication changes, dependency additions, and CLI support were preserved. This pass adds no new launcher types. The launcher remains Terminal, Claude Code, Codex, AGY, and Other CLI.

No saved AGY profiling scripts or measurements were found in the repository or relevant temporary files. Their location was requested. The measurements below are new measurements of the preserved working-tree baseline and the revised build, not AGY's original results or measurements of a deployed commit.

## Findings and changes

- The production browser and AGY's built stylesheet contained no `.xterm-helper-textarea` rules. Chrome reported its terminal input as visible (`opacity: 1`, `position: static`). Rebuilding the imported xterm stylesheet restored the terminal renderer and hidden input (`opacity: 0`, `position: absolute`). A web regression test checks the emitted stylesheet.
- `/api/state` performed repository, project, task, memory, and session work on the same loop that delivered PTY input/output. A concurrent local request measured input waiting approximately 90 ms. Full state computation now runs in a worker using a generation-checked cache; authentication and PTY ownership stay in the existing host. Checks were already offloaded and remain so.
- `/api/workspace` returns the known project identity, deployment identity, and currently hosted sessions without Git status, ledger reads/reconciliation, tasks, memory, or attention calculations. The browser uses it first and for lightweight background refreshes. Full state and checks load separately.
- New sessions use the host's already resolved project identity. Memory path construction no longer resolves that identity again, and saving the new session does not repeat the identity scan. Isolated checkout creation retains its existing behavior.
- Session panes mount directly from launch results. The isolated flag is captured before the launch form resets. A session revision guards against an earlier state request discarding a just-launched or explicitly closed session.
- Only currently hosted live sessions mount automatically. Recoverable/unconfirmed/ended sessions are in collapsed History. The selected pane occupies the workspace; existing split controls remain available. Switching uses already mounted panes and immediately requests current output.
- Active pane output drains immediately when bytes arrive, uses a 100 ms idle cadence including request time, and receives a prompt poll after input. Other panes poll once per second. Input stays ordered and buffered; large pastes are split at the endpoint's 8192-byte UTF-8 limit.
- Unchanged state does not rerender the view. Session counts show active sessions, the selected row is highlighted, history expansion survives refreshes, and session names remain legible. The redundant Sessions hero disappears while working. The final 1224 × 693 browser viewport fits without document scrolling.
- `VERB_NODE_NAME` and `VERB_DEPLOYMENT=remote|local` configure the deployment label. Existing `VERB_ALLOWED_ORIGIN` configuration provides the remote default when deployment mode is omitted. Normal local installations retain “Running on this computer / LOCAL.” Remote mode defaults to “PocketFabric Node 1 / REMOTE.” No hosting configuration was changed.
- Phone controls appear only when a phone is connected or owns input. Authentication and phone-control architecture were not changed.
- History removal requires confirmation and is restricted to ended sessions belonging to this project. Hosted active sessions, other projects, and agent transcripts are preserved. Existing session locks provide the host-ownership guard.
- Unversioned web assets now use no-store to avoid carrying an incompatible JS/CSS pair through an hour of browser caching. Both assets are emitted by the existing build.

## Measurements

All times are milliseconds, measured on this Mac in isolated temporary projects. These are not OnePlus or Cloudflare timings. The fixture used ten ended records plus a running terminal. Server API measurements used the same workload before and after; terminal creation is a median over eleven launches, and idle input/output are medians over ten requests. Other API results are individual samples. Chrome used the actual browser, a loopback forwarding fixture, and temporary in-page timing instrumentation; automation tool elapsed time was not used as application latency.

| Path | AGY baseline | Revised build | Interpretation |
|---|---:|---:|---|
| Initial usable workspace, Chrome | 812 | 148–277 | Time from temporary instrumentation startup through workspace/pane DOM creation; samples vary with browser startup/rendering |
| Lightweight workspace API | unavailable | 16.38 | Secondary repository work does not gate this response |
| Cold `/api/state` | 201.46 | 130.51 | Full state remains secondary and can still take time |
| Terminal creation, server median | 99.46 | 22.22 | Known identity avoids repeated resolution |
| New pane visible, Chrome | 258.8 | 48.2 | Time from launch fetch initiation to mounted xterm DOM |
| Idle terminal input, server median | 0.345 | 0.305 | Essentially unchanged; both are sub-millisecond locally |
| Input while uncached state runs | 89.60 | 0.68 | State work no longer stalls terminal input |
| Idle output polling, server median | 0.340 | 0.295 | Essentially unchanged; both are sub-millisecond locally |
| Switch to another active pane, Chrome | 32.4 | 11.1 | Click to next animation frame; this is a frame proxy, not a GPU presentation timestamp |
| Full state with ten ended records | 153.56 | 157.08 | No material improvement claimed; history is collapsed and this work is off the hot path |

Sampled idle key-to-render observations were about 23–29 ms before and 21–43 ms after. There is no demonstrated idle typing speedup in those small local samples. The measured improvement is removal of state-induced input stalls and substantially faster terminal creation/initial workspace readiness.

Network/Cloudflare/tunnel latency was not measured or attributed. The read-only Chrome automation scope did not expose resource performance entries. No native developer-tool bypass was attempted. No PTY-delivery timestamps or GPU presentation timings were captured. A temporary slow-Git wrapper experiment did not intercept Verb's trusted Git resolution and was excluded from the reported results.

## OnePlus Node 1 verification

The node is a OnePlus 9 running Termux supervision and Debian PRoot. SSH administration used the user-provided existing key and port 8022. Verb, sshd, and cloudflared were running. No service configuration, Cloudflare route, DNS, AdGuard, LanguageOps, Android APK, or tunnel was changed. The Mac is a build/admin client; production remains independent of it.

The existing `/usr/local/bin/verb` was measured against an ARM64 Linux release candidate at `/tmp/verb-ux-candidate`. The candidate SHA-256 is `de47275acd4ecf0b1e518f49f4164aaaac86de5aeae20266d52095198505ead0`, verified on both machines. Cross-compilation used the already installed rustup ARM64 target and cargo-zigbuild. A harmless deprecated linker optimization warning was emitted. Both binaries executed on the OnePlus, sequentially, in separate disposable Git projects and homes/state directories. Each workload launched eleven `cat` terminals and explicitly closed ten, leaving ten historical sessions and one active terminal. Production sessions and `/root/.verb` were not used for the benchmark.

| OnePlus path, ms | Current deployed binary | Candidate | Evidence |
|---|---:|---:|---|
| Initial lightweight workspace API | 404, absent | 71.40 | Cold request; not browser readiness |
| Cold `/api/state` | 4,343.44 | 4,143.95 | Individual samples; full state remains slow |
| Terminal creation | 1,687.71 | 40.79 | Median of eleven launches |
| Input during uncached state | 2,156.86 | 7.19 | Median of three overlaps |
| Idle input | 3.36 | 4.16 | Median of ten; no idle improvement claimed |
| Idle output polling | 3.15 | 3.70 | Median of ten; no idle improvement claimed |
| Full state with ten historical sessions | 3,607.67 | 3,387.27 | Individual samples; secondary work remains expensive |
| Lightweight workspace with ten historical sessions | absent | 3.32 | Individual warm request |

Before input-overlap samples were 2,679.80 / 2,156.86 / 1,685.06 ms; candidate samples were 322.67 / 7.19 / 3.80 ms. The 323 ms candidate outlier remains a limitation: offloading state removes the serialized state wait, but does not eliminate device scheduling, storage, or CPU contention. Candidate creation also had a 268.31 ms outlier. These samples are too small for percentile claims and do not attribute all residual lag to a specific resource.

A separate read-only request against the actual running production process at `127.0.0.1:3005` measured `/api/state` at 4,522.76 ms with nine sessions, three hosted. Its `/api/workspace` returned 404. This confirms a server-side delay exists before Cloudflare/browser effects; it does not measure those effects. Through the real HTTPS URL, a newly created verification shell returned `/root/verb-test-project`, a clean short Git status, and `Linux ... aarch64 GNU/Linux`, confirming execution on the node.

The raw node API results are retained at `/private/tmp/verb-node-api-results.jsonl`. The temporary script initially counted ended sessions using a nonexistent `live` field; that count is invalid and excluded. The eleven total records and ten explicit closures define the benchmark history workload. No timing values were changed.

The user explicitly approved temporary loopback-only SSH forwarding after automatic approval review initially rejected it under the earlier SSH boundary. Browser verification then used the actual Chrome browser, a temporary Python proxy on the OnePlus, and existing SSH transport. The proxy supplied the disposable host token internally and inserted temporary timing instrumentation; production source/assets contain no instrumentation. Both hosts used ten ended sessions plus one active shell before browser testing, and the baseline fixture was stopped before starting the candidate fixture.

| OnePlus browser path, ms | Current binary | Candidate |
|---|---:|---:|
| Initial usable workspace and restored pane | 5,389.7 | 221.6 |
| New Terminal request | 3,376.6 | 82.0 |
| New Terminal pane visible | 3,550.8 | 106.3 |
| Switch to existing active pane, next-frame proxy | 723.8 | 16.6 / 36.2 |

These are individual browser observations, not medians or percentiles. Readiness starts at the temporary in-page instrumentation startup, rather than navigation initiation; pane visibility starts at the launch fetch and ends when xterm DOM mounts. Switching measures the next animation frame, not GPU presentation. SSH/proxy/LAN costs are included, and Cloudflare is excluded. The baseline creation observation overlapped background state activity, so it is slower than the isolated server median above.

Key-to-render observations were variable: approximately 6–89 ms in the baseline, plus one 5,718 ms delayed observation, versus approximately 2–150 ms in the candidate. The instrumentation follows DOM output mutations after key events and does not prove that every mutation represents the matching key, especially across multiple sessions. No idle typing speedup or exact PTY-delivery latency is claimed from these samples.

Candidate acceptance in the isolated OnePlus workspace passed: Sessions rendered before branch/status completed; Terminal opened; `pwd`, `git status`, and `uname -a` returned the disposable Debian project and ARM64 Linux; Claude and Codex each opened in existing node CLIs; switching used the mounted panes; closing and reopening the browser tab restored all four active sessions; ten ended sessions stayed in collapsed History; the selected shell used the main workspace; the label was PocketFabric Node 1 / REMOTE; and no Phone badge appeared. Claude's first-run theme prompt and Codex's sign-in prompt were left untouched in the disposable home. This verifies process launch/pane continuity, not authenticated model interaction. AGY was not found on the inspected Debian PATH, and no installation or agent configuration was changed. Chrome reported no warnings/errors.

The final UI screenshot was displayed during browser verification and showed the four active rows, collapsed History (10), a large selected terminal with ARM64 Linux output, and the REMOTE label. Temporary test hosts and forwarding were closed after verification. Final health checks confirmed production Verb (PID 13012) and cloudflared (PID 9165) remained running with their original process IDs. An unexpected supervision issue was observed twice: `sv status sshd` reported `runsv not running`, while the original `sshd -D -e` process (PID 26923) remained alive and SSH connections still worked. No SSH supervisor repair or restart was attempted under the scope boundary; the cause was not measured. The candidate binary and measurement files remain under Debian `/tmp` for review. The real HTTPS URL still serves the previous binary, so revised production acceptance is not claimed. Production replacement must preserve a rollback binary and restart only Verb; its existing live PTYs would be interrupted. No other service needs restarting.

## Verification

- Complete Rust suite: 248 passed, 2 ignored, 0 failed. The ignored tests are pre-existing.
- Web suite: 11 passed, 0 failed. Web production build passed.
- New deterministic integration test holds the project registry lock during a state request, then confirms workspace, terminal input, and output remain available while that state request is still blocked.
- Integration tests cover workspace availability despite corrupt unrelated history, deployment labels, authentication of the workspace route, preservation of active sessions, and preservation of other projects during cleanup.
- Real Chrome against the local revised build: Terminal pane and `pwd`, `git status`, `uname -a` output verified; Claude and Codex panes launched and switched; closing and reopening the Verb tab restored all four active sessions; ten ended sessions remained collapsed. CLI first-run setup prompts were left untouched in the isolated home. No model-authentication flow was completed.
- Chrome console reported no warnings or errors in the inspected local session.
- The real production URL was inspected read-only and the missing xterm CSS confirmed there. The revised build was not deployed to it, so the requested production acceptance sequence has not passed yet.
- `git diff --check` passed. No temporary measurement scripts or debug instrumentation were added to the repository. Temporary scripts and isolated fixtures were staged only under Debian `/tmp`. No new credentials were added. Existing synthetic RSA test fixtures and deployment identity examples are inherited AGY changes. Android, runtime, SSH, DNS, Cloudflare configuration, AdGuard, LanguageOps, and tunnel files were not edited.

## Review artifacts

The final local preview is `http://127.0.0.1:60937/`. It uses a disposable project and test deployment labels, not the OnePlus node. Temporary instrumentation is present only in this loopback fixture. Production assets contain no timing instrumentation.

Measurements and Rust/web test logs are retained under `/private/tmp/verb-ux-before.json`, `/private/tmp/verb-ux-after.json`, `/private/tmp/verb-rust-tests-final.log`, and `/private/tmp/verb-web-tests-final.log`.

This pass modifies desktop session startup identity handling, the web host state/workspace paths, the existing browser UX, built assets, and regression tests. Cargo dependencies and authentication logic are inherited, not introduced by this pass. Formatting touched some inherited lines in these same files.
