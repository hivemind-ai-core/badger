#!/usr/bin/env python3
"""Check codec outputs/rooting with source and old/new saved MIR under GC stress."""
import argparse
import gzip
import json
from pathlib import Path
import subprocess
import tempfile

import applications as app

FIXTURES = ["codec_paths", "schema_wire", "array_values", "data_symbol_keys"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    binaries = {name: getattr(args, name).resolve() for name in ["baseline", "candidate"]}
    report = dict(binaries={name: dict(path=str(p), sha256=app.rw.digest(p))
                            for name, p in binaries.items()}, sources={}, programs={}, runs=[])
    clean = app.rw.clean_env() | {"BADGER_GC_POOL_MAX": "unsupported"}
    with tempfile.TemporaryDirectory(prefix="badger-codec-checks-") as directory:
        tmp = Path(directory)
        for fixture in FIXTURES:
            source = app.gc.ROOT / f"tests/run/{fixture}.bdg"
            expected = source.with_suffix(".out").read_text()
            report["sources"][str(source.relative_to(app.gc.ROOT))] = app.rw.digest(source)
            programs = {"source": source}
            for label, binary in binaries.items():
                mir = tmp / f"{fixture}-{label}.bdgmir"
                result = subprocess.run([str(binary), "compile", str(source), "-o", str(mir)],
                                        cwd=app.gc.ROOT, env=clean, text=True, capture_output=True, timeout=60)
                assert result.returncode == 0, result.stderr
                programs[label] = mir
            report["programs"][fixture] = {label: app.rw.digest(p) for label, p in programs.items()}
            for generation in ["0", "1"]:
                for workers in [1, 2, 4, 8]:
                    for verify in ["0", "1"]:
                        for mode, program in programs.items():
                            env = clean | dict(BADGER_GC_STRESS="1", BADGER_GC_VERIFY=verify,
                                               BADGER_GC_GENERATIONAL=generation, BADGER_CHAOS="20261019")
                            command = [str(binaries["candidate"]), "run", "--release", "--workers",
                                       str(workers), str(program)]
                            result = subprocess.run(command, cwd=app.gc.ROOT, env=env, text=True,
                                                    capture_output=True, timeout=180)
                            assert result.returncode == 0 and result.stdout == expected, (
                                fixture, mode, generation, workers, result.stdout, result.stderr)
                            report["runs"].append(dict(fixture=fixture, mode=mode, generation=generation,
                                                       workers=workers, verify=verify, command=command,
                                                       stdout=result.stdout, stderr=result.stderr))
            print(f"{fixture}: 48 source/old/new MIR GC stress checks passed", flush=True)
        for mode in ["codec-flat", "codec-nested", "codec-wire", "codec-invalid"]:
            config = dict(mode=mode, n=17, rounds=1, payload=1, retain=1)
            for generation in ["0", "1"]:
                for workers in [1, 2, 4, 8]:
                    env = clean | dict(GC_BENCH_CONFIG=json.dumps(config), BADGER_GC_STRESS="1",
                                       BADGER_GC_VERIFY="1", BADGER_GC_GENERATIONAL=generation)
                    command = [str(binaries["candidate"]), "run", "--release", "--workers",
                               str(workers), "bench/gc/coverage.bdg"]
                    result = subprocess.run(command, cwd=app.gc.ROOT, env=env, text=True,
                                            capture_output=True, timeout=180, check=True)
                    app.gc.validate_coverage(json.loads(result.stdout), config)
                    report["runs"].append(dict(mode=mode, generation=generation, workers=workers,
                                               command=command, stdout=result.stdout, stderr=result.stderr))
    assert all(report["binaries"][label]["sha256"] == app.rw.digest(p) for label, p in binaries.items())
    assert all(sha == app.rw.digest(app.gc.ROOT / p) for p, sha in report["sources"].items())
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_bytes(gzip.compress((json.dumps(report, indent=2) + "\n").encode(), mtime=0))
    print(f"{len(report['runs'])} codec checks passed")


if __name__ == "__main__":
    main()
