"""Separate saved-MIR/JIT changes from native binary/compiler-startup changes."""
from pathlib import Path
import gzip, hashlib, json, random, statistics, subprocess, sys
ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'bench/gc'))
import applications as app

BIN = Path('/home/dan/orca/projects/language/target/memory-binaries-2026-10-18')
RUNTIME = BIN / 'snapshot-epochs-specialized-candidate'
OLD = BIN / 'prefetch32-candidate'
BASE = ROOT / 'reports/memory-management'
ARTIFACTS = BASE / 'I5b-validation'
ENV = app.rw.clean_env()

def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()

with app.gc.benchmark_lock():
    programs = {}
    for label, compiler in [('old', OLD), ('new', RUNTIME)]:
        program = ARTIFACTS / f'messaging-{label}.bdgmir'
        subprocess.run([str(compiler), 'compile', '--workers', '2',
                        str(app.rw.PROGRAMS / 'workload.bdg'), '-o', str(program)],
                       cwd=ROOT, env=ENV, capture_output=True, check=True, timeout=180)
        programs[label] = program
    launcher = ARTIFACTS / 'mir-measure'
    subprocess.run(['cc', '-O2', '-Wall', '-Wextra', '-Werror',
                    str(app.rw.SUITE / 'measure.c'), '-o', str(launcher)], check=True)
    binaries = {k: dict(path=str(RUNTIME), sha256=sha(RUNTIME)) for k in ['baseline', 'candidate']}
    sources = app.rw.source_hashes()
    sources[str(Path(__file__).relative_to(ROOT))] = sha(__file__)
    n = app.rw.SIZES['messaging']
    checksum = app.rw.oracle('messaging', n, 37)
    for cohort, baseline, seed in [('I5b-messaging-mir', programs['old'], 2026101844),
                                    ('I5b-messaging-mir-control', programs['new'], 2026101845)]:
        directory = BASE / cohort
        directory.mkdir(exist_ok=True)
        targets = {'baseline': baseline, 'candidate': programs['new']}
        report = dict(schema=1, status='running', revision=app.rw.output(['git', 'rev-parse', 'HEAD']),
                      diagnostic=False, arguments=dict(reps=15, workers=[2], warmups=5, samples=15,
                      baseline_revision='49f281c', candidate_revision='49f281c',
                      baseline_compiler='38855c7' if baseline == programs['old'] else '49f281c',
                      candidate_compiler='49f281c', seed=seed, runtime=str(RUNTIME)),
                      binaries=binaries, sources=sources,
                      effective_programs={k:dict(path=str(p.relative_to(ROOT)), sha256=sha(p)) for k,p in targets.items()},
                      compiler_binaries={str(p):sha(p) for p in [OLD, RUNTIME]},
                      launcher_sha256=sha(launcher), runs=[])
        rng = random.Random(seed)
        for repetition in range(15):
            order = list(targets)
            rng.shuffle(order)
            for variant in order:
                command = [str(RUNTIME), 'run', '--release', '--bench', '--workers', '2', str(targets[variant]),
                           '--', 'messaging', str(n), '37', '2', '5', '15', 'unused.jsonl']
                before = app.gc.host_context()
                process = app.rw.process(command, ENV, 180, expect_ready=True, launcher=launcher)
                after = app.gc.host_context()
                app.validate(process, 'realworld', 'messaging', n, 37, 5, 15, checksum)
                report['runs'].append(dict(suite='realworld', case='messaging', workers=2, variant=variant,
                     repetition=repetition, n=n, process=process, gc=None, allocations=None,
                     host_before=before, host_after=after))
                print(cohort, repetition, variant, flush=True)
        assert sources == {p:app.rw.digest(ROOT / p) for p in sources}
        assert all(sha(p)==report['effective_programs'][k]['sha256'] for k,p in targets.items())
        assert sha(RUNTIME)==binaries['baseline']['sha256']
        report['summary'] = app.summarize(report['runs'])
        report['status'] = 'complete'
        (directory / 'measurements.json.gz').write_bytes(gzip.compress((json.dumps(report,indent=2)+'\n').encode(),mtime=0))
    launcher.unlink()
