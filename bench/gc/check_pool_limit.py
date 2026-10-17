#!/usr/bin/env python3
"""Verify a 256/512 comparison actually uses different allocator binaries."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile

from run import ROOT


SOURCE = '''import std.array
import std.codec
import std.inspect
import std.io
import std.text
pub fn main() -> Result<Unit, io.Error> {
    let before = inspect.heap()
    let values = array.build(16384, fn(i: Int) { text.repeat("x", 264 + i % 2) })
    var checksum = 0
    for value in values { checksum = checksum + value.len() }
    let after = inspect.heap()
    io.println(codec.encode_text({ before, after, checksum }))?
    ok(())
}
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    env = {k: v for k, v in os.environ.items() if not k.startswith("BADGER_")}
    reports = {}
    with tempfile.TemporaryDirectory(prefix="badger-pool-limit-") as directory:
        source = Path(directory) / "pool.bdg"
        source.write_text(SOURCE)
        for name in ["baseline", "candidate"]:
            binary = getattr(args, name).resolve()
            result = subprocess.run([str(binary), "run", "--release", "--workers", "1", str(source)],
                                    cwd=ROOT, env=env, capture_output=True, text=True, timeout=60)
            assert result.returncode == 0, (name, result.stdout, result.stderr)
            record = json.loads(result.stdout)
            assert record["checksum"] == 16384 * 264 + 8192, record
            assert record["after"]["collections"] == record["before"]["collections"], record
            growth = record["after"]["pool_reserved_bytes"] - record["before"]["pool_reserved_bytes"]
            if name == "baseline":
                assert growth <= 256 * 1024, ("expected 256-byte pool cutoff", record)
            else:
                assert growth >= 16384 * 288, ("expected 512-byte pool cutoff", record)
            reports[name] = dict(binary=str(binary), record=record, pool_growth_bytes=growth)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(reports, indent=2) + "\n")
    print("verified distinct 256/512 allocator behavior")


if __name__ == "__main__":
    main()
