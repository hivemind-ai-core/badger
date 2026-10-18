"""Interleave retained before/after executables to check gains and regressions.

Usage: confirm_main.py BEFORE.json AFTER.json OUTPUT.json
Seven measured repetitions after one warmup, four workers only. Reuses the
exact commands, fixtures and immutable binaries from the original runs.
"""
from pathlib import Path
import argparse
import json
import os
import random
import re
import statistics
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("before", type=Path)
    parser.add_argument("after", type=Path)
    parser.add_argument("dest", type=Path)
    parser.add_argument("--workers", type=int, nargs="+", default=[4])
    parser.add_argument("--cases", nargs="+", default=["spmc", "spsc_cap64", "mpmc", "spsc_byte_budget", "messages"])
    parser.add_argument("--reps", type=int, default=7)
    args = parser.parse_args()
    before, after, dest = args.before, args.after, args.dest
    inputs = {"before": json.loads(before.read_text()), "after": json.loads(after.read_text())}
    cases = args.cases
    env = {k: v for k, v in os.environ.items() if not k.startswith("LANG_")}
    output = {"before_file": str(before), "after_file": str(after),
              "method": f"Workers {args.workers}. Sequential before/after binaries interleaved in seed-1 shuffled order each round, one warmup plus {args.reps} measured fresh runs per case/version. Same commands and immutable executables as retained runs. No CPU pinning or frequency control; background load uncontrolled. --bench excludes compilation.",
              "runs": [], "summary": [], "components": []}
    rng = random.Random(1)
    for repeat in range(args.reps + 1):
        jobs = [(case, version, workers) for case in cases for version in inputs for workers in args.workers]
        rng.shuffle(jobs)
        for case, version, workers in jobs:
            data = inputs[version]
            row = next(row for row in data["runs"] if row["case"] == case and row["workers"] == workers)
            p = subprocess.run(row["cmd"], cwd=data["source_snapshot"]["snapshot"], env=env,
                               capture_output=True, text=True, timeout=60, check=True)
            match = re.search(r"bench: run ([\d.]+) ms", p.stderr)
            assert match, p.stderr
            output["runs"].append({"case": case, "version": version, "workers": workers, "warmup": repeat == 0,
                                   "cmd": row["cmd"], "ms": float(match[1]),
                                   "stdout": p.stdout, "stderr": p.stderr})
    for case in cases:
        for workers in args.workers:
            row = {"case": case, "workers": workers}
            for version in inputs:
                vals = [r["ms"] for r in output["runs"] if r["case"] == case and r["version"] == version and r["workers"] == workers and not r["warmup"]]
                row[version] = {"median_ms": statistics.median(vals), "min_ms": min(vals), "max_ms": max(vals)}
            row["change_percent"] = (row["after"]["median_ms"] / row["before"]["median_ms"] - 1) * 100
            output["summary"].append(row)
            print(json.dumps(row), flush=True)
    for version in inputs:
        samples = {}
        for row in output["runs"]:
            if row["case"] == "messages" and row["version"] == version and not row["warmup"]:
                for line in row["stdout"].splitlines():
                    label, value = line.rsplit(": ", 1)
                    samples.setdefault((row["workers"], label), []).append(int(value.split()[0]))
        for (workers, label), vals in samples.items():
            row = {"version": version, "workers": workers, "label": label, "median_ns": statistics.median(vals), "min_ns": min(vals), "max_ns": max(vals)}
            output["components"].append(row)
            print(json.dumps(row), flush=True)
    dest.write_text(json.dumps(output, indent=2) + "\n")


if __name__ == "__main__":
    main()
