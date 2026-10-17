#!/usr/bin/env python3
"""Verify ordinary sweep policy and absence of diagnostics through Linux procfs."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

from run import ROOT


SOURCE = '''import std.array
import std.codec
import std.fs
import std.inspect
import std.io
import std.map
import std.task
import std.text

fn churn(seed: Int) -> Int {
    var values: Map<Int, Int> = map.empty()
    for i in 0..131072 { values = values.insert((i + seed) % 1024, i + seed) }
    var total = values.len()
    for (k, v) in values { total = total + k + v }
    total
}

pub fn main() -> Result<Unit, io.Error> {
    let checksum = task.scope(fn(s) -> Result<Int, String> {
        let jobs = array.build(8, fn(seed: Int) { s.spawn(fn() { churn(seed) }) })
        var total = 0
        for job in jobs { total = total + job.join() }
        ok(total)
    }).unwrap_or(0)
    let heap = inspect.heap()
    var helpers = 0
    for name in fs.list("/proc/self/task")? {
        let comm = fs.read_text(text.concat(text.concat("/proc/self/task/", name), "/comm")).unwrap_or("")
        if comm.starts_with("badger-gc-") { helpers = helpers + 1 }
    }
    io.println(codec.encode_text({ checksum, heap, helpers }))?
    ok(())
}
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--candidate-policy", choices=["cached", "scoped"], default="cached")
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    assert Path("/proc/self/task").is_dir(), "requires Linux procfs"
    expected = 0
    for seed in range(8):
        values = {(i + seed) % 1024: i + seed for i in range(131072)}
        expected += len(values) + sum(values) + sum(values.values())
    reports = {}
    env = {k: v for k, v in os.environ.items() if not k.startswith("BADGER_")}
    with tempfile.TemporaryDirectory(prefix="badger-sweep-check-") as directory:
        source = Path(directory) / "sweep.bdg"
        source.write_text(SOURCE)
        for name in ["baseline", "candidate"]:
            binary = getattr(args, name).resolve()
            phase = Path(directory) / "phases.json"
            census = Path(directory) / "allocations.json"
            result = subprocess.run([str(binary), "run", "--release", "--workers", "4", str(source)],
                                    cwd=ROOT, env=env | dict(BADGER_GC_PROFILE=str(phase),
                                                          BADGER_ALLOC_PROFILE=str(census)),
                                    capture_output=True, text=True, timeout=180)
            assert result.returncode == 0, (name, result.stdout, result.stderr)
            assert not phase.exists() and not census.exists(), "ordinary binaries must omit GC diagnostics"
            record = json.loads(result.stdout)
            assert record["checksum"] == expected, record
            assert record["heap"]["collections"] > 0, record
            if name == "baseline" or args.candidate_policy == "scoped":
                assert record["helpers"] == 0, ("expected scoped helpers", record)
            else:
                assert 0 < record["helpers"] <= 3, ("expected cached helpers", record)
            with binary.open("rb") as file:
                sha = hashlib.file_digest(file, "sha256").hexdigest()
            reports[name] = dict(binary=str(binary), sha256=sha, record=record)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(reports, indent=2) + "\n")
    print(f"verified scoped/{args.candidate_policy} sweep-helper behavior")


if __name__ == "__main__":
    main()
