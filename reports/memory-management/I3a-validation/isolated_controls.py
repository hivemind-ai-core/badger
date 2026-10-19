"""Isolate untouched controls from the changed branches in coverage.bdg."""
from pathlib import Path
import gzip
import json
import random
import re
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'bench/gc'))
import applications as app

gc = app.gc
BASE = ROOT / 'reports/memory-management'
ARTIFACTS = BASE / 'I3a-validation'
BIN = Path('/home/dan/orca/projects/language/target/memory-binaries-2026-10-18')
binaries = {'baseline': BIN / 'map-helpers-candidate', 'candidate': BIN / 'stored-sequence-candidate'}
sha = app.rw.digest
CASES = ['visible', 'bytes-transient', 'stored-escape']
with gc.benchmark_lock():
    env = app.rw.clean_env()
    coverage = (ROOT / 'bench/gc/coverage.bdg').read_text()
    buffer_body = coverage.split('fn buffers(c: Config, narrow: Bool) -> Int {')[1].split('\nfn buffer_growth')[0]
    header = '\n'.join(f'import std.{name}' for name in ['array', 'codec', 'env', 'inspect', 'io', 'list', 'seq', 'time']) + '\ntype Config = { mode: String, n: Int, rounds: Int, payload: Int, retain: Int }\n'
    main = '''pub fn main() -> Result<Unit, io.Error> {
    let c = codec.decode_text<Config>(env.get("GC_BENCH_CONFIG").unwrap_or("")).unwrap_or({ mode: "invalid", n: 1, rounds: 1, payload: 1, retain: 1 })
    let before = inspect.heap()
    let start = time.now()
    let checksum = work(c)
    let elapsed_ns = time.as_nanos(time.now() - start)
    let after = inspect.heap()
    io.println(codec.encode_text({ config: c, checksum, elapsed_ns, before, after }))?
    ok(())
}
'''
    programs, sources, equality = {}, {}, {}
    for case in CASES:
        if case == 'bytes-transient':
            body = 'fn buffers(c: Config, narrow: Bool) -> Int {' + buffer_body + '\nfn work(c: Config) -> Int { buffers(c, true) }\n'
        elif case == 'visible':
            body = 'fn work(c: Config) -> Int { var n = 0; for _ in 0..c.rounds { n = n + seq.range(0, c.n).map(fn(x: Int) { x * 2 }).filter(fn(x: Int) { x % 3 == 0 }).fold(0, fn(a: Int, x: Int) { a + x }) }; n }\n'
        else:
            body = 'fn consume(s: Seq<Int>) -> Int { s.fold(0, fn(a: Int, x: Int) { a + x }) }\nfn work(c: Config) -> Int { var n = 0; for _ in 0..c.rounds { let s = seq.range(0, c.n).map(fn(x: Int) { x * 2 }).filter(fn(x: Int) { x % 3 == 0 }); n = n + consume(s) }; n }\n'
        source = ARTIFACTS / f'isolated-{case}.bdg'
        assert not source.exists(), source
        source.write_text(header + body + main)
        sources[case] = source
        for label, binary in binaries.items():
            path = ARTIFACTS / f'isolated-{case}-{label}.bdgmir'
            subprocess.run([str(binary), 'compile', str(source), '-o', str(path)], cwd=ROOT, env=env, check=True, capture_output=True, timeout=180)
            programs[case, label] = path
        equality[case] = sha(programs[case, 'baseline']) == sha(programs[case, 'candidate'])
        print('Identical isolated MIR:', case, equality[case], flush=True)
    configs = {name: gc.COVERAGE[name] for name in CASES}
    expected = {name: gc.expected_coverage(config) for name, config in configs.items()}
    source_hashes = {str(p.relative_to(ROOT)): sha(p) for p in [Path(__file__), *sources.values(), ROOT / 'bench/gc/run.py', ROOT / 'bench/gc/rss_spawn.c']}
    with tempfile.TemporaryDirectory(prefix='badger-isolated-controls-') as tmp:
        tmp = Path(tmp)
        launcher = tmp / 'measure'
        subprocess.run(['cc', '-O2', '-std=c11', '-Wall', '-Wextra', '-Werror', str(ROOT / 'bench/gc/rss_spawn.c'), '-o', str(launcher)], check=True)
        for cohort, same_native, seed in [('I3a-isolated-source', False, 2026101950), ('I3a-isolated-native', True, 2026101951)]:
            directory = BASE / cohort
            directory.mkdir(exist_ok=False)
            report = dict(schema=2, status='running', revision=app.rw.output(['git', 'rev-parse', 'HEAD']),
                          arguments=dict(reps=15, workers=[1, 8], cases=CASES, seed=seed, same_native=same_native),
                          binaries={label: dict(path=str(binaries['candidate' if same_native else label]), sha256=sha(binaries['candidate' if same_native else label])) for label in binaries},
                          effective_programs={f'{case}/{label}': dict(path=str(path.relative_to(ROOT)), sha256=sha(path)) for (case, label), path in programs.items()},
                          isolated_mir_identical=equality, sources=source_hashes, runs=[])
            rng = random.Random(seed)
            for rep in range(-1, 15):
                cells = [(name, workers, variant) for name in CASES for workers in [1, 8] for variant in binaries]
                for name, workers, variant in gc.measurement_order(cells, rng, paired=True):
                    binary = binaries['candidate' if same_native else variant]
                    program = programs[name, variant] if same_native else sources[name]
                    command = [str(launcher), str(binary), 'run', '--release', '--bench', '--workers', str(workers), str(program)]
                    before = gc.host_context()
                    out, err, process = gc.t.run_process(command, env | dict(GC_BENCH_CONFIG=json.dumps(configs[name])), 180)
                    after = gc.host_context()
                    rss = re.search(r'^\[gc-bench child rss\] (\d+)$', err, re.MULTILINE)
                    assert rss, err
                    process['launcher_tree_peak_rss_kib'] = process['peak_rss_kib']
                    process['peak_rss_kib'] = int(rss[1])
                    record = json.loads(out)
                    assert record['config'] == configs[name] and record['checksum'] == expected[name] and record['elapsed_ns'] > 0
                    if rep >= 0:
                        a, b = record['before'], record['after']
                        report['runs'].append(dict(case=name, workers=workers, variant=variant, repetition=rep,
                            record=record, process=process, stderr=err, gc=None, allocations=None,
                            host_before=before, host_after=after, heap_collections=b['collections']-a['collections'],
                            gc_pause_ns=b['pause_total_ns']-a['pause_total_ns']))
                    print(cohort, rep, name, workers, variant, flush=True)
            assert all(sha(ROOT / p) == value for p, value in source_hashes.items())
            report['status'] = 'complete'
            report['summary'] = gc.summarize(report['runs'])
            (directory / 'measurements.json.gz').write_bytes(gzip.compress((json.dumps(report, indent=2) + '\n').encode(), mtime=0))
