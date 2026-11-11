# Terminal-first Verb Desktop Bench

The owner approved this desktop layout. The terminal is where the user talks to an agent. The surrounding panels hold why the work exists, what the agent received, what happened and what needs review. The user can describe the outcome without reading code.

```text
+---------------+-------------------------------------+-------------------+
| Project       | [Codex 004 Implement] [Claude 004   | Plan Review Preview|
| Needs you   3 |  Review] [Scratch] [+]              +-------------------+
| Board         +-------------------------------------+ Why (BRD)       ok |
|               |                                     | What (PRD)      ok |
| WORK          |            Live terminal            | How (Spec 004)    |
| v Checkout    |                                     |                   |
|   * 004 Build |                                     | Done when         |
|   o 005 Plan  |                                     | ok R3 proven      |
| > Login fix   +-------------------------------------+ -- R4 unproven    |
| > Unlinked  2 | Knows: Spec 004, R3 R4, 12 notes    | !  R5 stale       |
| Context       | [Resend context] [Hand off]          | [Draft with agent]|
| Settings      |                                     |                   |
+---------------+-------------------------------------+-------------------+
```

The values in the wireframe illustrate the approved layout. They are examples, not observations about the current checkout.

## Sidebar

The slim sidebar collapses. Project switching sits at the top, followed by Needs you with a count and Board. The owner can open a running workspace using its own Verb URL. It opens in a separate browser tab and authenticates independently, so the source workspace stays mounted. The work tree expands each work item to show its specs and stage markers. A spec selection focuses its existing terminal or shows its Plan with a start action. Unlinked legacy specs sit in a collapsed group with a count. Context and Settings sit at the bottom.

## Centre

Tabs show agent, spec number, role and observed status, using working, waiting for me, idle and ended. A terminal without a spec is labelled Scratch. Two terminals can appear side by side. The plus button opens a launch form with agent, spec or Scratch, and role.

The context strip is display-only. It shows the spec, requirement IDs and note count given to that session. Resend context and Hand off are its actions. A spec-bound terminal that ends offers to append a session note containing outcome, files and checks. The note is attributed as reported and unverified and appears in the next agent's context for the spec.

Board and Needs you replace the centre display while terminals stay alive. The new desktop layout keeps conversation in the terminal. The existing chat-style conversation surfaces remain accessible from the phone and the old layout.

## Right panel

The collapsible panel starts at about one third of the width, remembers its width and follows the active terminal's spec.

- **Plan** shows Why (BRD), What (PRD), How (Spec) and their states. Done when shows criteria with their requirement, goal and proof state. The panel lists tasks for that spec. Draft with agent opens a planning terminal to write the BRD or PRD into the work item folder for owner approval. The owner approves the exact planning instruction before it lands.
- **Review** starts with checks and Proof, then changes since the last known good mark, diff, commit and Git transfer. Reviewed handoff shows revision, role, scope and checks, and labels the reported outcome unverified. It handles stale revisions, pending submits and refusals.
- **Preview** reuses the project file browser and document, image and app previews, editor and browser. Hide the tab when the project has nothing to preview.

Use Why (BRD), What (PRD), How (Spec), Done when and Proof on screen. File names and CLI output retain the original terms.

## Width and continuity

At 390 pixels, show one surface at a time with sidebar and right panel overlays. Keep existing phone behavior. A terminal, transcript and draft survive all navigation, panel changes and layout switches. Keep the Settings switch to the old layout until the cutover. During the transition, every old view remains reachable from Settings or the palette.

## Surface destinations

| Existing surface | Destination |
| --- | --- |
| Sessions | Terminal tabs and per-spec sidebar session access. |
| Studio work thread | Active terminal's spec in Plan. |
| Studio previews, editor and browser | Preview with existing functionality. |
| Project files | Preview file browser. |
| Project brief and sync | Context. |
| Shared notes | Context. |
| Tasks | Spec tasks in Plan and full queue in Settings. |
| Host, Connections and phone pairing | Settings. |
| Overview | Useful counts move to Board and the separate view is hidden. |
| Observer and inbox | Feed observed Needs you entries and terminal tab markers. |
| Palette, shortcuts and Ask | Global. |

Board shows items needing the owner first, then the remaining work items with spec counts by stage, then unlinked specs. Work item recency uses its latest spec audit or evidence entry. Archiving is an explicit owner action and preserves files. Needs you uses only observed waits for input or approval, failed checks, recorded gate warnings, stale proof and handoffs awaiting review. Each entry opens the corresponding terminal and panel where the underlying records identify them.

## Current code and data gaps

These are implementation blockers or differences to resolve in their assigned slices. This design records the approved behavior without inventing substitute data.

| Approved behavior | Current source and gap |
| --- | --- |
| Work tree, named work item documents, traceability and archive | The data slices read and publish numbered work folders, reserve IDs in `work/.next-id`, and audit warnings and spec links. Archive actions are absent. The Bench design itself lives in an unnumbered planning folder. |
| Per-spec context, resend and a truthful context strip | The hub publishes matching bounded blocks on explicit sync, including a selected session checkout. `verb shared full` and `verb shared spec` read the generated views. Separate-account `verb shared read --checkout` reads the published block without private ledger access. A per-session record of exactly what was sent and a resend action are absent. A spec launch audit or shared-read receipt alone does not establish the displayed requirement IDs or note count. |
| Spec and role on every terminal tab | Session records have project and agent identity. Spec starts are inferred from short IDs in spec audit entries. The server accepts assignment roles for spec launches, but a durable full session-to-spec and role association is absent. The existing Sessions tabs and split mode can be reused. |
| Exact tab status and Needs you routing | Existing session state and conversation readiness supply some observations. Working, idle and waiting are not established for every CLI. Observer signals, inbox events, check results, gate warnings and proof freshness are separate. There is no unified Needs you list, observed pending-handoff record, or guaranteed terminal target for a spec-level signal. |
| Session note on terminal end | Reviewed handoffs record reported outcomes, files and checks. Session end has no equivalent spec note action or propagation into a later agent's spec context. |
| Plan states and spec tasks | The reader exposes documents, goals, requirements and link warnings. There is no approved BRD/PRD state or owner-approval record. Existing task records link to sessions and projects, not specs. The optional Bench task spec ID is pending. |
| Draft with agent | Server roles are Implement, Review and Investigate. Planning wording and a reviewable BRD/PRD draft flow are absent. The publish and approval path needs the data slice and the owner's wording approval. |
| Proof and last-known-good review | SDLC exposes criterion evidence and freshness. Last-known-good comparison exists in Rust, but the Review panel does not exist. The existing evidence UI and Git controls are separate. The reviewed server handoff exists, with its matching browser form still pending. |
| Preview availability and draft continuity | Studio and Project have separate preview and file surfaces. There is no combined tab eligibility rule in the new shell. Existing editor and preview behavior should move intact, with continuity verified across the new surfaces. |
| Nonblocking stage gates and archive | The current stage routes warn and record readiness, including for Ship. They refuse stale spec or evidence revisions as write safeguards. Verify the new Board preserves that distinction. Archive and unarchive are absent. |
| Project switcher | The sidebar opens an owner-supplied workspace URL in a separate tab. Each host still serves one project. The switcher stores no destination history and starts no hosts. |
| Old-layout switch, new shell and cutover | Current top-level views still drive navigation. The Settings switch, three-region shell, width persistence and narrow overlays do not exist. Terminal, conversation and draft continuity across the new mount points is unverified. |
| Context and Settings | Existing brief, sync, notes, Host, Connections, phone and task controls exist in separate views or dialogs. The new destinations and Context size, omission and stale displays are pending. |
| Documentation cutover | `docs/README.md` already exists. The final slice updates it as the current-document index and preserves superseded docs under `docs/history/`. |

Source map: [inventory](../../docs/UI_INVENTORY_2026-11.md), [work item reader](../../desktop/src/work_items.rs), [sessions and agent registry](../../desktop/src/session.rs), [terminal layout](../../desktop/web/src/app.js), [task records](../../desktop/src/workbench.rs), [SDLC](../../desktop/src/sdlc.rs), [last known good](../../desktop/src/good.rs), [server routes](../../desktop/src/web.rs), and [context hub](../../desktop/src/hub.rs).

## DAX backlog note

`desktop/src/session.rs` registers named agents in `Agent` and maps aliases, labels, executables and availability. Native availability checks Verb's PATH. Isolated external availability comes from the boundary capability report and program policy in `desktop/src/isolation/external.rs`. Web availability is surfaced by `/api/agents`; the web launch controls and the server's spec-agent admission also enumerate supported choices. An unknown native command can follow the custom-command path, which does not give it a first-class agent identity.

Adding DAX would require confirming its executable, aliases, launch and context arguments, and supported session, resume and readiness capabilities. Then register the supported behavior in the Rust registry and launch controls, add capability and boundary admission only where provisioned, and test local launch and any supported isolated path. DAX's executable and capabilities are unverified.
