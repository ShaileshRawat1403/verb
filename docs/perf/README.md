# Verb Android performance

`scripts/bench/verb_bench.py` measures Verb on a real phone over adb and writes one JSON file and
one Markdown summary per run into `results/`. Results are committed, so a regression shows up as a
diff between two files rather than as a feeling.

```sh
python3 scripts/bench/verb_bench.py                                   # all scenarios, 5 runs, .debug app
python3 scripts/bench/verb_bench.py --runs 10 --scenarios cold,warm
python3 scripts/bench/verb_bench.py --baseline docs/perf/results/<earlier>.json
```

Before a run: phone unlocked, on charge, brightness fixed, Verb's debug app open on a **shell**
terminal (not an agent), and left alone for the duration. The script refuses to start otherwise
where it can tell.

## Scenarios

| Scenario | What it does | Metrics |
|---|---|---|
| `cold` | `am force-stop`, then `am start -W` | `total_ms`: process start to first frame, as Android reports it |
| `warm` | HOME, then `am start -W` on the live process | `total_ms` |
| `memory` | Forces a GC (debuggable builds only), then `dumpsys meminfo` | PSS, RSS, Java/native heap, graphics, live Activities and Views, threads |
| `idle` | 10 s on screen with nothing happening | `cpu_pct` of one core, `fps` (cursor blink is about 2) |
| `output` | Types `seq 1 <lines>` into the active terminal | `render_seconds` until frames stop, frame percentiles, janky % |
| `scroll` | Fills scrollback, then 8 flings through it | frame percentiles, janky % |
| `rotation` | Four rotations, 20 s, two forced GCs | live `activities`; a single-Activity app should settle at 1 |

Every scenario runs one discarded warm-up and then `--runs` measured samples, reported as median,
p90, min and max with the sample count.

## Known limits

- **One device is one data point.** Numbers from the Vivo I2202 say little about low-end phones.
- **`total_ms` stops at the first frame,** not at a usable shell prompt. Time-to-prompt needs an
  in-app marker that does not exist yet.
- **`render_seconds` is inferred** from the frame rate falling back to cursor-blink level, so it
  carries about 1.5 s of detection slack. Compare it between builds; do not quote it as a latency.
- **Frame percentiles come from `gfxinfo`,** which buckets in whole milliseconds and counts a frame
  janky against the panel's current refresh rate (the I2202 switches between 60, 90 and 120 Hz).
- **GC-dependent metrics need a debuggable build.** On the release package `memory` reports without
  a forced GC and `rotation` is skipped.
- **Benchmarking the real app** (`--allow-real-app`) is possible but `cold` force-stops it, which
  ends every terminal and agent session inside.

## Results

`results/2026-10-02-0.1.0-beta.12-vivo-I2202-manual.json` predates the script: hand-run single
samples from the installed release app, kept as an indicative beta.12 baseline. Its memory figures
were taken while the app was backgrounded (27 MB swapped), and its scroll row combined heavy output
with scrolling. Later files are script runs and are the ones to trust.
