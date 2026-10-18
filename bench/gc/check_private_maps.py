#!/usr/bin/env python3
"""Check private-map reuse, immutable aliases and saved-program GC behavior."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import random
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
    expected = (ROOT / "tests/run/private_map_updates.out").read_text()
    runs, probes, generated = [], [], []
    compatible_private = None

    def execute(command, settings=None, output=expected):
        result = subprocess.run(list(map(str, command)), cwd=ROOT, env=env | (settings or {}),
                                capture_output=True, text=True, timeout=180)
        assert result.returncode == 0, (command, result.stdout, result.stderr)
        if output is not None:
            assert result.stdout == output, (command, result.stdout, result.stderr)
        return dict(command=list(map(str, command)), stdout=result.stdout, stderr=result.stderr)

    setup = "var m: Map<Int, Int> = map.empty()\nfor i in 0..7 { m = m.insert(i, i) }\n"
    cases = {
        "private": (setup + "io.println(m.len())?", "7\n", 1),
        "snapshot": (setup + "let old = m\nm = m.insert(99, 1)\nio.println(old.len() + m.len())?", "15\n", 0),
        "hidden": (setup + "let hidden = Cell.new(m)\nm = m.insert(99, 1)\nio.println(hidden.get().len() + m.len())?", "15\n", 0),
        "cursor": (setup + "var count = 0\nfor (k, v) in m { m = m.insert(k + 16, v); count = count + 1; assert(count <= 7) }\nio.println(count + m.len())?", "21\n", 0),
        "pointer": ("var m: Map<Int, String> = map.empty()\nfor i in 0..7 { m = m.insert(i, text.repeat(\"x\", i + 1)) }\nio.println(m.len())?", "7\n", 0),
        "reads": ("var m: Map<Int, Int> = map.empty()\nfor i in 0..65 { let old = m; m = m.insert(i % 7, old.get_or(i % 7, 0) + i) }\nio.println(m.len())?", "7\n", 1),
        "branches": ("var m: Map<Int, Int> = map.empty()\nfor i in 0..7 { if i % 2 == 0 { m = m.insert(i, i) } else { m = m.insert(i, i * 2) } }\nio.println(m.len())?", "7\n", 2),
    }
    with tempfile.TemporaryDirectory(prefix="badger-private-map-checks-") as directory:
        tmp = Path(directory)
        for name, (body, output, calls) in cases.items():
            source = tmp / f"{name}.bdg"
            source.write_text("import std.io\nimport std.map\nimport std.text\npub fn main() -> Result<Unit, io.Error> {\n" + body + "\nok(())\n}\n")
            for compiler, optimized, no_inline in [(previous, True, False), (binary, False, False),
                                                    (binary, True, False), (binary, True, True)]:
                capture = execute([compiler, "run", "--dump-mir", *(["--release"] if optimized else []), source],
                                  {"BADGER_NO_INLINE": "1"} if no_inline else None, output)
                count = capture["stderr"].count('name: "badger_rt_i_map_insert_private"')
                if compiler == previous and name == "private":
                    # Compatible binaries may predate I5a or include it. Calibrate
                    # their capability on this one-site chain; candidate checks
                    # stay strict. Both kinds of old saved MIR must keep working.
                    assert count in [0, 1], capture
                    compatible_private = count
                wanted = calls if compiler == binary and optimized and not no_inline else (
                    calls * compatible_private if compiler == previous else 0)
                assert count == wanted, (name, optimized, no_inline, count, wanted, capture)
                owned = capture["stderr"].count('name: "badger_rt_i_map_insert_owned"')
                wanted_owned = (2 if name in ["snapshot", "hidden", "cursor"] else 0) if compiler == binary and optimized and not no_inline else 0
                assert owned == wanted_owned, (name, optimized, no_inline, owned, wanted_owned, capture)
                probes.append(capture | dict(case=name, optimized=optimized, no_inline=no_inline))
        saved = []
        for name, compiler in [("current", binary), ("previous", previous)]:
            mir = tmp / f"{name}.bdgmir"
            execute([compiler, "compile", "tests/run/private_map_updates.bdg", "-o", mir], output=None)
            saved.append(mir)
        for verify in [0, 1]:
            for generation in [0, 1]:
                settings = dict(BADGER_GC_STRESS="1", BADGER_GC_VERIFY=str(verify),
                                BADGER_GC_GENERATIONAL=str(generation), BADGER_CHAOS="839")
                for workers in [1, 4, 8]:
                    for target, optimized in [("tests/run/private_map_updates.bdg", False),
                                             ("tests/run/private_map_updates.bdg", True),
                                             *[(mir, True) for mir in saved]]:
                        capture = execute([binary, "run", *(["--release"] if optimized else []),
                                           "--workers", workers, target], settings)
                        runs.append(capture | dict(settings=settings))
                for workers in [1, 8]:
                    for mode in ["persistent-int", "scalar-snapshots"]:
                        config = dict(mode=mode, n=257, rounds=1, payload=17, retain=17)
                        capture = execute([binary, "run", "--release", "--workers", workers, "bench/gc/coverage.bdg"],
                                          settings | dict(GC_BENCH_CONFIG=json.dumps(config)), output=None)
                        validate_coverage(json.loads(capture["stdout"]), config)
                        runs.append(capture | dict(settings=settings, config=config))
                # Grow several trie levels while every allocation collects;
                # old private nodes must retain newly allocated descendants.
                for workers in [1, 8]:
                    config = dict(mode="persistent-int", n=4097, rounds=1, payload=17, retain=4096)
                    capture = execute([binary, "run", "--release", "--workers", workers, "bench/gc/coverage.bdg"],
                                      settings | dict(GC_BENCH_CONFIG=json.dumps(config)), output=None)
                    validate_coverage(json.loads(capture["stdout"]), config)
                    runs.append(capture | dict(settings=settings, config=config))
        # Real snapshot epochs: all saved values, packed/floating scalars, local
        # forks, deletion after sharing and a concurrently reading task.
        snapshot_source = ROOT / "tests/run/snapshot_map_updates.bdg"
        snapshot_expected = snapshot_source.with_suffix(".out").read_text()
        snapshot_saved = []
        for name, compiler in [("current", binary), ("previous", previous)]:
            mir = tmp / f"snapshots-{name}.bdgmir"
            execute([compiler, "compile", snapshot_source, "-o", mir], output=None)
            snapshot_saved.append(mir)
        for verify in [0, 1]:
            for generation in [0, 1]:
                settings = dict(BADGER_GC_STRESS="1", BADGER_GC_VERIFY=str(verify),
                                BADGER_GC_GENERATIONAL=str(generation), BADGER_CHAOS="839")
                for workers in [1, 2, 4, 8]:
                    for target, optimized in [(snapshot_source, False), (snapshot_source, True),
                                             *[(mir, True) for mir in snapshot_saved]]:
                        capture = execute([binary, "run", *(["--release"] if optimized else []),
                                           "--workers", workers, target], settings, snapshot_expected)
                        runs.append(capture | dict(settings=settings))
        # Independent Python maps model snapshots and dead aliases in generated
        # source. Include snapshots before updates, final-only publication, and
        # entirely private loops; keep every key/value check, not just a sum.
        for seed in range(12):
            n = random.Random(seed).randrange(33, 65)
            current, versions = {}, []
            for i in range(n):
                if seed % 3 == 0 and i % 13 == 0:
                    versions.append(current.copy())
                key = (i * 7 + seed) % 5 - 2
                current[key] = current.get(key, 0) + i + 1
            versions.append(current.copy())
            checks = []
            for index, version in enumerate(versions):
                checks.append(f"let old{index} = saved[{index}]\nassert(old{index}.len() == {len(version)})")
                for key in range(-2, 3):
                    checks.append(f"check(old{index}, {key}, {version.get(key, -1)})")
            snapshot = "if i % 13 == 0 { versions.append(m) }" if seed % 3 == 0 else ""
            source = tmp / f"generated{seed}.bdg"
            source.write_text('''import std.io
import std.map
import std.list
fn check(m: Map<Int, Int>, key: Int, expected: Int) { assert(m.get_or(key, -1) == expected) }
pub fn main() -> Result<Unit, io.Error> {
    var m: Map<Int, Int> = map.empty()
    let versions = list.builder<Map<Int, Int>>()
    for i in 0..N {
        SNAPSHOT
        let key = (i * 7 + SEED) % 5 - 2
        let dead_alias = m
        m = m.insert(key, dead_alias.get_or(key, 0) + i + 1)
    }
    versions.append(m)
    let saved = versions.finish()
    CHECKS
    io.println("generated map ok")?
    ok(())
}
'''.replace("0..N", f"0..{n}").replace("SEED", str(seed)).replace("SNAPSHOT", snapshot).replace("CHECKS", "\n".join(checks)))
            for compiler, workers, settings in [(previous, 1, {}), (binary, 1, {}), (binary, 4,
                    dict(BADGER_GC_STRESS="1", BADGER_GC_VERIFY=str(seed % 2),
                         BADGER_GC_GENERATIONAL=str((seed // 2) % 2), BADGER_CHAOS="839"))]:
                capture = execute([compiler, "run", "--release", "--workers", workers, source], settings, "generated map ok\n")
                generated.append(capture | dict(seed=seed, settings=settings, source=source.read_text()))
    def sha(path):
        with path.open("rb") as stream:
            return hashlib.file_digest(stream, "sha256").hexdigest()
    report = dict(binary=dict(path=str(binary), sha256=sha(binary)), compatible=dict(path=str(previous), sha256=sha(previous)),
                  compatible_private_sites=compatible_private,
                  sources={p: sha(ROOT / p) for p in ["bench/gc/check_private_maps.py", "tests/run/private_map_updates.bdg", "tests/run/snapshot_map_updates.bdg",
                                                      "bench/gc/coverage.bdg"]}, runs=runs, probes=probes, generated=generated)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_bytes(gzip.compress((json.dumps(report, indent=2) + "\n").encode(), mtime=0))
    print(f"{len(runs)} source/saved-MIR GC checks, {len(probes)} MIR probes, {len(generated)} differential cases passed")


if __name__ == "__main__":
    main()
