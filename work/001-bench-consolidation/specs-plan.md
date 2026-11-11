# Bench consolidation spec plan

The [approved design](design.md) governs the layout. Complete one slice at a time and stop for owner approval. Preserve terminal, transcript and draft continuity. Keep the Settings old-layout switch until the final layout slice.

## Data prerequisites

The data slices provide reading, publishing and shared context. Launcher and instruction-file behavior remains unverified where there is no verified prompt path. The project-switcher prerequisite opens owner-supplied workspace URLs in separate tabs while keeping the source workspace mounted.

| Order | Requirements | Likely code | Verification and storage |
| --- | --- | --- | --- |
| 1. Read and validate work item documents | R1, R2, R3, R10 | specs.rs, work_items.rs, web.rs | Read both sizes and drafts, validate references, and preserve legacy reads. No existing files rewritten. |
| 2. Publish work items and trace criteria | R1, R2, R3, R10, R11 | specs.rs, work_items.rs, fsutil.rs, web.rs | Verify allocation, concurrent publication, required warnings, traceability and stale legacy links. Adds work item Markdown and optional spec metadata. Preserve criterion index and text for existing evidence. |
| 3. Publish shared and per-spec context and CLI commands | R7, R8, R9, R11 | hub.rs, lib.rs, web.rs, workbench.rs | Verify both approved CLI commands, shared-read prompts, bounds, omissions, redaction, marker errors, actual launch checkout and explicit recovery. Changes generated blocks and uses existing Shared notes. |

## Project switching prerequisite

Use the existing owner scope and open only an explicitly supplied Verb workspace URL. Verify that the destination uses its own credentials and that the original terminal, output and draft remain available. No destination URLs are stored.

## Layout slices

| Slice | Requirements | Scope and likely code | Required journeys and storage |
| --- | --- | --- | --- |
| 0. Record the design | R4, R6, R13, R14 | Design, PRD, slice plan and DAX backlog. | Document coverage and source gaps. Documentation only. |
| 1. Shell | R4, R6, R10 | index.html, app.js and existing styles and panels. Add the three regions, sidebar tree, remembered panel width and Settings switch. | Default new layout, expand item, select spec, collapse and restore both sides, switch layout with a terminal running. Store only layout preferences. |
| 2. Terminals and context strip | R6, R9, R12, R13, R14 | Existing terminal UI, web.rs, session.rs, hub.rs and specs.rs. Use the reviewed server handoff contract. | Four terminals on different specs, observed status changes, split, resend, end and save a reported note, next agent receives it. Requires explicit session binding and delivery facts plus spec session notes. |
| 3. Plan tab | R1, R2, R3, R5, R6, R10 | Existing specs and task UI, specs.rs, workbench.rs and server launch wiring. | Trace criterion to goal, draft and approve a BRD, small item and unlinked spec. Add optional spec task ID. Propose exact planning role wording and stop for owner approval before landing it. |
| 4. Review tab | R5, R12 | Existing SDLC, Git, last-known-good and handoff controls. | Failed check first, record proof, scoped handoff and stale review refusal. Verify pending submit and refusal handling. |
| 5. Preview tab | R4, R6 | Existing Studio, project file browser, editor and preview modules. | Document, image and app previews. Draft survives tab and terminal switches. Hide Preview when empty. |
| 6. Board and Needs you | R4, R5, R10 | Existing spec board and observed signals, work item reader and approved archive action. | Every observed signal appears and clears, route to the right terminal and tab when identified, warn and audit stage move, archive and unarchive. Archive persists without moving files. |
| 7. Context, Settings and cutover | R4, R6, R7, R8, R10 | Existing Project, notes, Host, Connections, phone and Tasks. Consolidate CSS tokens using the design direction. Update the docs index and move superseded docs to history. | Verify Context refresh, stale and size, all Settings destinations, continuity and cutover. Retire old code after replacement journeys pass. Report views, dialogs, CSS and JS counts before and after. |

For implementation slices, run cargo fmt, Clippy with warnings denied, Rust all-target tests and web unit tests. Run browser journeys at 1440 and 390 pixels. Save changed-screen screenshots at both widths in light and dark under `work/001-bench-consolidation/screens/<slice>/`. Rebuild the browser bundle once per implementation slice in its own commit. Slice 0 changes documentation and has no screens or browser bundle changes.

## Backlog

- When an isolated checkout cannot find an initialized allocator, tell the owner the exact command to run in the main checkout.
