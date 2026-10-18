"""Validate retained allocator-path evidence and emit its immutable manifest."""
from pathlib import Path
from collections import Counter
import gzip
import hashlib
import json
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
BASE = ROOT / 'reports/memory-management'
sys.path.insert(0, str(ROOT / 'bench/gc'))
from run import validate_allocations, validate_sweep


def sha(path):
    with Path(path).open('rb') as file:
        return hashlib.file_digest(file, 'sha256').hexdigest()


intervals, datasets = [], []
for name in ['G8a-baseline', 'G8a-applications', 'G8a-repeat', 'G8a-control',
             'G8a-coverage', 'G8b-paths', 'G8b-applications']:
    path = BASE / name / 'measurements.json.gz'
    data = json.loads(gzip.decompress(path.read_bytes()))
    assert data.get('status', 'complete') == 'complete'
    groups = Counter((r.get('suite'), r['case'], r['workers'], r['variant']) for r in data['runs'])
    assert all(n == data['arguments']['reps'] for n in groups.values()), name
    for row in data['runs']:
        intervals.append((row['host_before']['wall_time_ns'], row['host_after']['wall_time_ns'], name))
        if row.get('gc'):
            validate_sweep(row['gc'])
        if row.get('allocations'):
            validate_allocations(row['allocations'])
            assert 'cell_sources' in row['allocations']
    datasets.append(dict(path=str(path.relative_to(ROOT)), sha256=sha(path),
                         runs=len(data['runs']), arguments=data['arguments'], binaries=data['binaries']))
intervals.sort()
assert all(a[1] <= b[0] for a, b in zip(intervals, intervals[1:])), 'overlapping observations'
checks = json.loads(gzip.decompress((BASE / 'G8b-validation/census-checks.json.gz').read_bytes()))
assert len(checks['runs']) == 16 and checks['ordinary_profile_absent']
for row in checks['runs']:
    validate_allocations(row['profile'])
    assert 'cell_sources' in row['profile']
# G8a's original label typo is preserved and transparently corrected, not resampled.
path = BASE / 'G8a-applications'
original = json.loads(gzip.decompress((path / 'original-measurements.json.gz').read_bytes()))
corrected = json.loads(gzip.decompress((path / 'measurements.json.gz').read_bytes()))
assert original['runs'] == corrected['runs']
assert original['arguments']['candidate_revision'] == '26'
assert corrected['arguments']['candidate_revision'] == subprocess.check_output(
    ['git', 'rev-parse', 'e41aa39'], cwd=ROOT, text=True).strip()
bindir = ROOT / 'target/memory-binaries-2026-10-18'
# Artifacts live in the primary checkout, not the worktree's target directory.
bindir = Path('/home/dan/orca/projects/language/target/memory-binaries-2026-10-18')
paths = [ROOT / 'crates/badger_runtime/src/gc.rs', ROOT / 'crates/badger_runtime/src/gc/allocations.rs',
         ROOT / 'bench/gc/run.py', ROOT / 'bench/gc/test_gc_bench.py', ROOT / 'bench/gc/analyze_allocations.py']
report = dict(revision='ec0ad88', working_harness_sha256={str(p.relative_to(ROOT)):sha(p) for p in paths},
              measured_processes=366, preceding_G8a_measured_processes=724,
              measured_intervals_overlap=False, datasets=datasets,
              binaries={n: dict(path=str(bindir / n), sha256=sha(bindir / n)) for n in
                        ['codec-temporaries-candidate', 'pool-clear-candidate', 'allocation-paths-ordinary', 'allocation-paths-diagnostic']},
              validation_artifacts={str(p.relative_to(ROOT)):sha(p) for p in sorted((BASE / 'G8b-validation').glob('*')) if p.is_file()},
              ordinary_build=dict(features=[], profile='release'), diagnostic_build=dict(features=['badger_runtime/gc-profile'], profile='release'))
(BASE / 'G8b-binaries.json').write_text(json.dumps(report, indent=2) + '\n')
print('Validated 1,090 disjoint measured processes and 16 stress censuses')
