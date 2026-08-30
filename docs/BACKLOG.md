# Verb backlog

Everything known to be undone, and what it costs to say yes. The current sprint is at the top; the
rest is a menu, not a plan.

Milestone definitions live in `docs/ROADMAP.md`. M2 is now defined by the evidence-bound assistant
surface recorded below. Anything proposed beyond it must first pass the admission test in
`docs/UX_FOUNDATION.md`: does it make an existing development moment easier, clearer or safer, or
does it merely add another capability?

---

## Closed sprint — beta.5 Truth & Reliability (30 August)

| # | Gate | Description | Status |
| --- | --- | --- | --- |
| 1 | **P1** | Session-bound agent lifecycle: coordinator observers and command dispatch bound directly to `runtimeOf(sessionId)` | Done — validated by construction |
| 2 | **P2** | Multi-agent restoration identity: Activity/ViewModel recreation resolves coordinators by exact `terminalSessionId` | Done — ambiguous foreground binding refused |
| 3 | **P3** | Executable regression invariants in `MultiTerminalLifecycleIsolationTest.kt` (session isolation, UI independence, restoration identity) | Done — 647 Android + 123 Rust unit/integration tests green |
| 4 | **P4** | Physical hardware proof on Vivo I2202: concurrent $T_1$ (Claude Code) + $T_2$ (OpenAI Codex) + $T_3$ (Shell), selective $T_2$ `^C` interrupt leaving $T_1$ live, and orientation recreation | Done — physically accepted on Vivo I2202 |
| 5 | **P5** | Documentation truth: canonical documents reconciled (`README`, `PRD`, `ROADMAP`, `ARCHITECTURE`, `RELEASE_NOTES`) | Done |
| 6 | **P6** | Supply-chain hardening: cryptographically pinned GitHub Actions commit SHAs across all workflows and remote CI verified | Done — all CI gates green |
| 7 | **Release** | `v0.1.0-beta.5` signed release published with badging artifact identity enforcement (`versionName=0.1.0-beta.5`, `versionCode=5`) | Done — published |

---

## Historical beta-closure pass — 24 August

| # | Item | Status |
| --- | --- | --- |
| 1 | Replace Android subsystem tabs with one terminal workspace and searchable named tasks | done — physically accepted 26 Aug |
| 2 | Ensure a Verb overlay takes keyboard/back ownership from the mounted terminal | done — physically accepted 26 Aug |
| 3 | Public-source license, security policy and release checklist | done |
| 4 | Complete desktop and both-flavor Android verification on the return revision | done — 26 Aug |
| 5 | Physical Android workspace, Claude/Codex and Android↔desktop continuity acceptance | done — Claude/Codex resume 22 Aug; workspace and continuity round-trip 26 Aug |
| 6 | Integrate into the primary full-history repository and audit the final release archive | pending primary machine |

### What was proven on 26 August

Both flavors verified on the Vivo I2202: `fullCliDevice` (daily-driver, updated in place) and — for
the first time — `playDevice`, built, installed and booted into its own sandbox. On desktop, 117
Rust tests pass, the release binary builds, and `verb context` renders live evidence. The terminal
workspace was re-accepted by driving the real app over adb: tap opens the IME, back hides it,
scrolling and flinging keep it hidden, back again leaves the workspace, and a pinched font size
survives a full process stop.

The Android↔desktop continuity round-trip (G2) completed in both directions: the phone exported a
`.vcont` to Downloads, the desktop imported it preview-first then applied, the desktop exported its
own envelope including that imported evidence, and the phone previewed and applied it as read-only
evidence ("1 imported session record", no local session or Resume action changed). The round-trip
caught two real desktop defects — `verb continuity export` crashed on legacy event logs (integer
millis timestamps, a snake_case `session_id`) and wrote lowercase event states the Android host
correctly rejects — both fixed with regression tests.

The optional provider interpretation screen was rejected, and has now been removed rather than
merely marked so — it shipped for a further pass after this line was written, which is how a doc
stops being evidence. Ask Verb's second stage is the evidence-linked assistant described in the
completion pass below, the same one the terminal opens. It makes no execution or
terminal-observation claim.

## Completed sprint — Mobile touch and layout

> **A person can read the terminal with their finger on it: scroll, select and zoom stay gestures,
> typing stays one tap away, and every control survives the soft keyboard.**

All items below are verified on the Vivo I2202 (Android 14) production `device` build; details and
the hard-won constraints behind them live in `docs/HANDOFF_MOBILE_UX_SLICE.md`.

| # | Item | Status |
| --- | --- | --- |
| 1 | Scrolling no longer opens the IME — gesture verdicts come from the Termux recognizer, not a Compose watcher | Done — device-verified |
| 2 | Dock collapses animated under the keyboard; canvas keeps a 96dp floor | Done — device-verified |
| 3 | Essential key row always visible; edge fades drawn transparent-at-edge (DstIn gradient clamp trap) | Done — device-verified |
| 4 | Command field is a compact 42dp `BasicTextField` with preserved focus ring and test tag | Done — device-verified |
| 5 | Tap-to-focus rides the view's own `onSingleTapUp` (`TermuxTerminalRuntimeAdapter.onCanvasTap`) | Done — device-verified |
| 6 | Restarting a live session asks first; restarting a dead one is immediate | Done — device-verified |
| 7 | `RunsSheet` consumes navigation-bar insets | Done — device-verified |
| 8 | AI sheet scrolls, key rows fade at edges, keys tick haptically, overflow is a 48dp target | Done — device-verified |
| 9 | Output auto-follow scoped to the Compose fallback view only; diagnostics log follows only while read near the bottom | Done — device-verified |

This also closes NEXT_SPRINT's "same layout pattern exists elsewhere" carry-over: Runs was the last
fraction-height sheet — SemanticLens, Verb, Project and FileExplorer sheets were audited clean.

Still open from this slice: landscape layout (deliberately deferred — the terminal workspace is a
portrait-first surface).

## Added to beta — 26 August, second pass

| # | Item | Status |
| --- | --- | --- |
| 1 | Theme follows the system dark setting (the light scheme was dead code) | Done — verified on device in both modes; the terminal workspace keeps a dark canvas by design |
| 2 | Agent Runtime file pickers clear of the scroll thumb (NEXT_SPRINT carry-over, cosmetic) | Done — the Choose buttons reserve the thumb strip instead of sitting under it |
| 3 | **M2 vertical slice** — ask your own question about the terminal moment | Done — device-verified |
| 4 | E1 — foreground service holds the process while a session is live | Done — service verified foreground on device |

The M2 slice is the roadmap's first M2 increment: the user asks their own question about this
terminal moment, Verb attaches exactly its evidence envelope (structural facts only — never command
text, paths or PTY output), the prompt requires the answer to name the facts it used, and the same
evidence renders as a "Based on" block beside every answer. The model stays replaceable; the
context is Verb. What the sheet shows as "what the model saw" is snapshotted before the request, so
it cannot drift from what was sent.

The E1 foreground service lowers the probability of a background kill and is not immunity: it owns
nothing, follows the session (RUNNING holds the claim, a finished session releases it), and a
force-stop still ends everything — exactly as `docs/DURABLE_SESSION.md` promised.

## M2 beta completion pass — 26 August, third pass

| # | Item | Status |
| --- | --- | --- |
| 1 | Enriched structural context: session, shell integration, command-boundary tail and agent facts | Implemented — privacy/unit tests pass |
| 2 | Bounded follow-up thread: last five exchanges retained, last three sent to the provider | Implemented — unit-tested |
| 3 | Evidence-linked answer rendering with small markdown support | Implemented — unit-tested |
| 4 | Provider boundary remains free of command text, paths and PTY output | Implemented — regression-tested |
| 5 | The panel reads the envelope back in plain language, from the same snapshot | Implemented — unit-tested |
| 6 | Asking opens without spending a provider call; the thread survives an accidental dismissal | Implemented — Compose-tested |
| 7 | One assistant: Ask Verb's second stage is the same evidence-bound panel the terminal opens | Implemented — Compose-tested |
| 8 | Physical-device acceptance of the follow-up flow | Done — Vivo I2202, 26 August |

Review of the first draft of this pass found four defects the unit tests could not see, all fixed
here and all now covered:

- The envelope labelled the command tail "oldest first" and rendered it newest first. The model was
  handed a false claim about which command ran last — the exact `Inference ≠ Fact` failure this
  surface exists to prevent.
- `TerminalEvidence` accepted pre-formatted agent lines from its caller, which is a free-text hole
  through the privacy boundary the type exists to be. It now takes `AgentWorkFact` fields.
- The unprompted-explanation path published no evidence snapshot, so an answer could be displayed
  beside evidence belonging to an earlier question.
- The newest answer was drawn twice: once as the thread's tail and once as the standing explanation.

Two UX rules were also being broken by the new surface. The panel showed the contract's vocabulary
(`RUNNING`, `LIVE`, `FAILED`) directly on screen, against the plain-language rule in
`docs/UX_FOUNDATION.md`; it now reads back through `VerbStatusVocabulary`. And the only way in was
an overflow item that fired a canned analysis, so a person could not reach the question box without
first paying for an answer they had not asked for.

An earlier draft of this section recorded "this shell has no Java runtime or `adb`" as a blocker;
that was wrong — both exist, off `PATH`. No execution authority or transcript transport was added by
this pass.

### Physical acceptance, Vivo I2202, 26 August

Asked "Why did my last command fail" after a real `ls /definitely-not-here`. The answer said *exit
code 2, ran for 35 ms* and the panel beneath it read `✕ failed · exit 2 · 35ms` — the same fact in
two vocabularies from one snapshot, which is the whole claim this surface makes. The model stated
plainly that it had not been given the command text. The thread, the follow-up placeholder, `Clear`,
and the unprompted-explanation path were all exercised in both hosts.

Four further defects surfaced only on the device and are fixed:

- The terminal's sheet opened itself whenever the shared assistant state filled, so asking from Ask
  Verb slid the terminal sheet up over that screen with the same conversation inside. It is now
  shown only when the user opened it there.
- The evidence panel read "Last 1 commands".
- Ask Verb stated the provider boundary and the panel stated it again underneath.
- The model answered in the contract's uppercase spellings beside a panel using the plain ones; the
  system instruction now asks for the plain reading.

**Closed.** The soft keyboard covered the question box in the terminal's bottom sheet. The cause
was Material3 1.3.0: `ModalBottomSheet` renders in its own `WindowManager` window that never
receives IME insets, so no padding inside the sheet could move a field the window did not know was
covered. The sheet is gone — the terminal opens Ask Verb on the assistant stage, which is the host
that works and the one a person goes to in order to ask. Verified with the keyboard open.

## C2 — the working tree, observed — 26 August

The assistant could not answer the question the product is named for. The envelope carried session
lifecycle, exit codes and durations, and nothing at all about the world the agent was changing.

`GitObserver` reads the tree through the guest userland — inside-work-tree, branch presence, HEAD,
porcelain status — bounded to four seconds and off the terminal's hot path, because this is proot on
a phone. Every command boundary takes a snapshot, and the reading it replaces becomes the baseline,
so the delta says what that command did.

`GitSnapshot` carries how much moved, never what moved: no paths, no file names, no diff, and
deliberately no branch name. A repository root like `/private/acme-corp` names a client and
`feature/acme-login` carries the same disclosure, and neither is needed to answer how much changed.
A test plants both and asserts neither survives.

Two mistakes the device caught that the tests could not:

- The delta first measured from "the last command that exited 0". The command that changes the tree
  usually exits 0 too, so it became its own baseline and every delta was zero — three new files
  reported as no change. The baseline is now the tree *before* the command.
- Unknown had to stay unknown throughout: a `git` that could not run is not a clean tree, and an
  uncomparable delta is silent rather than zero.

Device-verified on the Vivo I2202: two files created, answer read "2 more files being changed, with
the HEAD remaining unchanged", while stating plainly that it could not name them.

**Still open in C2:** `docs/BACKLOG.md` C3 (last-known-good tracking) is the natural next step —
a delta against a state the *user* considered working, rather than against the previous command.

## Completed sprint — Mobile reliability

> **A mobile Verb installation can be upgraded, reinstalled and recovered without losing the user's
> working world, and every state Verb displays comes from one evidence-backed resolver.**

That sentence is the whole scope. No new vendors, no new surfaces, nothing that is not one of these
four.

| # | Item | Status |
| --- | --- | --- |
| 1 | `verb export` / `verb import` — the working world in one explicit, encrypted, versioned, checksummed archive | Done — device-verified 22 Aug |
| 2 | One status resolver — every displayed state comes from evidence, through a single place | Done — `AgentStatusResolver` |
| 3 | Install and build hardening — release builds on the device, no backup flag, one signing key | Done — `device` build type |
| 4 | Re-prove Claude and Codex end to end on the device | Done — both resumed prior context |

### What was proven on the device, 22 August

The phone was upgraded in place (`adb install -r`, debug → `device` build) and the world survived.
Then, in order: `verb export` wrote a 35 MB encrypted archive of a 69 MB world; **System → Working
world → Save to Downloads** copied it to `Download/world.vbak`, which an uninstall cannot reach;
**Bring an archive in** staged it back as `~/imported-world.vbak`; `verb import` printed the
manifest, verified the payload checksum, listed the seven paths it would replace, and changed
nothing. Claude resumed with its prior transcript intact, and Codex resumed its own session after
Claude exited. Throughout, exactly one agent read `Running` and the other read `Session
recoverable` — the state both used to claim at once.

### Why this sprint exists

The phone's entire world — the Linux userland, Claude's login, Codex's login, `~/.env`, session
records — was created fresh at 12:45 on 21 August, because the package was installed fresh. An
uninstall, a variant switch or a "clear storage" resets all of it, and the setup work starts again.
Nothing in Verb defends against that, and it has cost real evenings.

### Backup design constraints

Recovery must not become a credential-leak surface. Therefore:

* **Verb-owned metadata may be snapshotted automatically.** Session and project records contain no
  credentials.
* **Third-party credentials are never auto-snapshotted anywhere.** An archive containing Claude and
  Codex auth or `~/.env` is **explicit, user-triggered, encrypted, versioned and integrity-checked**,
  and nothing about it happens silently.
* Import shows a **manifest and a dry-run preview** before replacing anything, and takes its own
  snapshot of what it is about to overwrite.

```text
verb export ~/verb-world.vbak
verb import ~/verb-world.vbak            # preview only
verb import ~/verb-world.vbak --apply    # after reading the preview
```

## Previous sprint — "Nothing to memorise, evidence on demand"

**Theme:** someone who has never read our documentation should be able to open Verb, understand what
they are looking at, and find everything without knowing a key.

**Audience decision (2026-08-22):** approachable, still terminal-first. The PRD's initial user is
unchanged — this is quality, not scope. Verb does not become a tool for people who do not use a
terminal; it becomes a tool that does not punish people who are not fluent in one.

| # | Item | Status |
| --- | --- | --- |
| 1 | Desktop CI — fmt, clippy, tests, release build on every PR | done |
| 2 | Mouse support — wheel, click to select, click away to close; captured only while a Verb surface is open | done |
| 3 | Scrollback and search in the terminal pane (`Leader [`) | done |
| 4 | Discoverability — leader menu, first-run welcome, fuller help, empty states that name the next action | done |
| 5 | Plain-language state, with the technical term still visible | done |
| 6 | `Leader v` evidence overlay — `verb context` rendered in the workspace | done |
| 7 | Palette subsequence matching, minimum-size guard, forget a session record | done |

Then a short dogfood, which decides M2's direction.

**Sequence:** 1 → 2 → 3 → 4 → 5 → 6. CI first because everything after it is verified by it; the
evidence overlay last so it lands on a workspace that is already comfortable to sit in.

**Not in this sprint:** M2 itself, deeper observation (see C below), phone work, Windows TUI,
distribution, splitting PR #2.

---

## A. Quality and approachability

| # | Item | Size |
| --- | --- | --- |
| A1 | ~~Desktop CI~~ | done |
| A2 | ~~Scrollback + search~~ | done |
| A3 | ~~Palette subsequence matching~~ (recency ordering still open) | partly |
| A4 | ~~Minimum-size guard~~ | done |
| A5 | ~~Forget a session record~~ | done |
| A6 | ~~Mouse support~~ inside surfaces | done |
| A8 | ~~Mouse capture by default, so bar actions are clickable~~ — decided from dogfooding on 22 Aug: the first thing reached for in the workspace was the bar. Captured by default, Option-drag still selects, `leader m` hands the mouse back. Status segments are still not clickable | done |
| A7 | Windows support for the TUI (CLI already falls back to inherited stdio) | L |
| A9 | ~~Apply an evidence-based Agents admission rule~~ — only Claude, Codex and OpenCode appear; unverified runtime profiles no longer become catalogue cards | done |

## B. Blocked on the Android device

| # | Item | Blocker |
| --- | --- | --- |
| B1 | Codex and OpenCode leader-collision rows, so the leader default stops being provisional | device |
| B2 | OpenCode Android recovery proof | needs a signed-in provider |
| B3 | Decide the fate of `cmake`/`clang`/`make` left in the guest during the dsh attempt | your call |
| B4 | `dsh` | upstream: `koffi` has no Android build. Recorded as unavailable with the reason |

## C. Observation gaps — possibly M0 rather than M2

The mockups in `docs/TUI_VISION.md` promise more than Verb currently observes. Whether that matters
is exactly what dogfooding answers: if seeing the evidence makes the next question obvious, M2 is an
assistant; if the evidence itself is thin, the next milestone is better observation wearing an M2
costume.

| # | Item | Unlocks |
| --- | --- | --- |
| C1 | ~~Agent sessions emit nothing structural~~ — both hosts now follow the record the agent writes for itself and emit turn/tool/failure events, worded as reported rather than witnessed. OpenCode has no reader yet | done for Claude and Codex |
| C2 | ~~Git snapshot at command boundaries~~ — the tree is observed and the delta says what the last command did; see the C2 section above | done |
| C3 | Last-known-good tracking | comparison and recovery |
| C4 | Runtime version facts (node, python, …) | the runtime-mismatch scenario in the mockups |
| C5 | Richer contextual triggers: risky Git operation, runtime mismatch | two of the four bands in `TUI_VISION.md` |

## C0. Agent compatibility matrix

Recorded so a candidate cannot become a tile on a screen. Nothing here is visible in the product
until it has a reason to be, and each line says what has actually been verified.

```text
Claude Code      hosted CLI        upstream: yes   on Verb Android: yes, signed in, recovery proven
Codex            hosted CLI        upstream: yes   on Verb Android: yes, signed in, recovery proven
OpenCode         hosted CLI        upstream: yes   on Verb Android: runs; sign-in not required for use
Gemini CLI       listed already    upstream: yes (@google/gemini-cli 0.56.0)
                                   on Verb Android: not verified · priority: deferred
                                   hidden by the A9 admission rule; no product card
Ollama           candidate         shape: remote provider endpoint, not a hosted agent.
                                   The npm package is a client library; the runtime is a Go binary.
                                   Verb would point at a server elsewhere · priority: deferred
Antigravity      excluded          nothing hostable exists: @google/antigravity is a 404 and the
                                   unscoped `antigravity` package is a placeholder joke
dsh              excluded          koffi has no Android build; the card says why
```

## D0. Shape constraints for work that has not started

Not tasks — the form these must take if they are ever built, recorded so a future sprint does not
quietly invent a different shape.

| Item | Required shape |
| --- | --- |
| Configuration (leader, colours, defaults) | Found by name in the palette, never a settings tree. `docs/UX_FOUNDATION.md`, "Configuration is searched, not browsed" |
| Anything agent-related | A source underneath a human moment, never a "Agents" top-level surface |
| Any new region | There is no budget for one: quiet chrome is two rows, a moment is four |

## D. Repository and release

| # | Item | Size |
| --- | --- | --- |
| D1 | Integrate this source-only transfer into the primary repository's full history; do not publish the temporary transfer Git repository | S–M |
| D2 | Android signed prerelease workflow exists; desktop distribution remains `cargo install`/source build only | partly |
| D3 | The Rust crate has no library target, so integration tests drive the binary | S |
| D4 | ~~Dated snapshots (`HANDOFF.md`, `NEXT_SPRINT.md`, V0 validation) are marked, not rewritten~~ | done |
| D5 | ~~Add CONTRIBUTING and architecture overview~~ | done |

## E. Android — paused, not abandoned

| # | Item |
| --- | --- |
| E1 | Foreground service for the background-kill gap `docs/DURABLE_SESSION.md` leaves open |
| E2 | More than one concurrent agent session |
| E3 | Codex runs under qemu because proot refuses its static non-PIE binary; needs a patched proot or an out-of-proot spawn |

## F. Frozen by design

M2 — explain, compare, or guided action. Decided by friction notes, not by this list. See
`docs/DOGFOODING.md`.

## G. Cross-host continuity

| # | Item | Status |
| --- | --- | --- |
| G1 | Manual checksummed `.vcont` export, preview and read-only import on Android and desktop | implemented; automated and desktop round-trip verified |
| G2 | Physical Android→desktop→Android file round-trip and UI acceptance | done — 26 Aug, both directions on the Vivo I2202 |
| G3 | Optional encrypted wrapper for continuity metadata | post-beta; no credentials may enter the envelope |
| G4 | Automatic/background/cloud/LAN sync, remote process resume, transcript transport | rejected for beta; requires separate product and threat-model approval |
