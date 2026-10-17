#!/usr/bin/env python3
"""GC attribution and broader coverage; all diagnostic policies are experiments."""
import argparse
import collections
import datetime
import gzip
import hashlib
import json
import os
from pathlib import Path
import random
import re
import statistics
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "bench"))
import telemetry as t

COVERAGE = {
    "map-new": dict(mode="map-new", n=8192, rounds=16, payload=1, retain=8192),
    "map-replace": dict(mode="map-replace", n=65536, rounds=8, payload=1, retain=256),
    "map-strings": dict(mode="map-strings", n=16384, rounds=8, payload=32, retain=1024),
    "set-builder": dict(mode="set-builder", n=65536, rounds=8, payload=1, retain=1024),
    **{name: dict(mode=name, n=16384, rounds=1, payload=32, retain=capacity)
       for name, capacity in [("messages", 64), ("messages-slow", 1),
                              ("messages-retained", 256), ("messages-fanout", 64),
                              ("messages-int", 64), ("messages-int-slow", 1),
                              ("messages-int-retained", 256), ("messages-int-fanout", 64)]},
    **{f"{mode}-sustained": dict(mode=mode, n=262144, rounds=1, payload=32, retain=capacity)
       for mode, capacity in [("messages-int", 64), ("messages-int-slow", 1),
                              ("messages-int-retained", 256), ("messages-int-fanout", 64)]},
    "bytes-transient": dict(mode="bytes", n=65536, rounds=1, payload=256, retain=32),
    "words-transient": dict(mode="words", n=65536, rounds=1, payload=256, retain=32),
    "bytes-survivors": dict(mode="bytes", n=65536, rounds=1, payload=256, retain=8192),
    "snapshots": dict(mode="snapshots", n=65536, rounds=1, payload=96, retain=32),
    **{f"persistent-{size}": dict(mode="persistent-int", n=131072, rounds=1, payload=1, retain=size)
       for size in [64, 256, 4096]},
    **{f"persistent-{size}-major": dict(mode="persistent-int", n=524288, rounds=1, payload=1, retain=size)
       for size in [64, 4096]},
    "snapshots-major": dict(mode="snapshots", n=262144, rounds=1, payload=96, retain=32),
    **{name: dict(mode=name, n=65536, rounds=8, payload=1, retain=1)
       for name in ["visible", "stored", "collected", "eager"]},
}
POLICIES = {
    "control": {},
    "young64m": {"BADGER_GC_MINOR_BYTES": str(64 << 20)},
    "no-trim": {"BADGER_GC_TRIM": "0"},
    "minors32": {"BADGER_GC_MAX_MINORS": "32"},
    "pool256": {"BADGER_GC_POOL_MAX": "256"},
    "pool512": {"BADGER_GC_POOL_MAX": "512"},
    "pool1024": {"BADGER_GC_POOL_MAX": "1024"},
}


def expected_coverage(c):
    if c["mode"] == "persistent-int":
        latest = {i % c["retain"]: i for i in range(c["n"])}
        return len(latest) + sum(latest) + sum(latest.values())
    if c["mode"].startswith("map-") or c["mode"] == "set-builder":
        latest = {i % c["retain"]: i for i in range(c["n"])}
        total = len(latest) + sum(latest)
        if c["mode"] == "map-strings":
            total += sum(c["payload"] + i % 3 for i in latest.values())
        elif c["mode"] != "set-builder":
            total += sum(latest.values())
        return total * c["rounds"]
    if c["mode"].startswith("messages"):
        def payload_sum(first, end):
            return (31 * c["payload"] * (first + end - 1) * (end - first) // 2
                    + (end - first) * c["payload"] * (c["payload"] - 1) // 2)
        total = payload_sum(0, c["n"])
        if c["mode"].endswith("-retained"):
            total += payload_sum(max(0, c["n"] - c["retain"]), c["n"])
        return total * (2 if c["mode"].endswith("-fanout") else 1)
    if c["mode"] in ["bytes", "words"]:
        n = c["n"]
        q, r = divmod(n, 251)
        return q * 251 * 250 // 2 + r * (r - 1) // 2 + min(n, c["retain"]) * c["payload"] + max(0, c["retain"] - n)
    if c["mode"] == "snapshots":
        history = [0] * c["retain"]
        for i in range(0, c["n"], 64):
            history[(i // 64) % c["retain"]] = min(i + 1, 1024)
        return sum(c["payload"] + i % 3 for i in range(c["n"])) + sum(history) + min(c["n"], 1024)
    m = (c["n"] - 1) // 3
    return 3 * m * (m + 1) * c["rounds"]


def validate_coverage(record, config):
    if record["config"] != config or record["checksum"] != expected_coverage(config):
        raise ValueError(f"coverage oracle mismatch: {record}")
    if record["elapsed_ns"] <= 0:
        raise ValueError("invalid elapsed time")


def validate_allocations(profile):
    """Check byte and object conservation, not just successful JSON output."""
    if profile["schema"] != 1 or profile["scope"] != "process-managed-allocations":
        raise ValueError("unsupported allocation census")
    totals = profile["totals"]
    for metric in ["objects", "requested_bytes", "cell_bytes"]:
        for counts in [totals, *(s["counts"] for s in profile["sites"])]:
            n = {key: value[metric] for key, value in counts.items()}
            if (any(v < 0 for v in n.values())
                    or n["allocated"] != n["reclaimed_before_survival"] + n["reclaimed_after_survival"] + n["remaining"]
                    or n["first_survival"] != n["reclaimed_after_survival"] + n["remaining_after_survival"]
                    or n["remaining_after_survival"] > n["remaining"]
                    or n["minor_survivor_observations"] + n["full_survivor_observations"] < n["first_survival"]):
                raise ValueError("allocation census violates conservation")
        for key, volume in totals.items():
            if volume[metric] != sum(site["counts"][key][metric] for site in profile["sites"]):
                raise ValueError("allocation sites do not sum to totals")
        sizes = profile["allocation_sizes"]
        if totals["allocated"][metric] != (sizes["large_system_cells"][metric]
                + sum(c["allocated"][metric] for c in sizes["small_pool_classes"])):
            raise ValueError("allocation size classes do not sum to totals")
        if "requested_size_classes" in profile:
            sizes = profile["requested_size_classes"]
            if totals["allocated"][metric] != (sizes["over_1024"][metric]
                    + sum(c["allocated"][metric] for c in sizes["through_1024"])):
                raise ValueError("requested size classes do not sum to totals")
    if totals["remaining"]["objects"] != profile["remaining_metadata_objects"]:
        raise ValueError("allocation metadata disagrees with remaining objects")
    if totals["allocated"]["objects"] <= 0:
        raise ValueError("empty allocation census")


def summarize(rows):
    groups = collections.defaultdict(list)
    for row in rows:
        groups[(row["case"], row["workers"], row["variant"])].append(row)
    result = []
    for (case, workers, variant), runs in sorted(groups.items()):
        result.append(dict(case=case, workers=workers, variant=variant, repetitions=len(runs),
            elapsed_ms=t.stats([r["record"]["elapsed_ns"] / 1e6 for r in runs]),
            peak_rss_kib=t.stats([r["process"]["peak_rss_kib"] for r in runs]),
            collections=t.stats([r["heap_collections"] for r in runs]),
            gc_pause_ms=t.stats([r["gc_pause_ns"] / 1e6 for r in runs])))
    return result


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--baseline", type=Path, required=True)
    ap.add_argument("--candidate", type=Path)
    ap.add_argument("--baseline-revision", default="unspecified")
    ap.add_argument("--candidate-revision", default="unspecified")
    ap.add_argument("--diagnostic", type=Path)
    ap.add_argument("--allocations", action="store_true", help="add a separately perturbed managed-allocation census variant")
    ap.add_argument("--suite", choices=["coverage", "telemetry", "policy", "samples", "smoke"], required=True)
    ap.add_argument("--workers", type=int, nargs="+", default=[1, 2, 4, 8])
    ap.add_argument("--events", type=int, default=262144)
    ap.add_argument("--reps", type=int, default=3)
    ap.add_argument("--cases", nargs="+", choices=COVERAGE)
    ap.add_argument("--profiles", nargs="+", choices=t.PROFILES, default=list(t.PROFILES))
    ap.add_argument("--policies", nargs="+", choices=POLICIES, default=list(POLICIES))
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()
    if args.allocations and (args.diagnostic is None or args.suite in ["policy", "samples"]):
        ap.error("--allocations requires --diagnostic and coverage, telemetry or smoke")
    if args.suite in ["policy", "samples"] and args.diagnostic is None:
        ap.error("--diagnostic is required for policy and samples")
    if args.reps < 1 or any(w < 1 for w in args.workers):
        ap.error("positive repetitions and worker counts are required")
    if args.candidate and args.suite in ["policy", "samples"]:
        ap.error("--candidate is for ordinary coverage/telemetry comparisons")
    if args.cases and args.suite not in ["coverage", "smoke"]:
        ap.error("--cases requires coverage or smoke")
    args.out.mkdir(parents=True, exist_ok=True)
    binaries = {"baseline": args.baseline.resolve()}
    if args.candidate:
        binaries["candidate"] = args.candidate.resolve()
    if args.diagnostic:
        binaries["diagnostic"] = args.diagnostic.resolve()
    variants = {"baseline": ("baseline", {})}
    if args.candidate:
        variants["candidate"] = ("candidate", {})
    if args.diagnostic:
        variants["diagnostic"] = ("diagnostic", {})
        variants["diagnostic-off"] = ("diagnostic", {})
        if args.allocations:
            variants["diagnostic-allocations"] = ("diagnostic", {})
    if args.suite == "policy":
        variants = {k: ("diagnostic", v) for k, v in POLICIES.items() if k in args.policies}
    if args.suite == "samples":
        variants = {"samples": ("diagnostic", {"BADGER_PROFILE_SCOPE": "runtime", "BADGER_RUNTIME_STATS": "1"})}
    coverage = args.suite in ["coverage", "smoke"]
    cases = {k: v.copy() for k, v in COVERAGE.items()} if coverage else {
        name: {**t.DEFAULT, **t.PROFILES[name], "events": args.events} for name in args.profiles}
    if args.cases:
        cases = {k: cases[k] for k in args.cases}
    if args.suite == "smoke":
        for c in cases.values():
            c.update(n=257, rounds=2, payload=17, retain=4)
    # Calculate oracles once, outside subprocess measurements.
    oracles = {}
    if not coverage:
        cache = args.out.parent / "oracles"
        cache.mkdir(exist_ok=True)
        oracle_source = hashlib.sha256((ROOT / "bench/telemetry.py").read_bytes()).hexdigest()
        for name, config in cases.items():
            key = hashlib.sha256(json.dumps([oracle_source, config], sort_keys=True).encode()).hexdigest()
            path = cache / f"{key}.json"
            if not path.exists():
                path.write_text(json.dumps(t.oracle(config), sort_keys=True))
            oracles[name] = json.loads(path.read_text())
    rss_source = ROOT / "bench/gc/rss_spawn.c"
    rss_hash = hashlib.sha256(rss_source.read_bytes()).hexdigest()
    rss_helper = (args.out.parent / f"rss-spawn-{rss_hash[:16]}").resolve()
    if not rss_helper.exists():
        subprocess.run(["cc", "-O2", "-std=c11", "-Wall", "-Wextra", "-Werror", str(rss_source), "-o", str(rss_helper)], check=True)
    cells = [(name, w, v) for name in cases for w in args.workers for v in variants]
    rng = random.Random(20261017)
    runs = []
    def sha(path):
        # Avoid faulting a large debug executable into the runner's heap:
        # fork/exec children can inherit that RSS high-water mark.
        with path.open("rb") as file:
            return hashlib.file_digest(file, "sha256").hexdigest()
    report = dict(schema=2, timestamp=datetime.datetime.now(datetime.timezone.utc).isoformat(),
        revision=t.command_output(["git", "rev-parse", "HEAD"]),
        dirty=t.command_output(["git", "status", "--porcelain"]),
        affinity=sorted(os.sched_getaffinity(0)), arguments=vars(args) | {
            "baseline": str(args.baseline), "candidate": str(args.candidate),
            "diagnostic": str(args.diagnostic), "out": str(args.out)},
        host=t.command_output(["uname", "-a"]), toolchain=t.command_output(["rustc", "-Vv"]),
        production_tree=t.command_output(["git", "ls-tree", "HEAD", "crates", "Cargo.lock", "Cargo.toml"]),
        binaries={k: dict(path=str(p), sha256=sha(p)) for k, p in binaries.items()},
        sources={str(p.relative_to(ROOT)): sha(p) for p in [Path(__file__).resolve(), rss_source, ROOT / "bench/gc/coverage.bdg", ROOT / "bench/telemetry.bdg", ROOT / "bench/telemetry.py"]},
        runs=runs)
    for rep in range(-1, args.reps):
        order = cells.copy()
        rng.shuffle(order)
        for name, workers, variant in order:
            print(f"{args.suite} rep={rep} {name} w{workers} {variant}", flush=True)
            binary, settings = variants[variant]
            env = {k: v for k, v in os.environ.items() if not k.startswith("BADGER_") and k not in ["TELEMETRY_CONFIG", "GC_BENCH_CONFIG"]}
            env.update(settings)
            env["LC_ALL"] = "C"
            stem = f"{name}-w{workers}-{variant}-{rep}"
            gc_path = args.out.resolve() / f"{stem}.gc.json"
            allocation_path = args.out.resolve() / f"{stem}.allocations.json"
            if binary == "diagnostic" and variant != "diagnostic-off":
                env["BADGER_GC_PROFILE"] = str(gc_path)
            if variant == "diagnostic-allocations":
                env["BADGER_ALLOC_PROFILE"] = str(allocation_path)
            if args.suite == "samples":
                env["BADGER_PROFILE"] = str(args.out.resolve() / f"{stem}.folded")
            config = cases[name]
            env["GC_BENCH_CONFIG" if coverage else "TELEMETRY_CONFIG"] = json.dumps(config)
            command = [str(binaries[binary]), "run", "--release", "--bench", "--workers", str(workers),
                       "bench/gc/coverage.bdg" if coverage else "bench/telemetry.bdg"]
            out, err, process = t.run_process([str(rss_helper), *command], env, 180)
            rss = re.search(r"^\[gc-bench child rss\] (\d+)$", err, re.MULTILINE)
            if rss is None:
                raise ValueError("RSS helper did not report language-child usage")
            process["launcher_tree_peak_rss_kib"] = process["peak_rss_kib"]
            process["peak_rss_kib"] = int(rss[1])
            record = json.loads(out)
            if coverage:
                validate_coverage(record, config)
            else:
                t.validate(record, config, workers, oracles[name])
            before, after = (record["before"], record["after"]) if coverage else (record["heap_before"], record["heap_after"])
            phases = json.loads(gc_path.read_text()) if gc_path.exists() else None
            if "BADGER_GC_POOL_MAX" in settings:
                if phases is None or phases["policy"].get("pool_max_bytes") != int(settings["BADGER_GC_POOL_MAX"]):
                    raise ValueError("diagnostic binary did not apply the pool-size policy")
            gc_path.unlink(missing_ok=True)
            allocations = None
            if variant == "diagnostic-allocations":
                allocations = json.loads(allocation_path.read_text())
                validate_allocations(allocations)
                allocation_path.unlink()
            if rep >= 0:
                runs.append(dict(case=name, workers=workers, variant=variant, repetition=rep,
                    record=record, process=process, stderr=err, gc=phases, allocations=allocations,
                    heap_collections=after["collections"] - before["collections"],
                    gc_pause_ns=after["pause_total_ns"] - before["pause_total_ns"]))
                report["summary"] = summarize(runs)
                (args.out / "measurements.json.gz").write_bytes(gzip.compress((json.dumps(report, indent=2) + "\n").encode(), mtime=0))
    print(json.dumps(report["summary"], indent=2))


if __name__ == "__main__":
    main()
