# Performance budget

Verb has to feel immediate on its slowest real host, a phone-hosted node behind an execution
boundary. This page sets the budget, says where each number comes from, and records what was
measured.

## Budget

| What | Target | Must not exceed | Basis |
|---|---|---|---|
| Keystroke to echo in a terminal | under 50 ms | 100 ms | RAIL: a response within 100 ms reads as instant |
| A button or toggle shows its result | under 100 ms | 200 ms | Core Web Vitals: INP is "good" at 200 ms or less |
| A background read (specs, hub, work items, Git, files), median | under 1 s | | RAIL: keep the user's flow under 1 s |
| A background read, p75 | | 2.5 s | Matches LCP's 2.5 s |
| A background read, p90 | | 4 s | Matches LCP's "poor" threshold of 4 s |
| First useful page (terminals and work tree visible) | under 2.5 s | 4 s | Core Web Vitals: LCP is "good" at 2.5 s or less |
| Every panel filled after load | under 5 s | 10 s | RAIL: beyond 10 s the user loses the task |
| "Host is busy" shown on a normal page load | never | never | A busy host is a queue, not an error |

Notes on the budget:
- Each read is sampled 10 times. It is `slow` when the median is over 1 s, and `fail` when p75 is
  over 2.5 s or p90 over 4 s (nearest-rank percentiles).
- Opening times each panel separately (sidebar branch, work tree, Plan, hub, Git). The branch and
  work tree make the first useful page (4 s); every panel must be filled within 10 s.
- The terminal journey allows no terminal socket reconnects.
- The desktop host must meet them with a wide margin.
- The latency e2e test already holds the median keystroke below 10 ms and P99 below 30 ms on CI.

## How a read spends its time on the node

Every Git process behind the execution boundary is its own boundary entry, about one second
each on the phone. A whole read inside the boundary helper (`__project-read`, `__git-snapshot`)
is also one entry. So the budget is mostly a count of entries per read.

| Read | rc.3 entries | rc.4 entries | rc.5 entries | With helper at rc.4+ | rc.7 measured | After fewer crossings, cold / warm |
|---|---|---|---|---|---|---|
| `/api/specs` | 14 | 5 | 5 | 1 | 1 | 1 |
| `/api/state` | 18 (6 crossing) | 6 | 6 | 2 | 0 | unchanged |
| `/api/hub` | 4 | 2 | 1 | 1 | 2 | 1 / 0 |
| `/api/work-items` | 2 | 1 | 0 | 0 | 1 | 1 / 0 (0 after any read that carries the common dir) |

rc.7 measured on the candidate (Server-Timing, 10 samples): each entry costs about 0.75 s and work
time is under 10 ms, so the count is the budget. What the hub and work-item entries were, named by
`hub::tests::node_reads_cross_the_boundary_once_cold_and_never_warm` on a simulated node (the host
cannot identify the checkout's `.git` there):

- hub: `hub::files < hub::context_files < hub::context_parts < hub::context_section < hub::hub`
  (the visible file list), and `project::common_dir < project::location < project::identity <
  workbench::shared_memory < hub::context_parts < ...` (`git rev-parse --git-common-dir`).
- work items: `project::common_dir < project::location < project::identity < work_items::read`.

How they are removed:

- **Common dir.** Where the host cannot identify `.git`, a checkout's common dir is reused for 60 s
  (it changes only on a re-clone or a worktree conversion). The helper's `files`, `git` and
  `source` replies carry it as `commonDir`, which refreshes it at no cost. A helper from before
  this omits the field and the host asks Git as before; a host from before ignores it.
- **Hub file list.** The hub's context preview reuses the visible file list for 60 s while the
  places context comes from keep their modification times (project root, top-level `.gitignore`,
  `docs/`, `docs/project/`, `specs/`, `specs/sessions/`, `work/` and each work item). Adding or
  renaming a brief, spec or work item document asks again at once. Agent launches and syncs never
  reuse it.

## First load on a node

A node's boundary serves one project read at a time. `/api/workspace` says so
(`boundaryReads: 1`; absent on a host without a boundary), and the page then keeps two reads in
flight (one crossing, one ready behind it), started in this order: state, work items, specs, hub,
Git, files, then anything else. One at a time (rc.9) left the work tree waiting 5.2 s; four at a
time (rc.7) drew a "busy" answer. Panels show
their remembered answers from this tab meanwhile. Target on the node: first useful page under
2.5 s, every panel within 5 s.

## Where one read spends its time: `Server-Timing`

Every `/api/*` reply carries a `Server-Timing` header with these phases, in milliseconds:

| Phase | What it covers |
|---|---|
| `queue` | From the request reaching the reply code until a worker starts the read |
| `boundary` | Time inside execution-boundary entries; `desc="n=<count>"` is the number of entries |
| `work` | Everything else: parsing, JSON, the host's own code |
| `total` | All of it |

Example: `Server-Timing: queue;dur=12, boundary;dur=980;desc="n=1", work;dur=40, total;dur=1032`

An entry is counted where it happens: every Git or helper process a read runs (`exec`'s bounded
runs, `project_read::request`, the direct Git reads in `specs`, the recovery probe). On a host
without a boundary the same processes run directly and are still counted, so a desktop run shows
how many entries the read would make on the node. The header carries numbers only, never paths,
names or ids. The journeys record the phases of each read's median sample.

## Measured on Node 1 (public URL, signed-in browser, one read at a time)

| Read | rc.3 (15 Nov, 12:06) | rc.4 (15 Nov, 17:20) |
|---|---|---|
| `/api/specs` | 14.8 s | 6.4 s |
| `/api/state` | not measured | 3.8 s |
| `/api/hub` | 3.7 s | 2.8 s |
| `/api/work-items` | 3.1 s | 1.1 s |
| `/api/git` | 2.9 s | 1.1 s |
| `/api/files` | 1.2 s | 2.4 s |
| Busy errors after load | yes | none |

## Remaining work against the budget

1. **Update the boundary recovery helper** (`/usr/local/libexec/verb-boundary-recovery`) to an rc.4
   or later build. This lets specs and state use the one-entry `source` read. It is a boundary
   change: a reviewed PocketFabric PR, a rehearsal and owner approval.
2. **Make each boundary entry cheaper.** About one second per entry is the floor that every read
   above pays. Profile it first, client start-up against controller setup, then fix the larger
   part.
3. **Measure the first useful page and every panel on the node** with the browser's own timing, not
   only single reads.

## Terminal stream reconnects

- A stream whose sign-in has ended (session, Access login or scoped token) is closed by the host
  with code `4401` ("sign-in expired"), flushed before the socket is dropped. A terminal ticket
  request from an Access login whose `exp` has passed is refused with 401. On either, the page
  stops reconnecting and shows "Signed out. Reload to sign in again." with a Reload button.
- Any other close reconnects after 1, 2, 4, 8, 16, then 30 s (up to 20% jitter). The back-off
  starts again after a stream has stayed open for 60 s.
- The page logs the first close and then only a change, not every retry.

The opening journey times three openings, each in a fresh browser, and judges the median. The
first one may meet a cold node (its caches expire after a minute idle); it is kept as `cold` in the
report.
