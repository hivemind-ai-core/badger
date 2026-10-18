#!/usr/bin/env python3
"""Exercise object flags, buffer freezing and backpatched saved-MIR descriptors."""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--ordinary', type=Path, required=True)
    ap.add_argument('--compatible', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    args = ap.parse_args()
    binary, previous = args.ordinary.resolve(), args.compatible.resolve()
    env = {k: v for k, v in os.environ.items() if not k.startswith(('BADGER_', 'LANG_'))}
    binaries = {str(p): sha(p) for p in [binary, previous]}
    rows, sources = [], {}
    with tempfile.TemporaryDirectory(prefix='badger-finalizer-check-') as directory:
        tmp = Path(directory)
        for name in ['packed_arrays', 'array_values', 'gc_old_containers']:
            source = ROOT / 'tests/run' / f'{name}.bdg'
            expected = source.with_suffix('.out')
            sources[str(source)] = sha(source)
            sources[str(expected)] = sha(expected)
            if name == 'gc_old_containers':
                # Keep all barrier/native-container assertions; a collection
                # on every allocation makes the original churn unnecessary.
                body = source.read_text()
                original = 'if frequent { 600 } else { 450000 }'
                assert body.count(original) == 1
                source = tmp / 'gc_old_containers-bounded.bdg'
                source.write_text(body.replace(original, 'if frequent { 8 } else { 8 }'))
                sources['derived/gc_old_containers-bounded.bdg'] = sha(source)
            targets = [source]
            for label, compiler in [('old', previous), ('new', binary)]:
                saved = tmp / f'{name}-{label}.bdgmir'
                subprocess.run([str(compiler), 'compile', str(source), '-o', str(saved)],
                               cwd=ROOT, env=env, capture_output=True, check=True, timeout=180)
                targets.append(saved)
            for verify in [0, 1]:
                for gen in [0, 1]:
                    for workers in [1, 8]:
                        settings = dict(BADGER_GC_STRESS='1', BADGER_GC_VERIFY=str(verify),
                                        BADGER_GC_GENERATIONAL=str(gen), BADGER_CHAOS='20261018')
                        for target in targets:
                            command = [str(binary), 'run', '--release', '--workers', str(workers), str(target)]
                            result = subprocess.run(command, cwd=ROOT, env=env | settings,
                                                    capture_output=True, text=True, timeout=180)
                            assert result.returncode == 0, (command, settings, result.stdout, result.stderr)
                            assert result.stdout == expected.read_text(), (command, result.stdout)
                            rows.append(dict(command=command, settings=settings,
                                             stdout=result.stdout, stderr=result.stderr))
            print(f'{name}: 24 source/old-MIR/new-MIR stress checks passed', flush=True)
    assert binaries == {str(p): sha(p) for p in [binary, previous]}, 'binary changed during checks'
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_bytes(gzip.compress((json.dumps(dict(binaries=binaries, sources=sources, runs=rows),
                                                    indent=2) + '\n').encode(), mtime=0))


if __name__ == '__main__':
    main()
