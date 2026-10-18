"""Recheck rejected HAMT allocation experiment, retained code and raw evidence."""
from collections import Counter
import gzip
import hashlib
import json
from pathlib import Path
import statistics
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
BASE = ROOT / 'reports/memory-management'
sys.path.insert(0, str(ROOT / 'bench/gc'))
from run import validate_allocations, validate_sweep


def sha(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def source(rev, path):
    return subprocess.check_output(['git', 'show', f'{rev}:{path}'], cwd=ROOT)


names = ['G8c-baseline', 'G8c-applications', 'G8c-coverage', 'G8c-census', 'G8c-repeat', 'G8c-control']
intervals, datasets, reports = [], [], {}
for name in names:
    path = BASE / name / 'measurements.json.gz'
    data = json.loads(gzip.decompress(path.read_bytes()))
    assert data.get('status', 'complete') == 'complete', name
    groups = Counter((r.get('suite'), r['case'], r['workers'], r['variant']) for r in data['runs'])
    assert all(n == data['arguments']['reps'] for n in groups.values()), name
    for row in data['runs']:
        intervals.append((row['host_before']['wall_time_ns'], row['host_after']['wall_time_ns'], name))
        if row.get('allocations'):
            validate_allocations(row['allocations'])
        if row.get('gc'):
            validate_sweep(row['gc'])
    datasets.append(dict(path=str(path.relative_to(ROOT)), sha256=sha(path),
                         runs=len(data['runs']), arguments=data['arguments'], binaries=data['binaries']))
    reports[name] = data
intervals.sort()
assert len(intervals) == 1460
assert all(a[1] <= b[0] for a, b in zip(intervals, intervals[1:])), 'overlapping measured intervals'
traffic = []
for case in ['records', 'snapshots', 'state', 'reconcile']:
    rows = [r for r in reports['G8c-census']['runs'] if r['case'] == case]
    totals = [r['allocations']['totals']['allocated'] for r in rows]
    for metric in ['objects', 'cell_bytes']:
        assert len({t[metric] for t in totals}) == 1, (case, metric)
    traffic.append(dict(case=case, runs=len(rows), objects=totals[0]['objects'], cell_bytes=totals[0]['cell_bytes'],
                        requested_bytes_range=[min(t['requested_bytes'] for t in totals), max(t['requested_bytes'] for t in totals)]))
lines = ['# G8c paired coverage changes', '', 'Median of seven adjacent candidate/baseline process ratios; parentheses count faster pairs. Negative is faster. All samples remain.', '',
         '| Case | w1 | w2 | w4 | w8 |', '|---|---:|---:|---:|---:|']
rows = reports['G8c-coverage']['runs']
for case in sorted({r['case'] for r in rows}):
    cells = []
    for workers in [1, 2, 4, 8]:
        elapsed = {(r['repetition'], r['variant']):r['record']['elapsed_ns'] for r in rows if r['case'] == case and r['workers'] == workers}
        changes = [(elapsed[(i, 'candidate')] / elapsed[(i, 'baseline')] - 1) * 100 for i in range(7)]
        cells.append(f'{statistics.median(changes):+.1f}% ({sum(c < 0 for c in changes)}/7)')
    lines.append('| ' + case + ' | ' + ' | '.join(cells) + ' |')
(BASE / 'G8c-coverage/paired-summary.md').write_text('\n'.join(lines) + '\n')
production = ['crates/badger_runtime/src/gc.rs', 'crates/badger_runtime/src/gc/sweep.rs', 'crates/badger_runtime/src/hamt.rs']
for path in production:
    # The only retained HAMT changes are inside its test module.
    current, baseline = (ROOT / path).read_bytes(), source('27e9653', path)
    if path.endswith('hamt.rs'):
        current, baseline = current.split(b'#[cfg(test)]')[0], baseline.split(b'#[cfg(test)]')[0]
    assert current == baseline, ('production drift', path)
bindir = Path('/home/dan/orca/projects/language/target/memory-binaries-2026-10-18')
report = dict(baseline_revision='27e9653', prototype_build_revision='5b91953', validation_revision='9f96361',
              production_revert='a9d51d7', measured_processes=len(intervals), measured_intervals_overlap=False,
              datasets=datasets, managed_traffic=traffic,
              source_sha256={rev:{path:hashlib.sha256(source(rev, path)).hexdigest() for path in production}
                             for rev in ['27e9653', '5b91953', '9f96361', 'a9d51d7']},
              binaries={name:dict(path=str(bindir / name), sha256=sha(bindir / name)) for name in
                        ['allocation-paths-ordinary', 'allocation-paths-diagnostic', 'hamt-init-candidate', 'hamt-init-diagnostic']},
              validation_artifacts={str(p.relative_to(ROOT)):sha(p) for p in sorted((BASE / 'G8c-validation').glob('*')) if p.is_file()},
              retained_production_matches_main_27e9653=True)
(BASE / 'G8c-binaries.json').write_text(json.dumps(report, indent=2) + '\n')
print('Validated 1,460 disjoint measured processes; census traffic unchanged; production allocator/HAMT restored')
