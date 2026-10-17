#!/usr/bin/env python3
"""Check local split lookup MIR, escaping ABI, payload roots and saved programs."""
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
    ap.add_argument('--ordinary', type=Path, required=True)
    ap.add_argument('--compatible', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    args = ap.parse_args()
    binary, previous = args.ordinary.resolve(), args.compatible.resolve()
    clean = {k: v for k, v in os.environ.items() if not k.startswith('BADGER_') and k != 'GC_BENCH_CONFIG'}
    clean['BADGER_GC_POOL_MAX'] = 'unsupported'  # Ordinary artifacts must ignore it.
    expected = (ROOT / 'tests/run/lookup_split.out').read_text()
    results, probes = [], []
    with tempfile.TemporaryDirectory(prefix='badger-lookup-checks-') as directory:
        saved = []
        for name, compiler in [('current', binary), ('previous', previous)]:
            mir = Path(directory) / f'{name}.bdgmir'
            subprocess.run([str(compiler), 'compile', 'tests/run/lookup_split.bdg', '-o', str(mir)],
                           env=clean, cwd=ROOT, capture_output=True, check=True, timeout=60)
            saved.append(mir)
        for optimize in [False, True]:
            command = [str(binary), 'run', '--dump-mir', *(['--release'] if optimize else []), 'tests/run/lookup_split.bdg']
            output = subprocess.run(command, cwd=ROOT, env=clean, capture_output=True, text=True, timeout=60)
            assert output.returncode == 0 and output.stdout == expected, (command, output.stdout, output.stderr)
            for name in ['map', 'list']:
                present = f'badger_rt_i_{name}_get_split' in output.stderr
                assert present == optimize, (name, optimize)
            assert 'name: "badger_rt_i_map_get"' in output.stderr  # Stored/compared options retain boxed ABI.
            probes.append(dict(command=command, optimized=optimize, mir=output.stderr))
        for verify in [0, 1]:
            for generation in [0, 1]:
                env = clean | dict(BADGER_GC_STRESS='1', BADGER_GC_VERIFY=str(verify),
                                   BADGER_GC_GENERATIONAL=str(generation), BADGER_CHAOS='839')
                for workers in [1, 4, 8]:
                    for target, optimize in [(Path('tests/run/lookup_split.bdg'), False),
                                             (Path('tests/run/lookup_split.bdg'), True),
                                             *[(mir, True) for mir in saved]]:
                        command = [str(binary), 'run', *(['--release'] if optimize else []), '--workers', str(workers), str(target)]
                        output = subprocess.run(command, env=env, cwd=ROOT, capture_output=True, text=True, timeout=180)
                        assert output.returncode == 0 and output.stdout == expected, (command, verify, generation, output.stdout, output.stderr)
                        results.append(dict(command=command, verify=verify, generational=generation, stdout=output.stdout, stderr=output.stderr))
                for workers in [1, 8]:
                    for mode in ['lookup-map-int', 'lookup-map-pointers', 'lookup-array-int', 'lookup-list-pointers', 'lookup-map-stored']:
                        config = dict(mode=mode, n=257, rounds=1, payload=17, retain=4)
                        command = [str(binary), 'run', '--release', '--workers', str(workers), 'bench/gc/coverage.bdg']
                        output = subprocess.run(command, env=env | dict(GC_BENCH_CONFIG=json.dumps(config)), cwd=ROOT,
                                                capture_output=True, text=True, timeout=180)
                        assert output.returncode == 0, (command, config, output.stdout, output.stderr)
                        validate_coverage(json.loads(output.stdout), config)
                        results.append(dict(command=command, config=config, verify=verify, generational=generation,
                                            stdout=output.stdout, stderr=output.stderr))
    def sha(path):
        with path.open('rb') as file:
            return hashlib.file_digest(file, 'sha256').hexdigest()
    report = dict(binary=str(binary), sha256=sha(binary), compatible_binary=str(previous), compatible_sha256=sha(previous),
                  source_sha256={p: sha(ROOT / p) for p in ['bench/gc/check_lookup_split.py', 'bench/gc/coverage.bdg', 'tests/run/lookup_split.bdg']},
                  probes=probes, runs=results)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with gzip.open(args.out, 'wt') as file:
        json.dump(report, file, indent=2)
    print(f'{len(results)} lookup ABI/root/payload stress checks and 2 optimized/unoptimized MIR probes passed')


if __name__ == '__main__':
    main()
