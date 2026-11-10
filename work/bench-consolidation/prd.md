# Bench consolidation PRD

Proposed design traced to the [BRD](brd.md). Existing data claims come from the [UI inventory](../../docs/UI_INVENTORY_2026-11.md). Owner-dependent details are listed in [open questions](open-questions.md).

## Requirements

| ID | Goal | Requirement |
| --- | --- | --- |
| R1 | G1 | A work item contains a BRD, a PRD and one or more specs when ready for execution. A draft can start with a BRD. Standard items use full documents. Small items use a three-line BRD and one-page PRD. |
| R2 | G1 | Store new work item documents in project Git as UTF-8 Markdown at proposed paths work/<work-id>/brd.md and work/<work-id>/prd.md. Keep existing spec Markdown under specs/. Stable work item ID and size are proposed structured metadata. |
| R3 | G1, G4 | Each criterion on a new or explicitly linked spec ends with its requirement ID in parentheses, for example `(R3)`. Each PRD requirement names its BRD goal. Use plain-text IDs (G1, G2 and R1, R2). IDs are never renumbered. A retired ID stays in its file marked retired. Validate references on document read and show missing links. Preserve legacy criterion text and report it as unlinked until an owner supplies a mapping. |
| R4 | G2 | Board is the desktop home, with work item pages and single-spec Benches beneath it. Context and Settings are the other top-level destinations. Use the route and surface mapping below. |
| R5 | G2 | Keep the six spec stages, criteria, evidence and append-only audit. Gates surface and record warnings and allow stage movement. Evidence remains separately inspectable. Git, terminal and write operations retain their own operation-specific safeguards. |
| R6 | G2, G4 | Maintain the existing terminal and conversation instances across navigation. The Bench selects a spec and reuses session, project file, Studio and Git controls. Expose legacy tasks through a Bench panel and a project-level Settings entry so unlinked tasks remain reachable. |
| R7 | G3 | Generate identical Verb blocks in root AGENTS.md and CLAUDE.md. Include the project brief, decisions, and one line per in-progress spec identifying its work item. The shared block has no session-specific active spec. Publish through the existing hub boundary and preserve all bytes outside an existing well-formed marker pair. |
| R8 | G3 | Limit the entire shared section to 8 KB of UTF-8. On overflow, omit content in a stated priority order and end the section with a line identifying omissions and the command that prints full context. Reserve room for that line and the end marker. Regenerate on app-managed changes to included sources or spec progress, and on manual sync. Detect external changes and show stale state until the approved refresh mechanism runs. |
| R9 | G3 | Every supported hosted CLI launch must receive a verified way to read the shared block in its actual working environment. A session launched for a spec receives that spec and its linked PRD requirements and BRD goals in the launch prompt. Provide a CLI command that prints the context for a given spec, and a documented invocation that prints full context referenced by the omission line. Test main and isolated checkouts and separate-account launch modes that are supported. Keep verb shared read and existing Shared notes readable. |
| R10 | G4 | Read legacy specs, task JSON and note Markdown in place. Introduce optional associations only through explicit owner action. Preserve original audit and evidence IDs. Rollback restores previous navigation and the previous generated projection from current compatible sources, while legacy readers remain usable. |
| R11 | G3, G4 | Publish new or updated work item documents and managed context with file locking and complete-file replacement. Detect concurrent edits before replacement. Treat multi-file publish and retry behavior as an explicit design and test case. |
| R12 | G2 | Add a reviewed handoff UI in the Bench after owner approval of the server wording. Show the spec revision, assignment role, approved scope and checks, and reported outcome, files and check results as unverified. Require a fresh review if the revision changes and prevent repeated submissions while a launch is pending. |

## Document shape

Proposed project layout

```text
work/<work-id>/brd.md
work/<work-id>/prd.md
specs/<existing-id>-<slug>.md
```

A standard BRD has Problem, Audience, Goals, Success measures and Scope sections. A standard PRD has requirements with IDs, goal references, behavior and verification, plus workflow and migration details when relevant. A small BRD has one line each for problem and audience, goal, and measurable success and boundary. The goal line contains its stable G ID. A small PRD retains numbered requirements, goal references and verification on one page. The work item metadata syntax and page rule are open.

Proposed link shape is spec work-item ID plus a plain-text criterion suffix such as `(R3)`. Example of the logical model is `spec 001 / criterion 2 → R3 → G1`. IDs are scoped to the work item. The representation must remain readable by legacy spec readers, retain existing criteria and audit syntax, and preserve evidence bindings. Relationship metadata and association lookups are new data.

## Text wireframes

Legend: **Existing** means data or control in the inventory. **New** means new data or a new derived view.

```text
Board
  Navigation and Board home [New navigation]
  Work item cards: title, size, BRD/PRD links [New]
  Item progress grouped from linked spec stages [Existing stages, new grouping]
  Unlinked specs: title, stage, criteria and evidence summary [Existing]
  New work item [New write]

Work item
  Title and size [New]
  BRD goals and PRD requirements with backlinks [New]
  Linked specs: title, stage, evidence summary [Existing spec fields, new links]
  Open spec Bench [Existing spec selection, new destination]
  Open or launch a spec session [Existing launch, new context]

Bench
  Spec title, stage, criteria, audit, evidence and Git [Existing]
  Requirement and goal beside each criterion [New]
  Sessions, terminal and conversation [Existing]
  Files, previews and Git changes [Existing]
  Tasks and task actions [Existing, no inferred link to spec]
  Guided check and review evidence [Existing]
  Reviewed handoff with role and revision [New]

Context
  Project brief and generated file status [Existing]
  Decisions and in-progress specs with work item links [New]
  Managed context preview, stale status and omission notice [Existing generated status, new projection and notice]
  Shared notes and Add note [Existing]
  Sync and size indicator [Existing sync, new size data]
```

## View destinations

| Current view | Disposition |
| --- | --- |
| studio | Merge work thread into Bench. Move files, previews and conversation to Bench panels. |
| sessions | Move to Bench terminal and session panel. |
| project | Move files to Bench and brief and sync to Context. |
| specs | Keep the stage board under Board. Move selected-spec detail to Bench. |
| host | Move to Settings. |
| overview | Merge useful board counts into Board and machine or runtime checks into Settings. Hide the separate view. |
| tasks | Move to a Bench panel and retain access to the project-wide queue in Settings. |
| memory | Move Shared notes to Context. |
| connections | Move to Settings. |

## Dialog destinations

| Current dialog | Disposition |
| --- | --- |
| phone-dialog | Keep with Bench session controls. Existing phone behavior. |
| end-session-dialog | Keep with Bench session controls. |
| session-dialog | Move to Bench. |
| spec-dialog | Move to work item and Bench creation actions. |
| stage-dialog | Keep for Board moves and Bench stage changes. |
| evidence-dialog | Keep in Bench criteria for legacy evidence text. |
| git-transfer-dialog | Move to Bench Git. |
| sdlc-dialog | Move to Bench checks and evidence. |
| commit-dialog | Move to Bench Git. |
| palette-dialog | Keep globally with commands mapped to new destinations. |
| shortcuts-dialog | Keep globally. |
| brief-dialog | Move to Context. |
| ask-dialog | Keep globally. |
| handoff-dialog | Move to Bench spec handoff. Add the reviewed handoff in R12. |
| observer-panel | Keep globally and route its spec actions to Bench. |
| task-dialog | Move to the Bench and Settings task surfaces. |
| note-dialog | Move to Context. |
| action-dialog | Move with task details. |
| inbox-dialog | Move to Bench session history. |
| paste-dialog | Keep with the Bench terminal. |

## Migration and rollback

Load existing records using the current readers. Display legacy specs in an unlinked lane and legacy tasks and notes through their existing data paths. Associate legacy specs only after owner action and validation. The association write must preserve existing Markdown and identities. Tasks remain standalone unless an explicit later decision adds a relationship. No task conversion, evidence reconstruction or note import is needed.

Roll out compatibility and routing independently of any stored metadata. Keep a reversible navigation switch during browser validation. On rollback restore legacy routes and regenerate the previous Verb block from the still-present brief and specs. Leave new Markdown readable in Git. Back up and restore only the managed projection if a publish is interrupted, with concurrency and malformed-marker behavior decided before implementation.
