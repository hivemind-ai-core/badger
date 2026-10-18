"""Retained channel investigation. No runtime source edits.

Build the current language tool first. This script copies that executable to
scratch storage so concurrent builds cannot replace it during measurement.
Run native experiments only after language measurements finish.
"""
from pathlib import Path
import hashlib
import json
import os
import platform
import re
import shutil
import statistics
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
ENV = {k: v for k, v in os.environ.items() if not k.startswith("LANG_")}


def capture(cmd):
    return subprocess.check_output(cmd, cwd=ROOT, text=True).strip()


def source_hashes():
    paths = list((ROOT / "crates").rglob("*.rs")) + list((ROOT / "crates").rglob("*.lang"))
    return {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(paths)}


def program(producers, consumers, capacity, byte_budget=0):
    return f'''// Generated fixed-work topology probe: {producers} producer(s), {consumers} consumer(s).
import std.channel
import std.io
import std.task
pub enum AppError {{
    config(source: channel.ConfigError),
    send(source: channel.SendError<Int>),
    io(source: io.Error),
}} derive(Error, From)
pub fn main() -> Result<Unit, AppError> {{
    let per = {200000 // producers}
    let ends = channel.create<Int>({{ ..channel.defaults, capacity: {capacity}, byte_budget: {byte_budget} }})?
    let got = task.scope(fn(s) -> Result<List<{{ count: Int, sum: Int }}>, AppError> {{
        let ps = (0..{producers}).map(fn(p: Int) {{
            s.spawn(fn() -> Result<Unit, AppError> {{
                for i in 0..per {{ ends.sender.send(p * per + i)? }}
                ok(())
            }})
        }}).collect()
        let cs = (0..{consumers}).map(fn(_c: Int) {{
            s.spawn(fn() {{
                var count = 0
                var sum = 0
                loop {{
                    match ends.receiver.recv() {{
                        .some(v) => {{ count = count + 1; sum = sum + v }},
                        .none => {{ break }},
                    }}
                }}
                {{ count, sum }}
            }})
        }}).collect()
        s.try_all(ps)?
        ends.sender.close()
        ok(cs.map(fn(t) {{ t.join() }}))
    }})?
    assert(got.fold(0, fn(a: Int, v: {{ count: Int, sum: Int }}) {{ a + v.count }}) == 200000)
    assert(got.fold(0, fn(a: Int, v: {{ count: Int, sum: Int }}) {{ a + v.sum }}) == 19999900000)
    io.println("200000 messages: count and sum conserved")?
    ok(())
}}
'''


def main():
    scratch = Path(tempfile.mkdtemp(prefix="lang-channel-topology-"))
    binary = scratch / "lang"
    shutil.copy2(ROOT / "target/release/lang", binary)
    output = {
        "head_start": capture(["git", "rev-parse", "HEAD"]),
        "dirty_start": capture(["git", "status", "--short"]),
        "tracked_diff_start": capture(["git", "diff", "--", "crates"]),
        "sources_start": source_hashes(),
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "cpu": next(l.split(":", 1)[1].strip() for l in Path("/proc/cpuinfo").read_text().splitlines() if l.startswith("model name")),
        "platform": platform.platform(),
        "affinity": sorted(os.sched_getaffinity(0)),
        "method": "Sequential fresh processes, optimized host and JIT, one warmup plus three measured runs. --bench excludes compilation. No CPU pinning; separate instrumented runs. Stable executable copy made after cargo build. Runtime inputs may advance concurrently; see source snapshots.",
        "runs": [], "summary": [], "stats": [],
    }
    fixtures = HERE / "fixtures"
    fixtures.mkdir(exist_ok=True)
    cases = []
    for name, p, c, cap, budget in [
        ("spsc_cap1", 1, 1, 1, 0),
        ("spsc_cap64", 1, 1, 64, 0),
        ("spsc_cap1024", 1, 1, 1024, 0),
        ("spsc_byte_budget", 1, 1, 1024, 512),
        ("mpsc", 4, 1, 1024, 0),
        ("spmc", 1, 4, 1024, 0),
        ("mpmc", 4, 4, 1024, 0),
    ]:
        path = fixtures / f"{name}.lang"
        path.write_text(program(p, c, cap, budget))
        cases.append((name, path))
    cases.append(("messages", ROOT / "bench/messages.lang"))
    for name, path in cases:
        for workers in (1, 2, 4):
            cmd = [str(binary), "run", "--release", "--bench", "--workers", str(workers), str(path)]
            times = []
            for repeat in range(4):
                p = subprocess.run(cmd, cwd=ROOT, env=ENV, capture_output=True, text=True, timeout=45)
                match = re.search(r"bench: run ([\d.]+) ms", p.stderr)
                row = {"case": name, "workers": workers, "warmup": repeat == 0, "cmd": cmd, "exit": p.returncode, "stdout": p.stdout, "stderr": p.stderr}
                output["runs"].append(row)
                if p.returncode or not match:
                    (HERE / "language.json").write_text(json.dumps(output, indent=2) + "\n")
                    raise RuntimeError(row)
                row["run_ms"] = float(match[1])
                if repeat:
                    times.append(row["run_ms"])
            summary = {"case": name, "workers": workers, "median_ms": statistics.median(times), "min_ms": min(times), "max_ms": max(times)}
            output["summary"].append(summary)
            print(json.dumps(summary), flush=True)
    for name in ("spsc_cap1", "spsc_cap1024", "spsc_byte_budget", "mpmc", "messages"):
        path = next(path for label, path in cases if label == name)
        cmd = [str(binary), "run", "--release", "--bench", "--workers", "4", str(path)]
        p = subprocess.run(cmd, cwd=ROOT, env={**ENV, "LANG_RUNTIME_STATS": "1"}, capture_output=True, text=True, timeout=45)
        if p.returncode:
            raise RuntimeError((cmd, p.stdout, p.stderr))
        output["stats"].append({"case": name, "cmd": cmd, "stdout": p.stdout, "stderr": p.stderr})
        print(json.dumps(output["stats"][-1]), flush=True)
    output["head_end"] = capture(["git", "rev-parse", "HEAD"])
    output["sources_end"] = source_hashes()
    output["sources_unchanged_during_measurement"] = output["sources_start"] == output["sources_end"]
    (HERE / "language.json").write_text(json.dumps(output, indent=2) + "\n")


if __name__ == "__main__":
    main()
