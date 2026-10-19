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
    programs = {(case, label): ARTIFACTS / f'isolated-{case}-{label}.bdgmir' for case in CASES for label in binaries}
    sources = {case: ARTIFACTS / f'isolated-{case}.bdg' for case in CASES}
    equality = {case: sha(programs[case, 'baseline']) == sha(programs[case, 'candidate']) for case in CASES}
    assert all(equality.values()), equality
    configs = {name: gc.COVERAGE[name] for name in CASES}
    expected = {name: gc.expected_coverage(config) for name, config in configs.items()}
    source_hashes = {str(p.relative_to(ROOT)): sha(p) for p in [Path(__file__), *sources.values(), ROOT / 'bench/gc/run.py', ROOT / 'bench/gc/rss_spawn.c']}
    with tempfile.TemporaryDirectory(prefix='badger-isolated-controls-') as tmp:
        tmp = Path(tmp)
        launcher = tmp / 'measure'
        subprocess.run(['cc', '-O2', '-std=c11', '-Wall', '-Wextra', '-Werror', str(ROOT / 'bench/gc/rss_spawn.c'), '-o', str(launcher)], check=True)
        for cohort, same_native, seed in [('I3a-isolated-cross-native', False, 2026101954)]:
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
                    program = programs[name, 'candidate']
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
