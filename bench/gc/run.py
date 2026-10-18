#!/usr/bin/env python3
"""GC attribution and broader coverage; all diagnostic policies are experiments."""
import argparse
import collections
import datetime
from contextlib import contextmanager
import fcntl
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
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "bench"))
import telemetry as t

COVERAGE = {
    **{name: dict(mode=name, n=1024, rounds=512, payload=32, retain=1024)
       for name in ["entries-immediate", "entries-stored"]},
    "records-window": dict(mode="records-window", n=3000000, rounds=1, payload=4, retain=2000),
    **{name: dict(mode=name, n=1048576, rounds=1, payload=32, retain=1024)
       for name in ["lookup-map-misses", "lookup-list-misses", "lookup-map-int", "lookup-map-pointers", "lookup-array-int",
                    "lookup-list-pointers", "lookup-map-stored"]},
    **{name: dict(mode=name, n=64, rounds=524288, payload=32768, retain=64)
       for name in ["tasks-input", "tasks-result"]},
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
    "buffers-empty": dict(mode="buffers-empty", n=524288, rounds=1, payload=1, retain=1),
    "buffers-grow": dict(mode="buffers-grow", n=65536, rounds=1, payload=32, retain=64),
    "buffers-pointers": dict(mode="buffers-pointers", n=32768, rounds=1, payload=8, retain=128),
    **{f"buffers-capacity-{width}": dict(mode="buffers-capacity", n=65536, rounds=1, payload=width, retain=64)
       for width in [31, 32, 33]},
    "buffers-capacity-grow": dict(mode="buffers-capacity-grow", n=65536, rounds=1, payload=65, retain=64),
    "buffers-capacity-pointers": dict(mode="buffers-capacity-pointers", n=32768, rounds=1, payload=8, retain=128),
    "bytes-257": dict(mode="bytes", n=65536, rounds=1, payload=257, retain=32),
    "snapshots": dict(mode="snapshots", n=65536, rounds=1, payload=96, retain=32),
    **{f"persistent-{size}": dict(mode="persistent-int", n=131072, rounds=1, payload=1, retain=size)
       for size in [64, 256, 4096]},
    **{f"persistent-{size}-major": dict(mode="persistent-int", n=524288, rounds=1, payload=1, retain=size)
       for size in [64, 4096]},
    "snapshots-major": dict(mode="snapshots", n=262144, rounds=1, payload=96, retain=32),
    "parallel-maps": dict(mode="parallel-persistent-int", n=131072, rounds=8, payload=1, retain=1024),
    "parallel-snapshots": dict(mode="parallel-snapshots", n=65536, rounds=8, payload=96, retain=32),
    **{f"pool-burst-{retain}": dict(mode="pool-burst", n=65536, rounds=524288, payload=256, retain=retain)
       for retain in [1, 16, 256]},
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
    "sweep-scoped": {"BADGER_GC_SWEEP_REUSE": "0"},
    "sweep-reuse": {"BADGER_GC_SWEEP_REUSE": "1"},
    **{f"refill{kib}": {"BADGER_GC_POOL_REFILL_KIB": str(kib)} for kib in [256, 512, 2048]},
    **{f"inline{n}": {"BADGER_BUFFER_INLINE_BYTES": str(n)} for n in [0, 128, 256]},
    "inline256-pool256": {"BADGER_BUFFER_INLINE_BYTES": "256", "BADGER_GC_POOL_MAX": "256"},
}


def host_context():
    """Outside child timing; host-wide pressure is context, not attribution."""
    context = dict(wall_time_ns=time.time_ns(), load_average=os.getloadavg())
    for name, path in [("cpu_pressure", "/proc/pressure/cpu"),
                       ("cgroup_root_cpu_stat", "/sys/fs/cgroup/cpu.stat"),
                       ("cgroup_root_cpu_max", "/sys/fs/cgroup/cpu.max")]:
        try:
            context[name] = Path(path).read_text().strip()
        except OSError:
            pass
    return context


def measurement_order(cells, rng, paired=False):
    if not paired:
        order = cells.copy()
        rng.shuffle(order)
        return order
    groups = collections.defaultdict(list)
    for cell in cells:
        groups[cell[:2]].append(cell)
    pairs = list(groups.values())
    rng.shuffle(pairs)
    order = []
    for pair in pairs:
        rng.shuffle(pair)
        order.extend(pair)
    return order


@contextmanager
def benchmark_lock(path=None):
    path = path or Path(tempfile.gettempdir()) / f"badger-gc-benchmark-{os.getuid()}.lock"
    with path.open("a") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise RuntimeError("another GC benchmark is running; wait for it to finish") from error
        yield


def expected_coverage(c):
    if c["mode"].startswith("entries-"):
        return c["rounds"] * (c["n"] + sum(k * 31 + c["payload"] + k % 3 for k in range(c["n"])))
    if c["mode"] == "records-window":
        first = (c["n"] - 1) // c["retain"] * c["retain"]
        count = c["n"] - first
        return (first + c["n"] - 1) * count // 2 + count * (c["payload"] + 1)
    if c["mode"] in ["lookup-map-misses", "lookup-list-misses"]:
        return c["n"]
    if c["mode"].startswith("lookup-"):
        total = 0
        for i in range(c["n"]):
            k = i % (2 * c["retain"]) - 1
            if 0 <= k < c["retain"]:
                total += c["payload"] + k % 3 if c["mode"].endswith("pointers") else k * 31 + 7
        return total
    if c["mode"] in ["tasks-input", "tasks-result"]:
        payload_sum = 31 * c["payload"] * c["n"] * (c["n"] - 1) // 2
        payload_sum += c["n"] * c["payload"] * (c["payload"] - 1) // 2
        drained = expected_coverage(c | dict(mode="persistent-int", n=c["rounds"], retain=64))
        return payload_sum * (2 if c["mode"] == "tasks-result" else 1) + drained
    if c["mode"] == "buffers-empty":
        return c["n"]
    if c["mode"] in ["buffers-grow", "buffers-pointers", "buffers-capacity", "buffers-capacity-grow", "buffers-capacity-pointers"]:
        def value(i):
            first = 32 + i % 3 if c["mode"].endswith("pointers") else i * 31
            return first * c["payload"] + c["payload"] * (c["payload"] - 1) // 2
        return sum(value(i) for i in range(c["n"])) + sum(value(i) for i in range(max(0, c["n"] - c["retain"]), c["n"]))
    if c["mode"] == "pool-burst":
        allocated = sum(c["payload"] + i % 3 for i in range(c["n"]))
        retained = sum(c["payload"] + (slot * c["n"] // c["retain"]) % 3 for slot in range(c["retain"]))
        drained = expected_coverage(c | dict(mode="persistent-int", n=c["rounds"], retain=64))
        return allocated + retained + drained
    if c["mode"] in ["parallel-persistent-int", "parallel-snapshots"]:
        return c["rounds"] * expected_coverage(c | dict(mode=c["mode"].removeprefix("parallel-"), rounds=1))
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
    if config["mode"] in ["pool-burst", "tasks-input", "tasks-result"]:
        shots = record.get("checkpoints", [])
        if len(shots) != 3 or any(s["pool_reserved_bytes"] <= 0 for s in shots):
            raise ValueError("missing burst/churn/idle memory snapshots")
        # Collections are monotonic; idle means yielding, not an explicit GC.
        counts = [record["before"]["collections"], *(s["collections"] for s in shots)]
        if counts != sorted(counts):
            raise ValueError("nonmonotonic collection checkpoints")


def validate_sweep(profile):
    for collection in profile["collections"]:
        heap = collection["heap"]
        if "sweep_work_per_worker" not in heap:
            continue
        work, helpers = heap["sweep_work_per_worker"], heap["sweep_work_per_helper"]
        created = heap.get("sweep_helpers_created", len(helpers))
        if (not work or any(type(n) is not int or n < 0 for n in work + helpers)
                or sum(work) != heap["swept"]
                or len(helpers) != heap["sweep_helpers"]
                or len(helpers) > max(0, sum(n > 0 for n in work) - 1)
                or any(n == 0 for n in helpers) or sum(helpers) > sum(work)
                or type(created) is not int or not 0 <= created <= len(helpers)):
            raise ValueError("invalid sweep work/helper accounting")


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
    if "buffer_storage" in profile:
        storage = profile["buffer_storage"]
        events = storage["events"]
        required = {"native_alloc", "native_realloc_growth", "native_adopt", "native_free", "inline", "inline_to_native"}
        if (set(events) != required or any(set(event) != {"calls", "bytes"} for event in events.values())
                or any(type(v) is not int or v < 0 for event in events.values() for v in event.values())
                or any(type(storage[k]) is not int for k in ["native_owned_bytes_remaining", "native_owned_blocks_remaining"])):
            raise ValueError("invalid buffer storage events")
        charged = sum(events[k]["bytes"] for k in ["native_alloc", "native_realloc_growth", "native_adopt"])
        blocks = events["native_alloc"]["calls"] + events["native_adopt"]["calls"]
        remaining = charged - events["native_free"]["bytes"]
        live_blocks = blocks - events["native_free"]["calls"]
        if (remaining < 0 or live_blocks < 0 or storage["native_owned_bytes_remaining"] != remaining
                or storage["native_owned_blocks_remaining"] != live_blocks
                or events["inline_to_native"]["bytes"] != 0
                or events["inline_to_native"]["calls"] > min(events["inline"]["calls"], events["native_alloc"]["calls"])):
            raise ValueError("buffer storage ownership violates conservation")


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
    ap.add_argument("--suite", choices=["coverage", "telemetry", "policy", "coverage-policy", "samples", "smoke"], required=True)
    ap.add_argument("--workers", type=int, nargs="+", default=[1, 2, 4, 8])
    ap.add_argument("--events", type=int, default=262144)
    ap.add_argument("--reps", type=int, default=3)
    ap.add_argument("--seed", type=int, default=20261017, help="shuffle seed, saved with raw measurements")
    ap.add_argument("--paired", action="store_true", help="keep each case/worker's variants adjacent, shuffling groups and variant order")
    ap.add_argument("--cases", nargs="+", choices=COVERAGE)
    ap.add_argument("--profiles", nargs="+", choices=t.PROFILES, default=list(t.PROFILES))
    ap.add_argument("--policies", nargs="+", choices=POLICIES, default=list(POLICIES))
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()
    if args.allocations and (args.diagnostic is None or args.suite == "samples"):
        ap.error("--allocations requires --diagnostic and a nonsampling suite")
    if args.suite in ["policy", "coverage-policy", "samples"] and args.diagnostic is None:
        ap.error("--diagnostic is required for policy, coverage-policy and samples")
    if args.reps < 1 or any(w < 1 for w in args.workers):
        ap.error("positive repetitions and worker counts are required")
    if args.candidate and args.suite in ["policy", "coverage-policy", "samples"]:
        ap.error("--candidate is for ordinary coverage/telemetry comparisons")
    if args.cases and args.suite not in ["coverage", "coverage-policy", "smoke"]:
        ap.error("--cases requires coverage, coverage-policy or smoke")
    with benchmark_lock():
        run_benchmarks(args)


def run_benchmarks(args):
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
    if args.suite in ["policy", "coverage-policy"]:
        variants = {k: ("diagnostic", v) for k, v in POLICIES.items() if k in args.policies}
    if args.suite == "samples":
        variants = {"samples": ("diagnostic", {"BADGER_PROFILE_SCOPE": "runtime", "BADGER_RUNTIME_STATS": "1"})}
    coverage = args.suite in ["coverage", "coverage-policy", "smoke"]
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
    rng = random.Random(args.seed)
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
        order = measurement_order(cells, rng, args.paired)
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
            census = variant == "diagnostic-allocations" or (args.allocations and args.suite in ["policy", "coverage-policy"])
            if census:
                env["BADGER_ALLOC_PROFILE"] = str(allocation_path)
            if args.suite == "samples":
                env["BADGER_PROFILE"] = str(args.out.resolve() / f"{stem}.folded")
            config = cases[name]
            env["GC_BENCH_CONFIG" if coverage else "TELEMETRY_CONFIG"] = json.dumps(config)
            command = [str(binaries[binary]), "run", "--release", "--bench", "--workers", str(workers),
                       "bench/gc/coverage.bdg" if coverage else "bench/telemetry.bdg"]
            host_before = host_context()
            out, err, process = t.run_process([str(rss_helper), *command], env, 180)
            host_after = host_context()
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
            if phases is not None:
                validate_sweep(phases)
            if "BADGER_GC_POOL_MAX" in settings:
                if phases is None or phases["policy"].get("pool_max_bytes") != int(settings["BADGER_GC_POOL_MAX"]):
                    raise ValueError("diagnostic binary did not apply the pool-size policy")
            if "BADGER_GC_SWEEP_REUSE" in settings:
                if phases is None or phases["policy"].get("reuse_sweep_helpers") != (settings["BADGER_GC_SWEEP_REUSE"] == "1"):
                    raise ValueError("diagnostic binary did not apply the sweep-helper policy")
            if "BADGER_GC_POOL_REFILL_KIB" in settings:
                if phases is None or phases["policy"].get("pool_refill_bytes") != int(settings["BADGER_GC_POOL_REFILL_KIB"]) << 10:
                    raise ValueError("diagnostic binary did not apply the pool-refill policy")
            if "BADGER_BUFFER_INLINE_BYTES" in settings:
                if phases is None or phases["policy"].get("buffer_inline_bytes") != int(settings["BADGER_BUFFER_INLINE_BYTES"]):
                    raise ValueError("diagnostic binary did not apply the inline-buffer policy")
            gc_path.unlink(missing_ok=True)
            allocations = None
            if census:
                allocations = json.loads(allocation_path.read_text())
                validate_allocations(allocations)
                allocation_path.unlink()
            if rep >= 0:
                runs.append(dict(case=name, workers=workers, variant=variant, repetition=rep,
                    record=record, process=process, stderr=err, gc=phases, allocations=allocations,
                    host_before=host_before, host_after=host_after,
                    heap_collections=after["collections"] - before["collections"],
                    gc_pause_ns=after["pause_total_ns"] - before["pause_total_ns"]))
                report["summary"] = summarize(runs)
                (args.out / "measurements.json.gz").write_bytes(gzip.compress((json.dumps(report, indent=2) + "\n").encode(), mtime=0))
    print(json.dumps(report["summary"], indent=2))


if __name__ == "__main__":
    main()
