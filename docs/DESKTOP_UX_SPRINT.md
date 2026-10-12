# Desktop usability sprint

Scope: a focused pass on terminal/project/session flows before general release. Preserve native
PTY input, echo, signals, shell configuration and installed CLI behavior. Use the current product;
do not introduce a new framework, dashboard or mandatory spec workflow for this sprint.

## Acceptance tasks

| Task | Acceptance |
| --- | --- |
| Start work | Open a project and launch Terminal or an installed agent without creating a spec |
| Switch work | Run two sessions in one project; identify the active session and checkout; switch with keyboard and mouse without losing input focus |
| Recover | Reload while a shell runs; reattach to the same process; distinguish retained output from an exact native conversation resume |
| Disconnect | Lose the client connection; show a clear reconnecting state; restore screen and focus without echoing or replaying input twice |
| Understand failure | A launch, Git or permission failure gives an observed reason and a useful next action rather than a blank tile or endless loading label |
| Finish work | Exit or interrupt normally; see the exit result and retained output; distinguish ending a process from deleting durable history |

Terminal-first entry and ordinary native input are automated and verified on the hosted candidate.
Existing browser journeys also cover optional specs, project files, handoff and hidden input. Real
PTY tests cover resize, Unicode, Ctrl-C, alternate-screen apps and reconnection. These are evidence
for those behaviors, not a completed keyboard/accessibility/usability study of every path above.

## Findings to prioritize

- Host service rows show unknown without explaining that the unprivileged account cannot read
  supervisor state. Root operator health was healthy during the same check. Explain unreadable
  status and its scope; do not widen privileges or present guessed healthy states.
- Git status unavailable should distinguish detached/unborn/non-Git/unreadable conditions and
  name the next useful action. The indefinite loading placeholder was removed, but the diagnosis
  is still too broad.
- Multiple plain terminals rely heavily on short IDs. Test whether native titles and existing
  checkout context are sufficient to distinguish tasks before adding a naming feature.
- Verify keyboard focus across session switching, dialogs, reconnects and narrow layouts with a
  real shell/editor; do not infer accessibility from passing click-driven tests.

Keep the pass bounded: observe the tasks, fix demonstrated friction, add behavior regressions,
then rerun the existing terminal contracts. Release requires no data loss, no hidden-input exposure,
no privilege fallback and no blocker in the advertised supported workflow. Record remaining
friction separately from future product features.
