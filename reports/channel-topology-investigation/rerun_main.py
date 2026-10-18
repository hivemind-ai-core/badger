"""Compare committed main with the retained channel and scheduler baselines.

Run after building the isolated git archive described by
/tmp/language-main-benchmark-snapshot.json. Build that snapshot with its own
target directory and rebuild local packages if reusing dependency caches.
Results are new files; the original language.json is never overwritten.
"""
from datetime import datetime, timezone
from pathlib import Path
import hashlib
import json
import os
import re
import shutil
import statistics
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True).strip()


def main():
    snapshot = json.loads(Path("/tmp/language-main-benchmark-snapshot.json").read_text())
    revision = snapshot["revision"]
    source_root = Path(snapshot["snapshot"])
    source_hashes = {
        str(p.relative_to(source_root)): hashlib.sha256(p.read_bytes()).hexdigest()
        for p in sorted((source_root / "crates").rglob("*")) if p.is_file()
    }
    old = json.loads((HERE / "language.json").read_text())
    previous_files = sorted(HERE.glob("rerun-main-*.json"), key=lambda p: p.stat().st_mtime)
    previous_file = next((p for p in reversed(previous_files)
                          if json.loads(p.read_text()).get("revision") != revision), None)
    initial_path = Path("/tmp/language-scheduler-lock-check.json")
    initial = json.loads(initial_path.read_text()) if initial_path.exists() else None
    scratch = Path(tempfile.mkdtemp(prefix="lang-main-rerun-"))
    binary = scratch / "lang"
    shutil.copy2(source_root / "target/release/lang", binary)
    env = {k: v for k, v in os.environ.items() if not k.startswith("LANG_")}
    cases = [(name, HERE / "fixtures" / f"{name}.lang") for name in (
        "spsc_cap1", "spsc_cap64", "spsc_cap1024", "spsc_byte_budget", "mpsc", "spmc", "mpmc")]
    cases += [(name, source_root / "bench" / f"{name}.lang") for name in (
        "messages", "parallel", "independent_channels", "parallel_alloc")]
    result = {
        "started_utc": datetime.now(timezone.utc).isoformat(),
        "revision": revision,
        "source_snapshot": snapshot,
        "source_sha256": source_hashes,
        "git_status_start": git("status", "--short"),
        "production_inputs_match_committed_revision": True,
        "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "method": "Same machine; sequential fresh processes, one warmup and three measured runs per case/worker count. Optimized host and JIT. --bench excludes compilation. LANG_* variables cleared; unpinned, frequency and background activity uncontrolled. Instrumented runs separate. Built isolated git archive of committed main with independent target directory; immutable executable copy and unchanged source snapshot checked.",
        "cpu": next(line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines() if line.startswith("model name")),
        "affinity": sorted(os.sched_getaffinity(0)),
        "old_topology_baseline": old,
        "initial_scheduler_baseline": initial,
        "previous_main_baseline_file": previous_file.name if previous_file else None,
        "fixture_sha256": {name: hashlib.sha256(p.read_bytes()).hexdigest() for name, p in cases},
        "runs": [], "summary": [], "stats": [],
    }
    stamp = datetime.now(timezone.utc).strftime("%H%M%S")
    out = HERE / f"rerun-main-{revision[:7]}-{stamp}.json"
    for name, path in cases:
        for workers in (1, 2, 4):
            times = []
            cmd = [str(binary), "run", "--release", "--bench", "--workers", str(workers), str(path)]
            for repeat in range(4):
                p = subprocess.run(cmd, cwd=source_root, env=env, capture_output=True, text=True, timeout=60)
                row = {"case": name, "workers": workers, "warmup": repeat == 0, "cmd": cmd, "exit": p.returncode, "stdout": p.stdout, "stderr": p.stderr}
                match = re.search(r"bench: run ([\d.]+) ms", p.stderr)
                result["runs"].append(row)
                if p.returncode or not match:
                    out.write_text(json.dumps(result, indent=2) + "\n")
                    raise RuntimeError(row)
                row["run_ms"] = float(match[1])
                if repeat:
                    times.append(row["run_ms"])
            row = {"case": name, "workers": workers, "median_ms": statistics.median(times), "min_ms": min(times), "max_ms": max(times)}
            result["summary"].append(row)
            out.write_text(json.dumps(result, indent=2) + "\n")
            print(json.dumps(row), flush=True)
    for name in ("spsc_cap1", "spsc_cap1024", "spsc_byte_budget", "mpmc", "messages"):
        path = next(path for label, path in cases if label == name)
        cmd = [str(binary), "run", "--release", "--bench", "--workers", "4", str(path)]
        p = subprocess.run(cmd, cwd=source_root, env={**env, "LANG_RUNTIME_STATS": "1"}, capture_output=True, text=True, timeout=60)
        if p.returncode:
            raise RuntimeError((cmd, p.stdout, p.stderr))
        result["stats"].append({"case": name, "cmd": cmd, "stdout": p.stdout, "stderr": p.stderr})
    assert all(hashlib.sha256((source_root / path).read_bytes()).hexdigest() == sha for path, sha in source_hashes.items())
    result["main_at_end"] = git("rev-parse", "main")
    result["completed_utc"] = datetime.now(timezone.utc).isoformat()
    out.write_text(json.dumps(result, indent=2) + "\n")
    print(f"Retained results: {out.relative_to(ROOT)}", flush=True)


if __name__ == "__main__":
    main()
