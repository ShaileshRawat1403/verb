# Brief: AI-assisted development in Verb Desktop

> Owner's direction, 2026-10-07. Builds on `SDLC_WORKBENCH_BRIEF.md` (specs, Git, audit trail) and
> `DESKTOP_TERMINALS_BRIEF.md` (real terminals). The rules there still hold: everything easy, nothing
> forced, auditability paramount. Record progress at the bottom.

## Order

1. ✅ Light and dark themes, layout and motion (`79484e5`)
2. ✅ Project context hub and agent-context sync (`31d11a8`), with a file tree
3. ✅ Host page (read-only health) (`78d7b83`, `0dc65b3`)
4. ✅ Ask Verb v1 (deterministic, cited; the model summary path is still to come)
5. ✅ Session board, handoffs, context meter (`65293fd`, this commit's successor)
6. ✅ Observer badges (6 of 7 signals; edit collision deferred, see progress log)
7. ✅ Modern terminal: command blocks, splits, tabs (`717aea9`, `8c3da16`)

Browser smoke tests grow with each step: there are none yet, and every UI check so far was manual.

## 2. Project context hub

**Nudge, never force.** One **Project Brief** to start: problem, users, scope, constraints, glossary.
It lives at `docs/project/BRIEF.md` in the repo, written through a guided form or by hand.

- Further documents (PRD, BRD, decision records, architecture notes) are offered when they would
  help, never required. For example: "3 specs now, and no brief: want to write one? (2 minutes)",
  or "this spec touches 4 areas; a short decision record would help reviewers".
- **The payoff is shared agent context.** Verb keeps one marked section of `AGENTS.md` and
  `CLAUDE.md` in sync from the hub: product summary, scope, constraints, the current spec and its
  open criteria. Anything outside the markers belongs to the user and is never touched:
  `<!-- verb:context:start --> … <!-- verb:context:end -->`
- Every regeneration is audited: what changed, and from which document.
- Specs link to the brief. Ask Verb and agents read the brief before answering or starting.

## 4. Ask Verb v1

Answers **only from evidence it can cite**: specs and their audit trails, the project brief, Git
(log, diff summaries, branches), the sessions list, and observer facts. Every answer cites its
sources.

- **Deterministic answers first**, for the common questions, with no model involved: "what's left on
  spec 012?", "what changed today?", "who is working on what?", "why did spec 9 skip review?" (from the
  audit trail).
- **A model only to summarise or explain**, through the user's existing agent login (`claude -p` or
  `codex exec`), so no new API key is needed. Inputs are redacted for secrets first (see the observer).
- Entry points: the command palette ("Ask Verb: …") and an Ask tab in the proof panel. Never a chat
  box that opens by itself.
- Ask Verb never types into terminals or edits files. It can suggest a palette action ("Start Codex on
  spec 012 with this handoff?") that the user runs.

## 6. Observer: approved boundaries, chosen use cases

**Boundaries (owner-approved):** opt-in per project, local-only, read-only, redacts secrets, shows
badges and never pop-ups.

**Use cases**, chosen for practical value and few false alarms:

| # | Signal | Detects | Evidence used | Badge says | Guard against noise |
|---|---|---|---|---|---|
| 1 | **Agent waiting for you** | An agent paused at a prompt (permission, y/n, a question) | The terminal's last screen lines (shell-integration marks plus known prompt patterns for Claude, Codex, Antigravity) | "Claude is waiting for your answer: Terminal 2" | Only after 60 s unanswered; one badge per prompt |
| 2 | **Agent stuck** | An agent process alive with no output and no file changes | Output timestamps and Git status | "Codex has been quiet for 12 min" | Only for agent sessions; at most once per 30 min |
| 3 | **Failing loop** | The same command failing repeatedly | Shell-integration exit codes (OSC 133) | "`npm test` failed 4× in 10 min. Ask Verb?" | 3+ failures of the same command within 15 min |
| 4 | **Edit collision** | Two live sessions changing the same file | Per-session changed files in the working tree or worktrees | "Claude and Codex both edited `src/auth.ts`" | Same file within 10 min, different sessions |
| 5 | **Context pressure** | An agent session near its context limit | Token usage in the agent's local session log (Verb already reads Claude and Codex logs) | "This Claude session is ~80% full. Hand off?" | At 80%, then again at 95% only |
| 6 | **Work on the wrong branch** | Changes accumulating on `main` while the selected spec has its own branch | Git branch plus the selected spec | "You're on main; spec 012 has its own branch" | Only once a spec has a branch and there are uncommitted changes |
| 7 | **Possible secret in a terminal** | A token, key or `.env` value printed on screen | Pattern matching on terminal output, done locally and never stored | "A secret may have shown in Terminal 1. Consider rotating it" | High-confidence patterns only (key prefixes, long high-entropy strings next to KEY/TOKEN/SECRET) |

**Best practices built in:**

- **Facts are ephemeral.** The observer keeps signals in memory only. Raw terminal text is never
  written to disk or sent anywhere. If the user acts on a badge (for example, a handoff), that action
  goes in the spec's audit trail; the observation itself does not.
- **Every badge explains itself.** "Why am I seeing this?" shows the evidence and the rule that fired.
- **Easy to quiet.** Dismiss, or "not for this session". Each signal can be switched off in settings.
- **Redact before anything leaves the observer,** including to Ask Verb's model: secrets are masked
  before any text reaches a model, even a local one.
- **No automatic actions,** ever. The most a badge does is offer a palette action.
- **Measured.** Count shown, dismissed and acted-on badges per signal, locally, so noisy signals can be
  tuned or dropped.

## 5. Sessions and context across agents

- **Session board per spec:** agent, state, last activity, files touched, context meter.
- **One-click handoff** (for example Claude → Codex): Verb writes a handoff note into the spec's audit
  trail from the last session's commits, the spec, open criteria and the previous agent's final
  message. The next agent starts with that note in its brief.
- **Context meter** from local session logs, feeding observer signal 5.

## 3. Host page (Node 1)

Inside Verb, behind the same login; not a separate app. Read-only in v1: battery and charging,
temperature, memory, storage, uptime, service states, deployed commit, and recent crashes from the
service log (for example, the unexplained SIGBUS on 2026-10-07). Restart and deploy stay in
the node's private operations repository until a helper outside the Verb process can do them safely.

## 7. Modern terminal

A real PTY always (`vim`, `tmux` and `ssh` keep working). On top of it: command blocks with exit
status and duration (from OSC 133), "Explain this failure" on a red block (Ask Verb), splits and
tabs, searchable history, and a notification when a long command finishes in a background tab.

## Agent stream (opt-in content), decided 2026-10-09

The owner chose to show agent sessions as a conversation (the flagship mockup's Mode 1). Until now
Verb read agent logs for structure only (`observe`, `meter`). The rule is now: **structure by
default; content only when the person turns the stream on for a project.** `desktop/src/transcript.rs`
is the only reader of content, and it:

- is off until turned on per project (`stream.json` in the identity store), and can be turned off
  from any stream;
- reads on demand (the tail of the agent's own log), stores nothing, writes nothing to the audit
  trail, and returns only to the authenticated browser, never to a model;
- redacts keys, tokens, private keys and `password=`-style values before anything leaves it;
- is bounded (2 MB of log, 300 items, 6000 characters per message).

The composer types into the agent's own terminal (a bracketed paste and Enter): nothing is read to
send. Claude Code and Codex are supported, including Codex's newer `exec` script tool; other agents
keep their terminal only.

## Progress log

- 2026-10-10, Claude: Antigravity Talk, optional. agy's JSON mode gives a clean conversation stream,
  but headless agy cannot ask permission (auto-denies, even with --sandbox), so Verb drives it in
  plan mode with --add-dir and offers "Continue in terminal to approve" (same conversation via
  --conversation) when a turn ends on a refused action. Never --dangerously-skip-permissions, never
  allow-rules written for the person. Live result was weak (agy claimed success without changes), so
  agy starts in its terminal like every CLI agent and Talk is a secondary "Plan in Talk".

- 2026-10-09, Claude: closing the mockup and the user-test gaps. "Needs you" on agent tabs now works
  without the observer (owner's direction): it is the agent explicitly asking permission, so only
  that is reported for everyone; every other observer signal stays opt-in. A "Commit N files"
  primary in the top bar; the composer shows agent, spec and context; when an agent says a criterion
  is met, the stream offers "Mark proven", which opens the evidence dialog prefilled with its words
  (never automatic). Antigravity gets the composer but no stream: its conversations are protobuf
  blobs inside SQLite databases agy holds open, undocumented and liable to change, so reading them
  would be guessing. A future option is driving agy through its `--output-format stream-json` mode.

- 2026-10-09, Claude: inline diffs (`777d0c8`) and the agent stream with its composer. Verified end
  to end with a stand-in agent and a realistic Claude log in a temporary home: requests, replies and
  folded steps render, a fake token in a command came back redacted, and the composer's message
  reached the agent's input. Parsers were also checked against real Claude and Codex logs on the
  Mac (counts only).

- 2026-10-09, Claude: step 7 done. Command blocks from Verb's OSC 633 marks (edge per command, time
  over 0.1 s, exit code on failure, Copy, and "Ask agent", which pastes a factual note into an open
  agent's prompt and never presses Enter); Cmd/Ctrl+Up/Down between commands; a tab strip with
  status dots, Split/Single, and + for a new terminal. Changed from the plan: no "Explain this
  failure", because Ask Verb uses no model; the hand-off to an agent the user already runs replaces
  it. Not yet: a notification when a long command ends in a background tab. Also: the new monochrome
  design, a responsive top bar and spec layout by panel width, and three flaky tests fixed (bash
  alias labels, macOS non-blocking accept, timing bounds).

- 2026-10-08, Claude: observer shipped with six signals: waiting, stuck, failing, secret (server, from
  memory) and context, branch (browser). Edit collision is deferred: in a shared checkout Verb cannot
  attribute an edit to a session, so it would be a guess. It needs per-session worktrees first.

- 2026-10-08, Claude: session board (from audit-trail session lines), handoff with a dated note in
  the spec and a brief that points the next agent at it, and a context meter (Codex: % of its
  recorded window plus rate limits; Claude: tokens only, since its window is not recorded). Found
  and fixed a CSP rule that had silently disabled terminal images.

- 2026-10-08, Claude: Ask Verb v1. Six evidence-only intents (what's left, what changed, where are we,
  history/why, what is this project, who is working on what), each answer citing specs, files,
  commits or sessions; honest decline otherwise. Dialog, Alt+A, palette entry. No model yet.

- 2026-10-08, Claude: Host page. Verb version, deployed commit (from deploy manifests), uptime from
  process start, memory; machine model; memory and storage; temperatures; battery or why not; runit
  services; restart/crash timeline. Proot's fake uptime and load are hidden with a note. A temporary
  second instance on the OnePlus found two bugs (a manifest-less folder hid the commit; services
  outside runit showed "fail"), both fixed and re-verified there. Battery charge needs Termux:API.
- 2026-10-07, Claude: context hub (Project Brief, agent-context sync, nudges) plus a file tree that
  never lists or opens Git-ignored files.

- 2026-10-07, Claude: step 1 shipped (`79484e5`): theme tokens, light and dark, xterm palettes that
  pass a contrast test, motion layer with reduced-motion support. Verified in Chrome.
