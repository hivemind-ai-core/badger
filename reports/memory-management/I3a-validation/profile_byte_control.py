"""Sample isolated byte churn in immutable ordinary binaries; profiling perturbs time/RSS."""
from collections import Counter
from pathlib import Path
import gzip
import json
import random
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'bench/gc'))
import applications as app

gc = app.gc
BASE = ROOT / 'reports/memory-management'
ARTIFACTS = BASE / 'I3a-validation'
BIN = Path('/home/dan/orca/projects/language/target/memory-binaries-2026-10-18')
binaries = {'baseline': BIN / 'map-helpers-candidate', 'candidate': BIN / 'stored-sequence-candidate'}
config = gc.COVERAGE['bytes-transient'] | dict(n=1048576)
with gc.benchmark_lock():
    directory = BASE / 'I3a-byte-profile'
    directory.mkdir(exist_ok=False)
    report = dict(config=config, seed=2026101955, settings=dict(BADGER_PROFILE_SCOPE='runtime'),
                  binaries={n: dict(path=str(p), sha256=app.rw.digest(p)) for n, p in binaries.items()}, runs=[])
    env = app.rw.clean_env()
    rng = random.Random(report['seed'])
    for mode in ['source', 'mir']:
        for rep in range(3):
            variants = list(binaries)
            rng.shuffle(variants)
            for variant in variants:
                folded = (directory / f'{mode}-{rep}-{variant}.folded').resolve()
                program = ARTIFACTS / ('isolated-bytes-transient.bdg' if mode == 'source' else 'isolated-bytes-transient-candidate.bdgmir')
                command = [str(binaries[variant]), 'run', '--release', '--workers', '1', str(program)]
                before = gc.host_context()
                result = subprocess.run(command, cwd=ROOT, env=env | report['settings'] | dict(GC_BENCH_CONFIG=json.dumps(config), BADGER_PROFILE=str(folded)), text=True, capture_output=True, timeout=180)
                after = gc.host_context()
                assert result.returncode == 0, result.stderr
                record = json.loads(result.stdout)
                gc.validate_coverage(record, config)
                own = Counter()
                for line in folded.read_text().splitlines():
                    stack, count = line.rsplit(' ', 1)
                    own[stack.split(';')[-1]] += int(count)
                assert sum(own.values()) > 500
                report['runs'].append(dict(mode=mode, repetition=rep, variant=variant, command=command, record=record,
                    stderr=result.stderr, self_samples=dict(own), folded=str(folded.relative_to(ROOT)), folded_sha256=app.rw.digest(folded),
                    host_before=before, host_after=after, source_sha256=app.rw.digest(program)))
                print(mode, rep, variant, sum(own.values()), own.most_common(5), flush=True)
    (directory / 'profiles.json.gz').write_bytes(gzip.compress((json.dumps(report, indent=2)+'\n').encode(), mtime=0))
