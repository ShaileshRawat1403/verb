#!/usr/bin/env python3
"""
Verb Android benchmark: repeatable performance numbers from a real phone over adb.

Every number this prints was measured on the device in front of you, in the run that printed it.
Each scenario repeats, the first (warm-up) sample is discarded, and results are reported as
median / p90 / min / max with the sample count, never as a single run.

    python3 scripts/bench/verb_bench.py                       # all safe scenarios, .debug app
    python3 scripts/bench/verb_bench.py --runs 10 --scenarios cold,warm
    python3 scripts/bench/verb_bench.py --baseline docs/perf/results/<earlier>.json

Results are written to docs/perf/results/<date>-<version>-<device>.json and a Markdown summary
beside it. See docs/perf/README.md for what each metric means and its known limits.

Safety:
  * The default target is com.aistudio.verb.app.debug. Benchmarking the real app
    (com.aistudio.verb.app) requires --allow-real-app, because cold-start runs force-stop the
    process, which ends every running terminal and agent in it.
  * The output and scroll scenarios type commands into the *active* terminal. They only ever type
    `seq` and `clear`.
  * Rotation settings are restored in a finally block, even if the run is interrupted.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import re
import shutil
import statistics
import subprocess
import sys
import time
import xml.etree.ElementTree as ET
from pathlib import Path

DEBUG_PACKAGE = "com.aistudio.verb.app.debug"
REAL_PACKAGE = "com.aistudio.verb.app"
ACTIVITY = "com.example.MainActivity"
ALL_SCENARIOS = ["cold", "warm", "memory", "idle", "output", "scroll", "rotation"]
REPO = Path(__file__).resolve().parents[2]


# --------------------------------------------------------------------------------------------- adb


class Adb:
    def __init__(self, serial: str | None):
        exe = shutil.which("adb") or "/opt/homebrew/share/android-commandlinetools/platform-tools/adb"
        if not Path(exe).exists():
            sys.exit("adb not found on PATH or in the Homebrew command-line tools location.")
        self.base = [exe] + (["-s", serial] if serial else [])

    def run(self, *args: str, timeout: float = 60, check: bool = False) -> str:
        proc = subprocess.run(self.base + list(args), capture_output=True, text=True, timeout=timeout)
        if check and proc.returncode != 0:
            raise RuntimeError(f"adb {' '.join(args)} failed: {proc.stderr.strip() or proc.stdout.strip()}")
        return proc.stdout

    def sh(self, cmd: str, timeout: float = 60) -> str:
        return self.run("shell", cmd, timeout=timeout)


def log(msg: str) -> None:
    print(f"[{dt.datetime.now():%H:%M:%S}] {msg}", flush=True)


# ------------------------------------------------------------------------------------------ device


class Device:
    def __init__(self, adb: Adb, package: str):
        self.adb = adb
        self.pkg = package

    def wake(self) -> None:
        self.adb.sh("input keyevent 224")
        time.sleep(0.5)

    def keyguard_showing(self) -> bool:
        return "isKeyguardShowing=true" in self.adb.sh("dumpsys window | grep -m1 isKeyguardShowing")

    def ime_shown(self) -> bool:
        return "mInputShown=true" in self.adb.sh("dumpsys input_method | grep -m1 mInputShown")

    def hide_ime(self) -> None:
        # BACK closes the keyboard when it is open, but with no keyboard it leaves the app.
        if self.ime_shown():
            self.adb.sh("input keyevent 4")
            time.sleep(0.8)

    def pid(self) -> int | None:
        out = self.adb.sh(f"pidof {self.pkg}").strip().split()
        return int(out[0]) if out else None

    def focused_on_app(self) -> bool:
        return self.pkg + "/" in self.adb.sh("dumpsys window displays | grep mCurrentFocus")

    def start(self) -> dict:
        out = self.adb.sh(f"am start -W -n {self.pkg}/{ACTIVITY}", timeout=90)
        fields = dict(re.findall(r"^(\w+): (\S+)", out, re.M))
        return fields

    def bring_to_front(self) -> None:
        self.start()
        time.sleep(1.5)

    def version(self) -> tuple[str, str]:
        out = self.adb.sh(f"dumpsys package {self.pkg}")
        name = re.search(r"versionName=(\S+)", out)
        code = re.search(r"versionCode=(\d+)", out)
        return (name.group(1) if name else "?", code.group(1) if code else "?")

    def debuggable(self) -> bool:
        flags = self.adb.sh(f"dumpsys package {self.pkg} | grep -m1 'pkgFlags='")
        return "DEBUGGABLE" in flags

    def force_gc(self) -> bool:
        """Heap dumps run a full GC first. Only possible on a debuggable package."""
        path = "/data/local/tmp/verb-bench-gc.hprof"
        out = self.adb.sh(f"am dumpheap {self.pkg} {path}", timeout=120)
        if "Error" in out or "Exception" in out:
            return False
        time.sleep(5)
        self.adb.sh(f"rm -f {path}")
        return True

    # ---- UI helpers

    def ui_nodes(self) -> list[tuple[str, tuple[int, int, int, int]]]:
        self.adb.sh("uiautomator dump /sdcard/verb-bench-ui.xml", timeout=30)
        raw = self.adb.sh("cat /sdcard/verb-bench-ui.xml")
        nodes = []
        try:
            root = ET.fromstring(raw[raw.find("<"):])
        except ET.ParseError:
            return nodes
        for n in root.iter("node"):
            label = (n.get("text") or "") + "|" + (n.get("content-desc") or "")
            m = re.match(r"\[(\d+),(\d+)\]\[(\d+),(\d+)\]", n.get("bounds") or "")
            if m:
                nodes.append((label, tuple(int(x) for x in m.groups())))
        return nodes

    def find(self, needle: str) -> tuple[int, int, int, int] | None:
        for label, b in self.ui_nodes():
            if needle.lower() in label.lower():
                return b
        return None

    def tap_bounds(self, b: tuple[int, int, int, int]) -> None:
        self.adb.sh(f"input tap {(b[0] + b[2]) // 2} {(b[1] + b[3]) // 2}")

    def type_command(self, command: str) -> None:
        """Types into Verb's command field and presses Enter. Only plain [a-z0-9 ] commands."""
        if not re.fullmatch(r"[a-z0-9 ]+", command):
            raise ValueError("refusing to type anything but simple commands")
        field = self.find("type a command")
        if field is None:
            raise RuntimeError("Verb's command field is not on screen; open a terminal first")
        self.tap_bounds(field)
        time.sleep(0.8)
        self.adb.sh("input text " + command.replace(" ", "%s"))
        self.adb.sh("input keyevent 66")

    # ---- frame stats

    def reset_frames(self) -> None:
        self.adb.sh(f"dumpsys gfxinfo {self.pkg} reset")

    def frames(self) -> dict:
        out = self.adb.sh(f"dumpsys gfxinfo {self.pkg}")
        r: dict = {}
        m = re.search(r"Total frames rendered: (\d+)", out)
        r["frames"] = int(m.group(1)) if m else 0
        m = re.search(r"Janky frames: (\d+) \(([\d.]+)%\)", out)
        r["janky_pct"] = float(m.group(2)) if m else 0.0
        for p in (50, 90, 95, 99):
            m = re.search(rf"{p}th percentile: (\d+)ms", out)
            r[f"p{p}_ms"] = int(m.group(1)) if m else None
        return r

    def frames_total(self) -> int:
        m = re.search(r"Total frames rendered: (\d+)", self.adb.sh(f"dumpsys gfxinfo {self.pkg} | grep -m1 'Total frames'"))
        return int(m.group(1)) if m else 0

    def wait_render_idle(self, timeout: float, quiet_fps: float = 6.0) -> float:
        """Seconds until the app stops producing frames beyond the cursor blink (~2 fps)."""
        start = time.monotonic()
        last = self.frames_total()
        quiet = 0
        while time.monotonic() - start < timeout:
            time.sleep(0.5)
            now = self.frames_total()
            quiet = quiet + 1 if (now - last) <= quiet_fps * 0.5 else 0
            last = now
            if quiet >= 3:
                return time.monotonic() - start - 1.5
        return float("nan")

    # ---- memory / cpu

    def meminfo(self) -> dict:
        out = self.adb.sh(f"dumpsys meminfo {self.pkg}")
        r: dict = {}
        for key, pat in {
            "pss_kb": r"TOTAL PSS:\s+(\d+)",
            "rss_kb": r"TOTAL RSS:\s+(\d+)",
            "java_heap_kb": r"Java Heap:\s+(\d+)",
            "native_heap_kb": r"Native Heap:\s+(\d+)",
            "graphics_kb": r"Graphics:\s+(\d+)",
            "activities": r"Activities:\s+(\d+)",
            "views": r"Views:\s+(\d+)",
        }.items():
            m = re.search(pat, out)
            if m:
                r[key] = int(m.group(1))
        return r

    def cpu_ticks(self, pid: int) -> int | None:
        stat = self.adb.sh(f"cat /proc/{pid}/stat").strip()
        if not stat:
            return None
        fields = stat[stat.rfind(")") + 2:].split()
        return int(fields[11]) + int(fields[12])  # utime + stime, in clock ticks

    def threads(self, pid: int) -> int | None:
        m = re.search(r"Threads:\s+(\d+)", self.adb.sh(f"cat /proc/{pid}/status"))
        return int(m.group(1)) if m else None


# ------------------------------------------------------------------------------------- statistics


def summarize(samples: list[float]) -> dict:
    vals = sorted(v for v in samples if v is not None and v == v)
    if not vals:
        return {"n": 0}
    p90 = vals[min(len(vals) - 1, int(round(0.9 * (len(vals) - 1))))]
    return {
        "n": len(vals),
        "median": round(statistics.median(vals), 2),
        "p90": round(p90, 2),
        "min": round(vals[0], 2),
        "max": round(vals[-1], 2),
        "samples": [round(v, 2) for v in samples],
    }


def collect(samples: list[dict]) -> dict:
    keys = sorted({k for s in samples for k in s})
    return {k: summarize([s.get(k) for s in samples]) for k in keys}


# -------------------------------------------------------------------------------------- scenarios


class Bench:
    def __init__(self, dev: Device, runs: int, output_lines: int):
        self.d = dev
        self.runs = runs
        self.output_lines = output_lines
        self.can_gc = dev.debuggable()

    def repeat(self, name: str, fn) -> dict:
        log(f"{name}: warm-up + {self.runs} runs")
        fn()  # warm-up, discarded
        samples = []
        for i in range(self.runs):
            self.d.wake()
            samples.append(fn())
            log(f"  {name} {i + 1}/{self.runs}: {samples[-1]}")
        return collect(samples)

    def cold(self) -> dict:
        def one():
            self.d.adb.sh(f"am force-stop {self.d.pkg}")
            time.sleep(2)
            f = self.d.start()
            time.sleep(3)
            return {"total_ms": float(f.get("TotalTime", "nan")), "wait_ms": float(f.get("WaitTime", "nan"))}
        return self.repeat("cold start", one)

    def warm(self) -> dict:
        def one():
            self.d.hide_ime()
            self.d.adb.sh("input keyevent 3")  # HOME
            time.sleep(2)
            f = self.d.start()
            time.sleep(1.5)
            return {"total_ms": float(f.get("TotalTime", "nan"))}
        return self.repeat("warm start", one)

    def memory(self) -> dict:
        def one():
            self.d.bring_to_front()
            gc = self.can_gc and self.d.force_gc()
            m = self.d.meminfo()
            pid = self.d.pid()
            m["threads"] = self.d.threads(pid) if pid else None
            m["after_gc"] = 1.0 if gc else 0.0
            return m
        return self.repeat("memory", one)

    def idle(self) -> dict:
        def one():
            self.d.bring_to_front()
            self.d.hide_ime()
            time.sleep(2)
            pid = self.d.pid()
            self.d.reset_frames()
            t0, c0 = time.monotonic(), self.d.cpu_ticks(pid)
            time.sleep(10)
            t1, c1 = time.monotonic(), self.d.cpu_ticks(pid)
            frames = self.d.frames_total()
            cpu = (c1 - c0) / 100.0 / (t1 - t0) * 100 if c0 is not None and c1 is not None else None
            return {"cpu_pct": cpu, "fps": frames / (t1 - t0)}
        return self.repeat("idle (10 s, on screen)", one)

    def output(self) -> dict:
        def one():
            self.d.bring_to_front()
            self.d.type_command("clear")
            time.sleep(1.5)
            self.d.wait_render_idle(10)
            self.d.reset_frames()
            self.d.type_command(f"seq 1 {self.output_lines}")
            seconds = self.d.wait_render_idle(timeout=180)
            f = self.d.frames()
            f["render_seconds"] = seconds
            self.d.type_command("clear")
            time.sleep(1)
            return f
        return self.repeat(f"output (seq 1 {self.output_lines})", one)

    def scroll(self) -> dict:
        # Fill scrollback once, then measure flinging through it.
        self.d.bring_to_front()
        self.d.type_command(f"seq 1 {self.output_lines}")
        self.d.wait_render_idle(timeout=180)
        self.d.hide_ime()

        def one():
            self.d.reset_frames()
            for _ in range(4):
                self.d.adb.sh("input swipe 540 900 540 1700 150")
                time.sleep(0.4)
            for _ in range(4):
                self.d.adb.sh("input swipe 540 1700 540 900 150")
                time.sleep(0.4)
            self.d.wait_render_idle(timeout=10)
            return self.d.frames()
        result = self.repeat("scroll (8 flings through scrollback)", one)
        self.d.type_command("clear")
        self.d.hide_ime()
        return result

    def rotation(self) -> dict:
        """Live Activities after rotations. A single-Activity app should settle at 1."""
        if not self.can_gc:
            log("rotation: skipped (needs a debuggable package to force GC)")
            return {"skipped": "not debuggable"}
        sh = self.d.adb.sh
        saved = (sh("settings get system accelerometer_rotation").strip(),
                 sh("settings get system user_rotation").strip())
        try:
            def one():
                self.d.bring_to_front()
                self.d.hide_ime()
                sh("settings put system accelerometer_rotation 0")
                for r in (1, 0, 1, 0):
                    sh(f"settings put system user_rotation {r}")
                    time.sleep(4)
                time.sleep(20)
                self.d.force_gc()
                self.d.force_gc()
                return {"activities": self.d.meminfo().get("activities")}
            return self.repeat("rotation (4 rotations, 20 s, 2x GC)", one)
        finally:
            sh(f"settings put system user_rotation {saved[1]}")
            sh(f"settings put system accelerometer_rotation {saved[0]}")


# -------------------------------------------------------------------------------------- reporting


def metadata(adb: Adb, dev: Device, args) -> dict:
    prop = lambda k: adb.sh(f"getprop {k}").strip()
    name, code = dev.version()
    battery = adb.sh("dumpsys battery")
    level = re.search(r"level: (\d+)", battery)
    plugged = re.search(r"(AC|USB|Wireless) powered: true", battery)
    rate = re.search(r"refreshRate=([\d.]+)", adb.sh("dumpsys display | grep -m1 refreshRate"))
    try:
        sha = subprocess.run(["git", "-C", str(REPO), "rev-parse", "--short", "HEAD"],
                             capture_output=True, text=True).stdout.strip()
    except OSError:
        sha = "?"
    return {
        "timestamp": dt.datetime.now().astimezone().isoformat(timespec="seconds"),
        "package": dev.pkg,
        "version_name": name,
        "version_code": code,
        "debuggable": dev.debuggable(),
        "repo_head": sha,
        "device": f"{prop('ro.product.manufacturer')} {prop('ro.product.model')}",
        "android": prop("ro.build.version.release"),
        "sdk": prop("ro.build.version.sdk"),
        "refresh_rate_hz": float(rate.group(1)) if rate else None,
        "battery_pct": int(level.group(1)) if level else None,
        "charging": bool(plugged),
        "runs": args.runs,
        "output_lines": args.lines,
    }


UNITS = {"_ms": "ms", "_pct": "%", "_kb": "KB", "fps": "fps", "seconds": "s"}


def unit(metric: str) -> str:
    for suffix, u in UNITS.items():
        if metric.endswith(suffix):
            return u
    return ""


def markdown(report: dict, baseline: dict | None) -> str:
    m = report["meta"]
    lines = [
        f"# Verb benchmark — {m['version_name']} on {m['device']}",
        "",
        f"- **When:** {m['timestamp']}",
        f"- **Package:** `{m['package']}` (versionCode {m['version_code']}, debuggable={m['debuggable']}), repo `{m['repo_head']}`",
        f"- **Device:** Android {m['android']} (SDK {m['sdk']}), {m['refresh_rate_hz']} Hz, battery {m['battery_pct']}%, charging={m['charging']}",
        f"- **Method:** 1 discarded warm-up + {m['runs']} measured runs per scenario; median / p90 / min / max.",
        "",
    ]
    if baseline:
        bm = baseline["meta"]
        lines += [f"Compared against **{bm['version_name']}** measured {bm['timestamp']}.", ""]
    head = "| Scenario | Metric | Median | p90 | Min | Max | n |" + (" Baseline median | Δ |" if baseline else "")
    lines += [head, "|" + "---|" * (head.count("|") - 1)]
    for scen, metrics in report["results"].items():
        if "skipped" in metrics:
            lines.append(f"| {scen} | skipped: {metrics['skipped']} | | | | | |")
            continue
        for metric, s in metrics.items():
            if not s.get("n"):
                continue
            u = unit(metric)
            row = f"| {scen} | {metric} | {s['median']} {u} | {s['p90']} | {s['min']} | {s['max']} | {s['n']} |"
            if baseline:
                b = baseline.get("results", {}).get(scen, {}).get(metric, {})
                if b.get("n"):
                    delta = s["median"] - b["median"]
                    pct = f" ({delta / b['median'] * 100:+.0f}%)" if b["median"] else ""
                    row += f" {b['median']} | {delta:+.2f}{pct} |"
                else:
                    row += " – | – |"
            lines.append(row)
    lines += ["", "See `docs/perf/README.md` for what each metric means and its limits."]
    return "\n".join(lines) + "\n"


# ------------------------------------------------------------------------------------------- main


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--package", default=DEBUG_PACKAGE)
    ap.add_argument("--serial", help="adb device serial, when more than one is connected")
    ap.add_argument("--runs", type=int, default=5, help="measured runs per scenario (default 5)")
    ap.add_argument("--scenarios", default=",".join(ALL_SCENARIOS),
                    help=f"comma-separated subset of {','.join(ALL_SCENARIOS)}")
    ap.add_argument("--lines", type=int, default=20000, help="lines of output for output/scroll")
    ap.add_argument("--out", default=str(REPO / "docs/perf/results"))
    ap.add_argument("--baseline", help="earlier result JSON to compare against")
    ap.add_argument("--allow-real-app", action="store_true",
                    help="permit benchmarking com.aistudio.verb.app (cold start kills its sessions)")
    args = ap.parse_args()

    scenarios = [s.strip() for s in args.scenarios.split(",") if s.strip()]
    unknown = set(scenarios) - set(ALL_SCENARIOS)
    if unknown:
        sys.exit(f"unknown scenarios: {', '.join(sorted(unknown))}")
    if args.package == REAL_PACKAGE and not args.allow_real_app:
        sys.exit("Refusing to benchmark the real app without --allow-real-app: the cold-start "
                 "scenario force-stops it, ending every terminal and agent session inside.")

    adb = Adb(args.serial)
    devices = [l for l in adb.run("devices").splitlines()[1:] if l.strip().endswith("device")]
    if not args.serial and len(devices) != 1:
        sys.exit(f"expected exactly one device, found {len(devices)}; pass --serial")
    dev = Device(adb, args.package)
    if "versionName" not in adb.sh(f"dumpsys package {args.package} | grep -m1 versionName"):
        sys.exit(f"{args.package} is not installed")

    dev.wake()
    if dev.keyguard_showing():
        sys.exit("The phone is locked. Unlock it and run again.")
    dev.bring_to_front()
    if not dev.focused_on_app():
        sys.exit(f"{args.package} did not come to the front")
    if {"output", "scroll"} & set(scenarios) and dev.find("type a command") is None:
        sys.exit("Verb is not showing a terminal with its command field. Open a shell terminal first.")

    meta = metadata(adb, dev, args)
    if not meta["charging"]:
        log("warning: phone is not charging; thermal and power state will vary between runs")
    log(f"benchmarking {meta['package']} {meta['version_name']} on {meta['device']} ({args.runs} runs)")

    bench = Bench(dev, args.runs, args.lines)
    results = {}
    started = time.monotonic()
    try:
        for s in scenarios:
            try:
                results[s] = getattr(bench, s)()
            except Exception as e:  # keep the rest of the run; record why this one failed
                log(f"{s}: FAILED: {e}")
                results[s] = {"skipped": f"failed: {e}"}
    finally:
        dev.hide_ime()

    report = {"meta": meta | {"duration_s": round(time.monotonic() - started)}, "results": results}
    baseline = json.loads(Path(args.baseline).read_text()) if args.baseline else None

    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    model = re.sub(r"[^A-Za-z0-9]+", "-", meta["device"]).strip("-")
    stem = f"{dt.date.today()}-{meta['version_name']}-{model}"
    (out / f"{stem}.json").write_text(json.dumps(report, indent=2) + "\n")
    md = markdown(report, baseline)
    (out / f"{stem}.md").write_text(md)
    print("\n" + md)
    log(f"wrote {out / stem}.json and .md")
    return 0


if __name__ == "__main__":
    sys.exit(main())
