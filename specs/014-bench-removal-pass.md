---
id: 014
title: Consolidate remaining desktop workflows
stage: review
branch: design/bench-consolidation
created: 2026-11-12
work-item: 001-bench-consolidation
---

# 014 · Consolidate remaining desktop workflows

## Acceptance criteria

- [x] Review includes Mark as good, criterion claims separate from accepted evidence, and lifecycle actions with history and export. (R5, R10)
- [x] Preview includes the file tree with Git status and Copy path. (R6, R10)
- [x] The launch form includes Antigravity Talk for a selected spec. (R10)
- [x] Plan and Settings share the task form, and session history shows inbox detail. (R10)
- [x] Overview, Project and Specs redirect to their replacements. Studio opens only through the palette. (R10)

## Scope

Keep Terminals, Phone pairing and Session history. Keep Studio's code. Remove CSS exclusive to the removed surfaces. CSS consolidation remains in slice 7b.

## Desktop views

| Before | After |
| --- | --- |
| Overview | Review → Checks for Mark as good. Live sessions appear in Terminals and Session history. The task queue is in Settings. |
| Project | Preview → Files for the tree, Git status and Copy path. |
| Specs | Board for navigation, Plan for stage moves, Review for criterion proof and lifecycle records, and the launch form for Antigravity Talk. The previous filters and per-spec board were removed. |
| Studio | Studio opens through Open Studio in the command palette. Attaching a connector result to a Studio draft also uses the palette. |
| Sessions | Terminals. |
| Bench Board | Board. |
| Needs you | Needs you. |
| Context | Context. |
| Settings | Settings, including Phone pairing and Session history. |

## Dialogs

| Removed dialog | Destination |
| --- | --- |
| `bench-plan-task-dialog` | The shared `task-dialog` opens from Plan or Settings. A task created from Plan is linked to its spec. |
| `evidence-dialog` | Review → Criterion proof uses `sdlc-dialog`. A completion claim remains distinct from evidence acceptance. |
| `inbox-dialog` | Settings → Session history shows the session detail and existing inbox actions inline. |

The 25 retained dialogs are `action-dialog`, `ask-dialog`, `bench-binding-dialog`, `bench-context-dialog`, `bench-draft-dialog`, `bench-draft-review-dialog`, `bench-note-dialog`, `bench-review-diff`, `brief-dialog`, `commit-dialog`, `end-session-dialog`, `git-transfer-dialog`, `handoff-dialog`, `note-dialog`, `observer-panel`, `palette-dialog`, `paste-dialog`, `phone-dialog`, `project-switch-dialog`, `sdlc-dialog`, `session-dialog`, `shortcuts-dialog`, `spec-dialog`, `stage-dialog`, and `task-dialog`.

| Source measure | Before | After |
| --- | ---: | ---: |
| Top-level views | 9 | 6 |
| Dialogs | 28 | 25 |
| JavaScript lines | 11,830 | 11,228 |
| CSS lines | 9,744 | 9,144 |

The unbundled diff removes 872 JavaScript lines and adds 270. It removes 623 CSS lines and adds 23. The baseline is `93d4ec5`.

## Screenshots

[36 removal-pass screenshots](../work/001-bench-consolidation/screens/removal/) show the changed workflows at 1440 and 390 in light and dark. [The cutover screenshots](../work/001-bench-consolidation/screens/cutover/) show the preceding shell.

## Verification

Rust formatting and Clippy passed with the local and CI-pinned toolchains. The all-targets Rust run passed 478 tests with five ignored. That run includes pinned-TLS phone control and revocation. The phone pairing browser journeys ran at both widths. A physical handset was outside the test setup. JavaScript passed 101 unit tests. The VS Code adapter, package, release contract, Studio guest helper and offline work-state restore checks passed. Cargo audit completed with one allowed warning for `rustix` 1.1.4 ([RUSTSEC-2026-0486](https://rustsec.org/advisories/RUSTSEC-2026-0486)).

Across the full browser run and targeted reruns, all 153 macOS-applicable journeys passed. The Linux-only stalled-cleanup journey was skipped locally. Review menus keep their open state during check refreshes. The reruns also cover the migrated SDLC and Git controls, Studio file-read recovery, and acknowledgement of a specific reconnected conversation device.
