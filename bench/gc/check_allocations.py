#!/usr/bin/env python3
"""Black-box allocation census checks, including GC stress and cell quarantine."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

from run import ROOT, validate_allocations


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--diagnostic", type=Path, required=True)
    parser.add_argument("--ordinary", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    binaries = {name: path.resolve() for name, path in
                [("diagnostic", args.diagnostic), ("ordinary", args.ordinary)]}
    env = {k: v for k, v in os.environ.items() if not k.startswith("BADGER_")}
    rows = []
    with tempfile.TemporaryDirectory(prefix="badger-allocation-check-") as directory:
        census = Path(directory) / "census.json"
        for generation in ["0", "1"]:
            for workers in [1, 2, 4, 8]:
                settings = env | dict(BADGER_ALLOC_PROFILE=str(census),
                                      BADGER_GC_GENERATIONAL=generation,
                                      BADGER_GC_STRESS="1", BADGER_GC_VERIFY="1",
                                      BADGER_CHAOS="20261017")
                command = [str(binaries["diagnostic"]), "run", "--release", "--workers",
                           str(workers), "tests/run/transient_maps.bdg"]
                result = subprocess.run(command, cwd=ROOT, env=settings, capture_output=True,
                                        text=True, timeout=180, check=True)
                assert result.stdout == "transient maps: ok\n", result
                profile = json.loads(census.read_text())
                validate_allocations(profile)
                assert profile["totals"]["first_survival"]["objects"] > 0
                assert profile["totals"]["reclaimed_before_survival"]["objects"] > 0
                assert profile["totals"]["reclaimed_after_survival"]["objects"] > 0
                rows.append(dict(generational=generation, workers=workers, profile=profile,
                                 stderr=result.stderr))
                census.unlink()
                print(f"census stress/verify: generation={generation} workers={workers}: ok", flush=True)
        result = subprocess.run([str(binaries["ordinary"]), "run", "--release",
                                 "tests/run/transient_maps.bdg"], cwd=ROOT,
                                env=env | dict(BADGER_ALLOC_PROFILE=str(census)),
                                capture_output=True, text=True, timeout=180, check=True)
        assert result.stdout == "transient maps: ok\n", result
        assert not census.exists(), "ordinary builds must omit allocation profiling"
    def sha(path):
        with path.open("rb") as file:
            return hashlib.file_digest(file, "sha256").hexdigest()
    report = dict(binaries={name: dict(path=str(path), sha256=sha(path))
                            for name, path in binaries.items()},
                  fixture_sha256=sha(ROOT / "tests/run/transient_maps.bdg"), runs=rows,
                  ordinary_profile_absent=True)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_bytes(gzip.compress((json.dumps(report, indent=2) + "\n").encode(), mtime=0))


if __name__ == "__main__":
    main()
