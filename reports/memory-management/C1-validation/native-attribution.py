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
ARTIFACTS = BASE / 'C1-validation'
ENV = app.rw.clean_env()

def sha(p):
    return hashlib.sha256(Path(p).read_bytes()).hexdigest()

with app.gc.benchmark_lock():
    program = BASE / 'I5b-validation/messaging-old.bdgmir'
    launcher = ARTIFACTS / 'mir-measure'
    subprocess.run(['cc', '-O2', '-Wall', '-Wextra', '-Werror',
                    str(app.rw.SUITE / 'measure.c'), '-o', str(launcher)], check=True)
    sources = app.rw.source_hashes()
    sources[str(Path(__file__).relative_to(ROOT))] = sha(__file__)
    n = app.rw.SIZES['messaging']
    checksum = app.rw.oracle('messaging', n, 37)
    for cohort, baseline, seed in [('I5b-native-mir', OLD, 2026101901),
                                    ('I5b-native-mir-control', RUNTIME, 2026101902)]:
        directory = BASE / cohort
        directory.mkdir(exist_ok=False)
        targets = {'baseline': baseline, 'candidate': RUNTIME}
        binaries = {k: dict(path=str(p), sha256=sha(p)) for k,p in targets.items()}
        report = dict(schema=1, status='running', revision=app.rw.output(['git', 'rev-parse', 'HEAD']),
                      diagnostic=False, arguments=dict(reps=15, workers=[2], warmups=5, samples=15,
                      baseline_revision='38855c7' if baseline == OLD else '49f281c', candidate_revision='49f281c',
                      compiler_revision='38855c7', seed=seed),
                      binaries=binaries, sources=sources,
                      effective_program=dict(path=str(program.relative_to(ROOT)), sha256=sha(program)),
                      compiler_binaries={str(p):sha(p) for p in [OLD, RUNTIME]},
                      launcher_sha256=sha(launcher), runs=[])
        rng = random.Random(seed)
        for repetition in range(15):
            order = list(targets)
            rng.shuffle(order)
            for variant in order:
                command = [str(targets[variant]), 'run', '--release', '--bench', '--workers', '2', str(program),
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
        assert sha(program)==report['effective_program']['sha256']
        assert all(sha(p)==binaries[k]['sha256'] for k,p in targets.items())
        report['summary'] = app.summarize(report['runs'])
        report['status'] = 'complete'
        (directory / 'measurements.json.gz').write_bytes(gzip.compress((json.dumps(report,indent=2)+'\n').encode(),mtime=0))
    launcher.unlink()
