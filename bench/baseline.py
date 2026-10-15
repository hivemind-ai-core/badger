#!/usr/bin/env python3
"""Reproducible performance baseline for the compiler, runtime, and
generated code.

    cargo build --release --offline
    python3 bench/baseline.py --out reports/benchmarks/NAME   # writes NAME.json, NAME.md

Each case runs in fresh processes: one warmup, then --reps measured runs;
the median and minimum are reported. Compiler phases come from
LANG_TIMINGS=1, run time from `lang run --bench` (after compilation).
Other LANG_* variables are removed from the environment.
"""

import argparse
import json
import os
import platform
import re
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ENV = {k: v for k, v in os.environ.items() if not k.startswith("LANG_")}
PROGRAMS = ["records", "collections", "messages", "heap", "runtime", "parallel", "independent_channels", "parallel_alloc"]
WORKERS = [1, 2, 4, 8]


def run(cmd, env=None, timeout=300):
    start = time.perf_counter()
    p = subprocess.run(cmd, cwd=ROOT, env={**ENV, **(env or {})}, capture_output=True, text=True, timeout=timeout)
    wall = (time.perf_counter() - start) * 1e3
    if p.returncode != 0:
        raise RuntimeError(f"{' '.join(map(str, cmd))} failed ({p.returncode}):\n{p.stderr[-2000:]}")
    return wall, p.stdout, p.stderr


def phases(stderr):
    return {m.group(1): float(m.group(2)) for m in re.finditer(r"\[timings\] (\w+): ([\d.]+) ms", stderr)}


def bench_run_ms(stderr):
    m = re.search(r"bench: run ([\d.]+) ms", stderr)
    return float(m.group(1)) if m else None


def repeat(fn, reps):
    fn()  # warmup
    return [fn() for _ in range(reps)]


def summary(values):
    return {"median": round(statistics.median(values), 3), "min": round(min(values), 3), "runs": [round(v, 3) for v in values]}


def synthetic(n):
    """A generated program with n functions, structs, and generic calls."""
    out = ["import std.io", "import std.list", ""]
    for i in range(n):
        out.append(f"pub struct S{i} {{ pub a: Int, pub b: String, pub xs: List<Int> }}")
        out.append(f"fn f{i}<T>(x: T, s: S{i}) -> (T, Int) {{")
        out.append(f"    let total = s.xs.fold(s.a, fn(acc: Int, v: Int) {{ acc + v * {i % 7 + 1} }})")
        out.append(f"    match total % 3 {{ 0 => (x, total), 1 => (x, total + {i}), _ => (x, s.b.len()) }}")
        out.append("}")
    out.append("pub fn main() -> Unit {")
    out.append("    var sum = 0")
    for i in range(n):
        out.append(f'    let s{i}: S{i} = {{ a: {i}, b: "s{i}", xs: [1, 2, 3] }}')
        out.append(f"    sum = sum + f{i}(s{i}.a, s{i}).1")
    out.append('    let _ = io.println($"{sum}")')
    out.append("}")
    return "\n".join(out) + "\n"


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--reps", type=int, default=3)
    ap.add_argument("--lang", type=Path, default=ROOT / "target/release/lang")
    args = ap.parse_args()
    lang = str(args.lang)
    result = {
        "date": time.strftime("%Y-%m-%d"),
        "revision": subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True).stdout.strip(),
        "dirty": subprocess.run(["git", "status", "--short", "--untracked-files=no"], cwd=ROOT, capture_output=True, text=True).stdout,
        "platform": platform.platform(),
        "cpu": next((l.split(":", 1)[1].strip() for l in open("/proc/cpuinfo") if l.startswith("model name")), "?"),
        "logical_cpus": os.cpu_count(),
        "lang": lang,
        "reps": args.reps,
        "compiler": {},
        "binary_mir": {},
        "runtime": {},
    }
    tmp = Path(tempfile.mkdtemp(prefix="lang-baseline-"))
    # compiler: phases of checking and compiling each program and synthetic inputs
    inputs = {name: ROOT / f"bench/{name}.lang" for name in PROGRAMS}
    for n in [50, 200, 800]:
        f = tmp / f"synthetic_{n}.lang"
        f.write_text(synthetic(n))
        inputs[f"synthetic_{n}"] = f
    minimal = tmp / "minimal.lang"
    minimal.write_text("pub fn main() -> Int { 0 }\n")
    inputs["minimal"] = minimal
    for name, f in inputs.items():
        walls, phase_runs = [], []
        def one():
            wall, _, err = run([lang, "compile", f, "-o", tmp / f"{name}.langmir"], env={"LANG_TIMINGS": "1"})
            walls.append(wall)
            phase_runs.append(phases(err))
            return wall
        repeat(one, args.reps)
        walls, phase_runs = walls[1:], phase_runs[1:]
        keys = phase_runs[0].keys()
        result["compiler"][name] = {
            "wall_ms": summary(walls),
            "phases_ms": {k: round(statistics.median(p[k] for p in phase_runs), 3) for k in keys},
            "source_lines": sum(1 for _ in open(f)),
        }
        print(f"compile {name}: {result['compiler'][name]['wall_ms']['median']} ms", file=sys.stderr)
    # binary MIR: load and run without the front end
    for name in ["minimal", "records"]:
        mir = tmp / f"{name}.langmir"
        walls = repeat(lambda: run([lang, "run", "--release", mir])[0], args.reps)
        result["binary_mir"][name] = {"size_bytes": mir.stat().st_size, "load_and_run_ms": summary(walls)}
    # runtime and generated code: --bench run time per worker count
    for name in PROGRAMS:
        for w in WORKERS:
            outs = []
            def one():
                _, out, err = run([lang, "run", "--release", "--bench", "--workers", str(w), f"bench/{name}.lang"])
                outs.append(out)
                return bench_run_ms(err)
            times = repeat(one, args.reps)
            result["runtime"][f"{name}_w{w}"] = {"run_ms": summary(times), "output": outs[-1].strip().splitlines()}
            print(f"run {name} w{w}: {result['runtime'][f'{name}_w{w}']['run_ms']['median']} ms", file=sys.stderr)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.with_suffix(".json").write_text(json.dumps(result, indent=2) + "\n")
    md = [f"# Performance baseline {result['date']}", "",
          f"Revision `{result['revision'][:10]}` on {result['cpu']} ({result['logical_cpus']} logical CPUs); "
          f"median of {args.reps} fresh runs after a warmup. Generated by `bench/baseline.py`.", "",
          "## Compiler (lang compile, ms)", "", "| Input | lines | total | parse | resolve | typecheck | lower | codegen |",
          "|---|---|---|---|---|---|---|---|"]
    for name, c in result["compiler"].items():
        p = c["phases_ms"]
        md.append(f"| {name} | {c['source_lines']} | {c['wall_ms']['median']} | " + " | ".join(str(p.get(k, "-")) for k in ["parse", "resolve", "typecheck", "lower", "codegen"]) + " |")
    md += ["", "## Binary MIR", "", "| Program | size (bytes) | load and run (ms) |", "|---|---|---|"]
    for name, b in result["binary_mir"].items():
        md.append(f"| {name} | {b['size_bytes']} | {b['load_and_run_ms']['median']} |")
    md += ["", "## Runtime and generated code (`--bench` run time, ms)", "", "| Program | " + " | ".join(f"{w} worker{'s' if w > 1 else ''}" for w in WORKERS) + " |", "|---|" + "---|" * len(WORKERS)]
    for name in PROGRAMS:
        md.append(f"| {name} | " + " | ".join(str(result["runtime"][f"{name}_w{w}"]["run_ms"]["median"]) for w in WORKERS) + " |")
    md += ["", "Per-operation figures printed by each program are in the JSON (`output`)."]
    args.out.with_suffix(".md").write_text("\n".join(md) + "\n")


if __name__ == "__main__":
    main()
