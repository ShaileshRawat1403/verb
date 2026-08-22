# Durable Session — diagnosis

Measured on the Vivo I2202 (Android 13, arm64) against `c2e21bd`. No persistence implemented; this
document exists to decide the architecture, not to justify one already chosen.

## What survives what

`proot` is the PTY child; `bash` is the login shell under it. "Verified" means PIDs were compared
before and after on the device. "Code" means the teardown is unambiguous in the source and was not
re-measured.

| # | Lifecycle event | PTY | shell | agent child | conversation on disk | how established |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | Tab switch (Terminal → Agents → Terminal) | survives | survives | survives | n/a | verified, same PIDs |
| 2 | Project switch | **destroyed** | **destroyed** | **destroyed** | survives | code |
| 3 | Background → foreground | survives | survives | survives | n/a | verified, same PIDs |
| 4 | Activity recreation (config change ×2) | survives | survives | survives | n/a | verified, same PIDs |
| 5 | Background + process kill (low memory / Recents) | **destroyed** | **destroyed** | **destroyed** | survives | verified, new PIDs after relaunch |
| 6 | Explicit terminal restart | **destroyed** | **destroyed** | **destroyed** | survives | code, by design |
| 7 | Force-stop | **destroyed** | **destroyed** | **destroyed** | survives | verified, gone |

Activity *finish* (backing out of the app) belongs with the destroyed rows: `VerbViewModel.onCleared()`
calls `terminalRuntime.destroy()`.

## The code paths responsible

```
VerbViewModel.onCleared()            -> terminalRuntime.destroy()
TerminalRuntime.selectProject()      -> refreshEnvironment() -> reconfigure() -> destroy() + startSession()
TerminalRuntime.activateAgentRuntime()  -> same
TerminalRuntime.deactivateAgentRuntime() -> same
TerminalRuntimeAdapter.restartSession()  -> destroy() + startSession()
```

`reconfigure()` is a destroy-and-restart, and `refreshEnvironment()` is its only caller. That is why
a project switch cannot preserve a running agent today: changing the launch directory and killing
the session are the same operation.

**A structural blocker for any reattach design:** `installTermuxBootstrap()` runs from the
`VerbViewModel` constructor and calls `refreshEnvironment()` once the bootstrap is ready. Every fresh
process therefore destroys and restarts the PTY during startup. Even if a session did outlive the
process, Verb would tear it down on the next launch before anything could attach to it.

There is **no Service in the manifest**. The PTY is owned entirely by an Activity-scoped ViewModel.

## Agent conversation is already durable, and that is the surprise

Independent of every row above, the agents persist their own transcripts:

```
~/.claude/projects/-data-data-...-projects-step1check-407f1c75/<session-uuid>.jsonl   (several)
~/.codex/history.jsonl, ~/.codex/sessions/
```

and the shipped `claude` binary supports `--continue` and `--resume`.

So "my agent conversation was lost" and "my agent process was killed" are **different problems**.
The conversation survives force-stop today; what is lost is the live process, its in-flight task, and
the UI's ability to find its way back. That materially changes the cost of a fix.

## Options

Survivability claims are deliberately conservative. A foreground service lowers the probability of a
background kill; it is not immunity, and nothing here survives force-stop.

| | Design | Fixes rows | Does **not** fix | Cost |
| --- | --- | --- | --- | --- |
| A | Lifecycle fix only — stop destroying on project switch and on Activity finish | 2, and Activity finish | 5, 7 | small, no new components |
| B | Foreground service owns `TerminalRuntime` | 2, Activity finish; makes 5 much less likely | 7; still killable under real memory pressure | medium — manifest, notification, `FOREGROUND_SERVICE` permission |
| C | `dtach` session layer | 2, 6 become reattach instead of restart | 5, 7 — the daemon is a child of the app UID and dies with it | medium — must be packaged; not currently installed |
| D | `tmux` session layer | as C, plus named sessions, scrollback, inspection | 5, 7 — same UID caveat | medium — must be packaged; larger than `dtach` |
| E | Service + detached layer | 2, 6, Activity finish; 5 much less likely | 7 | largest |

Neither `tmux`, `dtach`, nor `screen` is installed today.

**Row 5 is not fixed by C or D on their own.** The kill test confirmed the mechanism: `proot` died
when the app process was killed, because Android kills the UID's process group. A detached daemon is
in that group too. Only a service that keeps the process alive changes row 5, and only partially.

## Recommendation

Sequence it so the cheap work lands first and nothing is promised that cannot be delivered.

1. **A — lifecycle fix.** Rows 2 and Activity-finish are the ones users actually hit, and they are
   self-inflicted, not Android's doing. Separate "change the launch directory" from "kill the
   session" so a project switch stops being a restart. Remove the startup `refreshEnvironment()`
   teardown, which blocks every later design.
2. **Session identity.** The architectural change, and the one worth doing carefully: give a session
   an id and a lifetime that is not the screen's, so the UI attaches and detaches instead of owning.
   Rows 1, 3 and 4 already survive, which means the runtime is closer to this than it looks — what is
   missing is a name to reattach *to*.
3. **Agent-level resume.** `claude --continue` / `--resume` against the transcripts already on disk
   gives conversation continuity across every row including force-stop, without any process work. On
   current evidence this is the highest value per unit of effort in the whole list, and it is
   complementary to, not a substitute for, steps 1 and 2.
4. **B — foreground service**, once sessions have identity. Only then does keeping a process alive
   mean anything, because only then is there something to reattach to.
5. **Multiplexer, if still needed.** Prefer `tmux` over `dtach`: the extra size buys named sessions,
   scrollback and inspection, which the "durable session as a primitive" direction wants anyway.
   Deciding this last is deliberate — steps 1–4 may make it unnecessary, and `dtach`'s only real
   advantage is a smallness that stops mattering once a service exists.

**Force-stop stays a hard boundary.** It should be stated in the UI rather than engineered around.

## Not established

- Whether `tmux` is available in the Termux repository for this ABI, and its installed size.
- Whether Codex and OpenCode expose a resume flag equivalent to Claude's.
- Behaviour under real memory pressure with a foreground service, as opposed to an induced `am kill`.
- Recents swipe was approximated with a background process kill; the two are not guaranteed
  identical on every OEM, and this is a Vivo.
