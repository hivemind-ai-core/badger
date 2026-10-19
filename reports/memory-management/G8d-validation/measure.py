"""Run fixed-revision G8d acceptance and the next map baseline sequentially.

Run only after own builds/tests finish. No production edits occur in this script.
"""
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'bench/gc'))
import applications as app

BIN = Path('/home/dan/orca/projects/language/target/memory-binaries-2026-10-18')
OUT = ROOT / 'reports/memory-management'


def run(name, arguments):
    print(name, flush=True)
    with (OUT / 'G8d-validation' / f'{name}.log').open('x') as log:
        subprocess.run([sys.executable, *map(str, arguments)], cwd=ROOT, env=app.rw.clean_env(),
                       stdout=log, stderr=subprocess.STDOUT, check=True)


run('current-native', ['reports/memory-management/G8d-validation/native.py'])
run('current-applications', ['bench/gc/applications.py', '--baseline', BIN / 'buffer-fixed-baseline',
    '--candidate', BIN / 'buffer-fixed-candidate', '--baseline-revision', 'b7cf709', '--candidate-revision', '7c2fb36',
    '--workers', 1, 2, 4, 8, '--consumers', 8, '--reps', 7, '--warmups', 15, '--samples', 15,
    '--seed', 2026101964, '--out', OUT / 'G8d-current-applications'])
run('current-app-census', ['bench/gc/applications.py', '--baseline', BIN / 'buffer-fixed-baseline-diagnostic',
    '--candidate', BIN / 'buffer-fixed-candidate-diagnostic', '--baseline-revision', 'b7cf709', '--candidate-revision', '7c2fb36',
    '--workers', 1, 8, '--consumers', 8, '--reps', 3, '--warmups', 3, '--samples', 5, '--diagnostic', '--allocations',
    '--seed', 2026101965, '--out', OUT / 'G8d-current-app-census'])
run('map-baseline', ['bench/gc/map_phases.py', '--baseline', BIN / 'buffer-fixed-candidate',
    '--baseline-revision', '7c2fb36', '--workers', 1, 2, 4, 8, '--reps', 3, '--seed', 2026101966,
    '--out', OUT / 'P1-baseline'])
run('map-census', ['bench/gc/map_phases.py', '--baseline', BIN / 'buffer-fixed-candidate-diagnostic',
    '--baseline-revision', '7c2fb36', '--workers', 1, 8, '--reps', 3, '--diagnostic', '--allocations',
    '--seed', 2026101967, '--out', OUT / 'P1-census'])
run('collector-baseline', ['bench/gc/run.py', '--suite', 'coverage', '--baseline', BIN / 'buffer-fixed-candidate',
    '--diagnostic', BIN / 'buffer-fixed-candidate-diagnostic', '--allocations', '--baseline-revision', '7c2fb36',
    '--cases', 'persistent-4096-major', 'snapshots-major', 'records-window', 'bytes-survivors', 'pool-burst-256',
    '--workers', 1, 8, '--reps', 3, '--paired', '--seed', 2026101968, '--out', OUT / 'P1-collector-baseline'])
run('parallel-map-baseline', ['bench/gc/run.py', '--suite', 'coverage', '--baseline', BIN / 'buffer-fixed-candidate',
    '--baseline-revision', '7c2fb36', '--cases', 'parallel-maps', 'parallel-scalar-snapshots', 'parallel-snapshots',
    '--workers', 1, 2, 4, 8, '--reps', 3, '--paired', '--seed', 2026101969, '--out', OUT / 'P1-parallel-baseline'])
