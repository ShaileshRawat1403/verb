# Capability architecture map: Verb as a full development workspace

Owner's direction, 2026-10-27. Revision 2, after architecture review. This supersedes the "not an
IDE" row in `POSITIONING.md` section 2. That document still needs rewriting; this map does not edit
it. "For non-developers" describes how people interact with Verb: intent, conversation, visible
results and plain explanations, with advanced controls still reachable. It does not mean a smaller
set of capabilities.

**Audited code.** The Studio worktree is at
`/Users/ananyalayek/.codex/worktrees/verb-git-deadlines/verb`, branch
`codex/production-git-read-deadlines`, HEAD `3f813a6`, and its changes are uncommitted. Paths below
are relative to `desktop/` in that worktree unless stated otherwise. `src/agent_chat.rs` is identical
in this checkout.

**Research basis.** Verb's code; the owner's observation of Claude.ai and the Codex desktop app;
public product behaviour. No proprietary source from Claude or Codex was inspected. REA has so far
been used only on copies of Verb's own Studio modules (`STUDIO_CONSOLE_DESIGN.md`). It is held back
for specific interoperability questions.

Status words used in this document:

- **Exists**: code and tests are present.
- **Partial**: some code exists, with gaps that are named.
- **Integrable**: the parts exist elsewhere and need wiring together.
- **Must build**: nothing usable exists yet.
- **Undecided**: needs evaluation before anyone commits to it.

---

## 1. Conversation: Partial

Three separate paths exist. None of them gives complete, reliable read and send for all three
agents: Claude Code, Codex and OpenCode.

| Path | Agents | Read | Send | Gating and privacy |
| --- | --- | --- | --- | --- |
| `src/agent_chat.rs` (435 lines), used by `src/tui/term.rs:150,191` → `src/mobile.rs:390` (`update_chat`) → phone `DesktopPhoneScreen.kt` | **Claude only** (`create_adapter` matches `"claude"`, `:41`) | Incremental tail of `~/.claude/projects/<slug>/<id>.jsonl`. The session is found by resume ID, or by `~/.claude/sessions/<pid>.json` plus a pid/`getsid` match (`:76-149`). Thinking blocks and wrapper tags are dropped. | No chat-specific send. The phone types raw PTY bytes under the control lease (`submit_input`, `:301`). | **Not gated by the stream opt-in, and not redacted** (see defects below). |
| `src/transcript.rs` (776 lines), via `GET /api/terminals/{id}/stream` (`src/web.rs:2405`) | Claude and Codex (`for_session`, `:574-611`). **No OpenCode reader.** | Bounded tail: 2 MB and 300 items. Items are `you`/`agent`/`step`, with the tool, its target and whether it failed. Text is redacted. | None (read-only). | Off by default for each project (`stream.json`). Checked at `web.rs:2408`. This is the sanctioned content reader. |
| `src/talk.rs` (661 lines), via `POST /api/specs/{id}/talk` (`web.rs:2274`) | **Antigravity only** | agy's documented NDJSON print mode, including `text_delta` streaming. | Yes. Verb runs one headless turn per message and continues with `--conversation <id>`. | Runs in plan mode and reads only. Anything that needs permission is refused, and Verb offers to continue in agy's terminal. It never uses `--dangerously-skip-permissions` (`:9-14`, test `:656`). |

**OpenCode.** `src/agents.rs:202-230` reads only session IDs from
`~/.local/share/opencode/opencode.db`, to decide whether a session can be resumed. There is no
conversation reader and no send path.

### Defects found in the reusable path (`agent_chat.rs`)

Fix these before anything is built on top of it.

1. **Privacy contract.** It reads conversation content without checking `transcript::load(store).enabled`.
   It applies no `redact()`, then sends the text to the phone. This breaks the rule that
   `transcript.rs` is the only content reader and that it runs only after opt-in. It also breaks the
   README's "structural memory" claim.
2. **Torn lines are lost.** `BufReader::lines()` consumes a partial trailing line while the agent is
   still writing it (`:203-258`). The bytes are counted even when the JSON fails to parse, so that
   message is skipped permanently. This matters most when streaming.
3. **Timestamps are wrong.** `parse_timestamp` returns the current time for ISO `timestamp` fields
   (`:313-318`). Claude's transcripts use ISO, so every message shows when it was read rather than
   when it was sent. The fix is to parse ISO timestamps properly. A timestamp that cannot be parsed
   must show as *unavailable*. It must never be replaced with the current time (requirement R4).
4. **"Awaiting approval" is guessed from screen text.** It matches substrings such as `"Esc to cancel"`
   and `"[y/N]"` (`:266-271`). It is a heuristic, open to false positives and negatives, and it is
   not an enforcement point (see §2).
5. **Input gating is advisory and fails open.** `can_send_prompt` only disables the phone's button.
   `submit_input` never checks it, and the Android client treats a missing field as `true`
   (`DesktopBridgeClient.kt:136`). Today one channel carries both chat messages and raw terminal
   keystrokes, so gating one would gate the other. Requirement R1 separates them.

### What can be reused

- The adapter trait shape (`poll_chat`), the method for discovering Claude sessions, the filtering
  of thinking blocks and wrapper tags, and the phone's chat state fields.
- `transcript.rs`'s Claude and Codex parsers, redaction and bounds. These should become the **single
  content source**. `agent_chat.rs` should consume them, not run a parallel parser.
- `talk.rs`'s turn model: an explicit conversation ID, streaming deltas, and refusals turned into a
  terminal handoff. This is the reference for "send a supported message" whenever a provider has a
  documented structured mode.

### Must build

- An OpenCode content reader. First check whether a documented export or API exists. Reading
  OpenCode's SQLite directly is a last resort.
- A **send contract** for each agent, stating which of these is actually supported:
  - (a) structured headless turns, like `talk.rs`;
  - (b) typing into the existing PTY, where delivery is confirmed only when the transcript shows the
    user turn;
  - (c) unsupported, so the user falls back to the terminal.
- **Sending is not the same as receiving.** Bytes written to a PTY are "sent". A message counts as
  received only after the agent's own log echoes it.

### Requirements R1–R4: chat input, delivery, continuity, stream off

These apply to every agent, to the desktop and the phone, and to the first slice.

**R1. Keep chat input separate from terminal control.**

- These are two channels with two different authorities:
  - a **chat send** carries a whole message, with an ID, that Verb composes for the agent;
  - **terminal input** carries raw keystrokes under the existing control lease (`mobile.rs`
    `submit_input`, `write_desktop`).
- Chat sends are checked on the server. They are refused, with a stated reason, unless the
  adapter reports *ready* from evidence. Unknown, uncertain, working, approval-required, a missing
  field, or no adapter all mean **no**. This fails closed.
- Terminal input is **not** subject to that readiness check. Whoever holds the lease can always:
  - interrupt execution (Ctrl-C, Esc);
  - answer a provider's own permission prompt;
  - type a recovery command.

  The lease, the token scopes and the existing input bounds stay the only gates on terminal input.
  Blocking it on "readiness unknown" would trap the person exactly when they need control.
- The UI always shows which channel an action uses. "Open in terminal" is one step away in every
  state.

**R2. Handle uncertain delivery explicitly.**

- Each chat send gets a Verb message ID and a state: `draft → sending → sent (written to the
  PTY) → confirmed (the agent's log shows a new user turn, after the send, in this conversation)`.
  It can also end as `failed`, `unknown` or `abandoned`.
- If the connection drops, or Verb restarts, between `sending` and `confirmed`, the state becomes
  **unknown**: the message may or may not have reached the agent.
  - Verb **never resends it automatically**.
  - The text and the pending state are kept.
  - The person chooses one of three actions:
    - "Check the conversation": re-read the log;
    - "Send again": an explicit new attempt with a new ID, warning that it may arrive twice;
    - "Discard".
- **Matching on text alone is not enough to confirm delivery.** A confirmation must tie the
  message to the agent's log by every one of these:
  - the conversation ID;
  - a log position after the offset recorded at send time;
  - the turn's own ID (`uuid`) and timestamp;
  - matching text.

  If any of these cannot be established, the message stays *unknown*. It does not become
  *confirmed*. An identical earlier message must never confirm a new one.

**R3. Keep conversation continuity separate from process continuity.**

- There are three separate identities:
  - the **Verb session**: the durable record with its own ID;
  - the **provider conversation**: the agent's own ID, such as Claude's `sessionId` or a Codex
    rollout ID;
  - the **execution attempt**: one process launch, with its start time and end reason, inside a
    session.

  All three are visible and kept separately.
- These are different events, recorded and displayed differently:
  - a browser or phone **reconnecting** (same process, same conversation);
  - an agent process **restarting** (a new attempt, perhaps the same conversation);
  - a conversation **resuming** (the provider confirms the same conversation ID);
  - a **new conversation** started inside the same Verb session.
- If Verb cannot prove that a new attempt continues the earlier conversation, it says *continuity
  not established*. Examples: ambiguous log discovery, a mismatched ID, a missing log. In that case
  it does not merge the histories. This matches the existing rule that an unidentifiable record
  stays interrupted rather than resuming the latest conversation (README, "What works today").

**R4. Make "stream off" reversible and immediate.**

- Turning the stream off for a project takes effect at once, without a restart.
  - Further content reads stop: adapters check the setting on every poll, not only at start-up.
  - Cached conversation content is cleared from Verb's memory, from the browser presentation and
    from the phone bridge state. The panes go back to structure-only views.
  - Pending chat drafts are kept, because they are the person's own words. Sending is unavailable
    while the stream is off.
- It **never** deletes or changes the agent's own logs, or anything under `~/.claude` or
  `~/.codex`.
- Turning it back on re-reads from the agent's records, subject to the same bounds.
- Historical timestamps that cannot be determined show as *unavailable*. They are never replaced
  with the current time.
- **Stale responses must not refill the view.** Each content read carries the opt-in generation
  that was current when it started. Turning the stream off increments that generation. A read that
  finishes under an old generation is dropped by the server, the bridge and the client, so a read
  already in flight cannot bring conversation text back after opt-out.
- **"Turned off here" is not "cleared everywhere".** A disconnected phone or browser cannot be
  made to clear its screen at once. The UI shows two separate facts:
  - "Stream turned off on this device";
  - for each other client, whether it has acknowledged the change.

  A client that has not acknowledged shows as *not yet confirmed cleared* until it reconnects and
  receives the new generation.

**Confirmation stays honest when evidence is missing.** Verb never invents a provider conversation
ID, a turn ID or a timestamp, and never treats a guessed value as evidence. If the adapter cannot
link a send to the agent's log, the message stays *delivery unknown* and the terminal fallback is
offered.

## 2. Approvals: three separate authorities, none of them unified yet

These must not be merged into one switch.

| Authority | Who enforces it | What Verb has today | What Verb can truthfully claim |
| --- | --- | --- | --- |
| **Provider permission**: may the agent run this tool? | The agent CLI (Claude Code permission modes, Codex approval policy and sandbox, agy plan mode) | Agents are launched without bypass flags. `talk.rs` runs agy in plan mode. `agent_chat.rs` *guesses* from screen text that a prompt is showing. | Verb can **observe** that a prompt seems to be visible, and can pass the person's keystrokes through. It cannot enforce or widen a provider permission, and it cannot see a prompt the provider does not render. |
| **Verb authorization**: may this client act on this session or project? | Verb: token scopes (read/drive/owner) in `src/tokens.rs`, the phone control lease (`src/mobile.rs`), Studio's capability checks | Exists and enforced, at session and endpoint level. | This is the only layer where Verb **is** the enforcement point. |
| **Human delivery approval**: is this work accepted for release? | Verb's SDLC ledger | `src/sdlc.rs:781-795`: a `review` record can be approved only when every criterion's evidence has been accepted. It requires a written note. `delivery` records require an approved review first (`:820-822`). | Exists and enforced, for review and delivery records. |

**No auto-approve control in this slice.** Before anyone proposes one, it needs a written contract.
That contract must state:

- its scope: which provider, session, tool class, project and time limit;
- its enforcement point. The provider's own setting is the only honest one, and Verb must never
  write allow-rules into the person's settings (`talk.rs:10`);
- what is audited;
- what happens on failure: an unknown prompt shape, a lost connection or a stale screen must fall
  back to "ask the human in the terminal", never to "approve".

Detecting a prompt on screen is not grounds for answering it.

## 3. Git: mostly exists, using the system git CLI

There is no `git2`, `gix` or `libgit2` dependency in `desktop/Cargo.toml`. All Git calls go through
`crate::exec::git` (the system binary, inside the execution boundary). Revision 1 of this map was
wrong to suggest libgit2.

| Operation | Route / code | Behaviour |
| --- | --- | --- |
| Summary (branch, changed files) | `GET /api/git` → `specs::git_summary` (`specs.rs:633`) | Runs off the request loop |
| File diff | `GET /api/git/diff?path=` → `diff::file_diff` | One file at a time, read-only. Reused by Studio. |
| Implementation diff for a Spec | `GET /api/specs/{id}/sdlc-diff` → `sdlc::implementation_diff` | |
| Commit | `POST /api/git/commit` → `specs::commit` (`specs.rs:708`) | Requires a message. Runs **`git add --all`**, then commits. The Spec's audit line lands in the same commit. **No selective staging.** |
| Switch to a Spec's branch | `POST /api/specs/{id}/branch` → `specs::switch_to_branch` (`:740`) | Refuses if there are uncommitted changes. Recorded. |
| Reviewed remote transfer | `GET/POST /api/git/transfer` → `git_sync::review` / `transfer` (`git_sync.rs:90,256`) | Actions are `check` (fetch only the tracking ref), `upload` (fast-forward push, only when ahead and not behind) and `download` (`merge --ff-only`, only when behind and not ahead). The reviewed state must still match at execution time. One transfer at a time. No tags, no force, no prune. Diverged branches are blocked. |
| Remote observation record | `sdlc` `remote` kind (`sdlc.rs:797-810`) | Ties the local commit to the remote state |

**Missing (must build):**

- staging selected files or hunks;
- creating, listing or deleting branches outside a Spec;
- browsing history and log (only the outgoing and incoming lists exist);
- reconciling diverged branches (non-fast-forward merge or rebase);
- conflict detection and a resolution UI;
- recovery actions: revert, restore a file, stash, set work aside;
- hosted pull requests and CI status. `SDLC_WORKBENCH_BRIEF.md` decision 5 says "Git-only for now".

Each of these should extend the existing pattern: review first, compare at execution time, never
force.

## 4. Delivery truth: SHIP is not a commit

The SDLC ledger already keeps these facts separate. The workflow must show them separately and
must never let one stand in for another.

1. **Stage transition.** Moving a Spec to `ship` (`specs.rs:18`) is a label. Gates warn but never
   block. `ship_state` (`:496`) warns about uncommitted changes and about a Spec branch not merged
   into the default branch.
2. **Commit**: `POST /api/git/commit`.
3. **Merge to the main line**: observed by `merge-base --is-ancestor` in `ship_state`.
4. **Remote**: the `remote` record, made through a reviewed transfer.
5. **Review approval**: the `review` record. It requires every criterion to be accepted.
6. **Delivery**: `delivery` records with milestone `artifact` | `deployment` | `health`
   (`sdlc.rs:820-840`). A health report is refused unless a successful deployment to the *same
   target* was recorded first. Freshness is checked against the current commit and intent, and
   labels such as "Deployment reported · fresh health report required" (`:414`) say so.

This sprint does no deploying, so the workflow ends at **approved review, plus a local commit, with
delivery shown as "Release not observed"**. That is a truthful state. It is not "shipped".

## 5. Editor: Undecided

Today `src/web/studio-editor.js` uses a plain `<textarea>`. Its comment calls it "an adapter, not a
replacement for the full IDE foundation". `STUDIO_WORKSPACE_CONTRACT.md` already names VS Code-derived
and Theia as candidates. Revision 1 of this map preferred CodeMirror 6 on bundle size alone, which
let approachability set a capability ceiling. That preference is withdrawn.

Evaluate every candidate (CodeMirror 6 with an LSP client, Monaco, a VS Code-derived build, Theia)
against the full roadmap:

| Criterion | Question |
| --- | --- |
| Language tooling | LSP client maturity: completion, diagnostics, go-to-definition, rename and refactor, workspace symbols |
| Debugging | Can a DAP client be integrated, with breakpoints, stepping and variables? |
| Extensibility | Is there an extension model? Can existing extensions be used (Open VSX licensing)? |
| Search | Project-wide search and replace across files |
| Authority model | Can the editor run with **no independent host access**, with every read and write going through Verb's workspace boundary (the revision-checked save)? |
| Phone | Touch editing, small screens, input methods |
| Accessibility | Screen reader support, keyboard-only use, high contrast |
| Footprint | Bundle size and memory on an Android host running proot |
| Approachability | Can advanced features stay out of the way without being removed? |

The outcome is an evaluation record, not an assumption. The `<textarea>` stays until that record
exists.

## 6. Other capabilities

| Capability | Status | Notes |
| --- | --- | --- |
| Three-pane console, 8 preview tabs, resize and keyboard navigation | Exists (uncommitted) | `src/web/studio-ui.js`, `studio-previews.js` |
| Image preview; sandboxed single-file HTML app preview | Exists (uncommitted) | `/api/studio/image`, `/api/studio/preview`; strict CSP; one-use tickets |
| Multi-file app preview, dev server, hot reload | Must build | `STUDIO_WORKSPACE_CONTRACT.md` refuses an arbitrary localhost proxy. Any dev-server preview must stay inside the same boundary and ticket model. |
| Terminal in the console | Integrable | Reuse the existing PTY and xterm.js path. Never create a second terminal store. |
| Session lifecycle across process death | Exists | `LIVE → INTERRUPTED → RECOVERABLE → ENDED`. Claude, Codex and OpenCode adapters. |
| Spec workflow, evidence, acceptance, review, delivery ledger | Exists | `specs.rs`, `sdlc.rs` |
| Project conventions ("memory") | Integrable | Structured project files, inspectable like Specs. Never stored conversation text. |
| Scheduled or recurring work | Must build | Needs its own authority contract (which session, which permissions, what a run may change) before any UI |
| CI status, hosted pull requests | Must build | Read-only first |
| Extensions, deployment workflows | Out of scope this sprint | |

---

## 7. Brief for Claude.ai: one complete workflow design

Paste this section into Claude.ai. Codex reviews the proposals.

> Design one complete Verb desktop workflow and its phone counterpart, for a non-developer working
> with an existing coding agent. Layout: project navigation on the left, conversation and work in the
> centre, resizable tabbed preview on the right (files, images, single-file app, Git diff), and a
> terminal that is always reachable.
>
> **Journey:** pick an existing agent session → read its real conversation → send a message → watch it
> work → inspect what it changed → review the diff → commit locally → see the honest delivery state.
>
> **Every one of these states must be designed, not only the happy path:**
> - *waiting*: the agent is idle, so input is allowed.
> - *streaming*: the agent is replying or running tools. Show steps (Read, Edit, Bash) separately
>   from prose.
> - *sent, not yet confirmed*: the message reached the terminal, but the agent's log has not echoed it.
> - *delivery unknown*: the connection dropped before confirmation. Never resend automatically.
>   Keep the message visible as pending, and offer "Check the conversation", "Send again (may
>   duplicate)" and "Discard".
> - *approval required*: the agent's own permission prompt is showing. Verb cannot answer it in chat.
>   Show it, explain it, and offer "Open in terminal to approve".
> - *interrupted*: the agent process ended. The session is recoverable or not, according to evidence.
> - *reconnecting*: the phone or browser lost its connection. Keep the identity, any drafts and the
>   last confirmed message visible.
> - *failed*: a tool failed, a send failed, or the log could not be read. Each needs a distinct
>   recovery action.
> - *unsupported*: this agent has no readable conversation, or does not support sending. Show the
>   terminal fallback.
> - *stream off*: the person has not opted in for this project, or has just turned the stream off.
>   Content disappears at once, structure stays, the agent's own logs are untouched, and drafts are
>   kept. Explain what turning it on reads.
> - *continuity*: show the Verb session, the provider conversation and the execution attempt as
>   separate things. A reconnect, a process restart, a conversation resume and a new conversation
>   each look different. Show "continuity not established" when Verb cannot prove it.
>
> **Two channels:** the chat composer (blocked unless the agent is confirmed ready) and the terminal
> (always available to whoever holds control: interrupt, answer a permission prompt, recover). The
> person must always be able to tell which one they are using.
>
> Timestamps that cannot be determined show as "time unavailable", never as the current time.
>
> **Truth labels** must keep these separate: sent / received / running / saved / committed / merged /
> pushed / review approved / deployed / healthy. "Ship" must not appear as done when only a commit
> exists.
>
> **Constraints:** no auto-approve switch. No fabricated messages. Content shows only after the
> per-project opt-in. Keyboard-accessible. The phone layout switches surfaces rather than shrinking
> the desktop layout. No dashboard cards, no decorative AI styling. Design only: no deployment, and
> nothing that touches PocketFabric Node.
>
> Mark each element as *exists*, *integrable* or *proposed*, using the architecture map.

## 8. The first implementation slice, after the UX review

**Goal:** select an existing session → read the actual conversation → send a supported message →
observe execution → inspect the output → reconnect without losing identity.

**Scope:** Claude first (the only agent with a resolved session, a transcript and PTY input today),
then Codex. Reuse the existing PTY. The terminal stays a one-step fallback.

Before it starts:

1. Route `agent_chat.rs` content through `transcript.rs`: one parser, the opt-in check, and redaction.
2. Fix torn-line handling. Hold back any trailing fragment until its newline arrives.
3. Fix ISO timestamp parsing. A timestamp that cannot be parsed shows as unavailable (R4).
4. Add a chat-send endpoint, separate from raw terminal input, that is checked on the server and
   fails closed when readiness is unknown (R1). Raw terminal input stays governed by the lease only.
5. Add message IDs and delivery states, including *unknown*, never resend automatically, and
   confirm only with the correlation rule (R2).
6. Record the Verb session ID, the provider conversation ID and the execution attempt ID
   separately, along with distinct reconnect, restart and resume events (R3).
7. Make stream-off take effect on the next poll and clear presented content everywhere (R4).

**Acceptance:** run on a real Claude session on the desktop and on the paired phone, following the
memory rule "green tests never mean working". Record the run with its date.

- Drop the phone's connection, then reconnect. Expect the same attempt and the same conversation,
  with no new attempt recorded.
- Kill and restart the agent. Expect a new attempt, the same Verb session, and the conversation
  either confirmed as resumed or shown as "not established".
- Cut the connection between send and confirmation. The message must show as *unknown*. Then:
  - "Check the conversation" resolves it;
  - no automatic resend happens;
  - an earlier identical message does not confirm it.
- While readiness is unknown or an approval is required:
  - chat send is refused by the server;
  - Ctrl-C and answering the permission prompt through the terminal still work.
- Turn the stream off while viewing. Content clears from the desktop and the phone on the next
  poll, the agent's logs stay byte-identical, and turning it on restores the view.

No deployment. No PocketFabric Node changes.
