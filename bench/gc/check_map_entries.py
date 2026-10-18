#!/usr/bin/env python3
"""Check temporary map-entry elision, retained lists and cursor root lifetimes."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

from run import ROOT, validate_coverage


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--ordinary", type=Path, required=True)
    ap.add_argument("--compatible", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()
    binary, previous = args.ordinary.resolve(), args.compatible.resolve()
    env = {k: v for k, v in os.environ.items() if not k.startswith(("BADGER_", "LANG_"))
           and k not in ["GC_BENCH_CONFIG", "TELEMETRY_CONFIG"]}
    expected = (ROOT / "tests/run/map_entries_elision.out").read_text()
    results, probes = [], []

    def execute(command, settings=None, output=expected):
        result = subprocess.run(list(map(str, command)), cwd=ROOT, env=env | (settings or {}),
                                capture_output=True, text=True, timeout=180)
        assert result.returncode == 0, (command, result.stdout, result.stderr)
        if output is not None:
            assert result.stdout == output, (command, result.stdout, result.stderr)
        return dict(command=list(map(str, command)), stdout=result.stdout, stderr=result.stderr)

    with tempfile.TemporaryDirectory(prefix="badger-map-entry-checks-") as directory:
        tmp = Path(directory)
        for stored in [False, True]:
            source = tmp / ("stored.bdg" if stored else "immediate.bdg")
            loop = "let entries = source.entries()\nfor (k, v) in entries" if stored else "for (k, v) in source.entries()"
            source.write_text('''import std.io
import std.map
pub fn main() -> Result<Unit, io.Error> {
    var source: Map<Int, Int> = map.empty()
    for k in 0..33 { source = source.insert(k, k) }
    var total = 0
    ''' + loop + ''' { total = total + k + v }
    io.println(total)?
    ok(())
}
''')
            for compiler, optimize, no_inline in [(previous, True, False), (binary, False, False),
                                                  (binary, True, False), (binary, True, True)]:
                command = [compiler, "run", "--dump-mir", *(["--release"] if optimize else []), source]
                capture = execute(command, {"BADGER_NO_INLINE": "1"} if no_inline else None, output="1056\n")
                boxed = 'name: "badger_rt_i_map_entries"' in capture["stderr"]
                assert boxed == (stored or not optimize or no_inline or compiler == previous), (stored, optimize, capture)
                if not boxed:
                    assert "badger_rt_map_cursor" in capture["stderr"]
                probes.append(capture | dict(stored=stored, optimized=optimize, no_inline=no_inline))
        saved = []
        for name, compiler in [("current", binary), ("previous", previous)]:
            mir = tmp / f"{name}.bdgmir"
            execute([compiler, "compile", "tests/run/map_entries_elision.bdg", "-o", mir], output=None)
            saved.append(mir)
        for verify in [0, 1]:
            for generation in [0, 1]:
                settings = dict(BADGER_GC_STRESS="1", BADGER_GC_VERIFY=str(verify),
                                BADGER_GC_GENERATIONAL=str(generation), BADGER_CHAOS="839")
                for workers in [1, 4, 8]:
                    for target, optimize in [("tests/run/map_entries_elision.bdg", False),
                                             ("tests/run/map_entries_elision.bdg", True),
                                             *[(mir, True) for mir in saved]]:
                        capture = execute([binary, "run", *(["--release"] if optimize else []),
                                           "--workers", workers, target], settings)
                        results.append(capture | dict(settings=settings))
                for workers in [1, 8]:
                    # The new saved program uses only pre-existing MIR/runtime APIs.
                    capture = execute([previous, "run", "--release", "--workers", workers, saved[0]], settings)
                    results.append(capture | dict(settings=settings))
                    for mode in ["entries-immediate", "entries-stored"]:
                        config = dict(mode=mode, n=33, rounds=3, payload=17, retain=4)
                        capture = execute([binary, "run", "--release", "--workers", workers,
                                           "bench/gc/coverage.bdg"], settings | dict(GC_BENCH_CONFIG=json.dumps(config)),
                                          output=None)
                        validate_coverage(json.loads(capture["stdout"]), config)
                        results.append(capture | dict(settings=settings, config=config))
    def sha(path):
        with path.open("rb") as stream:
            return hashlib.file_digest(stream, "sha256").hexdigest()
    report = dict(binary=dict(path=str(binary), sha256=sha(binary)),
                  compatible=dict(path=str(previous), sha256=sha(previous)),
                  sources={p: sha(ROOT / p) for p in ["bench/gc/check_map_entries.py", "tests/run/map_entries_elision.bdg",
                                                      "bench/gc/coverage.bdg"]}, runs=results, probes=probes)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_bytes(gzip.compress((json.dumps(report, indent=2) + "\n").encode(), mtime=0))
    print(f"{len(results)} map-entry source/saved-MIR GC checks and {len(probes)} MIR probes passed")


if __name__ == "__main__":
    main()
