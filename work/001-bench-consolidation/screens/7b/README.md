# Slice 7b: visual pass

`target/` is the design target. Open `target/verb-bench-mock.html` in a browser; it is
self-contained (fonts embedded) and switches between five screens and light or dark from the bar at
the bottom right. The PNGs are the same screens at 1440 px and 390 px.

Each `slice-N/` folder holds before and after screenshots of the real app for that slice, taken from
a throwaway fixture project built the same way as `e2e/bench-shell.spec.js`.

## The idea: a traveler

A job on a shop floor carries a traveler, a slip stamped at each station it passes. Verb's chain maps
onto it: Why (BRD), What (PRD), How (spec), Proof, Ship. The traveler is the one bold element; the
terminal stays the hero and everything else stays quiet.

## Rules the target keeps

- Monochrome, with the four colour roles of `docs/UX_FOUNDATION.md`: green confirmed, yellow needs a
  decision, red observed failure, dim secondary. The current stage is ink, not a colour. Every
  coloured thing also carries a word.
- Stamps are bordered words in Barlow Condensed. Open stages are dashed, the current stage is solid,
  completed stages tilt slightly. The newest completed stamp presses in once when it appears; reduced
  motion turns that off.
- The UI shows only states the host resolves (`CONTRIBUTING.md`: a screen must not calculate its own
  truth). A line in the target that the host does not provide is dropped, not computed.
- Agent output stays marked "Reported, unverified".
- Terminal and editor are unchanged.

## Slices

| Slice | Scope | Status |
|---|---|---|
| 1 | Barlow interface type, served locally | Done |
| 2 | Traveler in Plan, next step, inline warning reasons | Done |
| 3 | Sidebar: mini traveler, states as words, clipping fix | Done |
| 4 | Terminal tab waiting flag, "This agent knows" strip | Done |
| 5 | Review verdict, checks, changed files | Done |
| 6 | Board as stations, Needs you as tags, no empty-state flash | |
| 7 | First run with an empty traveler | |
| 8 | Phone pass at 390 px | |

"Write it myself" and bulk clearing of planning reports change behaviour and follow in 7c.
