#!/usr/bin/env python3
"""Adjacent-pair compiler comparisons using the existing application contracts."""
import argparse
import gzip
import importlib.util
import json
from pathlib import Path
import random
import statistics
import subprocess
import sys
import tempfile

import run as gc


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    loaded = importlib.util.module_from_spec(spec)
    sys.modules[name] = loaded
    spec.loader.exec_module(loaded)
    return loaded


rw = module("realworld_contract", gc.ROOT / "bench/realworld/run.py")
compare = module("compare_contract", gc.ROOT / "bench/compare/run.py")
CASES = [*(f"realworld/{name}" for name in ["cpu", "reconcile", "state", "json", "messaging", "pipeline"]),
         *(f"compare/{name}" for name in ["records", "snapshots", "batches"])]


def validate(record, suite, name, n, seed, warmups, samples, checksum=None):
    if checksum is None:
        checksum = (rw if suite == "realworld" else compare).oracle(name, n, seed)
    if suite == "realworld":
        return rw.validate(record, checksum, warmups, samples)
    values = compare.parse_samples(record["stdout"], warmups + samples, checksum)
    record["iterations_ns"] = [v["elapsed_ns"] for v in values]
    record["warm_ns"] = record["iterations_ns"][warmups:]
    record["first_ns"] = record["iterations_ns"][0]
    return record


def summarize(runs):
    rows = []
    cells = sorted({(r["suite"], r["case"], r["workers"]) for r in runs})
    for suite, name, workers in cells:
        selected = [r for r in runs if (r["suite"], r["case"], r["workers"]) == (suite, name, workers)]
        variants = {}
        paired = {}
        for label in ["baseline", "candidate"]:
            group = [r for r in selected if r["variant"] == label]
            if not group:
                continue
            variants[label] = {metric: rw.stats(values) for metric, values in {
                "kernel_ns": [statistics.median(r["process"]["warm_ns"]) for r in group],
                "peak_rss_bytes": [r["process"]["peak_rss_bytes"] for r in group],
            }.items()}
            paired[label] = {r["repetition"]: statistics.median(r["process"]["warm_ns"]) for r in group}
        row = dict(suite=suite, case=name, workers=workers, variants=variants)
        if "baseline" in paired and "candidate" in paired:
            complete = sorted(paired["baseline"].keys() & paired["candidate"].keys())
            ratios = [paired["candidate"][i] / paired["baseline"][i] - 1 for i in complete]
            row["paired_work_change"] = rw.stats(ratios)
            row["faster_pairs"] = sum(r < 0 for r in ratios)
        rows.append(row)
    return rows


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--baseline", type=Path, required=True)
    ap.add_argument("--candidate", type=Path)
    ap.add_argument("--baseline-revision", required=True)
    ap.add_argument("--candidate-revision")
    ap.add_argument("--reps", type=int, default=7)
    ap.add_argument("--warmups", type=int, default=3)
    ap.add_argument("--samples", type=int, default=5)
    ap.add_argument("--workers", type=int, nargs="+", default=[1, 2, 4, 8])
    ap.add_argument("--seed", type=int, default=20261018)
    ap.add_argument("--cases", nargs="+", choices=CASES, default=CASES)
    ap.add_argument("--diagnostic", action="store_true", help="collect GC phases in separately built diagnostic binaries")
    ap.add_argument("--allocations", action="store_true", help="add perturbed process-wide allocation counts to diagnostics")
    ap.add_argument("--smoke", action="store_true")
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()
    if min(args.reps, args.warmups, args.samples, *args.workers) < 1 or max(args.workers) > 64:
        ap.error("positive counts and 1..64 workers required")
    if args.candidate and not args.candidate_revision:
        ap.error("--candidate requires --candidate-revision")
    if args.allocations and not args.diagnostic:
        ap.error("--allocations requires --diagnostic")
    with gc.benchmark_lock():
        run(args)


def run(args):
    binaries = {"baseline": args.baseline.resolve()}
    if args.candidate:
        binaries["candidate"] = args.candidate.resolve()
    cells = [(suite, name, workers) for case in args.cases
             for suite, name in [case.split("/")] for workers in args.workers]
    runs = []
    sources = rw.source_hashes()
    for path in [Path(__file__), Path(gc.__file__), gc.ROOT / "bench/compare/run.py", compare.PROGRAMS / "workload.bdg"]:
        sources[str(path.relative_to(gc.ROOT))] = rw.digest(path)
    report = dict(schema=1, status="running", diagnostic=args.diagnostic, arguments=vars(args) | {
        "baseline": str(args.baseline), "candidate": str(args.candidate), "out": str(args.out)},
        revision=rw.output(["git", "rev-parse", "HEAD"]), host_before=rw.host(),
        binaries={name: dict(path=str(path), sha256=rw.digest(path)) for name, path in binaries.items()},
        sources=sources, runs=runs)
    args.out.mkdir(parents=True, exist_ok=True)
    rng = random.Random(args.seed)
    env = rw.clean_env()
    with tempfile.TemporaryDirectory(prefix="badger-memory-applications-") as temporary:
        tmp = Path(temporary)
        launcher = tmp / "measure"
        subprocess.run(["cc", "-O2", "-Wall", "-Wextra", "-Werror", str(rw.SUITE / "measure.c"),
                        "-o", str(launcher)], check=True)
        report["launcher_sha256"] = rw.digest(launcher)
        fixture = tmp / "orders.jsonl"
        rw.fixture(fixture, 257 if args.smoke else rw.SIZES["json"], 37)
        # Oracles are calculated once outside all measured process intervals.
        expected = {(suite, name): (257 if args.smoke else (rw.SIZES[name] if suite == "realworld"
                   else compare.WORKLOADS[name][0])) for suite, name, _ in cells}
        checksums = {(suite, name): (rw if suite == "realworld" else compare).oracle(name, n, 37)
                     for (suite, name), n in expected.items()}
        for repetition in range(args.reps):
            order = cells.copy()
            rng.shuffle(order)
            for suite, name, workers in order:
                variants = list(binaries)
                rng.shuffle(variants)
                n = expected[suite, name]
                for variant in variants:
                    binary = binaries[variant]
                    program = (rw.PROGRAMS if suite == "realworld" else compare.PROGRAMS) / "workload.bdg"
                    arguments = [name, n, 37, *([workers] if suite == "realworld" else []),
                                 args.warmups, args.samples, *([str(fixture)] if suite == "realworld" else [])]
                    command = [str(binary), "run", "--release", "--bench", "--workers", str(workers),
                               str(program), "--", *map(str, arguments)]
                    phases = tmp / "gc.json"
                    child_env = env | ({"BADGER_GC_PROFILE": str(phases)} if args.diagnostic else {})
                    allocations_path = tmp / "allocations.json"
                    if args.allocations:
                        child_env["BADGER_ALLOC_PROFILE"] = str(allocations_path)
                    before = gc.host_context()
                    process = rw.process(command, child_env, 180, expect_ready=suite == "realworld", launcher=launcher)
                    after = gc.host_context()
                    validate(process, suite, name, n, 37, args.warmups, args.samples, checksums[suite, name])
                    profile = json.loads(phases.read_text()) if args.diagnostic else None
                    if profile is not None:
                        gc.validate_sweep(profile)
                        phases.unlink()
                    allocations = None
                    if args.allocations:
                        allocations = json.loads(allocations_path.read_text())
                        gc.validate_allocations(allocations)
                        allocations_path.unlink()
                    runs.append(dict(suite=suite, case=name, workers=workers, variant=variant,
                                     repetition=repetition, n=n, process=process, gc=profile, allocations=allocations,
                                     host_before=before, host_after=after))
                    print(f"rep={repetition} {suite}/{name} w{workers} {variant}", flush=True)
                    report["summary"] = summarize(runs)
                    report["host_after"] = rw.host()
                    (args.out / "measurements.json.gz").write_bytes(gzip.compress(
                        (json.dumps(report, indent=2) + "\n").encode(), mtime=0))
    assert sources == {p: rw.digest(gc.ROOT / p) for p in sources}, "sources changed while measuring"
    assert report["binaries"] == {name: dict(path=str(path), sha256=rw.digest(path)) for name, path in binaries.items()}
    report["status"] = "complete"
    (args.out / "measurements.json.gz").write_bytes(gzip.compress(
        (json.dumps(report, indent=2) + "\n").encode(), mtime=0))
    print(json.dumps(report["summary"], indent=2))


if __name__ == "__main__":
    main()
