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
   within limit) or `fail`. Any `fail` fails the run.

## What is checked

| Journey | Checks | Budget |
|---|---|---|
| Open | first useful page; every panel filled; no "host is busy" | 4 s; 10 s |
| Reads | median of 3 for state, specs, hub, work items, Git, files | target 1 s, limit 2.5 s |
| Reload | the last known branch is on screen at once | 1.5 s |
| Tabs | Plan, Review and Preview open without an error | |
| Terminal | opens; keystroke echo; a command runs and prints | echo 100 ms |

Traces and videos are off so the access key is never recorded; a failure keeps a screenshot.
