#!/usr/bin/env python3
"""Exercise packed/growing/pointer buffers under GC stress, including binary MIR."""
import argparse
import json
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile

import run


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ordinary", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--previous", type=Path, help="optional old-format compiler to check ABI rejection")
    args = parser.parse_args()
    binary = args.ordinary.resolve()
    clean = {k: v for k, v in os.environ.items() if not k.startswith("BADGER_") and k != "GC_BENCH_CONFIG"}
    # A diagnostic-feature executable rejects this setting on allocation;
    # ordinary builds must ignore it, independently of the executable label.
    clean["BADGER_GC_POOL_MAX"] = "unsupported"
    results = []
    with tempfile.TemporaryDirectory(prefix="badger-buffer-checks-") as directory:
        mir = Path(directory) / "packed.bdgmir"
        older_mir_check = None
        if args.previous:
            previous = args.previous.resolve()
            older = Path(directory) / "previous.bdgmir"
            command = [str(previous), "compile", "tests/run/packed_arrays.bdg", "-o", str(older)]
            subprocess.run(command, cwd=run.ROOT, env=clean, capture_output=True, check=True, timeout=60)
            output = subprocess.run([str(binary), "run", str(older)], cwd=run.ROOT, env=clean,
                                    capture_output=True, text=True, timeout=60)
            assert output.returncode == 1 and "E0011" in output.stderr and "compile it again" in output.stderr, output
            with previous.open("rb") as file:
                previous_hash = hashlib.file_digest(file, "sha256").hexdigest()
            older_mir_check = dict(command=command, previous_sha256=previous_hash,
                                   mir_sha256=hashlib.sha256(older.read_bytes()).hexdigest(), stderr=output.stderr)
        compile_command = [str(binary), "compile", "tests/run/packed_arrays.bdg", "-o", str(mir)]
        subprocess.run(compile_command, cwd=run.ROOT, env=clean, capture_output=True, check=True, timeout=60)
        def check(source, workers, generational, config=None):
            env = clean | dict(BADGER_GC_STRESS="1", BADGER_GC_VERIFY="1", BADGER_GC_GENERATIONAL=str(generational))
            if config:
                env["GC_BENCH_CONFIG"] = json.dumps(config)
            command = [str(binary), "run", "--release", "--workers", str(workers), str(source)]
            output = subprocess.run(command, cwd=run.ROOT, env=env, capture_output=True, text=True, timeout=180)
            assert output.returncode == 0, (command, generational, output.stdout, output.stderr)
            if config:
                run.validate_coverage(json.loads(output.stdout), config)
            else:
                expected = (run.ROOT / "tests/run/packed_arrays.out").read_text()
                assert output.stdout == expected, output.stdout
            results.append(dict(command=command, generational=generational, config=config, stdout=output.stdout, stderr=output.stderr))
        for generational in [0, 1]:
            for workers in [1, 4, 8]:
                for source in ["tests/run/packed_arrays.bdg", mir]:
                    check(source, workers, generational)
            for workers in [1, 8]:
                for mode in ["buffers-empty", "buffers-grow", "buffers-pointers"]:
                    check("bench/gc/coverage.bdg", workers, generational,
                          dict(mode=mode, n=33, rounds=1, payload=9, retain=4))
    with binary.open("rb") as file:
        digest = hashlib.file_digest(file, "sha256").hexdigest()
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(dict(binary=str(binary), sha256=digest, compile_command=compile_command, older_mir_check=older_mir_check, runs=results), indent=2) + "\n")
    print(f"{len(results)} source/binary-MIR and buffer coverage stress checks passed")


if __name__ == "__main__":
    main()
