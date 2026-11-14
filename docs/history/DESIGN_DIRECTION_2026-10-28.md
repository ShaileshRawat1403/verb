# Verb visual direction — 28 October 2026

## Current references actually consulted

- Linear's March 2026 refresh: https://linear.app/now/behind-the-latest-design-refresh and
  https://linear.app/changelog/2026-03-12-ui-refresh . Takeaways: supporting navigation should recede,
  location/view controls should be predictable, and redundant framing should be reduced.
- Current Raycast v2 manual: https://manual.raycast.com/new-in-v2 . The useful reference for Verb is
  compact presentation and control consistency. Its native glass appearance is not adopted wholesale
  into a browser workspace.

These are selected product references, not proof of a universal design trend. Consulted 2026-10-28.
Zed's agent-panel documentation could not be fetched and is not used as evidence. Linear's image
asset also failed to fetch; the design rationale and changelog were read, not that image. No private
app session content, credentials or proprietary implementation was copied.

## Direction implemented

Neutral graphite/white workspace surfaces, subdued navigation, one restrained blue action/focus
accent, and flat comparable work rows. Avoid an all-over tint, repeated rounded panels, decorative
hero sections, gradients, oversized pills and fabricated progress. Product identity comes from
information hierarchy and the continuing task, not marketing decoration.

The shell is now one continuous workspace with a stable location bar. The overview drops its nested
panel frame. Desktop list rows share visual Execution/Evidence/Delivery column labels; each row keeps
its own accessible labels for screen readers. Stage badges are neutral metadata. Board layouts retain
stage grouping and full independent facts. Phones retain explicit labels and usable touch targets.
No evidence, execution or delivery records were merged or inferred by this visual change.

Studio uses the same neutral system. Task context, chosen stage, suggested action, conversation and
artifact workspace retain their existing behavior. Preview contents are project artifacts, so their
own styling remains separate from Verb chrome. No new dependency, asset/CDN, permission or service.

## Verification

25 affected browser cases passed in /private/tmp/verb-research-design.log;78web units in
/private/tmp/verb-research-design-unit.log. Build/syntax/diff checks passed. Existing matrices cover
light/dark1440/390, key contrast >=4.5,44px phone controls, horizontal overflow, stale moves, keyboard
focus, draft preservation, saved HTML sandbox, editor conflict/save race, and mode selection.
This is not an exhaustive accessibility audit or proof of real-provider chat behavior.

Manual CUA check used an isolated copy of the known playground: inspected the actual four-task list,
opened Studio's saved Fieldnotes preview, clicked its button and changed themes. The count remained1,
confirming preview state survived. No agent launched, file changed, stage moved or commit made.
Temporary test tab/server were closed afterward. Existing4870/4871/4873 and public service untouched.

Visualizations directory verb-browser/: researched-overview-light.jpg, researched-studio-light.jpg,
researched-studio-dark.jpg (manual), plus researched-work-{list,board}-*.png and
researched-studio-finish-*.png (automated). The user approved restart of local preview 4873 and a local commit after reviewing
this design. No deployment, Android or PocketFabric Node change. Git history records the reviewed local snapshot.
