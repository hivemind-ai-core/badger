#!/usr/bin/env python3
"""Check stored sequence fusion, escape fallbacks, roots and saved-MIR compatibility."""
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
    source = app.gc.ROOT / 'tests/run/sequence_stored.bdg'
    expected = source.with_suffix('.out').read_text()
    clean = app.rw.clean_env()
    tracked = [source, source.with_suffix('.out'), Path(__file__).resolve(), app.gc.ROOT / 'bench/gc/coverage.bdg']
    report = dict(binaries={n: dict(path=str(p), sha256=app.rw.digest(p)) for n, p in binaries.items()},
                  sources={str(p.relative_to(app.gc.ROOT)): app.rw.digest(p) for p in tracked}, probes=[], runs=[])

    def execute(binary, program, flags=(), settings=None, stdout=expected, code=0):
        command = [str(binary), 'run', *flags, str(program)]
        result = subprocess.run(command, cwd=app.gc.ROOT, env=clean | (settings or {}),
                                text=True, capture_output=True, timeout=180)
        assert result.returncode == code and (stdout is None or result.stdout == stdout), (command, result.returncode, result.stdout, result.stderr)
        return dict(command=command, settings=settings or {}, stdout=result.stdout, stderr=result.stderr)

    probes = [
        ('fold', 'let s = seq.range(0, 10).map(fn(x: Int) { x * 2 }); assert(s.fold(0, fn(a, x) { a + x }) == 90)', True),
        ('repeat_take', 'let s = seq.range(0, 10).take(3); assert(s.count() == 3); assert(s.count() == 3)', True),
        ('inline_extension', 'let s = seq.range(0, 10); assert(s.map(fn(x: Int) { x + 1 }).take(2).count() == 2)', True),
        ('for', 'let s = seq.range(0, 10).take(2); var n = 0; for x in s { n = n + x }; assert(n == 1)', True),
        ('alias_import', 'let s = q.range(0, 10).take(2); assert(q.count(s) == 2)', True),
        ('captured', 'let s = seq.range(0, 10).take(2); let f = fn() { s.count() }; assert(f() == 2); assert(s.count() == 2)', False),
        ('escaped', 'let s = seq.range(0, 10).take(2); assert(consume(s) == 2); assert(s.count() == 2)', False),
        ('collected', 'let s = seq.range(0, 10).take(2); assert(s.collect() == [0, 1]); assert(s.count() == 2)', False),
        ('mutable', 'var s = seq.range(0, 10); s = seq.range(0, 2); assert(s.count() == 2)', False),
        ('unknown', 'let s = unknown(); assert(s.count() == 2)', False),
        ('scoped', 'let s = seq.scoped(fn() { Cell.new(0) }, fn(c) { let x = c.get(); c.set(x + 1); if x < 2 { Option.some(x) } else { Option.none } }, fn(_) { () }); assert(s.count() == 2)', False),
        ('stored_alias', 'let s = seq.range(0, 10); let alias = s.take(2); assert(alias.count() == 2); assert(s.count() == 10)', False),
    ]
    with tempfile.TemporaryDirectory(prefix='badger-stored-sequence-checks-') as temporary:
        temporary = Path(temporary)
        for name, expression, fused in probes:
            probe = temporary / f'{name}.bdg'
            probe.write_text('import std.seq\nimport std.seq as q\nimport std.list\nfn consume(s: Seq<Int>) -> Int { s.count() }\nfn unknown() -> Seq<Int> { seq.range(0, 2) }\npub fn main() -> Int {\n' + expression + '\n0\n}\n')
            counts = {}
            for label, binary in binaries.items():
                result = execute(binary, probe, ['--release', '--dump-mir'], stdout='')
                main = result['stderr'].split('\nfn 1 ')[0]
                counts[label] = main.count('Alloc {')
                report['probes'].append(dict(label=label, case=name, fused=fused, allocations=counts[label], **result))
            if fused:
                assert counts['baseline'] > 0 and counts['candidate'] == 0, (name, counts)
            else:
                assert counts['baseline'] == counts['candidate'] and counts['candidate'] > 0, (name, counts)
        saved = {}
        for label, binary in binaries.items():
            path = temporary / f'{label}.bdgmir'
            result = subprocess.run([str(binary), 'compile', str(source), '-o', str(path)], cwd=app.gc.ROOT,
                                    env=clean, text=True, capture_output=True, timeout=180)
            assert result.returncode == 0, result.stderr
            saved[label] = path
        report['programs'] = {n: app.rw.digest(p) for n, p in saved.items()}
        for generation in ['0', '1']:
            for verify in ['0', '1']:
                for workers in [1, 2, 4, 8]:
                    env = dict(BADGER_GC_STRESS='1', BADGER_GC_VERIFY=verify,
                               BADGER_GC_GENERATIONAL=generation, BADGER_CHAOS='2026101943')
                    flags = ['--release', '--workers', str(workers)]
                    for label, program, extra in [('source', source, {}), ('no-inline', source, {'BADGER_NO_INLINE': '1'}),
                                                  ('old-mir', saved['baseline'], {}), ('new-mir', saved['candidate'], {})]:
                        report['runs'].append(dict(mode=label, **execute(binaries['candidate'], program, flags, env | extra)))
                    if workers in [1, 8]:
                        report['runs'].append(dict(mode='unoptimized', **execute(binaries['candidate'], source, ['--workers', str(workers)], env)))
                        report['runs'].append(dict(mode='reverse-new-mir', **execute(binaries['baseline'], saved['candidate'], flags, env)))
                print(f'fixture: generation={generation} verify={verify} passed', flush=True)
        for mode in ['visible', 'stored', 'stored-reused', 'stored-list', 'stored-pointers', 'stored-escape', 'stored-collected', 'collected', 'eager']:
            config = dict(mode=mode, n=9, rounds=3, payload=3, retain=2)
            for generation in ['0', '1']:
                for verify in ['0', '1']:
                    for workers in [1, 8]:
                        env = dict(BADGER_GC_STRESS='1', BADGER_GC_VERIFY=verify, BADGER_GC_GENERATIONAL=generation,
                                   BADGER_CHAOS='2026101943', GC_BENCH_CONFIG=json.dumps(config))
                        result = execute(binaries['candidate'], app.gc.ROOT / 'bench/gc/coverage.bdg',
                                         ['--release', '--workers', str(workers)], env, stdout=None)
                        app.gc.validate_coverage(json.loads(result['stdout']), config)
                        report['runs'].append(dict(mode=mode, **result))
        assert all(app.rw.digest(p) == report['binaries'][n]['sha256'] for n, p in binaries.items())
        assert all(app.rw.digest(app.gc.ROOT / p) == sha for p, sha in report['sources'].items())
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_bytes(gzip.compress((json.dumps(report, indent=2) + '\n').encode(), mtime=0))
    print(f"{len(report['probes'])} MIR probes and {len(report['runs'])} stored-sequence GC/semantic checks passed")


if __name__ == '__main__':
    main()
