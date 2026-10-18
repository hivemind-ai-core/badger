#!/usr/bin/env python3
"""Save checked application CPU profiles separately from performance timings.

Use a release binary built with native frame pointers. The runtime sampler
records each participating thread at 1 kHz of CPU time. Its runtime scope
includes fixture loading, warmups and all kernels, but excludes frontend/JIT.
GC diagnostics perturb costs; these samples attribute work, not speedups.
"""
import argparse
from collections import Counter
import ctypes
import gzip
import json
import os
from pathlib import Path
import random
import subprocess
import tempfile

import applications as app


def counts(folded):
    own, inclusive = Counter(), Counter()
    for line in folded.splitlines():
        stack, count = line.rsplit(" ", 1)
        frames = stack.split(";")
        count = int(count)
        own[frames[-1]] += count
        for frame in set(frames):
            inclusive[frame] += count
    return dict(samples=sum(own.values()), self=dict(own.most_common()),
                inclusive=dict(inclusive.most_common()))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--revision", required=True)
    parser.add_argument("--cases", nargs="+", choices=app.CASES, required=True)
    parser.add_argument("--workers", nargs="+", type=int, default=[1])
    parser.add_argument("--reps", type=int, default=3)
    parser.add_argument("--warmups", type=int, default=15)
    parser.add_argument("--samples", type=int, default=300)
    parser.add_argument("--seed", type=int, default=20261019)
    parser.add_argument("--diagnostic", action="store_true")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    if min(args.reps, args.warmups, args.samples, *args.workers) < 1 or max(args.workers) > 64:
        parser.error("positive counts and 1..64 workers required")
    if args.out.exists():
        parser.error("output directory must be new to preserve existing profiles")
    # The sandbox can permit timers but deny self-memory reads, silently
    # reducing the sampler to leaf PCs. Check before collecting attribution.
    class IOVec(ctypes.Structure):
        _fields_ = [("base", ctypes.c_void_p), ("size", ctypes.c_size_t)]
    source, destination = ctypes.c_uint64(0x1234), ctypes.c_uint64()
    local = IOVec(ctypes.addressof(destination), 8)
    remote = IOVec(ctypes.addressof(source), 8)
    libc = ctypes.CDLL(None, use_errno=True)
    copied = libc.process_vm_readv(os.getpid(), ctypes.byref(local), 1, ctypes.byref(remote), 1, 0)
    if copied != 8 or destination.value != source.value:
        parser.error(f"self process_vm_readv unavailable (errno {ctypes.get_errno()}); native caller attribution requires permitted execution")
    with app.gc.benchmark_lock():
        run(args)


def run(args):
    args.out.mkdir(parents=True)
    binary = args.binary.resolve()
    sources = app.rw.source_hashes()
    for path in [Path(__file__), Path(app.__file__), Path(app.gc.__file__),
                 app.compare.HERE / "run.py", app.compare.PROGRAMS / "workload.bdg"]:
        sources[str(path.relative_to(app.gc.ROOT))] = app.rw.digest(path)
    report = dict(schema=1, status="running", revision=app.rw.output(["git", "rev-parse", "HEAD"]),
                  arguments=vars(args) | {"binary": str(binary), "out": str(args.out)},
                  binary_sha256=app.rw.digest(binary), sources=sources, runs=[])
    env = app.rw.clean_env() | {"BADGER_PROFILE_SCOPE": "runtime"}
    cells = [(case, workers, rep) for case in args.cases
             for workers in args.workers for rep in range(args.reps)]
    random.Random(args.seed).shuffle(cells)
    expected = {}
    for case in args.cases:
        suite, name = case.split("/")
        n = app.rw.SIZES[name] if suite == "realworld" else app.compare.WORKLOADS[name][0]
        expected[case] = n, (app.rw if suite == "realworld" else app.compare).oracle(name, n, 37)
    with tempfile.TemporaryDirectory(prefix="badger-cpu-applications-") as temporary:
        tmp = Path(temporary)
        fixture = tmp / "orders.jsonl"
        app.rw.fixture(fixture, app.rw.SIZES["json"], 37)
        for case, workers, rep in cells:
            suite, name = case.split("/")
            stem = f"{suite}-{name}-w{workers}-r{rep}"
            folded, phases = args.out / f"{stem}.folded", tmp / "gc.json"
            program = (app.rw.PROGRAMS if suite == "realworld" else app.compare.PROGRAMS) / "workload.bdg"
            n, checksum = expected[case]
            arguments = [name, n, 37, *([workers] if suite == "realworld" else []),
                         args.warmups, args.samples, *([str(fixture)] if suite == "realworld" else [])]
            command = [str(binary), "run", "--release", "--bench", "--workers", str(workers),
                       str(program), "--", *map(str, arguments)]
            child_env = env | {"BADGER_PROFILE": str(folded.resolve())}
            if args.diagnostic:
                child_env["BADGER_GC_PROFILE"] = str(phases)
            before = app.gc.host_context()
            process = app.rw.process(command, child_env, 180, expect_ready=suite == "realworld")
            after = app.gc.host_context()
            app.validate(process, suite, name, n, 37, args.warmups, args.samples, checksum)
            cpu = counts(folded.read_text())
            assert 0 < cpu["samples"] < 262144, "empty or saturated CPU profile"
            profile = json.loads(phases.read_text()) if args.diagnostic else None
            if profile is not None:
                app.gc.validate_sweep(profile)
                phases.unlink()
            report["runs"].append(dict(case=case, workers=workers, repetition=rep,
                                       n=n, process=process, cpu=cpu, gc=profile,
                                       folded=folded.name, folded_sha256=app.rw.digest(folded),
                                       host_before=before, host_after=after))
            print(f"{stem}: {cpu['samples']} CPU samples", flush=True)
            save(args.out, report)
    assert report["binary_sha256"] == app.rw.digest(binary)
    assert sources == {p: app.rw.digest(app.gc.ROOT / p) for p in sources}
    report["status"] = "complete"
    save(args.out, report)


def save(out, report):
    (out / "profiles.json.gz").write_bytes(gzip.compress(
        (json.dumps(report, indent=2) + "\n").encode(), mtime=0))


if __name__ == "__main__":
    main()
