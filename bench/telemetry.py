#!/usr/bin/env python3
"""Run the language telemetry workload in fresh processes; see telemetry.md."""

import argparse
import datetime
import hashlib
import json
import math
import os
import platform
import random
import re
import shutil
import signal
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MASK = (1 << 64) - 1
DEFAULT = dict(events=65536, generators=4, parsers=8, batch_size=64,
               ingest_capacity=128, shards=8, shard_capacity=256, cache_capacity=128,
               sink_capacity=16, enrich_every=8, miss_yields=1, flush_every=10000,
               sample_every=16, hash_rounds=1, seed=20261016)
PROFILES = {
    "balanced": {},
    "backpressure": dict(ingest_capacity=1, shard_capacity=1, cache_capacity=1, sink_capacity=1),
    "fanout": dict(enrich_every=1, miss_yields=4),
    "cpu": dict(hash_rounds=8),
}
PERF_EVENTS = "task-clock,context-switches,cpu-migrations,cycles,instructions,cache-misses,LLC-loads,LLC-load-misses"


def mix(value):
    a = ((value ^ (value >> 30)) * 13787848793156543929) & MASK
    b = ((a ^ (a >> 27)) * 10723151780598845931) & MASK
    return b ^ (b >> 31)


def event_hash(event_id, seed, rounds):
    """Independent scalar oracle; never included in measured execution."""
    state = (mix((event_id + seed) & MASK) + 1) & MASK
    payload = bytearray()
    for _ in range(32):
        state ^= (state << 13) & MASK
        state ^= state >> 7
        state ^= (state << 17) & MASK
        payload.extend(state.to_bytes(8, "little"))
    h = 14695981039346656037
    for _ in range(rounds):
        for byte in payload:
            h = ((h ^ byte) * 1099511628211) & MASK
    return h


def oracle(config):
    """Check every payload byte, route, ID, tag, and cache-key cardinality."""
    totals = [dict(shard=i, count=0, id_sum=0, checksum=0) for i in range(config["shards"])]
    keys = set()
    samples = [0] * config["shards"]
    for event_id in range(config["events"]):
        h = event_hash(event_id, config["seed"], config["hash_rounds"])
        source = mix((event_id + config["seed"]) & MASK)
        ip = source % 64 if source % 10 < 8 else 64 + source % 4096
        tag = 0
        if event_id % config["enrich_every"] == 0:
            keys.add(ip)
            tag = mix(ip)
        shard = h % config["shards"]
        t = totals[shard]
        if event_id % config["sample_every"] == 0:
            samples[shard] += 1
        t["count"] += 1
        t["id_sum"] += event_id
        t["checksum"] = (t["checksum"] + (h ^ tag)) & MASK
    requests = (config["events"] - 1) // config["enrich_every"] + 1
    return dict(totals=totals, samples=samples, cache=dict(keys=len(keys), misses=len(keys), requests=requests))


def bucket_upper_ns(index):
    if index < 32:
        return (index + 1) * 1000 - 1
    group = (index - 16) // 16
    value = index - group * 16
    return (value + 1) * (1 << group) * 1000 - 1


def percentile(histogram, fraction):
    count = sum(histogram)
    if count == 0:
        return None
    target = math.ceil(count * fraction)
    seen = 0
    for i, n in enumerate(histogram):
        seen += n
        if seen >= target:
            # Last bucket is saturated; no finite upper bound there.
            return None if i == 511 else bucket_upper_ns(i)
    raise ValueError("invalid histogram")


def validate(record, config, workers, expected):
    if record.get("schema") != 1 or record.get("workload") != "telemetry":
        raise ValueError("unsupported workload output")
    if record["config"] != config or record["workers"] != workers:
        raise ValueError("workload configuration or worker count was not applied")
    if record["elapsed_ns"] <= 0:
        raise ValueError("invalid pipeline duration")
    result = record["result"]
    if result["totals"] != expected["totals"] or result["cache"] != expected["cache"]:
        raise ValueError("event conservation, hash, routing, or cache oracle mismatch")
    if [s["shard"] for s in result["shards"]] != list(range(config["shards"])):
        raise ValueError("missing/duplicate shard statistics")
    histogram = [0] * 512
    max_ns = 0
    samples = 0
    for s, t in zip(result["shards"], result["totals"]):
        h = s["histogram"]
        if len(h) != 512 or any(type(n) is not int or n < 0 for n in h):
            raise ValueError("invalid latency histogram")
        if sum(h) != s["samples"] or s["samples"] != expected["samples"][s["shard"]]:
            raise ValueError("latency sample accounting mismatch")
        expected_windows = (t["count"] + config["flush_every"] - 1) // config["flush_every"]
        if s["windows"] != expected_windows:
            raise ValueError("missing full/partial summary flush")
        histogram = [a + b for a, b in zip(histogram, h)]
        max_ns = max(max_ns, s["max_ns"])
        samples += s["samples"]
    if samples != (config["events"] - 1) // config["sample_every"] + 1:
        raise ValueError("wrong number of end-to-end samples")
    return dict(events_per_second=config["events"] * 1e9 / record["elapsed_ns"],
                histogram=histogram, samples=samples, max_ns=max_ns,
                latency_upper_ns={p: percentile(histogram, q) for p, q in
                                  [("p50", .5), ("p99", .99), ("p999", .999)]},
                semantic_allocations=dict(payload_arrays=config["events"],
                                          batch_arrays=(config["events"] + config["batch_size"] - 1) // config["batch_size"],
                                          enrichment_tasks=expected["cache"]["requests"],
                                          reply_channels=expected["cache"]["requests"]),
                allocator_calls=None)


def command_output(args):
    p = subprocess.run(args, cwd=ROOT, text=True, capture_output=True, check=True)
    return p.stdout.strip()


def run_process(command, env, timeout):
    """wait4 gives per-run RSS and context switches, without cumulative-child maxima.

    File-backed output avoids pipe deadlock. A new process group also lets a
    timeout terminate perf and its child together. No measurement sleeps in Lang.
    """
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        start = time.monotonic()
        p = subprocess.Popen(command, cwd=ROOT, env=env, stdout=stdout, stderr=stderr,
                             start_new_session=True)
        try:
            while True:
                pid, status, usage = os.wait4(p.pid, os.WNOHANG)
                if pid:
                    p.returncode = os.waitstatus_to_exitcode(status)
                    break
                if time.monotonic() - start > timeout:
                    raise TimeoutError(f"benchmark exceeded {timeout}s: {command}")
                time.sleep(.01)
        except BaseException:
            try:
                os.killpg(p.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            _, status, _ = os.wait4(p.pid, 0)
            p.returncode = os.waitstatus_to_exitcode(status)
            raise
        wall = time.monotonic() - start
        stdout.seek(0)
        stderr.seek(0)
        out = stdout.read().decode()
        err = stderr.read().decode()
        if p.returncode:
            raise RuntimeError(f"benchmark exit {p.returncode}:\n{err[-8000:]}\n{out[-2000:]}")
        cpu = usage.ru_utime + usage.ru_stime
        return out, err, dict(wall_s=wall, user_s=usage.ru_utime, system_s=usage.ru_stime,
                              cpu_percent_one_core=cpu / wall * 100,
                              peak_rss_kib=usage.ru_maxrss,
                              voluntary_context_switches=usage.ru_nvcsw,
                              involuntary_context_switches=usage.ru_nivcsw)


def parse_perf(raw):
    counters = {}
    for line in raw.splitlines():
        fields = line.split(";")
        if len(fields) < 3 or fields[2].strip() not in PERF_EVENTS.split(","):
            continue
        value, unit, name = (field.strip() for field in fields[:3])
        try:
            number = float(value)
        except ValueError:
            number = None
        counters[name] = dict(value=number, unit=unit, status=value if number is None else "counted",
                              runtime_and_scaling=fields[3:])

    def ratio(numerator, denominator):
        n = counters.get(numerator, {}).get("value")
        d = counters.get(denominator, {}).get("value")
        return n / d if n is not None and d else None

    return dict(counters=counters, ipc=ratio("instructions", "cycles"),
                llc_load_miss_ratio=ratio("LLC-load-misses", "LLC-loads"))


def run_one(lang, config, workers, timeout, expected, perf=False, runtime_stats=False):
    env = {k: v for k, v in os.environ.items() if not k.startswith("BADGER_") and k != "TELEMETRY_CONFIG"}
    env["TELEMETRY_CONFIG"] = json.dumps(config, sort_keys=True)
    env["LC_ALL"] = "C"
    if runtime_stats:
        env["BADGER_RUNTIME_STATS"] = "1"
    command = [str(lang), "run", "--release", "--bench", "--workers", str(workers), "bench/telemetry.bdg"]
    with tempfile.TemporaryDirectory(prefix="telemetry-perf-") as tmp:
        perf_file = Path(tmp) / "perf.csv"
        if perf:
            command = ["perf", "stat", "-x", ";", "-o", str(perf_file), "-e", PERF_EVENTS, "--", *command]
        out, err, process = run_process(command, env, timeout)
        record = json.loads(out)
        metrics = validate(record, config, workers, expected)
        bench = re.search(r"bench: run ([\d.]+) ms \(compilation ([\d.]+) ms", err)
        perf_raw = perf_file.read_text() if perf else None
        return dict(workload=record, metrics=metrics, process_including_jit=process,
                    cli_run_ms=float(bench[1]) if bench else None,
                    jit_compile_ms=float(bench[2]) if bench else None,
                    stderr=err, perf_including_jit=perf_raw,
                    perf_metrics_including_jit=parse_perf(perf_raw) if perf_raw else None)


def stats(values):
    return dict(median=statistics.median(values), min=min(values), max=max(values), runs=values)


def summarize(runs):
    groups = {}
    for r in runs:
        key = (r["profile"], r["workers"])
        groups.setdefault(key, []).append(r["measurement"])
    result = []
    for (profile, workers), measurements in sorted(groups.items()):
        histogram = [sum(m["metrics"]["histogram"][i] for m in measurements) for i in range(512)]
        result.append(dict(profile=profile, workers=workers,
                           throughput=stats([m["metrics"]["events_per_second"] for m in measurements]),
                           pipeline_ms=stats([m["workload"]["elapsed_ns"] / 1e6 for m in measurements]),
                           peak_rss_kib=stats([m["process_including_jit"]["peak_rss_kib"] for m in measurements]),
                           process_cpu_percent_one_core=stats([m["process_including_jit"]["cpu_percent_one_core"] for m in measurements]),
                           voluntary_context_switches=stats([m["process_including_jit"]["voluntary_context_switches"] for m in measurements]),
                           involuntary_context_switches=stats([m["process_including_jit"]["involuntary_context_switches"] for m in measurements]),
                           pooled_samples=sum(histogram),
                           pooled_latency_upper_ns={p: percentile(histogram, q) for p, q in
                                                    [("p50", .5), ("p99", .99), ("p999", .999)]}))
    baselines = {r["profile"]: r["throughput"]["median"] for r in result if r["workers"] == 1}
    for r in result:
        base = baselines.get(r["profile"])
        r["speedup"] = r["throughput"]["median"] / base if base else None
        r["efficiency"] = r["speedup"] / r["workers"] if base else None
    return result


def markdown(report):
    if report["arguments"]["smoke"]:
        return ("# Telemetry correctness smoke check\n\n"
                f"Passed {len(report['runs'])} fresh-process runs on revision `{report['revision']}`.\n\n"
                "Partial batches, rendezvous channels, empty shards, per-event fan-out, "
                "summary rollover, and final draining passed the independent oracle on 1, 2, and 4 workers. "
                "These small cases support correctness checks only. Raw records are in the JSON.\n")
    out = ["# Concurrent telemetry indexer", "",
           f"Revision `{report['revision']}`; {report['cpu']}; allowed CPUs {report['affinity']}.", "",
           "Pipeline throughput uses in-program monotonic timing from setup through drain. "
           "Latency is sampled per event, starting before its batch is built and ending after its aggregation. "
           "Quantiles below are pooled histogram bucket upper bounds. "
           "RSS, CPU, context switches, and optional perf counters cover the whole process, including JIT.", "",
           "| Profile | Workers | Events/s (median) | Speedup | Efficiency | p50 ≤ ms | p99 ≤ ms | p99.9 ≤ ms | Peak RSS MiB |",
           "|---|---:|---:|---:|---:|---:|---:|---:|---:|"]
    for r in report["summary"]:
        lat = r["pooled_latency_upper_ns"]
        cells = [r["profile"], str(r["workers"]), f"{r['throughput']['median']:,.0f}",
                 f"{r['speedup']:.2f}" if r["speedup"] is not None else "—",
                 f"{100 * r['efficiency']:.1f}%" if r["efficiency"] is not None else "—"]
        cells.extend(f"{lat[p] / 1e6:.3f}" if lat[p] is not None else "overflow" for p in ["p50", "p99", "p999"])
        cells.append(f"{r['peak_rss_kib']['median'] / 1024:.1f}")
        out.append("| " + " | ".join(cells) + " |")
    out += ["", "Process-wide counters (including compilation), medians:", "",
            "| Profile | Workers | CPU % of one core | Voluntary switches | Involuntary switches |",
            "|---|---:|---:|---:|---:|"]
    for r in report["summary"]:
        out.append(f"| {r['profile']} | {r['workers']} | {r['process_cpu_percent_one_core']['median']:.1f} | "
                   f"{r['voluntary_context_switches']['median']:,.0f} | {r['involuntary_context_switches']['median']:,.0f} |")
    out += ["", "All measured runs passed the independent payload/hash/routing/cache oracle. "
            "Raw runs, histograms, managed heap snapshots, process counters, config, and executable/source hashes are in the JSON.", "",
            "Semantic payload/task/channel allocation counts are recorded; allocator calls are unavailable "
            "and reported as null. Managed heap snapshots are neither cumulative allocations nor peak live heap. "
            "At least 100,000 latency samples per cell is recommended before interpreting p99.9. "
            "Memory scaling requires repeating at larger event counts (see bench/telemetry.md).", ""]
    return "\n".join(out)


def positive(text):
    value = int(text)
    if value <= 0:
        raise argparse.ArgumentTypeError("must be positive")
    return value


def parse_args(argv=None):
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", type=Path, required=True, help="output prefix (.json and .md)")
    ap.add_argument("--lang", type=Path, default=ROOT / "target/release/badger")
    ap.add_argument("--profiles", nargs="+", choices=PROFILES, default=list(PROFILES))
    ap.add_argument("--workers", nargs="+", type=positive, default=[1, 2, 4, 8])
    ap.add_argument("--events", type=positive, default=DEFAULT["events"])
    ap.add_argument("--parsers", type=positive, default=DEFAULT["parsers"])
    ap.add_argument("--sample-every", type=positive, default=DEFAULT["sample_every"])
    ap.add_argument("--seed", type=int, default=DEFAULT["seed"])
    ap.add_argument("--reps", type=positive, default=3)
    ap.add_argument("--warmups", type=int, default=1)
    ap.add_argument("--timeout", type=positive, default=300)
    ap.add_argument("--perf", action="store_true", help="separate diagnostic pass with process-wide perf stat")
    ap.add_argument("--runtime-stats", action="store_true", help="separate pass with BADGER_RUNTIME_STATS=1")
    ap.add_argument("--smoke", action="store_true", help="small edge cases, worker counts 1/2/4; no performance claims")
    args = ap.parse_args(argv)
    if not 0 <= args.seed <= MASK or args.warmups < 0:
        ap.error("seed must fit U64 and warmups must be nonnegative")
    if not hasattr(os, "wait4") or not sys.platform.startswith("linux"):
        ap.error("per-process resource measurement currently requires Linux wait4")
    if args.perf and shutil.which("perf") is None:
        ap.error("--perf requires perf in PATH")
    args.lang = args.lang.resolve()
    args.out = args.out.resolve()
    return args


def main(argv=None):
    args = parse_args(argv)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    affinity = sorted(os.sched_getaffinity(0))
    cpu = next((s.split(":", 1)[1].strip() for s in Path("/proc/cpuinfo").read_text().splitlines()
                if s.startswith("model name")), platform.processor())
    report = dict(schema=1, timestamp=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                  revision=command_output(["git", "rev-parse", "HEAD"]),
                  dirty=command_output(["git", "status", "--short"]),
                  platform=platform.platform(), cpu=cpu, affinity=affinity,
                  lang=str(args.lang), binary_sha256=hashlib.sha256(args.lang.read_bytes()).hexdigest(),
                  source_sha256={str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest()
                                 for p in [ROOT / "bench/telemetry.bdg", Path(__file__).resolve()]},
                  arguments={k: str(v) if isinstance(v, Path) else v for k, v in vars(args).items()},
                  warmups=[], runs=[], diagnostics=[])
    if args.smoke:
        cases = []
        for name, overrides in [
            ("partial", dict(events=257, flush_every=17, sample_every=1)),
            ("rendezvous", dict(events=65, batch_size=8, ingest_capacity=0, shard_capacity=0,
                                cache_capacity=0, sink_capacity=0, flush_every=1, enrich_every=1, sample_every=1)),
            ("empty-shards", dict(events=1, generators=8, parsers=2, shards=8, sample_every=1)),
        ]:
            cases.extend((name, w, {**DEFAULT, **overrides}) for w in [1, 2, 4])
        warmups, reps = 0, 1
    else:
        cases = [(name, w, {**DEFAULT, **PROFILES[name], "events": args.events,
                            "parsers": args.parsers, "sample_every": args.sample_every, "seed": args.seed})
                 for name in dict.fromkeys(args.profiles) for w in dict.fromkeys(args.workers)]
        warmups, reps = args.warmups, args.reps
    # Compute each unique oracle once, outside all timing windows.
    expected = {}
    for name, _, config in cases:
        if name not in expected:
            print(f"oracle: {name} ({config['events']} events)", file=sys.stderr, flush=True)
            expected[name] = oracle(config)
    rng = random.Random(args.seed)
    for stage, count in [("warmups", warmups), ("runs", reps)]:
        for repetition in range(count):
            order = cases.copy()
            rng.shuffle(order)
            for name, workers, config in order:
                print(f"{stage} {repetition + 1}: {name} w{workers}", file=sys.stderr, flush=True)
                measurement = run_one(args.lang, config, workers, args.timeout, expected[name])
                report[stage].append(dict(profile=name, workers=workers, repetition=repetition + 1, measurement=measurement))
                args.out.with_suffix(".partial.json").write_text(json.dumps(report, indent=2) + "\n")
    if args.perf or args.runtime_stats:
        # Instrumented results never contribute to uninstrumented medians.
        for name, workers, config in cases:
            print(f"diagnostics: {name} w{workers}", file=sys.stderr, flush=True)
            measurement = run_one(args.lang, config, workers, args.timeout, expected[name], args.perf, args.runtime_stats)
            report["diagnostics"].append(dict(profile=name, workers=workers, measurement=measurement))
    report["summary"] = summarize(report["runs"])
    args.out.with_suffix(".json").write_text(json.dumps(report, indent=2) + "\n")
    args.out.with_suffix(".md").write_text(markdown(report))
    args.out.with_suffix(".partial.json").unlink(missing_ok=True)
    print(args.out.with_suffix(".md"))


if __name__ == "__main__":
    main()
