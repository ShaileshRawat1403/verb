# Desktop workbench UX plan

**Current state:** The approved Workbench/Terminal mode split is implemented in `verb ui`. The
current pass makes session recovery boundaries, review ownership, and the next task action visible
at 80×24 and 120×30. Saving a new task opens that exact task for assignment, even if another item
has higher priority. Automated TUI checks cover those states; the first-time-user gate below still
needs observation with real users. See `DESKTOP_TUI_REDESIGN_PROPOSAL.md` for the shipped first pass.
The Workbench now also offers an isolated agent choice for Git projects and shows a session's
checkout, branch, and changed-file count in its inbox. This still needs the same first-user gate.
The overlay approach below is historical; the user journey and accessibility gates still apply.

This plan preserves the terminal-first rules in `UX_FOUNDATION.md` and `TUI_VISION.md`. Task and
memory capabilities land in the CLI before any new UI surface. The final interaction and visual
pass starts once the underlying session, task, and handoff contracts are stable.

The first clickable concept is `docs/mockups/desktop-workbench.html`. It uses sample data and
local-only interactions to review the task surface before implementation. The task overlay closes
back to the terminal, keeps one surface open at a time, and shows review handoff separately from
completion.

## The user journey to design for

1. **Orient:** Open one project and see which sessions are running, which task each agent owns, and
   whether a help request or review handoff needs attention. The terminal remains the main area.
2. **Assign:** Create a task with a short title and brief; put shared conventions in project memory;
   let a specific agent session claim the task. The UI names the owner without implying that the
   agent has read the brief or started work until evidence shows it.
3. **Collaborate:** A working agent requests help, another replies, or the owner hands off for review.
   Every note stays attached to its task and names the session that wrote it. A review handoff is
   visibly distinct from verified completion.
4. **Recover:** Reopen Verb after an interruption. Show task history and project memory immediately;
   show session recovery as confirmed, unknown, or unavailable based on exact agent evidence. Make
   the next action clear without silently choosing a different conversation.

## Surface design, after core stabilization

- Keep sessions and tasks as separate, small overlays reached from the command palette and keyboard.
  Do not add a permanent sidebar or shrink the terminal by default.
- The task list should show title, status, owner, and one attention signal: help requested or review
  ready. Opening a task should show brief, latest handoff, and a deliberate path to full history and
  shared memory. Long notes need readable wrapping and scrolling.
- Offer actions in the order a person needs them: open/claim, ask for help, reply, hand off, complete.
  The current owner and any blocked action must be stated in ordinary language.
- Use the same underlying commands for keyboard, mouse, and automation. Preserve the CLI as a full
  alternative for small terminals, SSH, screen readers, and scripts.
- Status must work without color. Distinguish observed facts, agent claims, and user-authored notes.
  Keep IDs available for precise actions, but make titles and agent names the primary reading path.

## Final UX gate

The interaction pass is complete only after a fresh user can perform these scenarios without
instructions from the developer:

1. Start two agents on one project, assign different tasks, switch sessions, and find each task again.
2. Request help from one session, reply from another, hand off for review, and complete after review.
3. Quit and reopen Verb, then find the same task history and resume the intended conversation.
4. Identify a missing agent conversation and understand why Resume is unavailable.
5. Complete the same journey by keyboard alone at 80×24, with `NO_COLOR`, and over SSH; verify mouse
   actions separately on a larger terminal.

Review the screen wording, focus order, empty states, error recovery, text clipping, and contrast
against these journeys. Observe at least one first-time user and revise the flow where they hesitate
or mistake a handoff for completion. A polished screen that fails these journeys does not pass.
