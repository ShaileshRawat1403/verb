# Handoff: validate and land the Android benchmark

> Written 2026-10-04 for whichever agent picks this up next (Claude or Codex). It is a task, not a
> history. When the task is done, update the **Status** line and keep the rest as written.

**Status: open.** The script exists and compiles. It has **never run against a phone**: the device
dropped off adb before the first run, and the owner paused the work.

## The task

1. Run `scripts/bench/verb_bench.py` on the Vivo I2202 (iQOO) against the `.debug` package.
2. Fix whatever breaks. Expect breakage: every device interaction in it is untested.
3. Do a full 5-run benchmark of beta.13 and compare it with the beta.12 baseline.
4. Commit the script, `docs/perf/`, and the result files.

Done means: one committed `docs/perf/results/<date>-0.1.0-beta.13-*.json` produced by a clean
script run, its `.md` summary, and a short note to the owner on how beta.13 compares to beta.12.

## Where things are

- Repo `verb`, branch `integration/verb-live-phone-2026-09-28`. HEAD `d048797`
  (pushed, CI green). Do **not** merge to `main`, tag, or publish the draft release
  `v0.1.0-beta.13`; those are the owner's calls.
- New files:
  - `scripts/bench/verb_bench.py`: the benchmark (Python 3, adb, no dependencies)
  - `docs/perf/README.md`: how to run it, what each metric means, known limits
  - `docs/perf/results/2026-10-02-0.1.0-beta.12-vivo-I2202-manual.json`: beta.12 baseline,
    hand-measured, single samples, **indicative only** (see its `meta.note`)
- Phone: real app `com.aistudio.verb.app` is the signed beta.13. Debug app
  `com.aistudio.verb.app.debug` reports `0.1.0-beta.12-debug` but carries the same app code as
  beta.13. It was built from `62fce66`; later commits touched only tests, the version number, and
  Verb Desktop. Rebuild it from HEAD before benchmarking if you want the label to match.

## Run it

```sh
cd verb
# Smoke test first, one run, fast scenarios:
python3 scripts/bench/verb_bench.py --runs 1 --scenarios cold,warm,memory,idle --out /tmp/bench-smoke
# Then the terminal scenarios:
python3 scripts/bench/verb_bench.py --runs 1 --scenarios output,scroll,rotation --out /tmp/bench-smoke
# Then the real run, compared with beta.12:
python3 scripts/bench/verb_bench.py --runs 5 \
  --baseline docs/perf/results/2026-10-02-0.1.0-beta.12-vivo-I2202-manual.json
```

Before running: phone unlocked and on charge, debug app open on a **shell** terminal (not an agent,
because the script types `seq` and `clear` into the active terminal), and nobody using the phone.

## Likely failure points, in order of suspicion

1. **`wait_render_idle`** decides "output finished" when frames fall to cursor-blink level
   (≤6 fps over 1.5 s). If the cursor blinks faster on this panel, or a TUI keeps redrawing,
   it returns `nan` or exits early. Sanity-check `render_seconds` against a stopwatch once.
2. **`type_command`** finds the field by the hint text "type a command" from a uiautomator dump.
   Once text is typed, the hint disappears and the lookup fails. It taps the field before every
   command, so after `seq ...` the field should be empty again. Verify.
3. **`hide_ime`** sends BACK only if `mInputShown=true`. BACK without a keyboard leaves the app,
   and every later scenario then measures the launcher.
4. **`cpu_ticks`** reads `/proc/<pid>/stat` from the adb shell. If the Vivo denies that, `cpu_pct`
   comes back `None`; fall back to `top -b -n 2 -d 5 -p <pid>`.
5. **`force_gc`** uses `am dumpheap` (debuggable builds only) and writes about 45 MB to
   `/data/local/tmp`. It removes the file afterwards. Check that it did.
6. **`metadata` refresh rate** parses the first `refreshRate=` in `dumpsys display`, which may be a
   supported mode rather than the active one.
7. **Scroll swipe coordinates** (`540 900` ↔ `540 1700`) assume a 1080×2400 portrait screen with
   the keyboard hidden.

## Rules from this device (do not relearn them the hard way)

- **Never benchmark the real app** without the owner's say-so. `cold` force-stops it, which ends
  every terminal and the agents running in them. The script refuses without `--allow-real-app`.
- **Never press screen-off (keyevent 26)**: the phone then needs a PIN that only the owner has.
  Use keyevent 224 to wake.
- **Do not type into other apps.** Before you type anything, confirm the focused window is Verb
  (`dumpsys window displays | grep mCurrentFocus`). In an earlier session, keystrokes landed in
  Google Messages.
- **Restore what you change.** The rotation scenario restores `accelerometer_rotation` and
  `user_rotation` in a `finally` block. Check both values after a run anyway.
- **Gradle masks failures with cached results.** Run the Android gate with `--rerun-tasks`. If a
  test passes on the Mac and fails on CI, see the Conscrypt note in the commit message of `044bcc1`.
- **Auto mode may block pushes, `gh workflow run`, and installs over the real app.** The owner then
  runs those via a typed `!` command. A pasted `!` line arrives as plain text and does not run.
- **Do not touch Verb Desktop** (`desktop/`) without explicit permission.

## Beta.12 numbers to compare against (2 Oct 2026, single samples)

| Metric | beta.12 |
|---|---|
| Warm start | 239 ms |
| Memory | PSS 33 MB, RSS 93 MB, 34 threads, **2 Activities** (leak, fixed in beta.13: expect 1) |
| Idle on screen | ~3% CPU, ~2 fps |
| Heavy output + scroll | p50 5 ms, p90 14 ms, p99 26 ms, 7.7% janky |

Cold start was never measured for beta.12. Beta.13's cold-start numbers become the first baseline.

## Report back to the owner

Give them: what you ran, the beta.13 vs beta.12 table, any script fixes (with the reason each was
needed), anything that still looks unreliable, and the commit SHA. Keep measured facts separate from
interpretation.
