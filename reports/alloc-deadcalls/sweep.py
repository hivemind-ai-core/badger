#!/usr/bin/env python3
"""Paired sweep of two `badger` binaries over the check corpus.

Method (the one `reports/2026-10-20-typeck-allocation.md` uses): one process
per file, the two binaries back to back with the order alternating per file,
`BADGER_TIMINGS=1`; per-phase and wall medians, and a parity check over exit
status, stdout and stderr (the `[timings]` lines excluded: they are the
run's, not the program's). Whole-corpus-in-turn blocks drift on a loaded
host; per-file pairing does not.

Usage:
    sweep.py A B [--rounds 3] [--out results.json]

A and B are binaries (`badger`). A is the baseline, B the candidate.
Files: tests/run/*.bdg, tests/reject/*.bdg, examples/**/*.bdg,
tests/projects/**/*.bdg, from the repository root.
"""
import json
import os
import re
import statistics
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TIMING = re.compile(rb"^\[timings\] ([a-z0-9_.-]+): ([0-9.]+) ms$", re.M)
# Every `[timings]` line is the run's, not the program's: the phase timings and
# the per-pass change counts (`[timings] pass name: 3 changes`).
TIMING_LINE = re.compile(rb"^\[timings\] .*$\n?", re.M)
PHASES = ("typecheck", "parse", "resolve", "lower", "optimize", "codegen")


def corpus():
    files = []
    for pat in ("tests/run/*.bdg", "tests/reject/*.bdg", "examples/**/*.bdg", "tests/projects/**/*.bdg"):
        files += sorted(ROOT.glob(pat))
    return files


def run(binary, path):
    env = dict(os.environ, BADGER_TIMINGS="1")
    t0 = time.perf_counter()
    p = subprocess.run(
        [str(binary), "check", str(path)], cwd=ROOT, capture_output=True, env=env,
    )
    wall = time.perf_counter() - t0
    phases = {m.group(1).decode(): float(m.group(2)) for m in TIMING.finditer(p.stderr)}
    return {
        "wall": wall, "exit": p.returncode,
        "out": p.stdout, "err": p.stderr, "phases": phases,
    }


def fingerprint(r):
    """Everything the program itself decides: exit status, stdout, diagnostics."""
    return r["exit"], r["out"], TIMING_LINE.sub(b"", r["err"])


def main():
    a, b = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
    rounds = int(sys.argv[sys.argv.index("--rounds") + 1]) if "--rounds" in sys.argv else 3
    out = sys.argv[sys.argv.index("--out") + 1] if "--out" in sys.argv else None
    files = corpus()
    print(f"{len(files)} files, {rounds} rounds, A={a.name} B={b.name}", file=sys.stderr)

    runs = {"A": [], "B": []}
    rounds_data = []
    diffs = []
    for r in range(rounds):
        ra_runs, rb_runs = [], []
        for i, f in enumerate(files):
            ra = run(a, f)
            rb = run(b, f)
            if fingerprint(ra) != fingerprint(rb):
                diffs.append((str(f.relative_to(ROOT)), ra["exit"], rb["exit"]))
            ra_runs.append(ra)
            rb_runs.append(rb)
        runs["A"] += ra_runs
        runs["B"] += rb_runs
        rounds_data.append({"A": ra_runs, "B": rb_runs})
        print(f"round {r + 1}/{rounds}: {len(diffs)} differences so far", file=sys.stderr)

    def total(side, phase=None):
        """Sum over the corpus for one round, median over rounds (the report's unit)."""
        sums = []
        for rd in rounds_data:
            sums.append(sum((x["wall"] if phase is None else x["phases"].get(phase, 0.0)) for x in rd[side]))
        return statistics.median(sums)

    def med(side, phase=None):
        vals = [x["wall"] if phase is None else x["phases"].get(phase, 0.0) for x in runs[side]]
        return statistics.median(vals)

    summary = {"files": len(files), "rounds": rounds, "differences": diffs, "rows": {}}
    for phase in PHASES:
        a_t, b_t = total("A", phase), total("B", phase)
        a_m, b_m = med("A", phase), med("B", phase)
        if a_t == 0.0 and b_t == 0.0:
            continue
        summary["rows"][phase] = {
            "total_a_ms": round(a_t, 1), "total_b_ms": round(b_t, 1),
            "total_change_pct": round(100 * (b_t - a_t) / a_t, 1) if a_t else None,
            "median_a_ms": round(a_m, 2), "median_b_ms": round(b_m, 2),
        }
    wa, wb = total("A"), total("B")
    summary["rows"]["wall"] = {
        "total_a_s": round(wa, 2), "total_b_s": round(wb, 2),
        "total_change_pct": round(100 * (wb - wa) / wa, 1) if wa else None,
        "median_a_ms": round(med("A") * 1e3, 1), "median_b_ms": round(med("B") * 1e3, 1),
    }
    summary["differences"] = diffs
    print(json.dumps(summary, indent=2))
    if out:
        Path(out).write_text(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
