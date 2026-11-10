# Bench consolidation BRD

This standard work item proposes a consolidated Verb Desktop web workflow. Its factual baseline is [the UI inventory](../../docs/UI_INVENTORY_2026-11.md).

## Problem and audience

The Verb project owner and developers working with CLI agents can track work through specs, standalone tasks, Studio, and sessions. A Studio work thread selects an existing spec, while task events link to sessions through a separate ledger. The generated agent context and the Shared notes store expose different project information. A developer must piece together the intended outcome, current work, and evidence across these surfaces.

## Goals and success measures

| ID | Goal | Acceptance walkthrough |
| --- | --- | --- |
| G1 | Make a traceable work item the unit of development. | Create a small and a standard work item. For each, open a spec criterion and follow its requirement back to a BRD goal. Reopen the project and repeat the lookup. |
| G2 | Make the SDLC board the home for development work and preserve live execution. | Start at Board, open a work item and its spec Bench, launch a terminal, navigate away and return to the same live terminal. Move through the six spec stages and observe a warning and its audit entry without a blocked move. Inspect recorded evidence separately. |
| G3 | Give CLI agents the same persistent project and active work context. | Run two sessions on different specs in the same checkout. Verify they see identical shared blocks and distinct spec context through launch and CLI lookup. Edit an included decision and confirm the change appears in both blocks while text outside the markers survives. |
| G4 | Preserve access to existing work through the transition. | Open a project with legacy specs, tasks and notes. Read each after upgrade, switch back to the previous experience and confirm the original records and live terminal remain available. |

## Scope

Desktop web navigation, work item documents and traceability, the spec Bench, shared agent context, and compatibility with legacy records.

## Out of scope

Phone redesign, editor and browser features, new connectors or extensions, deployment and release work, and the separate backup landing decisions.
