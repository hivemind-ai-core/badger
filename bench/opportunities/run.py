#!/usr/bin/env python3
"""Revision-pinned opportunity benchmarks; works with baseline and optimized APIs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import re
import statistics
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / "bench"))
from telemetry import run_process

EXPECTED = dict(helpers=19800000, options=9900000, sequence=13333266666,
                array_reads=204226912, comparisons=200000, plain_options=9900000,
                plain_sequence=13333266666, array_iterate=2096128000, byte_iterate=2000000)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--bin-dir", type=Path, required=True)
    ap.add_argument("--peer-bin-dir", type=Path, help="interleave a second immutable binary for final comparison")
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--reps", type=int, default=5)
    ap.add_argument("--workers", type=int, nargs="+", default=[1, 2, 4, 8])
    args = ap.parse_args()
    bins = args.bin_dir.resolve()
    modes = {"current": bins}
    if args.peer_bin_dir:
        modes["baseline"] = args.peer_bin_dir.resolve()
    args.out = args.out.resolve()
    args.out.mkdir(parents=True, exist_ok=True)
    env = {k: v for k, v in os.environ.items() if not k.startswith("LANG_")}

    def run(command, extra=None):
        out, err, resources = run_process([str(x) for x in command], {**env, **(extra or {})}, 180)
        return dict(stdout=out, stderr=err, **resources)

    report = dict(revision=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT).decode().strip(),
                  dirty=subprocess.check_output(["git", "status", "--short"], cwd=ROOT).decode(),
                  platform=platform.platform(), affinity=sorted(os.sched_getaffinity(0)),
                  binaries={mode: {p.name: sha(p) for p in directory.iterdir() if p.is_file()} for mode, directory in modes.items()},
                  sources={str(p.relative_to(ROOT)): sha(p) for p in [*HERE.glob("*.lang"),
                      Path(__file__), ROOT / "crates/lang_runtime/examples/opportunities.rs",
                      ROOT / "crates/lang_codegen/examples/mir_audit.rs"]}, samples=[], summaries={}, diagnostics={})
    files = subprocess.check_output(["git", "ls-files", "crates", "Cargo.toml", "Cargo.lock"], cwd=ROOT).decode().splitlines()
    report["production_sha256"] = {f: sha(ROOT / f) for f in files if "/examples/" not in f}

    def sample(mode, case, workers):
        result = run([modes[mode] / "lang", "run", "--release", "--workers", workers, HERE / f"{case}.lang"])
        if case == "text":
            matches = re.findall(r"kind=(\S+) chars=(\d+) repetitions=100 slice_ns=(\d+) check=(\d+)", result["stdout"])
            assert len(matches) == 8 and all(int(check) == 100 for _, _, _, check in matches), result
            result["ns"] = {f"slice-{kind}-{n}": int(ns) for kind, n, ns, _ in matches}
        else:
            matches = re.findall(r"^(\w+): (\d+) ns; check (\d+)", result["stdout"], re.M)
            assert len(matches) == (7 if case == "compute" else 2), result
            assert all(EXPECTED[name] == int(check) for name, _, check in matches), result
            result["ns"] = {name: int(ns) for name, ns, _ in matches}
        return dict(mode=mode, case=case, workers=workers, **result)

    cells = [(mode, case, w) for mode in modes for case in ["compute", "arrays", "text"] for w in args.workers]
    for mode, case, w in cells:
        sample(mode, case, w)
    order = cells * args.reps
    random.Random(20261016).shuffle(order)
    for mode, case, w in order:
        record = sample(mode, case, w)
        report["samples"].append(record)
        print(f"{mode} {case} w{w}: {record['ns']}", flush=True)
        (args.out / "partial.json").write_text(json.dumps(report, indent=2) + "\n")
    for mode, case, w in cells:
        samples = [r for r in report["samples"] if (r["mode"], r["case"], r["workers"]) == (mode, case, w)]
        report["summaries"][f"{mode}-{case}-w{w}"] = {
            metric: dict(median=statistics.median(values), min=min(values), max=max(values))
            for metric, values in [(name, [r["ns"][name] for r in samples]) for name in samples[0]["ns"]]
        }
        report["summaries"][f"{mode}-{case}-w{w}"]["process_peak_rss_kib"] = statistics.median(r["peak_rss_kib"] for r in samples)
    for case in ["buffer", "prefix", "pool", "old"]:
        for gen in ["0", "1"]:
            records = [run([bins / "opportunities", case], {"LANG_GC_GENERATIONAL": gen}) for _ in range(args.reps)]
            report["diagnostics"][f"{case}-gen{gen}"] = records
    report["diagnostics"]["lifetimes"] = run([bins / "lang", "run", "--release", "--workers", 1, HERE / "lifetimes.lang"])
    for case in ["compute", "arrays"]:
        saved = args.out / f"{case}.langmir"
        report["diagnostics"][f"{case}-compile"] = [run([bins / "lang", "compile", HERE / f"{case}.lang", "-o", saved], {"LANG_TIMINGS": "1"}) for _ in range(args.reps)]
        metrics = run([bins / "mir_audit", saved])
        metrics["saved_mir_bytes"] = saved.stat().st_size
        report["diagnostics"][f"{case}-code"] = metrics
    (args.out / "measurements.json").write_text(json.dumps(report, indent=2) + "\n")
    (args.out / "partial.json").unlink()


if __name__ == "__main__":
    main()
