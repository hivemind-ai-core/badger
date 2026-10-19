"""Compare old/new sequence-lowering MIR in one fixed native runtime."""
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
binaries = {name:BIN / value for name,value in dict(baseline='map-helpers-candidate', candidate='stored-sequence-candidate', diagnostic='stored-sequence-diagnostic').items()}
sha = app.rw.digest
CASES = ['visible', 'stored', 'stored-reused', 'stored-list', 'stored-pointers', 'stored-escape', 'stored-collected', 'collected', 'eager']
CONTROL = ['visible', 'stored', 'stored-pointers', 'bytes-transient']

with gc.benchmark_lock():
    env = app.rw.clean_env()
    programs = {}
    for label in ['baseline', 'candidate']:
        path = ARTIFACTS / f'coverage-{label}.bdgmir'
        assert not path.exists(), path
        subprocess.run([str(binaries[label]), 'compile', 'bench/gc/coverage.bdg', '-o', str(path)], cwd=ROOT, env=env, capture_output=True, check=True, timeout=180)
        programs[label] = path
    configs = {name:gc.COVERAGE[name] for name in set(CASES + CONTROL)}
    expected = {name:gc.expected_coverage(config) for name,config in configs.items()}
    sources = {str(p.relative_to(ROOT)):sha(p) for p in [ROOT / 'bench/gc/coverage.bdg', ROOT / 'bench/gc/run.py', ROOT / 'bench/gc/rss_spawn.c', Path(__file__)]}
    with tempfile.TemporaryDirectory(prefix='badger-map-helper-native-') as temporary:
        temporary = Path(temporary)
        launcher = temporary / 'rss-measure'
        subprocess.run(['cc', '-O2', '-std=c11', '-Wall', '-Wextra', '-Werror', str(ROOT / 'bench/gc/rss_spawn.c'), '-o', str(launcher)], check=True)
        for cohort, cases, reps, diagnostic, same, seed in [
            ('I3a-native-mir', CASES, 5, False, False, 2026101946),
            ('I3a-native-control', CONTROL, 5, False, True, 2026101947),
            ('I3a-native-census', CASES, 3, True, False, 2026101948),
        ]:
            directory = BASE / cohort
            directory.mkdir(exist_ok=False)
            binary = binaries['diagnostic' if diagnostic else 'candidate']
            targets = {'baseline':programs['candidate' if same else 'baseline'], 'candidate':programs['candidate']}
            report = dict(schema=2, status='running', revision=app.rw.output(['git','rev-parse','HEAD']),
                          arguments=dict(reps=reps, workers=[1,8], cases=cases, diagnostic=diagnostic, seed=seed, same_mir=same),
                          binaries={n:dict(path=str(binary),sha256=sha(binary)) for n in targets},
                          effective_programs={n:dict(path=str(p.relative_to(ROOT)),sha256=sha(p)) for n,p in targets.items()},
                          compiler_binaries={n:dict(path=str(p),sha256=sha(p)) for n,p in binaries.items()},
                          launcher_sha256=sha(launcher), sources=sources, runs=[])
            rng = random.Random(seed)
            for rep in range(-1,reps):
                cells = [(name,w,v) for name in cases for w in [1,8] for v in targets]
                for name,workers,variant in gc.measurement_order(cells,rng,paired=True):
                    gc_path, allocation_path = temporary / 'gc.json', temporary / 'allocations.json'
                    settings = env | dict(GC_BENCH_CONFIG=json.dumps(configs[name]))
                    if diagnostic:
                        settings.update(BADGER_GC_PROFILE=str(gc_path), BADGER_ALLOC_PROFILE=str(allocation_path))
                    command = [str(launcher),str(binary),'run','--release','--bench','--workers',str(workers),str(targets[variant])]
                    before = gc.host_context()
                    out,err,process = gc.t.run_process(command,settings,180)
                    after = gc.host_context()
                    rss = re.search(r'^\[gc-bench child rss\] (\d+)$',err,re.MULTILINE)
                    assert rss,err
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
                        a,b = record['before'],record['after']
                        report['runs'].append(dict(case=name,workers=workers,variant=variant,repetition=rep,
                            record=record,process=process,stderr=err,gc=profile,allocations=allocations,
                            host_before=before,host_after=after,heap_collections=b['collections']-a['collections'],
                            gc_pause_ns=b['pause_total_ns']-a['pause_total_ns']))
                    print(cohort,rep,name,workers,variant,flush=True)
            report['status'] = 'complete'
            report['summary'] = gc.summarize(report['runs'])
            assert all(sha(ROOT / p) == value for p,value in sources.items())
            assert all(sha(p) == report['compiler_binaries'][n]['sha256'] for n,p in binaries.items())
            assert all(sha(p) == report['effective_programs'][n]['sha256'] for n,p in targets.items())
            (directory / 'measurements.json.gz').write_bytes(gzip.compress((json.dumps(report,indent=2)+'\n').encode(),mtime=0))
