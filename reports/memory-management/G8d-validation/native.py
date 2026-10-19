"""Reproduce G8d comparisons with identical saved MIR and fixed native binaries."""
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
ARTIFACTS = BASE / 'G8d-validation'
BIN = Path('/home/dan/orca/projects/language/target/memory-binaries-2026-10-18')
sha = app.rw.digest
CASES = ['bytes-transient', 'bytes-257', 'words-transient', 'buffers-grow', 'buffers-pointers',
         'buffers-capacity-31', 'buffers-capacity-32', 'buffers-capacity-33',
         'buffers-capacity-grow', 'buffers-capacity-pointers', 'records-window', 'stored']

with gc.benchmark_lock():
    env = app.rw.clean_env()
    programs = {}
    for label in ['baseline', 'candidate']:
        path = ARTIFACTS / f'current-coverage-{label}.bdgmir'
        assert not path.exists(), path
        subprocess.run([str(BIN / f'buffer-fixed-{label}'), 'compile', 'bench/gc/coverage.bdg', '-o', str(path)],
                       cwd=ROOT, env=env, capture_output=True, check=True, timeout=180)
        programs[label] = path
    assert sha(programs['baseline']) == sha(programs['candidate'])
    configs = {name: gc.COVERAGE[name] for name in CASES}
    expected = {name: gc.expected_coverage(config) for name, config in configs.items()}
    sources = {str(p.relative_to(ROOT)): sha(p) for p in [ROOT / 'bench/gc/coverage.bdg', ROOT / 'bench/gc/run.py',
                                                        ROOT / 'bench/gc/rss_spawn.c', Path(__file__)]}
    with tempfile.TemporaryDirectory(prefix='badger-buffer-native-') as temporary:
        temporary = Path(temporary)
        launcher = temporary / 'rss-measure'
        subprocess.run(['cc', '-O2', '-std=c11', '-Wall', '-Wextra', '-Werror', str(ROOT / 'bench/gc/rss_spawn.c'),
                        '-o', str(launcher)], check=True)
        for cohort, cases, workers_list, reps, diagnostic, same, seed in [
            ('G8d-current-native', CASES, [1, 2, 4, 8], 7, False, False, 2026101961),
            ('G8d-current-control', ['bytes-transient', 'buffers-capacity-pointers', 'records-window', 'stored'], [1, 8], 7, False, True, 2026101962),
            ('G8d-current-census', CASES, [1, 8], 3, True, False, 2026101963),
        ]:
            directory = BASE / cohort
            directory.mkdir(exist_ok=False)
            binaries = {label: BIN / ('buffer-fixed-' + ('candidate' if same else label)
                                     + ('-diagnostic' if diagnostic else '')) for label in ['baseline', 'candidate']}
            program = programs['baseline']
            report = dict(schema=2, status='running', revision=app.rw.output(['git', 'rev-parse', 'HEAD']),
                          arguments=dict(reps=reps, workers=workers_list, cases=cases, diagnostic=diagnostic, seed=seed,
                                         baseline_revision='b7cf709', candidate_revision='7c2fb36', same_binary=same),
                          binaries={n: dict(path=str(p), sha256=sha(p)) for n, p in binaries.items()},
                          effective_programs={n: dict(path=str(program.relative_to(ROOT)), sha256=sha(program)) for n in binaries},
                          launcher_sha256=sha(launcher), sources=sources, runs=[])
            rng = random.Random(seed)
            for rep in range(-1, reps):
                cells = [(name, w, v) for name in cases for w in workers_list for v in binaries]
                for name, workers, variant in gc.measurement_order(cells, rng, paired=True):
                    gc_path, allocation_path = temporary / 'gc.json', temporary / 'allocations.json'
                    settings = env | dict(GC_BENCH_CONFIG=json.dumps(configs[name]))
                    if diagnostic:
                        settings.update(BADGER_GC_PROFILE=str(gc_path), BADGER_ALLOC_PROFILE=str(allocation_path))
                    command = [str(launcher), str(binaries[variant]), 'run', '--release', '--bench', '--workers', str(workers), str(program)]
                    before = gc.host_context()
                    out, err, process = gc.t.run_process(command, settings, 180)
                    after = gc.host_context()
                    rss = re.search(r'^\[gc-bench child rss\] (\d+)$', err, re.MULTILINE)
                    assert rss, err
                    process['launcher_tree_peak_rss_kib'] = process['peak_rss_kib']
                    process['peak_rss_kib'] = int(rss[1])
                    record = json.loads(out)
                    assert record['config'] == configs[name] and record['checksum'] == expected[name] and record['elapsed_ns'] > 0
                    profile = json.loads(gc_path.read_text()) if diagnostic else None
                    allocations = json.loads(allocation_path.read_text()) if diagnostic else None
                    if diagnostic:
                        gc.validate_sweep(profile)
                        gc.validate_allocations(allocations)
                        gc_path.unlink()
                        allocation_path.unlink()
                    if rep >= 0:
                        a, b = record['before'], record['after']
                        report['runs'].append(dict(case=name, workers=workers, variant=variant, repetition=rep,
                            command=command, record=record, process=process, stderr=err, gc=profile, allocations=allocations,
                            host_before=before, host_after=after, heap_collections=b['collections']-a['collections'],
                            gc_pause_ns=b['pause_total_ns']-a['pause_total_ns']))
                    print(cohort, rep, name, workers, variant, flush=True)
            report['status'] = 'complete'
            report['summary'] = gc.summarize(report['runs'])
            assert all(sha(ROOT / p) == value for p, value in sources.items())
            assert all(sha(p) == report['binaries'][n]['sha256'] for n, p in binaries.items())
            assert sha(program) == report['effective_programs']['baseline']['sha256']
            (directory / 'measurements.json.gz').write_bytes(gzip.compress((json.dumps(report, indent=2)+'\n').encode(), mtime=0))
