# Verb desktop MVP

This is the desktop host of Verb: a native Rust CLI, Ratatui workspace, and local browser
workbench that sit above the machine's existing shell, agents, and Git installation.

The product boundary is:

```text
Verb: project + Git context + session metadata + recovery
  ↓
native shell / agent process
  ↓
macOS, Linux, or Windows host tools
```

### Why Rust here

The desktop host owns PTYs, child processes, terminal parsing, session locks, and atomic task
writes. Rust makes those lifetimes explicit while producing a native CLI/TUI executable with low
idle overhead. Ratatui draws Verb inside the terminal; each coding agent still runs as its own
installed CLI. The Android app remains Kotlin. Rust is an implementation choice for this host,
not a requirement for agents to participate.

## Run

```bash
cargo run --manifest-path desktop/Cargo.toml -- status
cargo run --manifest-path desktop/Cargo.toml -- project --json
cargo run --manifest-path desktop/Cargo.toml -- project worktree
cargo run --manifest-path desktop/Cargo.toml -- isolated codex
cargo run --manifest-path desktop/Cargo.toml -- isolated agent agy
cargo run --manifest-path desktop/Cargo.toml -- ui
cargo run --manifest-path desktop/Cargo.toml -- web
cargo run --manifest-path desktop/Cargo.toml -- claude
cargo run --manifest-path desktop/Cargo.toml -- codex
cargo run --manifest-path desktop/Cargo.toml -- agent agy
cargo run --manifest-path desktop/Cargo.toml -- agent hermes
cargo run --manifest-path desktop/Cargo.toml -- agent openclaw tui
cargo run --manifest-path desktop/Cargo.toml -- sessions
cargo run --manifest-path desktop/Cargo.toml -- resume
cargo run --manifest-path desktop/Cargo.toml -- resume SESSION_ID
cargo run --manifest-path desktop/Cargo.toml -- memory show
cargo run --manifest-path desktop/Cargo.toml -- task list
cargo run --manifest-path desktop/Cargo.toml -- continuity export /tmp/project.vcont
cargo run --manifest-path desktop/Cargo.toml -- continuity import /tmp/project.vcont
cargo run --manifest-path desktop/Cargo.toml -- check
cargo run --manifest-path desktop/Cargo.toml -- runtime --json
cargo run --manifest-path desktop/Cargo.toml -- good mark
cargo run --manifest-path desktop/Cargo.toml -- good
cargo run --manifest-path desktop/Cargo.toml -- good files
```

`verb check` gathers what Verb can observe right now that calls for care, each with the safe next
step as text. It reads three things and runs none of the steps it names:

* **Repository state** from Git's own markers: an unfinished rebase, `am`, merge, cherry-pick,
  revert or bisect; unresolved conflicts; a detached HEAD; an upstream that has diverged or been
  deleted as of the last fetch. Verb never fetches.
* **Runtimes the project declares** (`.nvmrc`, `.node-version`, `package.json` engines,
  `.python-version`, `pyproject.toml` `requires-python`, `rust-toolchain(.toml)`, `Cargo.toml`
  `rust-version`, `go.mod`, `.ruby-version`, `.tool-versions`) against the runtime's own
  `--version`, run in the project directory. A requirement Verb cannot compare (`lts/*`, `stable`)
  is reported as unknown. A toolchain file that points at a binary inside the project is never run.
* **Distance from last-known-good**, once `verb good mark` has recorded a state you say works.
  The mark stores the commit id, an uncommitted count and a fingerprint of the tree; never file or
  branch names. `verb good files` lists what differs, read live.

`verb check --json` carries counts and versions only, so it is safe to hand to an assistant under the
same rule as `verb context`. The TUI band and the web workbench's "Reasons for care" panel show the
same facts.

Prebuilt binaries come from `.github/workflows/release-desktop.yml` on `desktop-vX.Y.Z` tags. The
macOS builds are not notarized; after a browser download, run `xattr -d com.apple.quarantine verb`.

`verb web` prints a local URL to open in a browser. It binds only to `127.0.0.1` and uses a
random token for its API. The browser shows the same project sessions, tasks, and memory as the
CLI and TUI, with clickable task actions and side-by-side live terminal panes. Session and task
lists can be searched, and a terminal's Focus control expands that pane while the others keep
running. Click the control again to show all panes. Run it from the project you want to work on;
`--port PORT` optionally fixes the local port. The browser host owns
only the PTYs it starts. Sessions running in another Verb process remain visible, but their live
terminal cannot be controlled from this page. Closing the web host stops its hosted processes and
keeps the durable work records. The page uses bundled assets and makes no external browser
requests.

To control a running web terminal from Verb Mobile, keep desktop and phone on the same network,
choose **Phone** on that terminal, and scan the QR code. Android opens **Control a desktop
session** with the link filled in; tap **Connect**, then **Take input control**. The desktop can
take control back, and **Stop sharing** revokes phone access. The pairing view names when the
one-use code is ready, used, or expired. **New pairing link**
renews the code without interrupting an already connected phone; pairing another phone replaces
the old device. For a session running in `verb ui`,
use `verb mobile share SESSION_ID` in a second desktop shell and open the printed link on the
phone. The CLI and agent process continue to run on the desktop. The phone sees the current
plain-text terminal screen, not the transcript. A firewall must allow the temporary TLS port
printed in the link. See `docs/DESKTOP_MOBILE_BRIDGE_PROTOCOL.md` for the security and lifetime
contract.

For a quick launch from this repository, run `./launch-web.sh`. To rebuild the browser assets
after editing `desktop/web/src`, run `npm ci --prefix desktop/web` and
`npm run build --prefix desktop/web` before compiling Rust. The compiled Rust binary embeds the
built browser assets; Node.js is unnecessary at runtime.

To install a local `verb` command:

```bash
cargo install --path desktop
verb status
```

Verb uses the current Git repository root as the project context when one exists. Each session has
its own durable record under `~/.verb/sessions`, so starting another agent in the same project does
not replace the first. Existing project-keyed records are read and migrated when that session is
saved. Read-only foreign evidence lives separately under
`~/.verb/imported`. Credentials, transcripts, and agent-specific state remain owned by the agent.
`VERB_STATE_DIR` can point tests or a development build at an isolated state folder.

`verb project` shows Verb's durable local project ID and the current workspace. Git worktrees
from the same repository share one project ID, memory, tasks, inboxes, and fetch receipts while
each session keeps its actual checkout path for native resume. The first access migrates older
path-keyed work records into the ID-keyed store. If two old stores contain data, Verb reports the
conflict instead of choosing one silently.

`verb isolated claude|codex|opencode` or `verb isolated agent CMD [ARGS...]` creates a separate Git
worktree and starts the agent there. `verb project worktree` creates one without launching an agent.
The Workbench's `[Isolated]` action offers the built-in agents and a generic CLI picker; the session inbox shows the
checkout path, branch, and changed-file count. A worktree starts from **committed HEAD**. Edits
that have not been committed in the source checkout are not copied. The worktree and its branch
remain after the agent exits so its files can be inspected; remove them with Git when finished.
These worktrees live under `~/.verb/worktrees` (or `VERB_STATE_DIR/worktrees`).

## Current guarantees

- `verb status` reports the project root, branch, changed-file count, and latest session, including
  its ID and the agent conversation an exact resume would land on.
- Bare `verb` opens the UI on a terminal, and prints help when it is piped, redirected, or run in
  CI. A bare command should not do something interactive that depends on where its output is going.
  `verb shell`, which used to be the bare default, is unchanged.
- `verb ui` opens the Workbench first. Sessions, priority tasks, shared memory, and observed activity
  are separate views. Tab changes the active list, arrows select, and Enter opens a task or switches
  to an available session. An ended, unconfirmed, or externally hosted session opens its recorded
  inbox instead. At 120×30 the Workbench shows sessions and tasks together; at 80×24 it shows
  the focused list. Session cards show the action a click will take, while the header keeps the
  project name visible when an isolated worktree is active. Visible tabs, action buttons,
  empty-state prompts, and task/session rows accept
  mouse clicks. Task and note editors have clickable Save and Cancel actions; the command palette
  runs a clicked choice. `1` opens Terminal, `2` returns to Workbench, `3` shows Activity, and `4` shows
  Memory. Terminal tiles every process hosted by this Verb instance. Click a pane, or press
  `Ctrl+Space` then its number, to focus it; `Ctrl+Space` then `z` zooms the focused pane and toggles
  back. Mouse events go to a child TUI only when it enables mouse tracking. Otherwise clicks focus
  panes and the wheel scrolls that pane's output. `Ctrl+Space` then `m` releases mouse capture for
  terminal text selection. `F2` or `Ctrl+Space` then `w` returns to Workbench; agent keystrokes
  otherwise go to the focused agent. Quitting closes hosted processes and keeps recoverable sessions
  in Verb's store.
- `verb sessions` lists every session Verb holds, newest first, with the ID needed for
  `verb resume SESSION_ID`. Without an ID, `verb resume` selects the newest recoverable session in
  the current project. An explicit ID can select a session in another project.
- `verb continuity export PATH` writes checksummed structural evidence for every session in the
  current project.
  `verb continuity import PATH` previews without changing state; `--apply` stores the validated file
  as read-only foreign evidence. Origin state is history and never enables Resume by itself.
- `verb context` assembles everything Verb currently knows about a project -- Git state read now,
  the session record, and the tail of its structural event log -- into one place, in text or with
  `--json`. It interprets nothing: there is no field for a conclusion, and no model behind it. It is
  the groundwork every M2 direction needs (explanation, comparison and guided action all start by
  gathering the same evidence), built before choosing between them.
- `--json` on `status` and `sessions` emits the durable record exactly as `docs/VERB_SESSION_SCHEMA.md`
  defines it, ISO-8601 timestamps included, so a consumer reading one host's output does not have to
  learn the other's. `sessions --json` on an empty state is `[]`, not a message.
- Exit codes are stable: `0` success, `1` failure, `2` a wrong command line, `3` nothing to do --
  no session, or recovery not confirmed. `3` exists so a caller that retries on failure does not
  retry on a correct "there is nothing recoverable here".
- `NO_COLOR` is honoured in the UI (no-color.org). The selected row is marked with a glyph rather
  than colour alone. The session list checks a per-session process lock: a held lock confirms a
  live agent, while an available lock triggers recovery reconciliation from the agent's exact
  conversation evidence. On Unix the agent inherits the lock, so a Verb host crash cannot launch
  a duplicate while that agent is still running.
- `verb claude`, `verb codex`, `verb opencode`, and `verb dsh` launch the installed agent in the
  current project through a Unix PTY, preserving interactive TUI behavior. Windows currently uses
  inherited terminal input/output as a compatibility fallback.
- `verb agent CMD [ARGS...]` hosts another installed CLI as a first-class Verb agent session. The
  Workbench launcher has AGY, Hermes, and OpenClaw TUI choices, plus an executable picker. Verb
  passes arguments unchanged, including flags such as `--json`. Use `verb run CMD [ARGS...]` for a
  generic command that is not a task actor. An external agent can read shared context, own a task,
  ask for help, and hand off work while its process is live.
- Isolated agents use separate Git worktrees within the same Verb project. Their file edits stay
  separate while task ownership and shared context remain common. Verb does not merge branches or
  copy uncommitted changes into an isolated checkout.
- Every launch gets `VERB_SESSION_ID` and `VERB_PROJECT_ROOT` environment context.
- Unix launches record structural JSONL events under `~/.verb/events/<project>/<session>.jsonl`:
  session/process lifecycle, state transitions, exit status, and -- from the shell's own OSC 7 and
  OSC 633/133 markers -- working-directory changes and command boundaries with an opaque command id
  and exit code. Raw terminal input and output are ephemeral and are never persisted, and the
  marker that carries the command line (`OSC 633;E`) is recognised only so it can be skipped.
- Verb owns the session registry, selection, state transitions, and exact resume decision. Claude,
  Codex, and OpenCode adapters verify the chosen conversation still exists, then ask that CLI to
  resume its own conversation ID (`desktop/src/agents.rs`). An absent, unsafe, or unverified ID never
  falls back to the agent's latest conversation. `dsh` and external CLIs have no verified native
  resume adapter. External sessions end when their host exits; their task and memory history remains.

| Launch path | Shared task actor | Automatic first-turn fetch | Exact native resume |
| --- | --- | --- | --- |
| Claude, Codex | Yes | Yes, when launched without caller arguments | Yes, with verified local identity |
| OpenCode | Yes | Manual `shared read` | Yes, with verified local identity |
| `dsh` | Yes | Manual `shared read` | Unverified |
| `verb agent CMD [ARGS...]` | Yes | Manual `shared read` | Unavailable until a CLI-specific identity adapter is verified |

These are Verb capabilities, independent of whether an agent uses a subscription, an API key, or
local models. Verb does not manage the agent's authentication or native conversation store.

## Shared memory and task handoffs

Verb keeps a project-scoped `memory.md` and task records under `~/.verb/work`. They are explicit
notes and agreements, not captured agent transcripts. Each task has one owner session at a time;
help requests, replies, review handoffs, and completion notes stay in its history. Writes use a
project file lock and atomic replacement so concurrent agent processes cannot overwrite one another.

```bash
verb memory set conventions.md             # create memory when empty
verb memory hash                           # current revision for replacing it
verb memory set conventions.md --base-hash HASH  # replace only that revision
verb memory set conventions.md --force      # intentional unconditional replacement
verb memory append new-fact.md              # add a note
verb memory show
verb shared read                          # one versioned memory + task/handoff snapshot
verb shared read --json                   # stable machine-readable snapshot
verb shared publish new-fact.md          # append an attributed note
verb shared status                        # see which sessions fetched this revision
verb inbox [SESSION_ID]                   # pending attention for one agent session
verb inbox [SESSION_ID] --json            # machine-readable inbox
verb task create "Implement parser" brief.md
verb task list
verb task context TASK_ID                  # memory, brief, and handoff history together
verb task claim TASK_ID SESSION_ID
verb task reassign TASK_ID NEW_SESSION_ID reason.md
verb task request-help TASK_ID SESSION_ID question.md
verb task reply TASK_ID HELPER_SESSION_ID answer.md
verb task handoff TASK_ID SESSION_ID summary.md
verb task claim TASK_ID REVIEWER_SESSION_ID
verb task done TASK_ID REVIEWER_SESSION_ID result.md
```

Use `-` instead of a note file to read from stdin. Inside a Verb-hosted agent, omit `SESSION_ID`:
Verb sets `VERB_SESSION_ID`, `VERB_PROJECT_ROOT`, `VERB_PROJECT_MEMORY_PATH`, and `VERB_BIN` in
its child environment. An agent can run `"$VERB_BIN" shared read` even if Verb is not installed
on `PATH`, and can run `"$VERB_BIN" shared publish -` to publish a note from stdin. The project
root keeps these calls in the same collaboration scope if the agent changes directory. A shared
read writes a per-session fetch receipt only after its output is written successfully.
`shared status` compares that receipt with a SHA-256 content revision of memory and the complete task
history. It reports a CLI fetch, not proof that the model attended to or followed the content.
The session inbox is derived from the same task ledger and the last successful fetch receipt.
It shows review and help requests available to that session, plus replies and assignments added
since its last fetch. Reading the inbox or opening a task does not acknowledge work or run an
agent. A review or help request remains until the task transition resolves it; a reply or
assignment clears from the inbox after `shared read` delivers the newer event. Receipts created
before event cursors existed report event freshness as unknown. A request shown as available is
project-wide; it is not an assignment to every agent that can see it.
When Verb starts or exactly resumes Claude or Codex without caller-supplied launch arguments, it
starts the interactive conversation with a short prompt to fetch the current snapshot and then
wait for the user's task. This uses the CLIs' positional prompt contract and leaves their normal
configuration alone. OpenCode and custom agents still have the shared CLI bridge but do not yet
receive an automatic prompt. A failed or skipped fetch remains visible in `shared status`.
Existing `memory` and `task` commands also change the revision. `memory append` serializes
concurrent contributions. Replacing an existing memory note with `memory set` requires its current
hash, unless `--force` explicitly overrides that check. The memory path is for reading; update it
through Verb so concurrent writes are serialized. `task list`,
`task show`, and `task context` accept `--json`. Task actors must be Verb agent sessions in the
same project. A handoff frees the task for another session to claim; a help reply leaves ownership
with the original agent. In Workbench, `t` creates a task, `n` opens new session choices, and `m`
opens shared memory. Select a session and use `[Inbox]` or `i` to see its attention and shared
context status. Inbox rows, `[Open task]`, `[Open session]`, and `[Refresh]` work with the mouse;
`Enter`, `s`, and `r` are keyboard equivalents. Task detail has claim, help, reply, handoff, reassignment, and completion
actions, each using the same durable transition code as the CLI. Memory view can append a note.
Saving a task in Workbench opens that exact task so its next step, assignment, and history are
visible immediately, even when older help or review items rank above it. Session cards distinguish
a pane hosted here, a live session in another Verb process, an exact recoverable conversation,
unknown recovery, and an unavailable conversation. Review cards name the session linked to the
handoff and make the unclaimed reviewer state explicit. These are recorded task agreements, not
proof that an agent read the note or verified the work.
CLI handoffs require the owner to have fetched the current memory and that task's revision. If
either changes after that fetch, the handoff is rejected until the owner reads again. A change to
an unrelated task does not block the handoff. Workbench checks the same scoped revision captured
when its handoff editor opened and asks for a retry if it changed while the note was being written.
`reassign` records a reason and can move an active task when its owner has stopped or cannot hand
it off; the previous owner remains visible in history.

## Recovery boundary

Verb persists session metadata and structural events. It does not persist agent transcripts or a
running process across a Verb restart. Exact recovery requires the corresponding agent's local
conversation store and CLI to remain available. A source ZIP or `.vcont` export carries neither
that store nor credentials; `.vcont` imports remain read-only evidence.

Project memory, tasks, and shared-context fetch receipts are currently local to this machine under
`~/.verb/work` (or `VERB_STATE_DIR/work`). Moving only the source ZIP to another machine does not
move that work store. See `docs/DESKTOP_CORE_INTEGRITY.md` for the verified invariants and
remaining boundaries.

This is deliberately not a credential broker, cloud service, remote process supervisor or transcript
sync layer. Broader assistant and transport work remains evidence-gated by the product roadmap.

## Continue on phone: desktop bridge preview

The TUI now creates an owner-only local bridge socket for each live hosted PTY. Its pairing offer is
one-use and expires after two minutes. A paired client can request a bounded current-screen snapshot,
take the input lease, send bounded input, disconnect, and reconnect while that exact desktop process
remains live. Desktop takeback and revocation clear queued phone input. A missing phone heartbeat
returns input control to desktop after 30 seconds. On process exit, the bridge and its tokens end.
Terminal bytes are held only in memory and are not added to durable session records or `.vcont`.

`verb mobile offer SESSION_ID` and `verb mobile request SESSION_ID` expose this **local process
protocol** for development. Requests are one bounded JSON line on stdin; secrets should not be put
in command arguments or shell history. The socket is accessible only to the current Unix account
and has no LAN listener. The existing Android app cannot receive this stream yet, so the Workbench
does not advertise a working Continue on phone action. A future mobile receiver must use an
authenticated, encrypted transport and render the screen/input states from the bridge. See
`docs/DESKTOP_MOBILE_CONTINUATION_PRODUCT.md` for the intended interaction and
`docs/DESKTOP_MOBILE_BRIDGE_PROTOCOL.md` for the versioned local contract.
