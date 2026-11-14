# Verb desktop TUI redesign proposal

Status: approved design, implemented in `verb ui` as the first Workbench pass. The mockups remain
visual references; the Rust TUI is the product surface.

## Why the current screen feels weak

At 120×30, `verb ui` starts an idle shell that fills almost the entire screen. The footer lists
generic F1–F4 actions, while the durable sessions and task handoffs that distinguish Verb are out
of sight. On macOS, function keys may require an extra `Fn` press. “What Verb knows” opens a centered
box with raw event data, a long path, and a clipped caveat. It gives little hierarchy or next action.
The result is a terminal wrapper rather than a mission control experience.

## Implemented interaction model

**Two deliberate modes:**

1. **Workbench** is the landing view for `verb ui`. It shows live/recoverable sessions beside task
   priority. A selected task reveals a short brief, latest handoff, and the next valid action. The
   shell need not start until the user enters Terminal. This avoids an empty default screen.
2. **Terminal** tiles every process hosted by this Verb instance. Each pane shows its exact agent
   session; one pane owns keyboard focus. A click or leader+number changes focus, and leader+z
   temporarily zooms the focused pane. Verb reserves only its leader chord while an agent has
   keyboard focus. `verb shell` remains a direct terminal entry point.

The screenshots in the companion mockups are drawn at **120 columns × 30 rows**. At that size,
Workbench uses two panes: sessions on the left, tasks on the right. Selecting a task opens a full
detail view. At 80×24, show one pane at a time with the same hierarchy and actions. No modal is
stacked on another modal. Terminal mode has no permanent sidebar.

## Navigation and language

| Situation | Primary action | Result |
| --- | --- | --- |
| Landing in `verb ui` | `↑↓`, `Tab`, `Enter` | Choose a session or task and inspect it |
| Task needs review | `c` Claim review | Records reviewer ownership; still not complete |
| Agent asks for help | `Enter`, then Reply | Writes a reply in task history; original owner remains |
| Owner is unavailable | `r` Reassign | Requires a reason and keeps previous ownership in history |
| Focus an agent | `Enter` on its session | Opens that terminal or exact Resume when confirmed |
| Several agents running | Click a pane or leader+1–9 | Changes keyboard focus without stopping other agents |
| Pane is too small | leader+z | Zooms the focused pane; repeat to restore tiles |
| Focused terminal | `Ctrl+Space` | Opens Verb commands without stealing ordinary agent keys |
| Any Workbench detail | `Esc` | Returns one level; never loses a note without warning |

In Workbench, visible `1 Terminal`, `2 Workbench`, `3 Activity`, and `4 Memory` labels make mode switching
discoverable. These plain keys apply only while Verb owns focus. Function keys can remain optional
aliases, but should no longer be the primary UI. The bottom line changes with the selected item;
it does not advertise commands irrelevant to the current task.

Use *running*, *recoverable*, *checking*, and *ended* for session states. Use *needs review*, *help
requested*, *in progress*, *open*, and *done* for tasks. Color repeats these words; it never carries
meaning alone. “Handed off” never appears as “done.” The task detail separates a user's brief,
an agent's recorded note, and facts Verb actually observed.

## Implementation and remaining validation

1. Done: Workbench reads the existing session, task, and memory stores; task transitions remain in
   the CLI core. The shell starts only when Terminal is opened.
2. Done: 120×30 two-pane overview, 80×24 focused pane, task detail, new task, note composers,
   shared memory, and selected session switching. The terminal offers F2 and leader+w to return.
3. Done: keyboard controls, clickable tabs, buttons, empty-state prompts, task and session rows,
   task filters, and an agent chooser for claim, reply, and reassign. Mouse clicks run command
   palette choices and Save/Cancel actions; hit areas derive from the rendered layout. Child TUIs
   receive pane-relative mouse events when they enable tracking;
   otherwise the wheel scrolls output. The same core validates task actions. Rust TestBackend layout
   tests and real PTY journeys cover create/memory/quit, two simultaneous agents, mouse focus, and
   SGR mouse forwarding.
4. Still needed for product sign-off: a first-time-user journey with two installed agents, help,
   review, restart, and exact resume; SSH/tmux and screen-reader review; contrast and long-note QA.

The design borrows the useful interaction patterns of pane navigation and contextual keys from
[Lazygit](https://github.com/jesseduffield/lazygit/blob/master/docs/Config.md), a dedicated session
manager from [Zellij](https://zellij.dev/documentation/session-manager-alias.html), and a command
palette from [OpenCode](https://opencode.ai/docs/tui/). Verb's focus remains its own durable task,
handoff, and session state.
