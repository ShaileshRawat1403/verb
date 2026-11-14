# Brief: the spec-driven, Git-powered workbench

> Owner's direction, 2026-10-07. Verb Desktop's web UI becomes an SDLC workbench built on two
> practices software teams rely on: **specs** and **Git**. It helps non-developers by giving them
> real structure, not simplified words. **Everything should be easy, and auditability is
> paramount.** Record progress at the bottom; keep the direction above it unchanged.

## The workflow

```
SPEC ──▶ PLAN ──▶ BUILD ──▶ VERIFY ──▶ REVIEW ──▶ SHIP
what &   tasks &  agent/you  tests vs   diff vs    merge,
why,     branch   in a       acceptance spec,      tag,
accept-           worktree   criteria   approve    release
ance
```

## Decisions (owner accepted the recommendations)

1. **Specs are Markdown** in `<project>/specs/NNN-slug.md`, with front matter (`id`, `title`, `stage`,
   `branch`, `created`) and fixed sections: Problem, Who it is for, Acceptance criteria (checkboxes),
   Out of scope, Audit trail. They are committed with the code, so they are readable on GitHub and by
   agents.
2. **Both people and agents write specs.** A guided form creates the file; agents can draft or refine
   it. Agents started from a spec get a brief that tells them to read it, meet the criteria one at a
   time, stay out of scope, and leave the audit trail alone.
3. **Gates warn and record; they never block.** Skipping stages or moving to review/ship with
   unproven criteria is allowed, and the trail and the stage bar say so.
4. **Folded into Phase 3** of `DESKTOP_TERMINALS_BRIEF.md`: specs are how terminals are grouped.
5. **Git-only for now.** No GitHub PR, issue or release integration yet.

## Auditability rules (non-negotiable)

- Every change Verb makes to a spec appends one line to its `## Audit trail`: UTC time, who (the
  project's Git `user.name` plus the surface, e.g. "via Verb web"), and what. Lines are never edited
  or removed. Content cannot forge separators or extra lines.
- Proving a criterion asks for evidence and records it. Reopening one is recorded too.
- A commit made from Verb with a spec selected records itself in that spec's trail *before*
  committing, so the record and the code land in the same commit.
- Branch switches refuse when there are uncommitted changes, and are recorded.
- The UI must show what the trail knows: for example, a skipped stage is marked as skipped, not done.

## Built (2026-10-07, Claude)

- `desktop/src/specs.rs`: spec parsing and writing, stages, criteria with evidence, audit trail,
  Git summary, commit, switch-to-branch, the agent brief. 7 unit tests on real temp Git repos.
- API: `GET/POST /api/specs`, `GET /api/specs/{id}`, `POST /api/specs/{id}/stage`,
  `POST /api/specs/{id}/criteria/{n}`, `POST /api/specs/{id}/branch`, `POST /api/specs/{id}/agent`,
  `GET /api/git`, `POST /api/git/commit`.
- Web UI: a **Specs** view (now the default) with a specs rail and Git card; a stage bar with plain
  guidance for each stage; terminals and agents opened from the spec appear in its work area; a proof
  panel with acceptance criteria, changed files and the audit trail. Dialogs for new spec, stage
  change (shows the warnings that will be recorded), evidence, and commit.
- Utilities: command palette (⌘K, or Ctrl+K outside a terminal), shortcuts Alt+N new spec, Alt+T
  terminal, Alt+C commit, Alt+1…5 views, `?` for the shortcut list. Duplicate start buttons are
  hidden in the Specs view.
- `desktop/web/src/workbench.js`: the UI's rules (stage warnings mirroring the server, skipped
  stages, palette ranking, Git labels), 7 tests.
- Verified in Chrome against a scratch repo: created specs, moved a stage with a skip warning,
  proved a criterion with evidence, committed from the dialog (the audit line landed in the same
  commit), searched the palette, and ran a shell in the spec's work area.

## Next

- Owner review of the new UI on Node 1.
- A Review stage view: the diff grouped by file, with each change linked to a criterion.
- Tasks inside a spec (reuse the task ledger), and spec-scoped isolated worktrees.
- Edit a spec's sections in the UI (today: edit the Markdown file).
- An API-level integration test for the spec routes. Today they are covered by the module's unit
  tests plus the manual browser run above.
- Optional GitHub integration (PR from a spec), once Git-only is proven.
