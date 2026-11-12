# Everyday utilities — 28 October 2026

Goal: let a person inspect, change, check and recover their project without needing an AI
provider for basic actions. Guided language and direct controls operate on the same files.

## Research used

Reviewed `BROWSER_ARCHITECTURE_RESEARCH.md` (the existing bounded REA investigation),
`VERB_NON_DEVELOPER_IDE_PLAN.md`, `STUDIO_CONSOLE_DESIGN.md` and the main checkout's
`CAPABILITY_ARCHITECTURE_MAP.md` revision 2. This pass reuses those findings; it does not
claim fresh competitor analysis. REA's skipped bundle, bounded coverage and semantic unknowns
remain limitations. No proprietary implementation or private conversation was accessed or copied.

The browser research supports integrated artifact inspection, visible runtime location,
recoverable controls and optional diagnostic evidence. The capability map identifies selective
staging and broader editing as gaps. The following priorities are Verb product decisions based
on those gaps, not assertions that REA proved competitor features.

## Priorities

| Utility | User need | Existing foundation | Work still required |
| --- | --- | --- | --- |
| Find & replace in a file | Locate words or identifiers and make a deliberate edit | Studio draft editor and explicit revision-checked saves | Implemented in this pass; browser acceptance below |
| Choose files to commit | Keep experimental or unrelated changes out of a saved Git checkpoint | Existing Git summary, diff and guarded commit | Define selected-file staging, pre-existing index behavior, races and boundary parity; current commit still adds all changes |
| Project checks | Run a known test/build without remembering shell syntax | PTY sessions and Spec evidence | Inspectable task definitions, explicit execution, exit status and exact-source provenance; no automatic package-script execution |
| Preview diagnostics | Understand why a preview is blank or broken | Isolated browser worker and preview identities | Bounded console/network reports, redaction, explicit evidence capture and revocation; no background browsing history |
| Project text search | Find where a phrase appears across a project | Existing allowed-file listing and read boundaries | Bounded server search inside those boundaries; ignored secrets stay excluded; review before multi-file replacements |
| File navigation | Jump to a line, reopen recent files, inspect unsaved drafts | Eight preview tabs and per-tab drafts | Accessible navigation and clear draft lifetime; no promise of crash recovery until persistence is designed |

## Implemented: file find and replace

Studio → Edit file → Find & replace. The panel is closed until requested. Cmd/Ctrl+F inside
the editor opens it and uses a short selected phrase when available. Enter/Shift+Enter navigate;
Escape returns focus to the editor. Previous/Next wrap and announce match position. Matching is
literal and optionally case-sensitive; regular expressions are not exposed.

Replace changes one match; Replace all changes the current file only. `$&`, `$1` and backslashes
are literal replacement text. The last replacement can be undone while its resulting draft remains
unchanged. Newer typing, a different file or an explicit successful disk reload invalidates that undo so it
cannot overwrite newer work or cross a file boundary. Existing draft preservation, conflict detection
and revision-checked explicit saves remain authoritative. No save, commit, command or preview
starts from searching or replacing. This utility adds no dependency or filesystem endpoint. Replacement output is capped at the same
200 KB UTF-8 limit as the editor, with a size check before concatenation to prevent replacement
amplification. Search offsets are cached by text, query and case mode; selection and save updates
do not rebuild unchanged match lists.

This remains a textarea utility, not language intelligence, project-wide replacement or a chosen IDE
foundation. A future editor adapter should preserve the same draft/save/identity contract.

## Validation and release status

Pure tests cover literal escaping, case, multiline/Unicode offsets, deletion and replacement text.
Browser tests cover keyboard navigation, match counts, single/all replacement, draft-only changes,
undo invalidation, explicit save and phone overflow. Existing Studio native tests also run to catch
save/conflict and preview regressions. Results: 82 web units and all 15 Studio native browser tests passed. After field styling, the two
utility cases passed again, including 44-pixel phone controls and screenshots inspected visually.
Web/native builds and git diff checks passed. Logs: `/private/tmp/verb-find-unit.log`,
`/private/tmp/verb-find-e2e.log`, `/private/tmp/verb-find-visual.log`.

This pass did not restart persistent development servers. Tests used disposable fixtures that closed
afterward. A final check found port 4870 running again as PID 54586 in the original scratch project;
it was not started by this pass and was left untouched. Ports 4871–4874 had no listeners.
No public deployment, Android edit or PocketFabric Node change.


## Independent review and hands-on acceptance

The owner requested a cheaper subagent review before committing. A GPT-6 Luna subagent independently
reviewed the diff, found that an undo could survive an equal-text disk reload, and re-reviewed the fix.
A subsequent large-file performance concern led to cached search offsets and removal of a temporary
capture array. Final review found no remaining correctness or commit blockers. This is a code review,
not a measured performance guarantee on a phone.

Hands-on Chrome acceptance used a new disposable project and temporary loopback port. The reviewer
opened Studio, selected notes.txt, searched for a word, replaced all three case-insensitive matches,
undid them, replaced again, saved explicitly and loaded the disk version. Disk checks confirmed that
nothing changed before Save, the save reached disk and the sibling file stayed unchanged. Reload
cleared replacement undo. Studio's actual Git diff showed the exact three changed lines. The tab and
temporary server were closed afterward. Existing public and local services were left untouched.


Final pre-commit validation: **83 web unit tests passed; 75 full browser acceptance tests passed,
one existing fixture skipped**. After the cached-search change, all three utility browser regressions
passed again. Web/native builds, JavaScript syntax checks and `git diff --check` passed. No Rust source
changed, so the Rust test suite was not repeated for this utility. Logs: `/private/tmp/verb-utility-review-unit-final.log`,
`/private/tmp/verb-utility-review-e2e.log`, `/private/tmp/verb-utility-final-e2e.log`.

## Local continuation — 29 October: editor navigation

Studio now has a compact Go to line control beside the file contents label, opened by its
button or Ctrl+G while the editor is focused. It works against the current draft, including
unsaved lines. Enter or Go places the caret at the selected line and scrolls inside the
unwrapped editor. Escape or Cancel preserves the selection and returns focus. Invalid,
fractional, out-of-range and unsafe numeric values keep the panel open with an inline error.
The readout below the editor shows line, column and total lines; columns count Unicode code
points, while textarea selection uses UTF-16 offsets. An empty file has one line, and a final
newline adds an empty last line. The line index is cached until draft text changes.

No file is saved, command executed or API called by navigation. Changing file or leaving
Edit closes the jump strip. Existing per-file drafts, explicit save/conflict handling and
find/replace remain in place. Phone fields and buttons have 44 px minimum height. No new
dependency or editor framework is introduced. Browser acceptance and final results are
recorded in the private local handoff. Work remains uncommitted; deployment permission was
revoked by the owner. Do not apply the previously staged release or access PocketFabric.

## Guided project checks in Studio — 29 October

The selected work thread now exposes **Run project check** and **Review evidence** beside
its verification summary. This connects the existing SDLC runner rather than creating a
second execution path. Without a selected Spec and criterion it cannot run. Opening a
check reads current source/evidence and shows the criterion, branch, commit and command;
only the dialog's explicit Run check submits execution. Cancelling does not run anything.

Supported checks remain Rust (`cargo test --offline`), Node (`npm --offline test`, including
package lifecycle scripts), and Python unittest discovery. No arbitrary task definition,
build runner, package installation or new boundary policy is introduced. These commands
run in the project root with the runner's existing permissions and three-minute bound.
Offline package options do not isolate test code from the network. The command explanation
makes this distinction explicit. Python discovery with zero tests and missing runtimes
remain inconclusive. Existing source/intent/version guards and private evidence storage
remain authoritative; raw output is not persisted as evidence.

Running status is visible inside the dialog and the matching Studio work thread. The
original Spec identity is frozen when the dialog opens. An uncertain response asks the
person to inspect evidence before trying again; execution requests are not automatically
resent. A recorded pass still needs separate evidence acceptance. Review evidence opens
the existing Verify surface, including provenance and freshness, rather than duplicating
its controls in Studio. Source changes continue to invalidate previous proof.

The owner subsequently authorized committing and deploying the validated work, clarified
as updating/restarting **only the Verb service** on the phone host. PocketFabric code,
services and configuration and Android remain outside scope. Actual release status and
host-specific proofs belong in the ignored operator handoff, not in this public document.
