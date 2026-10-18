#!/usr/bin/env python3
"""Correctness-gated Badger/Go/Node application benchmarks (Linux, stdlib only)."""
import argparse
from contextlib import contextmanager
import datetime as dt
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import re
import selectors
import shutil
import signal
import statistics
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
SUITE = Path(__file__).resolve().parent
PROGRAMS = SUITE / "programs"
SIZES = dict(cpu=120000, reconcile=60000, json=8000, state=16384,
             messaging=65539, pipeline=120003)
PARALLEL = {"messaging", "pipeline"}
CONTRACT = "realworld-v1"


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def output(command):
    return subprocess.check_output(command, cwd=ROOT, text=True, stderr=subprocess.STDOUT).strip()


def clean_env():
    removed = {"NODE_OPTIONS", "NODE_PATH", "GODEBUG", "GOGC", "GOMEMLIMIT", "GOMAXPROCS",
               "GOFLAGS", "GOEXPERIMENT", "GOTOOLCHAIN", "GOWORK", "LD_PRELOAD", "LD_LIBRARY_PATH"}
    return {k: v for k, v in os.environ.items()
            if k not in removed and not k.startswith(("BADGER_", "LANG_"))} | {
                "LC_ALL": "C", "GOTOOLCHAIN": "local", "GOWORK": "off"}


def process(command, env, timeout, expect_ready=False):
    """One isolated process; wait4 RSS includes all threads, not previous children.

    Observe READY with a pipe (no clock synchronization across processes).
    Drain output concurrently; kill the entire process group on any error/timeout.
    """
    with tempfile.TemporaryFile() as errors, selectors.DefaultSelector() as selector:
        start = time.perf_counter_ns()
        child = subprocess.Popen(command, cwd=ROOT, env=env, stdout=subprocess.PIPE,
                                 stderr=errors, start_new_session=True)
        selector.register(child.stdout, selectors.EVENT_READ)
        data = bytearray()
        ready = None
        usage = None
        try:
            while True:
                if time.perf_counter_ns() - start > timeout * 1e9:
                    raise TimeoutError(f"timeout after {timeout}s: {command}")
                for key, _ in selector.select(.01):
                    chunk = os.read(key.fd, 65536)
                    if chunk:
                        data.extend(chunk)
                        if ready is None and b"\n" in data:
                            if data.split(b"\n", 1)[0] == b"READY":
                                ready = time.perf_counter_ns() - start
                    else:
                        selector.unregister(key.fileobj)
                pid, status, usage = os.wait4(child.pid, os.WNOHANG)
                if pid:
                    child.returncode = os.waitstatus_to_exitcode(status)
                    # The child exited; drain remaining pipe bytes before parsing.
                    data.extend(child.stdout.read())
                    break
            elapsed = time.perf_counter_ns() - start
        except BaseException:
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            _, status, _ = os.wait4(child.pid, 0)
            child.returncode = os.waitstatus_to_exitcode(status)
            raise
        finally:
            child.stdout.close()
        errors.seek(0)
        stderr = errors.read().decode(errors="replace")
        stdout = data.decode()
        if child.returncode != 0:
            raise RuntimeError(f"exit {child.returncode}: {command}\n{stderr[-6000:]}\n{stdout[-2000:]}")
        if expect_ready and ready is None:
            raise ValueError(f"missing observed READY marker: {stdout[:200]}")
        return dict(command=command, wall_ns=elapsed, ready_ns=ready,
                    peak_rss_bytes=usage.ru_maxrss * 1024,
                    user_ns=round(usage.ru_utime * 1e9), system_ns=round(usage.ru_stime * 1e9),
                    voluntary_switches=usage.ru_nvcsw, involuntary_switches=usage.ru_nivcsw,
                    stdout=stdout, stderr=stderr)


def score_sum(n, seed, rounds):
    # Closed-form affine transform, rather than the ports' inner recurrence loop.
    modulus = 1000003
    a = pow(17, rounds, modulus)
    b = 23 * (a - 1) * pow(16, -1, modulus) % modulus
    return sum((a * ((i + seed) % modulus) + b) % modulus for i in range(n))


def order(i, seed):
    return dict(id=i, customer=i % 1024, amount=(i * 31 + seed) % 100000,
                active=i % 7 != 0, note=f'order "{i}" / café ☘')


def oracle(name, n, seed):
    if name in {"cpu", "pipeline", "messaging"}:
        return score_sum(n, seed, 2 if name == "messaging" else 128)
    if name == "reconcile":
        customers = {i % 1024 for i in range(n) if i % 7 != 0}
        return len(customers) + sum((i % 1024 + 1) *
                                   (((i * 31 + seed) % 100000) *
                                    (100 - ((i % 1024 * 7 + seed) % 20)) // 100)
                                   for i in range(n) if i % 7 != 0)
    if name == "json":
        total = 0
        for i in range(n):
            row = order(i, seed)
            if row["active"]:
                total += (row["customer"] + 1) * (row["amount"] + row["amount"] // 5)
                total += i + len(row["note"].encode())
        return total
    if name == "state":
        # Derive the last writer at each snapshot, without copying maps.
        total = 0
        for end in [*range(256, n + 1, 256), n]:
            for key in range(min(1024, end)):
                writer = key + (end - 1 - key) // 1024 * 1024
                total += (key + 1) * ((writer * 31 + seed) % 100000)
        return total
    raise ValueError(f"unknown workload {name}")


def fixture(path, n, seed):
    path.write_text("".join(json.dumps(order(i, seed), ensure_ascii=False, separators=(",", ":")) + "\n"
                            for i in range(n)), encoding="utf8")


def validate(record, expected, warmups, samples):
    lines = record["stdout"].splitlines()
    if not lines or lines[0] != "READY" or len(lines) != warmups + samples + 1:
        raise ValueError(f"invalid protocol / sample count: {lines[:4]}")
    values = []
    for i, line in enumerate(lines[1:]):
        match = re.fullmatch(r"BENCH (\d+) (\d+) (\d+)", line)
        if match is None:
            raise ValueError(f"invalid benchmark line: {line}")
        iteration, elapsed, checksum = map(int, match.groups())
        if iteration != i or elapsed <= 0 or checksum != expected:
            raise ValueError(f"oracle/timer/iteration mismatch: {line}; expected checksum {expected}")
        values.append(elapsed)
    record["iterations_ns"] = values
    record["first_ns"] = values[0]
    record["warm_ns"] = values[warmups:]
    compile_time = re.search(r"compilation ([\d.]+) ms", record["stderr"])
    record["compile_ns"] = round(float(compile_time[1]) * 1e6) if compile_time else None
    return record


def stats(values):
    if not values:
        return None
    return dict(median=statistics.median(values), min=min(values), max=max(values), count=len(values),
                cv=statistics.pstdev(values) / statistics.mean(values) if statistics.mean(values) else 0)


def summarize(records):
    # Each process contributes one median: avoid presenting correlated iterations as independent trials.
    return {name: stats(values) for name, values in {
        "kernel_ns": [statistics.median(r["warm_ns"]) for r in records],
        "first_ns": [r["first_ns"] for r in records],
        "startup_ns": [r["ready_ns"] for r in records],
        "wall_ns": [r["wall_ns"] for r in records],
        "peak_rss_bytes": [r["peak_rss_bytes"] for r in records],
        "compile_ns": [r["compile_ns"] for r in records if r["compile_ns"] is not None],
        "cpu_ns": [r["user_ns"] + r["system_ns"] for r in records],
    }.items()}


def source_hashes():
    paths = [*PROGRAMS.glob("*"), SUITE / "run.py"]
    paths += list((ROOT / "crates").rglob("*.rs")) + list((ROOT / "crates").rglob("*.bdg"))
    paths += [ROOT / "Cargo.lock", ROOT / "Cargo.toml"]
    return {str(p.relative_to(ROOT)): digest(p) for p in sorted(paths) if p.is_file()}


def host():
    cpu_info = Path("/proc/cpuinfo").read_text()
    cpu_name = re.search(r"model name\s*:\s*(.*)", cpu_info)
    result = dict(hostname=platform.node(), platform=platform.platform(), machine=platform.machine(),
                  cpu=cpu_name[1] if cpu_name else "unknown", logical_cpus=os.cpu_count(),
                  affinity=sorted(os.sched_getaffinity(0)), load=os.getloadavg())
    for name, path in [("cpu_max", "/sys/fs/cgroup/cpu.max"), ("cpu_pressure", "/proc/pressure/cpu"),
                       ("memory_max", "/sys/fs/cgroup/memory.max"), ("meminfo", "/proc/meminfo")]:
        if Path(path).exists():
            result[name] = Path(path).read_text().strip()
    return result


@contextmanager
def benchmark_lock():
    path = Path(tempfile.gettempdir()) / f"badger-realworld-{os.getuid()}.lock"
    with path.open("w") as lock:
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise RuntimeError("another realworld benchmark is running") from error
        yield


def run(args):
    env = clean_env()
    sizes = {w: 257 if args.smoke else SIZES[w] * args.scale for w in args.workloads}
    paths = dict(badger=args.badger.resolve(), go=Path(shutil.which(args.go) or args.go).resolve(),
                 node=Path(shutil.which(args.node) or args.node).resolve())
    for name, path in paths.items():
        if not path.is_file():
            raise ValueError(f"missing {name} executable: {path}")
    tools = {name: dict(path=str(path), sha256=digest(path), version=output([str(path),
                       "version" if name == "go" else "--version"])) for name, path in paths.items()}
    before = source_hashes()
    stamp = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H-%M-%S.%fZ")
    report = dict(schema=1, suite=CONTRACT, stamp=stamp, status="running", smoke=args.smoke,
                  revision=output(["git", "rev-parse", "HEAD"]),
                  dirty=output(["git", "status", "--porcelain"]),
                  host_before=host(), tools=tools, source_hashes=before, config=dict(
                      sizes=sizes, seed=args.seed, workers=args.workers, processes=args.processes,
                      warmups=args.warmups, samples=args.samples, build_samples=args.build_samples,
                      order_seed=args.order_seed), builds={}, cells=[], failures=[], order=[])
    # Workload/config/machine identity, intentionally independent of Badger compiler revision/hash.
    identity = dict(contract=CONTRACT, runner_sha256=digest(SUITE / "run.py"), config=report["config"],
                    workload_hashes={p.name: digest(p) for p in PROGRAMS.glob("*")},
                    machine={k: report["host_before"].get(k) for k in
                             ["hostname", "platform", "cpu", "affinity", "cpu_max", "memory_max"]},
                    controls={k: tools[k]["sha256"] for k in ["node", "go"]})
    report["cohort"] = hashlib.sha256(json.dumps(identity, sort_keys=True).encode()).hexdigest()
    report["cohort_identity"] = identity
    report["environment"] = dict(overrides={"LC_ALL": "C", "GOTOOLCHAIN": "local", "GOWORK": "off",
                                           "GOMAXPROCS": "per-cell worker count"},
                                  removed_keys=sorted(set(os.environ) - set(env)))
    args.reports.mkdir(parents=True, exist_ok=True)
    destination = args.reports / f"{stamp}-{report['revision'][:12]}.json"
    print(f"Report: {destination}", flush=True)
    try:
        with tempfile.TemporaryDirectory(prefix="badger-realworld-") as temporary:
            tmp = Path(temporary)
            go_exe = tmp / "workload-go"
            mir = tmp / "workload.bdgmir"
            build_commands = {
                "badger": [str(paths["badger"]), "compile", str(PROGRAMS / "workload.bdg"), "-o", str(mir)],
                "go": [str(paths["go"]), "build", "-trimpath", "-o", str(go_exe), str(PROGRAMS / "workload.go")],
            }
            for language, command in build_commands.items():
                print(f"Preparing {language}", flush=True)
                trials = [process(command, env, args.timeout) for _ in range(args.build_samples)]
                artifact = mir if language == "badger" else go_exe
                report["builds"][language] = dict(samples=trials, wall_ns=stats([t["wall_ns"] for t in trials]),
                                                artifact_bytes=artifact.stat().st_size, sha256=digest(artifact))
            report["builds"]["node"] = None

            def command(language, name, n, seed, workers, warmups, samples, input_path):
                prefix = {"badger": [str(paths["badger"]), "run", "--release", "--bench", "--workers",
                                     str(workers), str(PROGRAMS / "workload.bdg"), "--"],
                          "go": [str(go_exe)],
                          "node": [str(paths["node"]), str(PROGRAMS / "workload.js")]}[language]
                return prefix + list(map(str, [name, n, seed, workers, warmups, samples, input_path]))

            # Check queue/partial-batch and snapshot boundaries, multiple input seeds, every port/worker.
            cases = [(1, 0), (65, 991), (257, 37), (1025, 7)]
            for name, n in sizes.items():
                workers_list = args.workers if name in PARALLEL else [1]
                for workers in workers_list:
                    for language in paths:
                        cell = dict(language=language, workload=name, workers=workers, n=n,
                                    expected=oracle(name, n, args.seed), validation=[], records=[])
                        report["cells"].append(cell)
                        print(f"Verify {language:6} {name:10} w{workers}", flush=True)
                        for small, seed in cases:
                            input_path = tmp / "validation.ndjson"
                            if name == "json":
                                fixture(input_path, small, seed)
                            trial = process(command(language, name, small, seed, workers, 0, 1, input_path),
                                            env | {"GOMAXPROCS": str(workers)}, args.timeout, True)
                            validate(trial, oracle(name, small, seed), 0, 1)
                            cell["validation"].append(dict(n=small, seed=seed, result=trial))
            # All cells must pass before any performance samples are eligible.
            rng = random.Random(args.order_seed)
            for repetition in range(args.processes):
                order_cells = report["cells"].copy()
                rng.shuffle(order_cells)
                for cell in order_cells:
                    language, name, workers = cell["language"], cell["workload"], cell["workers"]
                    print(f"Measure {repetition+1}/{args.processes} {language:6} {name:10} w{workers}", flush=True)
                    report["order"].append([repetition, language, name, workers])
                    input_path = tmp / "input.ndjson"
                    if name == "json":
                        fixture(input_path, cell["n"], args.seed)
                        cell["fixture_sha256"] = digest(input_path)
                    record = process(command(language, name, cell["n"], args.seed, workers,
                                             args.warmups, args.samples, input_path),
                                     env | {"GOMAXPROCS": str(workers)}, args.timeout, True)
                    cell["records"].append(validate(record, cell["expected"], args.warmups, args.samples))
            for cell in report["cells"]:
                cell["summary"] = summarize(cell["records"])
            if source_hashes() != before or any(digest(paths[k]) != tools[k]["sha256"] for k in paths):
                raise RuntimeError("source or executable changed during measurement; reject this run")
            report["status"] = "complete"
    except Exception as error:
        report["status"] = "failed"
        report["failures"].append(f"{type(error).__name__}: {error}")
        print(report["failures"][-1], file=sys.stderr)
    finally:
        report["host_after"] = host()
        report["finished"] = dt.datetime.now(dt.timezone.utc).isoformat()
        # Unique timestamp, exclusive creation: historical runs are never overwritten.
        with destination.open("x") as stream:
            json.dump(report, stream, indent=2, allow_nan=False)
            stream.write("\n")
        print(f"{report['status']}: {destination}", flush=True)
    if report["status"] == "complete" and not args.no_html:
        args.html.parent.mkdir(parents=True, exist_ok=True)
        dashboard = process([str(paths["badger"]), "run", "--release", str(SUITE / "report.bdg"), "--",
                             str(args.reports.resolve()), str(args.html.resolve()), str(SUITE / "dashboard.html")],
                            env, args.timeout)
        print(dashboard["stdout"], end="")
    return 0 if report["status"] == "complete" else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--badger", type=Path, default=ROOT / "target/release/badger")
    parser.add_argument("--go", default="go")
    parser.add_argument("--node", default="node")
    parser.add_argument("--reports", type=Path, default=ROOT / "reports/realworld/runs")
    parser.add_argument("--html", type=Path, default=ROOT / "reports/realworld/index.html")
    parser.add_argument("--workloads", nargs="+", choices=list(SIZES), default=list(SIZES))
    parser.add_argument("--workers", nargs="+", type=int, default=[1, 2, 4, 8])
    parser.add_argument("--processes", type=int, default=5)
    parser.add_argument("--warmups", type=int, default=3)
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--build-samples", type=int, default=3)
    parser.add_argument("--scale", type=int, default=1)
    parser.add_argument("--seed", type=int, default=37)
    parser.add_argument("--order-seed", type=int, default=20261017)
    parser.add_argument("--timeout", type=float, default=180)
    parser.add_argument("--smoke", action="store_true")
    parser.add_argument("--no-html", action="store_true")
    args = parser.parse_args()
    if sys.platform != "linux":
        parser.error("Linux is required for wait4 peak RSS units and host provenance")
    if (min(args.processes, args.warmups, args.samples, args.build_samples, args.scale, *args.workers) < 1
            or len(set(args.workers)) != len(args.workers) or len(set(args.workloads)) != len(args.workloads)
            or max(args.workers) > 64 or not 0 <= args.seed <= 1000000 or args.timeout <= 0
            or args.scale > 100):
        parser.error("counts must be positive and unique, workers <= 64, scale <= 100, seed in [0,1000000]")
    if args.smoke:
        args.processes, args.warmups, args.samples, args.build_samples = 1, 1, 1, 1
    with benchmark_lock():
        return run(args)


if __name__ == "__main__":
    sys.exit(main())
