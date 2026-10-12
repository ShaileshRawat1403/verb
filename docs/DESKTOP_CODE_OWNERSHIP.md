# Desktop code ownership

Verb owns project context, durable sessions and terminal hosting. Provider CLIs own their
reasoning, authentication and native conversation records. PocketFabric owns node provisioning,
boot, service supervision and deployment. Keep those responsibilities in their respective repos.

## Boundaries

| Responsibility | Owner | Contract |
| --- | --- | --- |
| Session/provider records | `desktop/src/session.rs` | Existing record format, exact conversation identity, conservative recovery verdicts |
| Process lifecycle and persistence | `desktop/src/lib.rs`, `pty.rs`, `tui/term.rs` | One host owns a PTY; reconcile durable metadata with observed process state |
| HTTP admission and routing | `desktop/src/web.rs` | Authenticate, bound work, dispatch; do not block the PTY pump on workspace probes |
| Workspace snapshots | `desktop/src/web/state.rs` | Background snapshot/cache; observe host runtime independently of stored recovery state |
| Terminal connection | `desktop/web/src/terminal-transport.js` | Socket, reconnect, protocol framing; no DOM or durable session ownership |
| Stream rendering | `desktop/web/src/terminal-stream-view.js` | Render xterm output, acknowledge after consumption, show connection/exit state |
| Session view rules | `desktop/web/src/session-state.js` | Distinguish active sessions, replay availability and native resume |
| Browser orchestration | `desktop/web/src/app.js` | Compose views and user actions; new independent responsibilities belong in named modules |

This is an incremental boundary, not a claim that all large modules have been decomposed.
Continue extracting coherent responsibilities when changing them; avoid size-only file splitting,
framework migrations and unrelated dependency updates during release stabilization.

## Runtime and recovery contract

`state` describes the durable session/recovery verdict. It is not proof that this host owns a
running process. A live session in another Verb process can be visible without local control.

The additive API `terminal` object explicitly reports `running` and `replayAvailable`. A running
local PTY has both true. An exited retained buffer has running false and replayAvailable true.
A session with no buffer here has both false; its native conversation can still be resumable.
The legacy `hostedHere` and `hasTerminal` fields remain compatible: the latter includes replay.
Neither replay availability nor native resume permission grants a running-process capability.
Maintenance must refuse ambiguous process ownership rather than infer quiescence from a label.

Opening a project starts in Sessions without launching a process automatically. Terminal and agent
launch are immediate actions; specs remain an optional structured workflow. Existing shortcuts are
preserved. Native PTY output determines echo, input and process state.

## Change and release discipline

Use small commits with one reason to change. Keep protocol/schema changes backward compatible;
cover behavior and failure boundaries with real process tests. Do not rewrite tests merely to match
implementation. Run `desktop/scripts/check.sh`; the disposable Linux fixture verifies actual UID,
capability, resource and credential boundaries separately. Keep generated browser assets in sync.

The Desktop release workflow validates the selected release ref through the reusable Desktop CI
before creating a draft. Native artifacts are unpacked and exercised with fresh state using
`desktop/scripts/test-release.py` before upload. Cross-built Intel Mac artifacts do not have a native
installation proof in this workflow. Signing/notarization, platform acceptance, and the real-use
observation window remain release work; publishing a draft is not automated final acceptance.
