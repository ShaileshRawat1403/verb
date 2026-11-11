# Bench consolidation PRD

Proposed design traced to the [BRD](brd.md). Existing data claims come from the [UI inventory](../../docs/UI_INVENTORY_2026-11.md). Owner-dependent details are listed in [open questions](open-questions.md).

## Requirements

| ID | Goal | Requirement |
| --- | --- | --- |
| R1 | G1 | A work item contains a BRD, a PRD and one or more specs when ready for execution. A draft can start with a BRD. Standard items use full documents. Small items use a three-line BRD and one-page PRD. |
| R2 | G1 | Store new work item documents in project Git as UTF-8 Markdown at proposed paths work/<number>-<slug>/brd.md and work/<number>-<slug>/prd.md. Keep existing spec Markdown under specs/. Use a sequential number plus slug in the same style as spec IDs. Never reuse IDs. Put a plain-text Size: small or Size: standard line near the top of brd.md. |
| R3 | G1, G4 | Each criterion on a new or explicitly linked spec uses plain-text references. New criteria end with `(R3)` or `(R3, R5)`. For existing criteria, preserve index and text and put the mapping in a separate `## Traceability` section. References to retired requirements warn. Each PRD requirement names its BRD goal. Use plain-text IDs (G1, G2 and R1, R2). IDs are never renumbered. A retired ID stays in its file marked retired. Validate references on document read and show missing links. Preserve legacy criterion text and report it as unlinked until an owner supplies a mapping. |
| R4 | G2 | The terminal-first desktop shell has a collapsible project and work sidebar, a terminal centre, and a collapsible Plan, Review and Preview panel with remembered width. Board and Needs you open in the centre while terminals stay alive. Context and Settings are sidebar destinations. At 390 pixels show one surface at a time, with sidebar and right panel overlays. Board rows show work items and their spec counts by stage. Put items that need the owner first, then sort by the latest audit or evidence entry across their specs. Attention uses only an observed session waiting for input or approval, a failed check, a recorded gate warning, stale evidence, or a handoff awaiting review. Plan, build, verify and review count as in progress. An item completes when all its specs ship. Explicit owner archive hides its row behind a filter and keeps its files. |
| R5 | G2 | Keep the six spec stages, criteria, evidence and append-only audit. Every stage gate, including Ship, warns and records while allowing stage movement. Evidence remains separately inspectable. Git, terminal and write operations retain their own operation-specific safeguards. |
| R6 | G2, G4 | Maintain the existing terminal and conversation instances across navigation. Tabs identify agent, spec, role and observed status, with Scratch for a terminal without a spec. Support two terminals side by side and a launch form for agent, spec or Scratch, and role. Plan follows the active spec and shows BRD, PRD, criteria with their requirement and goal, and linked tasks. Draft with agent uses owner-approved planning wording and produces a BRD or PRD for owner approval. Keep desktop agent interaction in the terminal and retain the chat-style surfaces for phone and old layout. Tasks created from Bench store its spec ID in a new optional field. Legacy tasks stay unlinked and readable in the full Settings queue. |
| R7 | G3 | Generate identical Verb blocks in root AGENTS.md and CLAUDE.md. Include the project brief, Shared notes as the canonical decisions and facts, newest first, and one line per in-progress spec identifying its work item. Apply existing redaction to notes and exclude ignored files. The shared block has no session-specific active spec. Publish through the existing hub boundary and preserve all bytes outside an existing well-formed marker pair. |
| R8 | G3 | Limit the entire shared section to 8192 bytes of UTF-8. Always keep the brief problem, scope and constraints. On overflow, omit oldest notes first, then in-progress spec lines beyond the ten most recently changed, then the glossary, and end the section with a line identifying omissions and the command that prints full context. Reserve room for that line and the end marker. If required content alone exceeds the bound, cut at section boundaries and show a Context warning asking the owner to shorten the brief. Never publish over the bound. Check for external changes on load and before publish. Show stale state and refresh only on an explicit owner click. Use one project lock across both writes. After an interrupted publish, show stale for unequal blocks and republish on owner action. Refuse writes and show the problem for malformed or duplicate markers. |
| R9 | G3 | Every supported hosted CLI launch must receive a verified way to read the shared block in its actual working environment. A session launched for a spec receives that spec and its linked PRD requirements and BRD goals in the launch prompt. Publish in the actual launch checkout, including isolated checkouts. An unlinked legacy launch receives the shared block and spec with a line saying no work item is linked. Bound per-spec launch context to 8192 bytes using the same omission rule. Use `verb shared spec <spec-id>` and `verb shared full`. Every CLI launch prompt instructs the agent to run `verb shared read` for shared context. Verify that launch path independently of native instruction-file discovery. Both print plain text. Report instruction-file discovery by CLI and version, marking untested entries unverified. Test main and isolated checkouts and separate-account launch modes that are supported. Keep verb shared read and existing Shared notes readable. |
| R10 | G4 | Read legacy specs, task JSON and note Markdown in place. Only explicit owner action links a legacy spec and appends an audit line. Preserve original audit and evidence IDs. Rollback restores previous navigation and the previous generated projection from current compatible sources, while legacy readers remain usable. |
| R11 | G3, G4 | Publish new or updated work item documents and managed context with file locking and complete-file replacement. Detect concurrent edits before replacement. Treat multi-file publish and retry behavior as an explicit design and test case. |
| R12 | G2 | Expose the reviewed handoff in the context strip and Review. Show the spec revision, assignment role, approved scope and checks, and reported outcome, files and check results as unverified. Require a fresh review if the revision changes and prevent repeated submissions while a launch is pending. |
| R13 | G2, G3 | Show a display-only strip under the terminal with the spec, requirement IDs and note count actually sent to that session. Provide Resend context and Hand off. Missing delivery facts require a data change before rendering them as known. |
| R14 | G2, G3 | When a spec-bound session ends, offer to append an attributed session note with outcome, files and checks marked reported and unverified. Include it in the next agent's per-spec context. |

## Document shape

Proposed project layout

```text
work/<number>-<slug>/brd.md
work/<number>-<slug>/prd.md
specs/<existing-id>-<slug>.md
```

A standard BRD must have Problem, Audience, Goals, Success measures and Scope sections. Missing required sections warn and record. A standard PRD must list requirements with IDs, goal references and verification. Missing required content warns and records. A small BRD has one line each for problem and audience, goal, and measurable success and boundary. The goal line contains its stable G ID. A small PRD retains numbered requirements, goal references and verification within five requirements and 60 lines. Exceeding either limit warns and records.

The link uses a spec work-item ID and either a new criterion suffix or a legacy Traceability entry. SDLC hashes the criterion index and text for its key, and includes all criterion text in the intent digest (desktop/src/sdlc.rs:272 and :277). Preserve existing criterion text and order. Each legacy Traceability line records its criterion number, the existing computed criterion identity, and its requirement IDs. If the criterion text changes, show the link as stale and keep the recorded association until explicit relinking. Example of the logical model is `spec 001 / criterion 2 → R3 → G1`. IDs are scoped to the work item. The representation must remain readable by legacy spec readers, retain existing criteria and audit syntax, and preserve evidence bindings. Relationship metadata and association lookups are new data.

## Text wireframes

The [approved design](design.md) contains the desktop wireframe and source gaps. Existing facts and pending data are marked here.

```text
Sidebar                   Centre                          Right panel
Project [existing]        Agent tabs and split [existing] Plan | Review | Preview [new shell]
Needs you [new aggregate] Spec, role, status [new links]   Why (BRD), What (PRD), How (Spec)
Board [new destination]   Live terminal [existing]         Documents [existing reader], approval state [new]
Work tree [reader data]   Knows: spec, requirements, notes Done when: proof [existing], trace links [reader data]
Unlinked [reader data]      [new delivery record]           Spec tasks and Draft with agent [new data/action]
Context [new destination] Resend context [new action]      Checks, Proof, last known good and Git [existing facts]
Settings [new destination]Hand off [existing server]      Files and previews [existing]
```

Board and Needs you replace the centre display without ending terminals. Board shows observed attention, stage counts and the collapsed unlinked lane. Archive and unified Needs you routing require new data or actions. At 390 pixels the sidebar and right panel are overlays. The phone layout retains its existing behavior.

## View destinations

| Current view | Disposition |
| --- | --- |
| studio | Replace the work thread with the active spec in Plan. Move previews, editor and browser to Preview. |
| sessions | Move to centre terminal tabs and spec session access in the sidebar. Keep chat-style conversation available on phone and in the old layout. |
| project | Move file browser to Preview and project brief and sync to Context. |
| specs | Move the work overview to centre Board. Move selected-spec planning and review to the right panel. |
| host | Move to Settings. |
| overview | Move useful counts to Board and hide the separate view. |
| tasks | Show spec-linked tasks in Plan and the complete queue in Settings. |
| memory | Move Shared notes to Context. |
| connections | Move to Settings. |

## Dialog destinations

| Current dialog | Disposition |
| --- | --- |
| phone-dialog | Move pairing entry to Settings and retain the existing session continuation flow. |
| end-session-dialog | Keep with terminal tabs and offer a session note after a bound session ends. |
| session-dialog | Open from the centre plus button with agent, spec or Scratch, and role. |
| spec-dialog | Open from the work item and Plan creation flow. |
| stage-dialog | Keep for Board and Plan stage moves. |
| evidence-dialog | Move to Review Proof and retain legacy criterion text. |
| git-transfer-dialog | Move to Review Git. |
| sdlc-dialog | Move to Review checks and Proof. |
| commit-dialog | Move to Review Git. |
| palette-dialog | Keep globally and expose old views during the transition. |
| shortcuts-dialog | Keep globally. |
| brief-dialog | Move to Context. |
| ask-dialog | Keep globally. |
| handoff-dialog | Open from the context strip or Review using the reviewed server contract. |
| observer-panel | Feed Needs you and tab status, with the existing surface reachable during the transition. |
| task-dialog | Move to Plan and Settings task surfaces. |
| note-dialog | Move to Context. Session notes have their own end-session action. |
| action-dialog | Move with task details in Plan and Settings. |
| inbox-dialog | Feed Needs you and tab status. Keep session history access during the transition. |
| paste-dialog | Keep with the terminal. |

## Migration and rollback

Load existing records using the current readers. Display legacy specs in an unlinked lane and legacy tasks and notes through their existing data paths. Associate legacy specs only after owner action and validation. The association write must preserve existing Markdown and identities. Existing tasks remain standalone. Bench-created tasks carry the optional spec ID. No task conversion, evidence reconstruction or note import is needed.

Roll out compatibility and routing independently of any stored metadata. Keep the Settings switch to the old layout until layout slice 7 journeys pass. Keep all old views reachable from Settings or the palette during the transition. Redirect old routes to their new destinations. Keep palette, shortcuts, Ask, Observer, theme and project switching global. On rollback restore legacy routes and regenerate the previous Verb block from the still-present brief and specs. Leave new Markdown readable in Git. Back up and restore only the managed projection if a publish is interrupted, using the same project lock and marker checks.
