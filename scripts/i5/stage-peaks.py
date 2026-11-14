#!/usr/bin/env python3
"""Joins audit stage records with host RSS samples to give each stage a measured peak.

    scripts/i5/stage-peaks.py <evidence/runtime-dir> <host-rss.tsv>

A stage's peak is the largest total RSS of the app uid among samples taken between its start_epoch
and end_epoch. The app itself and any idle terminal are included, so the baseline sample just before
the stage is printed beside it; the difference is what the stage added. A stage shorter than the
sampling interval, or one with no samples in its window, is reported as UNKNOWN rather than guessed.
"""
import pathlib
import sys


def read_meta(path):
    meta = {}
    for line in path.read_text(errors="replace").splitlines():
        if "=" in line:
            key, value = line.split("=", 1)
            meta.setdefault(key, value)
    return meta


def main():
    evid = pathlib.Path(sys.argv[1])
    samples = []
    for line in pathlib.Path(sys.argv[2]).read_text().splitlines()[1:]:
        parts = line.split("\t")
        if len(parts) >= 3 and parts[0].isdigit():
            samples.append((int(parts[0]), int(parts[1]), parts[3] if len(parts) > 3 else ""))

    print("unit\tstage\tattempt\tstatus\tduration_s\tbaseline_rss_kb\tpeak_rss_kb\tadded_kb\tsamples\ttop_at_peak")
    for meta_path in sorted(evid.glob("*/*.attempt*.meta")):
        m = read_meta(meta_path)
        start, end = m.get("start_epoch", ""), m.get("end_epoch", "")
        row = [m.get("unit"), m.get("stage"), m.get("attempt"), m.get("status"), m.get("duration_s", "-")]
        if not (start.isdigit() and end.isdigit()):
            print("\t".join(map(str, row + ["UNKNOWN", "UNKNOWN", "UNKNOWN", 0, "no start/end epoch"])))
            continue
        start, end = int(start), int(end)
        window = [s for s in samples if start <= s[0] <= end]
        before = [s for s in samples if s[0] < start]
        baseline = before[-1][1] if before else None
        if not window:
            print("\t".join(map(str, row + [baseline or "UNKNOWN", "UNKNOWN", "UNKNOWN", 0, "no samples in window"])))
            continue
        peak = max(window, key=lambda s: s[1])
        added = peak[1] - baseline if baseline is not None else "UNKNOWN"
        print("\t".join(map(str, row + [baseline or "UNKNOWN", peak[1], added, len(window), peak[2]])))


if __name__ == "__main__":
    main()
