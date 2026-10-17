#!/usr/bin/env python3
"""Interleave immutable compilers using the telemetry harness and independent oracle."""
import argparse
import datetime
import gzip
import hashlib
import json
import os
from pathlib import Path
import random
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "bench"))
import telemetry as t


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--before", type=Path, required=True)
    ap.add_argument("--after", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--events", type=int, default=8192)
    ap.add_argument("--reps", type=int, default=3)
    args = ap.parse_args()
    binaries = dict(baseline=args.before.resolve(), current=args.after.resolve())
    configs = {name: {**t.DEFAULT, **overrides, "events": args.events} for name, overrides in t.PROFILES.items()}
    expected = {name: t.oracle(config) for name, config in configs.items()}
    sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
    report = dict(revision=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT).decode().strip(),
                  timestamp=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                  affinity=sorted(os.sched_getaffinity(0)),
                  binaries={k: dict(path=str(p), sha256=sha(p)) for k, p in binaries.items()},
                  sources={str(p.relative_to(ROOT)): sha(p) for p in [Path(__file__).resolve(), ROOT / "bench/telemetry.lang", ROOT / "bench/telemetry.py"]},
                  events=args.events, repetitions=args.reps, warmups_per_cell=1, runs=[])
    cells = [(mode, name, w) for mode in binaries for name in configs for w in [1, 2, 4, 8]]
    rng = random.Random(20261016)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    for stage, count in [("warmup", 1), ("run", args.reps)]:
        for repetition in range(count):
            order = cells.copy()
            rng.shuffle(order)
            for mode, name, workers in order:
                print(f"{stage} {repetition + 1}: {mode} {name} w{workers}", flush=True)
                measurement = t.run_one(binaries[mode], configs[name], workers, 180, expected[name])
                if stage == "run":
                    report["runs"].append(dict(mode=mode, profile=name, workers=workers,
                                               repetition=repetition + 1, measurement=measurement))
    report["summary"] = {mode: t.summarize([r for r in report["runs"] if r["mode"] == mode]) for mode in binaries}
    args.out.write_bytes(gzip.compress((json.dumps(report, indent=2) + "\n").encode(), mtime=0))


if __name__ == "__main__":
    main()
