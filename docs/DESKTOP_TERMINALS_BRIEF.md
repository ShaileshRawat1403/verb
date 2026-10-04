# Brief: real terminals in Verb Desktop

> **For the agent executing this (Antigravity first; Claude or Codex may continue).** Read this
> whole file before touching code, and re-read it at the start of every session on this work. It is
> the owner's direction, written 2026-10-04. Record progress in the **Progress log** at the bottom
> rather than editing the direction above it.

## The owner's goal, in their words

> "I want Verb to provide lightweight terminals where multiple projects can work. These should be
> terminals like VS Code or Zed provides, where anything can run."

Decisions the owner has made:

1. **Native web UI**: Verb Desktop's existing browser workbench. No Tauri or Electron window.
2. **Streaming first**: fix how terminal bytes travel before anything else.
3. **Terminals will one day run on another machine.** Design the protocol for that now; do not
   build the remote part yet.
4. **Most important: they must have every feature a real terminal has.** A Verb terminal that
   cannot do what iTerm2, the VS Code terminal or Zed's terminal does is a bug, not a limitation.

## What exists today (read before changing anything)

| Piece | Where | State |
|---|---|---|
| PTY host | `desktop/src/pty.rs` (`forkpty`) | Real PTYs. Lives in the same process as the UI server. |
| Screen model | `vt100` crate | Keeps a parsed copy of every terminal. Used for phone continuation and snapshots. |
| HTTP server | `tiny_http` (vendored and patched: `desktop/vendor/tiny_http/VERB_PATCH.md`) | One fixed project per server process (`project: PathBuf` in `web.rs`). |
| Terminal API | `desktop/src/web.rs`: `/api/terminals/{id}/output`, `/input`, `/resize`, `/control`, `/phone` | **Polling.** Output is fetched on a timer (`pollTerminal` in `web/src/app.js`); every keystroke is a separate POST. |
| Renderer | `desktop/web/src/app.js`, `@xterm/xterm` and `@xterm/addon-fit` | The renderer VS Code uses. Fine; keep it. |
| Model | A plain shell is a *session* (`"shell"` alongside `claude`, `codex`, ...) | Every shell gets session records, identity and event logs. Too heavy for "run `npm test`". |

Why it does not feel like VS Code yet: polling adds latency to every keystroke and caps throughput;
one project per server; shells carry agent-session weight; output passes through `vt100` on the hot
path.

## Non-negotiable constraints

- **Do not break what depends on Desktop today.** The phone bridge
  (`docs/DESKTOP_MOBILE_BRIDGE_PROTOCOL.md`), the session contract (`docs/VERB_SESSION_CONTRACT.md`)
  and agent launch/resume must keep working at every commit. Their tests stay green.
- **Build on what exists:** the Rust server, `pty.rs`, xterm.js. No framework rewrite.
- **One geometry authority** (owner's rule, 2026-09-12): the host owns the PTY. Exactly one active
  driver owns input and PTY size at a time. Observers never resize. Control transfer is an explicit
  state change followed by a single resize.
- **Phone observers get rendered screen state from `vt100`, not raw PTY bytes.** Raw bytes go only
  to the active driver's xterm.js.
- **Security:** bind to `127.0.0.1` only. Every WebSocket upgrade must carry the same auth the HTTP
  API requires, and check `Origin`. A terminal socket is a remote shell; treat it like one.
- **Remote execution is gated.** `docs/BACKLOG.md` G4 requires a separate product and threat-model
  approval before any cross-machine transport. Phase 4 below is design only until the owner
  approves it.
- **Never touch the Android app** (`app/`) as part of this work.
- **CI must pass on every push:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`
  (CI uses *latest stable* Rust, newer than Homebrew's, so lints can appear only on CI), the web
  tests, and the Android jobs.

## Phases

Ship each phase as its own commits with tests. Do not start a phase until the previous one meets
its acceptance criteria. Record evidence in the progress log.

### Phase 1: streaming (do first)

Goal: typing and output feel instant; heavy output never freezes the UI.

1. Add a WebSocket endpoint for terminals. `tiny_http` supports `Request::upgrade`; pair it with
   `tungstenite`. If that cannot be made robust, run a dedicated loopback WebSocket listener;
   record why in the progress log.
2. Protocol: **one socket per browser tab, multiplexed by terminal id.** Binary frames for PTY
   bytes in both directions, JSON text frames for control (`resize`, `signal`, `exit`, `title`,
   `cwd`, `bell`). Version the protocol (`{"v":1}` on connect) so remote hosts can speak it later.
3. Hot path: PTY read → socket → `term.write()`. **`vt100` gets a copy off the hot path**, for
   snapshots and phone observers only.
4. **Backpressure:** when the socket's send queue passes a high-water mark, stop reading from that
   PTY until it drains. Never buffer without bound. xterm.js `write(data, callback)` drives client
   acknowledgements.
5. Keep the polling API working until every caller has moved, then delete it in a separate commit.
6. Turn on `@xterm/addon-webgl`, with fallback to the DOM renderer when WebGL is unavailable.

**Acceptance (measure, do not estimate):**
- Keystroke to glyph locally: median < 10 ms, p99 < 30 ms. Measure with a timestamped echo test.
- `seq 1 1000000` completes, the UI stays responsive during it (a second terminal accepts input),
  and memory stays bounded.
- `yes` running for 60 s does not grow server or browser memory without bound.
- Closing the browser tab does not kill terminals; reopening reattaches with the screen intact.

### Phase 2: everything a real terminal does

This is the owner's most important requirement. Each item needs a test, or a recorded manual check
with the command used.

**Rendering and emulation**
- `TERM=xterm-256color`, `COLORTERM=truecolor`; 16, 256 and 24-bit colour.
- Bold, italic, underline styles (single, double, curly, coloured), strikethrough, dim, inverse,
  blink.
- Unicode: wide CJK, emoji (including ZWJ sequences), combining marks. Ligatures optional
  (`@xterm/addon-ligatures`).
- Alternate screen: `vim`, `less`, `htop`, `top`, `tmux`, `nano`, `lazygit` and full-screen agent
  TUIs enter and leave it cleanly.
- Cursor shapes (block, bar, underline, blinking), via DECSCUSR.
- OSC 8 hyperlinks, clickable. Plain URLs clickable too (`@xterm/addon-web-links`).
- OSC 0/2 window title, shown on the tab. OSC 7 current directory, tracked.
- OSC 52 clipboard (allow-listed), OSC 133 shell-integration marks (prompt, command start and exit
  code), bell (visual, with an unseen-activity badge on background tabs).
- Images: Sixel and iTerm2 inline images via `@xterm/addon-image`. Last priority in this phase,
  but in scope.

**Input**
- All keys and modifiers, including Alt/Meta as Escape-prefix, Option-as-Meta on macOS
  (configurable), function keys, Home/End/PageUp/PageDown, and keypad application mode.
- Ctrl-C/Z/D/\\ deliver the right signals to the **foreground process group**.
- Bracketed paste; a confirmation before pasting multi-line text into a shell prompt (configurable).
- Mouse reporting (SGR 1006) for `vim`, `htop` and `tmux`; Shift-drag selects text while an app
  owns the mouse.
- IME composition (CJK input) works.
- Copy on select (optional), Cmd/Ctrl-C copies when there is a selection, right-click menu.

**Behaviour**
- Resize reflows correctly. SIGWINCH reaches the app. Only the driver resizes (see constraints).
- Scrollback: configurable, default 10,000 lines; search with `@xterm/addon-search`, regex
  supported.
- Exit status on the tab; an exited terminal keeps its final screen until closed.
- Closing a tab kills the whole process group (SIGHUP, then SIGTERM, then SIGKILL after a grace
  period). No orphans: verify with `ps` after closing a tab running `sleep 1000 &`.
- Login shell by default (the user's `$SHELL -l`), so their PATH, aliases and prompt load. A known
  Verb lesson: a probe that skips startup files lies about readiness.
- Environment: inherit the user's, plus `VERB_*` identity variables.
- Font family, size and line height configurable; zoom with Cmd/Ctrl +/-.

**Acceptance:** run `vttest` (screens 1 to 3 at least) and record the results. Pass a written
checklist with real apps: `vim`, `htop`, `tmux` (including mouse), `less +F`, `git log` through the
pager, `npm run dev` with colour, `python3` REPL, `ssh localhost`, `claude`, `codex`, `agy`. Any
failure is fixed or logged as an open defect with a reproduction.

### Phase 3: many projects, light terminals

1. **Separate Terminal from Session.** A `Terminal` is a PTY in a directory: id, project, cwd, size,
   title, state. A `Session` (agent conversation, identity, events, resume) attaches to a terminal
   only when an agent is launched in it. Plain shells create no session records.
2. **Multi-project server:** one Desktop process holds a list of open projects (Git-first identity,
   as in the rest of Verb). The UI has a project sidebar, terminal tabs per project, splits
   (horizontal and vertical), and drag to reorder.
3. **Persistence:** reopen remembers projects, tabs, splits, titles, cwd and scrollback (capped). It
   does not pretend processes survived a restart of the host; it says so.
4. **Cost:** an idle terminal costs its PTY and its scrollback, nothing polls. Measure 20 idle
   terminals across 4 projects (memory, CPU over 60 s) and record the numbers.
5. Verb's layer stays opt-in: "Launch agent here", "Continue on phone", and Git status in the
   sidebar, never injected into the terminal itself.

### Phase 4: other machines (design now, build only after approval)

Write `docs/DESKTOP_TERMINALS_REMOTE.md` covering:
- Separating the host from the UI process (`verb-host` vs the UI), which is already the owner's #2
  architecture priority, so terminals outlive the UI.
- The Phase 1 protocol over an authenticated transport (SSH tunnel first; paired-device TLS like the
  phone bridge later).
- The trust model: device trust (paired or revoked) is separate from session authority (see,
  observe or drive). The owner's stages: SEE, then OBSERVE, then DRIVE, then CONTINUE HERE.
- Threat model, for owner approval. **No remote code until it is approved.**

## Working rules

- Run tests yourself and state results plainly. Green unit tests are not proof: open the workbench
  in a browser and use the terminal as a person would before claiming a phase is done.
- Small commits, each passing `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`.
- Do not merge to `main`, tag, or release. The owner does that.
- If something in this brief turns out to be wrong, say so in the progress log with the evidence.
  Do not quietly do something else.

## Progress log

Newest entry first. Each entry: date, agent, phase, what changed (commits), what was measured, what
is still open.

- 2026-10-04, Claude: brief written from the owner's direction. No code changed. Next: Phase 1.
