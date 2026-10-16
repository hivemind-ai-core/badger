#!/usr/bin/env python3
"""Interleave immutable before/after binaries for the local pointer-option probe."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import random
import re
import statistics
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--before", type=Path, required=True)
    ap.add_argument("--after", type=Path, required=True)
    args = ap.parse_args()
    env = {k: v for k, v in os.environ.items() if not k.startswith("LANG_")}
    binaries = {"before": args.before.resolve(), "after": args.after.resolve()}
    probe = HERE / "pointer-options.lang"
    output = dict(revision=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT).decode().strip(),
                  binaries={k: dict(path=str(p), sha256=hashlib.sha256(p.read_bytes()).hexdigest())
                            for k, p in binaries.items()}, source_sha256=hashlib.sha256(probe.read_bytes()).hexdigest(),
                  runs=[], summary={}, note="Uncontrolled host frequency/activity; five interleaved fresh runs after warmup.")

    def run(mode, workers, dump=False):
        cmd = [str(binaries[mode]), "run", "--release", "--workers", str(workers), str(probe)]
        p = subprocess.run(cmd[:2] + (["--dump-mir"] if dump else []) + cmd[2:], cwd=ROOT, env=env,
                           capture_output=True, text=True, timeout=120)
        assert p.returncode == 0, p.stderr
        ns = int(re.search(r"pointer_options_ns=(\d+) check=7000000", p.stdout)[1])
        if dump:
            (HERE / "evidence" / f"pointer-options-{mode}-w{workers}.mir").write_text(p.stderr)
        return dict(mode=mode, workers=workers, ns=ns, stdout=p.stdout, stderr=p.stderr if not dump else "saved separately")

    cells = [(mode, w) for mode in binaries for w in [1, 2, 4, 8]]
    for mode, w in cells:
        run(mode, w)
    order = cells * 5
    random.Random(20261016).shuffle(order)
    for mode, w in order:
        output["runs"].append(run(mode, w))
    for mode, w in cells:
        times = [r["ns"] for r in output["runs"] if (r["mode"], r["workers"]) == (mode, w)]
        output["summary"][f"{mode}-w{w}"] = dict(median_ns=statistics.median(times), min_ns=min(times), max_ns=max(times))
    for mode in binaries:
        run(mode, 1, dump=True)
    output["production_diff"] = subprocess.check_output(["git", "diff", "--", "crates"], cwd=ROOT).decode()
    output["production_source_sha256"] = {
        name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest() for name in [
            "crates/lang_mir/src/niche.rs", "crates/lang_mir/src/lib.rs", "crates/lang_driver/src/lib.rs"]}
    (HERE / "evidence" / "option-niche-comparison.json").write_text(json.dumps(output, indent=2) + "\n")
    print(json.dumps(output["summary"], indent=2))


if __name__ == "__main__":
    main()
