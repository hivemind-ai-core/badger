#!/usr/bin/env python3
"""Capture fixed-revision performance evidence without changing compiler code.

Build first: cargo build --release --offline
            cargo build --release --offline -p lang_runtime --example performance_audit
             cargo build --release --offline -p lang_codegen --example mir_audit
Run: python3 reports/performance-audit-2026-10-16/run.py
"""
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import re
import statistics
import subprocess
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
OUT = HERE / "evidence"
LANG = ROOT / "target/release/lang"
RUNTIME = ROOT / "target/release/examples/performance_audit"
MIR_AUDIT = ROOT / "target/release/examples/mir_audit"
ENV = {k: v for k, v in os.environ.items() if not k.startswith("LANG_")}
PROBE = HERE / "probes.lang"


def run(args, extra=None):
    start = time.perf_counter()
    p = subprocess.run([str(a) for a in args], cwd=ROOT, env={**ENV, **(extra or {})},
                       capture_output=True, text=True, timeout=120)
    result = dict(command=[str(a) for a in args], extra_env=extra or {},
                  exit_code=p.returncode, wall_ms=(time.perf_counter() - start) * 1000,
                  stdout=p.stdout, stderr=p.stderr)
    if p.returncode:
        raise RuntimeError(json.dumps(result, indent=2))
    return result


def main():
    OUT.mkdir(exist_ok=True)
    saved = OUT / "probes.langmir"
    compile_result = run([LANG, "compile", PROBE, "-o", saved], {"LANG_TIMINGS": "1"})
    results = dict(revision=run(["git", "rev-parse", "HEAD"])["stdout"].strip(),
                   date_utc=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                   platform=platform.platform(), cpu=next(line.split(":", 1)[1].strip()
                       for line in Path("/proc/cpuinfo").read_text().splitlines()
                       if line.startswith("model name")),
                   binary_sha256=hashlib.sha256(LANG.read_bytes()).hexdigest(),
                   runtime_probe_sha256=hashlib.sha256(RUNTIME.read_bytes()).hexdigest(),
                   mir_probe_sha256=hashlib.sha256(MIR_AUDIT.read_bytes()).hexdigest(),
                   compile=compile_result, samples=[], summaries={}, runtime={}, text=[])
    files = subprocess.check_output(["git", "ls-files", "crates", "Cargo.toml", "Cargo.lock"], cwd=ROOT).decode().splitlines()
    results["input_sha256"] = {f: hashlib.sha256((ROOT / f).read_bytes()).hexdigest() for f in files}
    audit_files = [PROBE, HERE / "text.lang", Path(__file__),
                   ROOT / "crates/lang_runtime/examples/performance_audit.rs",
                   ROOT / "crates/lang_codegen/examples/mir_audit.rs"]
    results["audit_sha256"] = {str(f.relative_to(ROOT)): hashlib.sha256(f.read_bytes()).hexdigest()
                               for f in audit_files}
    mir = run([MIR_AUDIT, saved])
    (OUT / "mir-metrics.txt").write_text(mir["stdout"])
    cells = [(mode, w) for mode in ["source", "no_inline", "saved"] for w in [1, 4]]

    def command(mode, w):
        args = [LANG, "run", "--release", "--bench", "--workers", w,
                saved if mode == "saved" else PROBE]
        extra = {"LANG_NO_INLINE": "1"} if mode == "no_inline" else {}
        return args, extra

    for mode, w in cells:
        args, extra = command(mode, w)
        run(args, extra)
        captured = run(args[:2] + ["--dump-mir"] + args[2:],
                       {**extra, "LANG_DUMP_ASM": "helpers,options,sequence,array_reads,comparisons"})
        (OUT / f"{mode}-w{w}.txt").write_text(captured["stderr"])
    order = cells * 5
    random.Random(20261016).shuffle(order)
    for mode, w in order:
        args, extra = command(mode, w)
        sample = run(args, extra)
        sample.update(mode=mode, workers=w)
        sample["cases_ns"] = {m[0]: int(m[1]) for m in re.findall(r"^(\w+): (\d+) ns; check", sample["stdout"], re.M)}
        assert len(sample["cases_ns"]) == 7
        results["samples"].append(sample)
        print(f"{mode} w{w}: {sample['cases_ns']}", flush=True)
    for mode, w in cells:
        samples = [s for s in results["samples"] if s["mode"] == mode and s["workers"] == w]
        results["summaries"][f"{mode}-w{w}"] = {
            case: dict(median_ns=statistics.median(s["cases_ns"][case] for s in samples),
                       min_ns=min(s["cases_ns"][case] for s in samples),
                       max_ns=max(s["cases_ns"][case] for s in samples))
            for case in samples[0]["cases_ns"]}
    for mode in ["buffer", "prefix", "pool", "old"]:
        for gen in (["0", "1"] if mode == "old" else ["0"]):
            results["runtime"][f"{mode}-generational-{gen}"] = run(
                [RUNTIME, mode], {"LANG_GC_GENERATIONAL": gen})
    for w in [1, 4]:
        args = [LANG, "run", "--release", "--workers", w, HERE / "text.lang"]
        run(args)
        for _ in range(5):
            sample = run(args)
            sample["workers"] = w
            sample["sizes_ns"] = {int(n): int(ns) for n, ns in re.findall(
                r"chars=(\d+) repetitions=100 slice_ns=(\d+) check=100", sample["stdout"])}
            assert len(sample["sizes_ns"]) == 4
            results["text"].append(sample)
    (OUT / "measurements.json").write_text(json.dumps(results, indent=2) + "\n")


if __name__ == "__main__":
    main()
