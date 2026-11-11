# Bench consolidation BRD

Size: standard

This standard work item proposes a consolidated Verb Desktop web workflow. Its factual baseline is [the UI inventory](../../docs/UI_INVENTORY_2026-11.md).

## Problem

The Verb project owner and developers working with CLI agents can track work through specs, standalone tasks, Studio, and sessions. A Studio work thread selects an existing spec, while task events link to sessions through a separate ledger. The generated agent context and the Shared notes store expose different project information. A developer must piece together the intended outcome, current work, and evidence across these surfaces.

## Audience

Verb project owners and developers using CLI agents.

## Goals

| ID | Goal |
| --- | --- |
| G1 | Make a traceable work item the unit of development. |
| G2 | Make the SDLC board the home for development work and preserve live execution. |
| G3 | Give CLI agents the same persistent project and active work context. |
| G4 | Preserve access to existing work through the transition. |

## Success measures

- G1 — Create a small and a standard work item. For each, open a spec criterion and follow its requirement back to a BRD goal. Reopen the project and repeat the lookup.
- G2 — Start at Board, open a work item and its spec Bench, launch a terminal, navigate away and return to the same live terminal. Move through the six spec stages and observe a warning and its audit entry without a blocked move. Inspect recorded evidence separately.
- G3 — Run two sessions on different specs in the same checkout. Verify they see identical shared blocks and distinct spec context through launch and CLI lookup. Edit an included decision and confirm the change appears in both blocks while text outside the markers survives.
- G4 — Open a project with legacy specs, tasks and notes. Read each after upgrade, switch back to the previous experience and confirm the original records and live terminal remain available.

## Scope

Desktop web navigation, work item documents and traceability, the spec Bench, shared agent context, and compatibility with legacy records.

## Out of scope

Phone redesign, editor and browser features, new connectors or extensions, deployment and release work, and the separate backup landing decisions.

## Audit trail

<!-- Written by Verb: one line per change, never edited. -->

- 2026-11-11T11:08:22Z · Shailesh Rawat via Verb web · published brd.md (warning: BRD is missing content in Problem | BRD is missing content in Audience | BRD is missing content in Goals | BRD is missing content in Success measures)
- 2026-11-11T14:46:10Z · Shailesh Rawat via Verb web · published brd.md (warning: Requirement R1 is missing verification | Requirement R2 is missing verification | Requirement R3 references missing goal "G1, G4" | Requirement R3 is missing verification | Requirement R4 is missing verification | Requirement R5 is missing verification | Requirement R6 references missing goal "G2, G4" | Requirement R6 is missing verification | Requirement R7 is missing verification | Requirement R8 is missing verification | Requirement R9 is missing verification | Requirement R10 is missing verification | Requirement R11 references missing goal "G3, G4" | Requirement R11 is missing verification | Requirement R12 is missing verification | Requirement R13 references missing goal "G2, G3" | Requirement R13 is missing verification | Requirement R14 references missing goal "G2, G3" | Requirement R14 is missing verification)
- 2026-11-11T14:50:07Z · Shailesh Rawat via Verb web · published brd.md
