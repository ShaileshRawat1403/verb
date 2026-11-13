# Durable Session — diagnosis

## beta.12 remeasurement (I-5, 11 September 2026)

Measured on the signed release `0.1.0-beta.12` (APK SHA-256 `c531b283…`, identical to the published
asset) on the same phone, now Android 14 / Funtouch OS 14. Phone charging (83–86 %), screen on; Doze
whitelist, Vivo background-power settings, autostart, phantom-process monitoring and the package set
untouched. No agent was started; a `sleep 86400 &` in the terminal stands in for "agent child". Every
row compares PIDs taken over adb before and after. Kills were confirmed in
`dumpsys activity exit-info`, never assumed from the command. Raw snapshots and screenshots:
`~/verb-i5-evidence/survival/`; the helper is `scripts/i5/survival-snap.sh`.

| # | Lifecycle event | PTY | shell | child | scrollback | keep-alive service after | how established |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | Open and close the workspace sheet, Verb palette and run history | survives | survives | survives | kept | foreground | verified, same PIDs |
| 2 | Project switch and back | survives | survives | survives | kept | foreground | verified, same PIDs; the open terminal stays in its directory |
| — | Back out to the launcher, relaunch | survives | survives | survives | kept | foreground | verified (unplanned, recorded) |
| 3 | HOME for 60 s, relaunch | survives | survives | survives | kept | foreground (oom_adj 200) | verified |
| 4 | Rotation ×2 | survives | survives | survives | kept | foreground | verified |
| 5 | `am kill` while backgrounded | survives | survives | survives | kept | foreground | verified: no-op, no exit record |
| 5b | Swipe away in Recents, wait 90 s, relaunch | survives | survives | survives | kept | foreground | verified: card removed, no exit record |
| 6 | Restart Session (confirmed) | **new** | **new** | **orphaned, keeps running** | cleared | **absent** | verified: `sleep` reparented to pid 1; service not re-claimed 40 s later |
| 6b | …then HOME and `am kill` | **destroyed** | **destroyed** | **destroyed** | lost | back on relaunch | verified: oom_adj 700, exit-info `reason=10 subreason=0`, `kill background` |
| 7 | `am force-stop` | **destroyed** | **destroyed** | **destroyed** | lost | back on relaunch | verified: exit-info `reason=10 subreason=21 (FORCE STOP)` |

What changed since `c2e21bd`: rows 2, 3, 5 and Recents now survive, because sessions belong to
`VerbTerminalSessionHolder` rather than the Activity and `TerminalHoldService` holds the process in the
foreground. A low-memory kill could not be induced while the service was foreground, so row 5 no longer
reproduces the original event; it shows that the service defeats `am kill`, not that it defeats memory
pressure.

What beta.12 gets wrong, by evidence:

1. **Restart Session drops the keep-alive and leaks the old process tree.** After row 6 the session is
   "running" but `TerminalHoldService` never comes back, so the next trip to the background leaves the
   process cached (oom_adj 700) and killable — row 6b. Background children of the old shell survive the
   restart, reparented to init, invisible in the UI; a dev server or `ollama serve` would keep its port.
2. **After process death nothing says what happened.** Rows 6b and 7 relaunch into the right project with
   an empty terminal, no scrollback and no notice. "Sessions and recovery" lists agent conversations
   recoverable from disk (Claude, Codex) and nothing about the shell session that was killed, although
   `exit-info` holds the reason.
3. **Force-stop by `am` and by Vivo look identical** except for the caller: `stop by 21275` here,
   `stop by com.vivo.abe` in the device history (every Verb death recorded since 5 September). What Vivo
   does with the screen off and the phone unplugged is the Track 2 soak, not this table.

Also observed: in landscape the terminal canvas has no visible height; OpenCode's status changed from
"Not installed" to "Session recoverable" across the restarts; before any test the release process had
been alive for about 33 hours with the service in the foreground.

## Historical measurement (`c2e21bd`)

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
