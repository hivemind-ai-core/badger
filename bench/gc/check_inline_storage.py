#!/usr/bin/env python3
"""Prove ordinary binaries differ in inline payload storage before GC can reclaim it."""
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
import std.inspect
import std.io
pub fn main() -> Result<Unit, io.Error> {
    let before = inspect.heap()
    let values = array.build(1024, fn(i: Int) {
        array.build(32, fn(j: Int) { i * 31 + j })
    })
    var checksum = 0
    for words in values { for word in words { checksum = checksum + word } }
    let after = inspect.heap()
    io.println(codec.encode_text({ checksum, before, after }))?
    ok(())
}
'''


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--baseline", type=Path, required=True)
    ap.add_argument("--candidate", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()
    env = {k: v for k, v in os.environ.items() if not k.startswith("BADGER_")}
    env["BADGER_GC_POOL_MAX"] = "unsupported"  # Reject diagnostic artifacts.
    results = {}
    with tempfile.TemporaryDirectory(prefix="badger-inline-storage-") as directory:
        source = Path(directory) / "storage.bdg"
        source.write_text(SOURCE)
        for name in ["baseline", "candidate"]:
            binary = getattr(args, name).resolve()
            output = subprocess.run([str(binary), "run", "--release", "--workers", "1", str(source)],
                                    env=env, cwd=ROOT, capture_output=True, text=True, timeout=60)
            assert output.returncode == 0, (name, output.stdout, output.stderr)
            record = json.loads(output.stdout)
            assert record["checksum"] == 31 * 32 * 1024 * 1023 // 2 + 1024 * 32 * 31 // 2
            assert record["after"]["collections"] == record["before"]["collections"], record
            growth = record["after"]["external_live_bytes"] - record["before"]["external_live_bytes"]
            with binary.open("rb") as file:
                digest = hashlib.file_digest(file, "sha256").hexdigest()
            results[name] = dict(binary=str(binary), sha256=digest, record=record, external_growth_bytes=growth)
    assert results["baseline"]["external_growth_bytes"] - results["candidate"]["external_growth_bytes"] == 1024 * 256, results
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(dict(source=SOURCE, runs=results), indent=2) + "\n")
    print("verified 1,024 native payload allocations replaced by ordinary managed storage")


if __name__ == "__main__":
    main()
