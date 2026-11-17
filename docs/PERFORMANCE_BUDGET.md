# Performance budget

Verb has to feel immediate on its slowest real host, a phone-hosted node behind an execution
boundary. This page sets the budget, says where each number comes from, and records what was
measured.

## Budget

| What | Target | Must not exceed | Basis |
|---|---|---|---|
| Keystroke to echo in a terminal | under 50 ms | 100 ms | RAIL: a response within 100 ms reads as instant |
| A button or toggle shows its result | under 100 ms | 200 ms | Core Web Vitals: INP is "good" at 200 ms or less |
| A background read (specs, hub, work items, Git, files) | under 1 s at p75 | 2.5 s | RAIL: keep the user's flow under 1 s; matches LCP's 2.5 s |
| First useful page (terminals and work tree visible) | under 2.5 s | 4 s | Core Web Vitals: LCP is "good" at 2.5 s or less |
| Every panel filled after load | under 5 s | 10 s | RAIL: beyond 10 s the user loses the task |
| "Host is busy" shown on a normal page load | never | never | A busy host is a queue, not an error |

Notes on the budget:
- Read targets are p75 on the phone node, the web-vitals convention.
- The desktop host must meet them with a wide margin.
- The latency e2e test already holds the median keystroke below 10 ms and P99 below 30 ms on CI.

## How a read spends its time on the node

Every Git process behind the execution boundary is its own boundary entry, about one second
each on the phone. A whole read inside the boundary helper (`__project-read`, `__git-snapshot`)
is also one entry. So the budget is mostly a count of entries per read.

| Read | rc.3 entries | rc.4 entries | rc.5 entries | With helper at rc.4+ |
|---|---|---|---|---|
| `/api/specs` | 14 | 5 | 5 | 1 |
| `/api/state` | 18 (6 crossing) | 6 | 6 | 2 |
| `/api/hub` | 4 | 2 | 1 | 1 |
| `/api/work-items` | 2 | 1 | 0 | 0 |

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
