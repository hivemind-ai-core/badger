#!/usr/bin/env python3
"""Check buffer push widths, roots, failures and unchanged saved-MIR ABI."""
import argparse
import gzip
import json
from pathlib import Path
import subprocess
import tempfile

import applications as app


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    binaries = {label: getattr(args, label).resolve() for label in ['baseline', 'candidate']}
    source = app.gc.ROOT / 'tests/run/buffer_push.bdg'
    expected = source.with_suffix('.out').read_text()
    clean = app.rw.clean_env()
    tracked = [source, source.with_suffix('.out'), Path(__file__).resolve(), app.gc.ROOT / 'bench/gc/coverage.bdg']
    report = dict(binaries={n: dict(path=str(p), sha256=app.rw.digest(p)) for n, p in binaries.items()},
                  sources={str(p.relative_to(app.gc.ROOT)): app.rw.digest(p) for p in tracked}, runs=[])

    def execute(binary, program, flags=(), settings=None, stdout=expected):
        command = [str(binary), 'run', *flags, str(program)]
        result = subprocess.run(command, cwd=app.gc.ROOT, env=clean | (settings or {}),
                                text=True, capture_output=True, timeout=180)
        assert result.returncode == 0 and (stdout is None or result.stdout == stdout), (command, result.returncode, result.stdout, result.stderr)
        return dict(command=command, settings=settings or {}, stdout=result.stdout, stderr=result.stderr)

    with tempfile.TemporaryDirectory(prefix='badger-buffer-push-checks-') as temporary:
        temporary = Path(temporary)
        saved = {}
        for label, binary in binaries.items():
            path = temporary / f'{label}.bdgmir'
            result = subprocess.run([str(binary), 'compile', str(source), '-o', str(path)], cwd=app.gc.ROOT,
                                    env=clean, text=True, capture_output=True, timeout=180)
            assert result.returncode == 0, result.stderr
            saved[label] = path
        report['programs'] = {n: app.rw.digest(p) for n, p in saved.items()}
        assert len(set(report['programs'].values())) == 1, 'runtime-only change altered saved MIR'
        for generation in ['0', '1']:
            for verify in ['0', '1']:
                for workers in [1, 2, 4, 8]:
                    env = dict(BADGER_GC_STRESS='1', BADGER_GC_VERIFY=verify,
                               BADGER_GC_GENERATIONAL=generation, BADGER_CHAOS='2026101958')
                    flags = ['--release', '--workers', str(workers)]
                    for label, program, extra in [('source', source, {}), ('no-inline', source, {'BADGER_NO_INLINE': '1'}),
                                                  ('old-mir', saved['baseline'], {}), ('new-mir', saved['candidate'], {})]:
                        report['runs'].append(dict(mode=label, **execute(binaries['candidate'], program, flags, env | extra)))
                    if workers in [1, 8]:
                        report['runs'].append(dict(mode='unoptimized', **execute(binaries['candidate'], source, ['--workers', str(workers)], env)))
                        report['runs'].append(dict(mode='reverse-new-mir', **execute(binaries['baseline'], saved['candidate'], flags, env)))
                print(f'fixture: generation={generation} verify={verify} passed', flush=True)
        for mode in ['bytes', 'words', 'buffers-grow', 'buffers-pointers', 'buffers-capacity', 'buffers-capacity-grow', 'buffers-capacity-pointers']:
            config = dict(mode=mode, n=19, rounds=2, payload=33, retain=4)
            for generation in ['0', '1']:
                for verify in ['0', '1']:
                    for workers in [1, 8]:
                        env = dict(BADGER_GC_STRESS='1', BADGER_GC_VERIFY=verify, BADGER_GC_GENERATIONAL=generation,
                                   BADGER_CHAOS='2026101958', GC_BENCH_CONFIG=json.dumps(config))
                        result = execute(binaries['candidate'], app.gc.ROOT / 'bench/gc/coverage.bdg',
                                         ['--release', '--workers', str(workers)], env, stdout=None)
                        app.gc.validate_coverage(json.loads(result['stdout']), config)
                        report['runs'].append(dict(mode=mode, **result))
        assert all(app.rw.digest(p) == report['binaries'][n]['sha256'] for n, p in binaries.items())
        assert all(app.rw.digest(app.gc.ROOT / p) == sha for p, sha in report['sources'].items())
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_bytes(gzip.compress((json.dumps(report, indent=2) + '\n').encode(), mtime=0))
    print(f"{len(report['runs'])} buffer GC/semantic checks passed; old/new saved MIR is identical")


if __name__ == '__main__':
    main()
