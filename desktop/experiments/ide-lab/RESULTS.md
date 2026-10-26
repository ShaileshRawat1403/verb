# First full-IDE comparison — 25 October 2026

Both foundations supplied real engineering workflows in disposable ARM64 Linux
containers on the Mac. The current result supports continuing with a VS Code-derived
baseline for capability integration and keeping Theia as a custom-product contender.
It does not select the final production architecture or establish phone suitability.

## Versions, scope and provenance

- code-server 4.143.1; VS Code API reports 1.143.1. Official image index:
  `codercom/code-server@sha256:377b66bec5645510ed52549bb00e648c273edc604d230a3f90fd2b2433afcb97`.
- Theia IDE 1.76.0; extension API reports 1.139.0. Official ARM64 manifest:
  `ghcr.io/eclipse-theia/theia-ide/theia-ide@sha256:85e2ffa998a6815b731cddda5b46e1fd8fc5d9857c02b318bfc2edda67d9b5e9`.
  Docker GHCR HEAD requests were denied, while public registry GET worked. A local
  importer verified the exact manifest, every blob and uncompressed layer before
  importing the official image. No upstream executable was substituted.
- Each derivative used the same dependency-free project, Node v22.23.3, local VSIX
  probe, 2 CPUs and 3 GiB memory. Python/Git differed with the base distributions:
  code-server Python 3.13.5/Git 2.47.3; Theia Python 3.11.2/Git 2.39.5.
- Trial IDEs ran sequentially, non-root, with no host mounts, credentials, Docker
  socket or public project. Their internal-only network reached a fixed-target
  loopback proxy; no phone or PocketFabric Node operation was performed.

## Functional observations

| Workflow | code-server | Theia |
| --- | --- | --- |
| Open, edit and save a real file | Passed; disk change and local commit verified | Passed; disk change and local commit verified |
| Local compatible extension command | Passed; actual workspace proof file | Passed after installing the VSIX through its remote file picker |
| Integrated terminal build | Built actual HTML output | Built actual HTML output |
| JS and Python tests | Both passed from integrated terminal | Both passed from integrated terminal |
| Language navigation | F12 crossed from import to exporting file | F12 crossed from import to exporting file |
| Rename refactor | Actual function rename preserved exported API; JS test passed afterwards | Actual function rename preserved exported API; JS test passed afterwards |
| Real debugger | Entry pause, step, local variables and stack verified in UI | Entry pause, step, locals, stack and explicit breakpoint verified; DAP recorded breakpoint reason |
| Conflict review/resolution | Conflict UI exposed actual file; editor resolution used | Dedicated incoming/base/current/result merge editor resolved and staged file |
| Complete Git merge | Generic UI prompted to stage all when resolution matched HEAD; cancelled. An explicit editor resolution and integrated-terminal commit completed a two-parent merge | UI Continue Merge failed to complete; integrated-terminal `git -c core.editor=true merge --continue` succeeded; two parents verified |
| Running app/preview | Terminal-launched app served HTTP and live button/API response | Same app served HTTP and live button/API response |
| Browser reload continuity | Same shell PID 470 and running preview process survived | Same shell PID 211 and running preview process survived |

The Theia log reported `Command with id '_chat.editSessions.accept' is not registered`
during this workflow. The UI was still showing Continue, `MERGE_HEAD` remained and
only one HEAD parent existed before the terminal fallback. Preserve this as an
observed upstream API/integration gap; the exact exception-to-button causal path
has not been independently traced or patched. A successful terminal fallback does
not turn that UI action into a passing check.

The code-server explicit-breakpoint scenario was not completed; its debugger entry
pause and stepping were verified. Language-server completions, cross-file rename,
additional languages/debuggers and arbitrary third-party extensions were not exhaustively
tested. Rename here was a function-symbol refactor within one file that deliberately
retained the exported property consumed by other files, not a cross-project API rename.

## Resource samples — not a performance ranking

| Observation | code-server derivative | Theia derivative |
| --- | --- | --- |
| Local image size reported by Docker, uncompressed logical size | 1,204,735,370 bytes | 2,078,030,771 bytes |
| One observed start → HTTP 200 interval, including proxy startup | 3.878 seconds | 3.625 seconds |
| First captured working-set sample | 565.6 MiB | 916.4 MiB |
| Later captured working-set sample | 998.5 MiB | 915.2 MiB |

Samples were taken at different workflow points, including language/debug tooling
and a browser reload. Images, plugin inventories and distributions differ. Neither
run is a clean-filesystem cold-start experiment, statistically comparable idle/peak
benchmark, or sustained load test. Docker memory excludes the Mac browser and VM
baseline. Browser heap, full interactive startup, typing latency, mobile IME and
physical-phone behaviour remain unmeasured. Do not infer that Theia is faster or
that either will fit the production phone budget from this small set of samples.

## Fixture and tooling findings

- The first fixture incorrectly put user-only update/extension settings in workspace
  settings. code-server surfaced the error. The reusable fixture now keeps valid
  workspace settings and the build helper places user settings in User/settings.json.
- Python bytecode files initially appeared as unrelated untracked paths in both
  trial repos. The reusable fixture ignores these and disables bytecode generation.
- Native accessibility automation had difficulty targeting Monaco's editor input;
  normal visible editor clicks/typing worked. Some shortcut attempts did not invoke
  the intended action; temporary synthetic buffer edits were restored. These are
  automation observations, not proof that user keyboard input is broken.
- The proxy was needed because Docker did not publish ports on the internal network.
  Its overhead is outside the IDE's reported container memory. The actual trial
  recipe and runtime receipts were retained privately for inspection.
- Eleven local fixture/lifecycle guard checks passed, including refusal of unrelated
  containers/proxies, host mounts, non-loopback bindings, non-internal networks,
  unsafe receipts and simultaneous IDEs, plus cleanup after failed proxy startup.
  These checks are not a production execution-boundary/security audit.

## Recommended next implementation

Define a **Verb workspace adapter** before embedding an IDE: map authorized project,
file revisions, tasks, existing terminal/session identity and exact-source evidence
between Verb and one IDE. Prove the adapter using a local VS Code-compatible extension
and mock/dedicated Verb state, without granting a generic IDE backend direct access
to the public workspace. Keep the complete IDE's editor/debugger/extension facilities
available while adding Spec, preview, check and delivery guidance.

Use code-server as the first capability baseline because the observed Git integration
gap adds remediation work to this Theia package, not because these samples prove
code-server lighter. Assess supported UI customization and choose between a Verb
extension and a custom Theia workbench after the adapter/authority contract is clear.
Both generic UIs still require significant beginner-focused product work.

All four owned trial containers (two IDEs and two proxies) were stopped after testing.
Synthetic state and local image caches remain. Public Verb was not changed, restarted
or redeployed. The earlier local Verb preview remains stopped. No install was made
into PocketFabric Node or the Mac's global package manager.

## Primary upstream references

- [code-server repository](https://github.com/coder/code-server), [installation and official ARM64 image](https://github.com/coder/code-server/blob/main/docs/install.md), [extension compatibility](https://github.com/coder/code-server/blob/main/docs/FAQ.md).
- [Theia IDE image package](https://github.com/eclipse-theia/theia-ide/pkgs/container/theia-ide%2Ftheia-ide), [browser image recipe](https://github.com/eclipse-theia/theia-ide/blob/master/browser.Dockerfile), [Theia extension architecture](https://theia-ide.org/docs/extensions/).
