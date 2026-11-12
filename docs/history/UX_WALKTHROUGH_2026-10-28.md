# User walkthrough — 28 October 2026

Tested the actual visible interface in Chrome and the narrow Codex browser panel. All file edits
were in the disposable Verb Playground. No real agent, deployment, Git commit or public action.

## Findings and fixes

| User action | Observed friction | Implemented change |
| --- | --- | --- |
| Studio → Files | Left the work thread for Project's read-only file viewer | Stays in Studio, restores the file surface and focuses file search; drafts survive |
| Studio → Review Git changes | Opened the commit dialog before showing any patch | Opens the existing diff surface; a separate Commit changes action opens the commit review |
| Browser | Document and Browser both appeared selected | Exclusive Browser selection; restored Document state follows the actual editor mode |
| First browser visit | Said a browser had been stopped before one had ever run | States Ready to start / No browser is running |
| Local page with no external dependencies | Chrome background Google/autofill requests appeared as page permission requests | Page request events, including redirects, populate the permission list; proxy still denies every ungranted destination |

Changes: desktop/web/index.html; src/studio-ui.js; src/studio-browser.js; runtime/browser-worker.mjs;
rebuilt dist; e2e/studio.spec.js, studio-native.spec.js and browser-runtime.spec.js.

## Hands-on verification

Original preview: opened a file through Project and reproduced the read-only detour; opened the same
file in Studio, added a harmless HTML comment through the editor, saved explicitly, opened the app
preview, clicked its button and selected Phone. The page retained its count. Git review reproduced
the premature commit dialog; cancelled without committing.

Rebuilt preview: Files kept Studio and focused search. Typed an unsaved draft, visited Files, returned
to Edit and confirmed it survived. Review displayed changed files and the actual wording diff while
no commit dialog was present. Cleared only the walkthrough's own unsaved draft afterward. Browser
showed truthful ready state, exclusive selection and the actual Fieldnotes page without background
permission noise. Narrow panel also kept Files/review usable. No permission granted to background sites.

## Automated verification

23 affected real-browser journeys passed, including a new desktop/390px test for retaining drafts and
focusing the intended surface. Existing write conflicts, save races, read/remote denial, preview
sandboxing, browser redirects, stale identities, worker crash and stop/cleanup tests passed.
78 web units passed. Build, JavaScript syntax and diff whitespace checks passed. No Rust source changes
in this pass; the previous full Rust results are historical, not a newly repeated run.

Logs: /private/tmp/verb-ux-walkthrough-e2e.log and /private/tmp/verb-ux-walkthrough-unit.log.

REA refreshed the changed worker as inert syntax: one file parsed, zero parse failures, 2,158 AST
nodes, 15 findings, 610 semantic unknowns. Evidence /private/tmp/verb-browser-worker-rea-ux-20261028.json;
summary /private/tmp/verb-browser-worker-rea-ux-summary-20261028.json. This does not prove security.

Screenshots in the chat's persistent visualization directory, verb-browser/:
ux-diff-review.jpg, ux-browser-fixed.jpg and ux-phone-review.jpg.

## Preview and remaining usability work

The rebuilt review preview is loopback-only port 4873, with a separate disposable project and local
sample site on 4874. Older 4870/4871 hosts were not restarted. Owner-only launch metadata and process
ownership are in the ignored private handoff; no access token is included here.

Further polish remains: the phone diff file list is dense; the editor remains basic; terminal access
still changes working surfaces; complete browser assistive-technology interaction and real-provider
conversation validation are unfinished. These are not claimed complete by this walkthrough.

All code remains uncommitted. Android and PocketFabric Node untouched.

## Studio task hierarchy follow-up

The next authorised pass prioritises the selected task and its suggested next step. Suggestions derive
from the chosen stage and are labelled as suggestions; they do not claim an agent ran, evidence was
accepted or a release happened. Review opens the real diff; other lifecycle actions open the existing
workbench; choosing how to work opens Sessions without starting anything.

For selected work, an empty new-change composer collapses behind Define another change. Explicit
open/closed choices survive background renders, and hiding preserves text/context. Existing task
loading no longer briefly presents an unrelated new-task invitation. Task details/criteria and audit
history are independently expandable; the current task and chosen-stage label remain visible.
Active document/browser/editor modes now have a visible selected state. Default raw Spec template
problem text is replaced with useful guidance in the presentation only; source Specs are untouched.

Phone diff layout puts a horizontally scrollable file picker above a full-width patch, rather than
squeezing file names and patches into side-by-side columns. Transient browser failures retain the
rendered page and Stop control; the next automatic request reads status, never retries a failed action.
Authentication/stale-session failures clear rendered content and readable text.

Manual judgment: task and next step are easier to identify; an unrelated composer no longer dominates;
expand/hide retains a draft; the suggested Review changes action reaches a real patch without a commit;
390px review preserves useful patch width. Tested through CUA in an isolated copy of the playground,
not through DOM mutation. Desktop/phone screenshots: verb-browser/studio-task-polish-{desktop,phone}.jpg
in the chat's visualization directory. The temporary judge server and its tabs were closed afterward.

Verification: 24 non-conversation browser cases passed in the final combined run. The conversation
sequence exposed a stale-workspace-cache assumption in its resume fixture; it now waits for the actual
terminal-exit record, and all 14 conversation cases passed separately. This covers all 38 affected
journeys. An earlier run also found the composer expansion being overwritten during rendering; explicit
user choice now takes precedence. All 78 web units passed, plus build, JS syntax and whitespace checks.
Logs: /private/tmp/verb-studio-task-polish-verified.log (30 pass, one fixture timing failure, seven skipped
because of that serial dependency), /private/tmp/verb-studio-conversation-final.log (14 pass),
/private/tmp/verb-studio-task-polish-unit-final.log (78 pass). Do not describe the first log as all green.

Changes this pass: index.html; src/studio-ui.js, studio-browser.js, studio.css; rebuilt dist; relevant
Studio/browser/conversation e2e tests. No Rust/worker/dependency change. Real Claude validation remains
unproven. No deployment or user-service restart. Existing preview4873 still serves the prior build;
loading this pass there requires the user's explicit restart approval under the standing instruction.

## Studio look and feel follow-up

Refined Studio's visual system: charcoal/sage dark palette, off-white/forest light palette, clearer
25–33px task titles, sentence-case context labels, lighter borders, consistent fields and button
shapes, and a restrained left-rule suggested-action section rather than another boxed card. Styled
agent-session selectors and readable empty-conversation hints consistently with the workspace.
Phone fields use16px text; selected-task content precedes the secondary new-change action. Phone
changed-file navigation is compact so the full-width patch has useful room.

Visual accessibility checks found the global theme-switch transition overrode reduced-motion because
its important selector was more specific. Fixed the existing reduced-motion rule in motion.css to
cover theme-switching explicitly. The test checks zero transition duration with reduced motion,
>=4.5:1 for key task/action/stage/conversation-hint text in both themes,44px phone actions, no page
horizontal overflow, and current-task-before-new-change ordering. These measured elements are not a
claim of an exhaustive WCAG audit. Theme fixture setup now seeds work before initial page load.

Current sources changed in this pass: studio.css, motion.css, one hasTask presentation attribute in
studio-ui.js, rebuilt dist and a visual regression in studio-native.spec.js. No dependency, Rust,
provider, browser-worker or deployment changes. Final verification log:
/private/tmp/verb-studio-finish-final.log. Four real-browser screenshots, inspected at desktop and
phone widths, saved in the visualization directory as verb-browser/studio-finish-{light,dark}-{1440,390}.png.
Existing user previews were not restarted; the earlier request for permission to refresh4873 remains
pending. Source changes remain local and uncommitted.

## Work overview screenshot-specific pass

Refined the user's actual Specs/Work overview screenshot: compact comparable desktop rows, stacked
board cards, coordinated filters and view switch, primary New spec, quieter Commit, shared Studio/
Specs palette and compact attention requests. All execution/evidence/delivery/review facts remain
separate and visible. Attention source says Based on chosen stage rather than raw spec-stage jargon;
source records and acceptance guards are unchanged. Phone filters use three rows and facts use short
label/value rows. Duplicate sidebar tasks are hidden only on a visible phone overview, retained for
spec details. Screenshot review caught a conflicting older selector; final specificity and a new
assertion ensure the duplicate strip is actually hidden.

Changed: index.html, app.js (specs-open class), specs-ui.js (presentation markup), workbench.css,
studio.css (shared palette only), rebuilt dist and spec-workbench.spec.js. No dependency/backend change.

Final verification: /private/tmp/verb-layout-buttons-verified.log, 36 affected browser cases spanning
Specs, SDLC, general workbench and Studio; /private/tmp/verb-layout-unit-final.log, 78 web units.
Includes light/dark list+board at1440/390, six tasks/18 facts, row-height budget, key text >=4.5:1,
no page overflow, keyboard/focus, stale reviewed moves and phone strip visibility. Build/syntax/diff
checks passed. Scoped checks are not a full WCAG audit. Fixture-only Clear filters click was corrected
to respect its disabled state before the final run.

Manual CUA review in an isolated copy of the known playground: checked same four tasks, light list,
selected List/Board state, stage counts and Review filter. No stage changed or commit made. Temporary
tab/server closed after ownership verification. Reused existing REA architecture notes; no fresh app
internals, private sessions or proprietary implementation inspected/copied.

Images in the chat's visualization directory, verb-browser/: work-overview-refined.jpg and
work-board-refined.jpg (manual); work-{list,board}-{light,dark}-{1440,390}.png (final matrix).
Final phone image was visually inspected after the selector fix. All source remains uncommitted.
Existing4870/4871/4873/public services unchanged;4873 still needs the pending explicit restart approval.
Android and PocketFabric Node untouched.

## Research-backed direction after repeated visual feedback

See DESIGN_DIRECTION_2026-10-28.md for actual current sources, deliberate choices, limitations and
verification. Neutral workspace chrome, subdued navigation and a restrained action accent replace
all-over sage. Desktop overview rows now share visual column headings, retain per-row accessible
labels and avoid individual rounded-card frames. Board and phone preserve their explicit facts.
25browser/78unit checks pass; manual app-preview state survives theme changes. No user-service restart.

## Local commit and preview approval

After the recorded design passes, the user explicitly approved committing the reviewed work and
restarting only local preview 4873. Earlier uncommitted/pending-approval notes are historical.
No public deployment, push, Android change or PocketFabric Node action was authorized.
