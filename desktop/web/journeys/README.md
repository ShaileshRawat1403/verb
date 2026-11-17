# User journeys

Playwright journeys that use a real, running Verb as a person does, then compare what they
measured with `docs/PERFORMANCE_BUDGET.md`. They are the acceptance step before a public release:
run them against the node candidate, then switch public only on a pass.

They only read the project. The terminal journey prints a marker with `echo` and exits; nothing is
created, committed or edited, because the candidate and public Verb share one project folder.

## Run against the node candidate

1. From the Mac, tunnel the candidate privately (nothing is exposed):
   `ssh -N -L 4007:127.0.0.1:3007 -p 2222 -i ~/.ssh/oneplus9_ed25519 root@192.168.68.114`
2. Put the candidate's token in `VERB_TOKEN` yourself (agents never read it), then:
   `cd desktop/web && VERB_URL=http://127.0.0.1:4007 npm run journeys`
3. Read `desktop/web/journeys-report/summary.json`. Every result is `pass`, `slow` (over target,
   within limit) or `fail`. Any `fail` fails the run. Each read also carries `details`: its median,
   p75 and p90, and the host's own `Server-Timing` phases for the median sample (`queue`,
   `boundary` with its entry count, `work`, `total`; see `docs/PERFORMANCE_BUDGET.md`).

## Run against the public URL (Cloudflare Access)

The owner creates a Cloudflare Access service token for the Verb application and keeps it; agents
never create, read or print it. With both parts in the environment, every request the suite makes
carries them as `CF-Access-Client-Id` and `CF-Access-Client-Secret`:

`VERB_URL=https://verb.pruningmypothos.com VERB_ACCESS_CLIENT_ID=… VERB_ACCESS_CLIENT_SECRET=… npm run journeys`

`VERB_TOKEN` is optional on this path: Access signs the run in. Set it as well to sign in with the
owner token instead.

## What is checked

| Journey | Checks | Budget |
|---|---|---|
| Open | time to each panel (sidebar branch, work tree, Plan, hub, Git), so a failure names the slow one; no "host is busy" | branch and work tree 4 s; every panel 10 s |
| Reads | 10 samples each of state, specs, hub, work items, Git, files: median, p75, p90 and the median's Server-Timing phases | slow if median over 1 s; fail if p75 over 2.5 s or p90 over 4 s |
| Reload | the last known branch is on screen at once | 1.5 s |
| Tabs | Plan, Review and Preview open without an error | |
| Terminal | opens; keystroke echo; a command runs and prints; terminal socket reconnects | echo 100 ms; 0 reconnects |

Traces and videos are off so the access key is never recorded; a failure keeps a screenshot.
