#!/usr/bin/env python3
"""Check map helper projections, callback roots/order and saved-MIR compatibility."""
import argparse
import gzip
import json
from pathlib import Path
import subprocess
import tempfile

import applications as app


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--baseline', type=Path, required=True)
    ap.add_argument('--candidate', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    args = ap.parse_args()
    binaries = {name: getattr(args, name).resolve() for name in ['baseline', 'candidate']}
    source = app.gc.ROOT / 'tests/run/map_helpers.bdg'
    expected = source.with_suffix('.out').read_text()
    clean = app.rw.clean_env()
    tracked = [source, source.with_suffix('.out'), Path(__file__).resolve(), app.gc.ROOT / 'bench/gc/coverage.bdg']
    report = dict(binaries={n:dict(path=str(p), sha256=app.rw.digest(p)) for n,p in binaries.items()},
                  sources={str(p.relative_to(app.gc.ROOT)):app.rw.digest(p) for p in tracked}, probes=[], runs=[])

    def execute(binary, program, flags=(), settings=None, stdout=expected, code=0):
        command = [str(binary), 'run', *flags, str(program)]
        result = subprocess.run(command, cwd=app.gc.ROOT, env=clean | (settings or {}),
                                text=True, capture_output=True, timeout=180)
        assert result.returncode == code and (stdout is None or result.stdout == stdout), (command, result.returncode, result.stdout, result.stderr)
        return dict(command=command, settings=settings or {}, stdout=result.stdout, stderr=result.stderr)

    with tempfile.TemporaryDirectory(prefix='badger-map-helper-checks-') as tmp:
        tmp = Path(tmp)
        for name, expr, old_count in [
            ('keys', 'm.keys().len()', 1), ('values', 'map.values(m).len()', 1),
            ('map_values', 'm.map_values(fn(v) { v * 3 }).len()', 1),
            ('filter', 'map.filter(m, fn(k, v) { k > 0 && v > 0 }).len()', 1),
            ('merge_with', 'm.merge_with(m, fn(a, b) { a + b }).len()', 2),
            ('stored_entries', 'let entries = m.entries(); let keys = entries.map(fn(e) { e.0 }); keys.len()', 1),
        ]:
            probe = tmp / f'{name}.bdg'
            probe.write_text('import std.map\nimport std.list\npub fn main() -> Int {\nlet m = map.from_list([(1, 3), (2, 7)])\n' + expr + '\n}\n')
            for label, binary in binaries.items():
                result = execute(binary, probe, ['--release', '--dump-mir'], stdout='', code=2)
                count = result['stderr'].count('name: "badger_rt_i_map_entries"')
                assert count == (old_count if label == 'baseline' or name == 'stored_entries' else 0), (label, name, count, result)
                report['probes'].append(dict(label=label, case=name, entries=count, **result))
        saved = {}
        for label, binary in binaries.items():
            path = tmp / f'{label}.bdgmir'
            result = subprocess.run([str(binary), 'compile', str(source), '-o', str(path)],
                                    cwd=app.gc.ROOT, env=clean, text=True, capture_output=True, timeout=60)
            assert result.returncode == 0, result.stderr
            saved[label] = path
        report['programs'] = {n:app.rw.digest(p) for n,p in saved.items()}
        for generation in ['0', '1']:
            for verify in ['0', '1']:
                for workers in [1, 2, 4, 8]:
                    env = dict(BADGER_GC_STRESS='1', BADGER_GC_VERIFY=verify,
                               BADGER_GC_GENERATIONAL=generation, BADGER_CHAOS='2026101931')
                    flags = ['--release', '--workers', str(workers)]
                    for label, program, extra in [('source', source, {}), ('no-inline', source, {'BADGER_NO_INLINE':'1'}),
                                                  ('old-mir', saved['baseline'], {}), ('new-mir', saved['candidate'], {})]:
                        result = execute(binaries['candidate'], program, flags, env | extra)
                        report['runs'].append(dict(mode=label, generation=generation, verify=verify, workers=workers, **result))
                    if workers in [1, 8]:
                        result = execute(binaries['candidate'], source, ['--workers', str(workers)], env)
                        report['runs'].append(dict(mode='unoptimized', generation=generation, verify=verify, workers=workers, **result))
                        result = execute(binaries['baseline'], saved['candidate'], flags, env)
                        report['runs'].append(dict(mode='reverse-new-mir', generation=generation, verify=verify, workers=workers, **result))
                print(f'fixture: generation={generation} verify={verify} passed', flush=True)
        for mode in [name for name in app.gc.COVERAGE if name.startswith('map-api-')]:
            config = dict(mode=mode, n=9, rounds=3, payload=3, retain=2)
            for generation in ['0', '1']:
                for verify in ['0', '1']:
                    for workers in [1, 8]:
                        env = dict(BADGER_GC_STRESS='1', BADGER_GC_VERIFY=verify, BADGER_GC_GENERATIONAL=generation,
                                   BADGER_CHAOS='2026101931', GC_BENCH_CONFIG=json.dumps(config))
                        result = execute(binaries['candidate'], app.gc.ROOT / 'bench/gc/coverage.bdg',
                                         ['--release', '--workers', str(workers)], env, stdout=None)
                        app.gc.validate_coverage(json.loads(result['stdout']), config)
                        report['runs'].append(dict(mode=mode, **result))
        assert all(app.rw.digest(p) == report['binaries'][n]['sha256'] for n,p in binaries.items())
        assert all(app.rw.digest(app.gc.ROOT / p) == sha for p,sha in report['sources'].items())
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_bytes(gzip.compress((json.dumps(report, indent=2) + '\n').encode(), mtime=0))
    print(f"{len(report['probes'])} MIR probes and {len(report['runs'])} map-helper GC/semantic checks passed")


if __name__ == '__main__':
    main()
