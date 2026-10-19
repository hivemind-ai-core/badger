"""Verify the corrected-main G8d evidence and the initial map/GC baseline."""
from collections import Counter, defaultdict
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
import applications as app
import map_phases

names = {
    'G8d-current-native': 672, 'G8d-current-control': 112, 'G8d-current-census': 144,
    'G8d-current-applications': 504, 'G8d-current-app-census': 108,
    'G8d-app-repeat': 240, 'G8d-app-control': 240,
    'P1-baseline': 180, 'P1-census': 90, 'P1-collector-baseline': 120, 'P1-parallel-baseline': 36,
}
datasets, intervals, oracles, reports = [], [], {}, {}
for name, count in names.items():
    path = BASE / name / 'measurements.json.gz'
    data = json.loads(gzip.decompress(path.read_bytes()))
    assert data.get('status', 'complete') == 'complete', name
    assert len(data['runs']) == count, (name, len(data['runs']))
    cells = Counter((r.get('suite'), r['case'], r['workers'], r['variant']) for r in data['runs'])
    assert all(n == data['arguments']['reps'] for n in cells.values()), name
    for r in data['runs']:
        intervals.append((r['host_before']['wall_time_ns'], r['host_after']['wall_time_ns'], name))
        if 'record' in r:
            config = r['record']['config']
            key = tuple(sorted(config.items()))
            if key not in oracles:
                oracles[key] = (map_phases.oracle(config) if 'size' in config else app.gc.expected_coverage(config))
            if 'size' in config:
                assert (r['record']['checksum'], r['record']['versions']) == oracles[key], name
            else:
                assert r['record']['checksum'] == oracles[key], name
        else:
            key = (r['suite'], r['case'], r['n'])
            if key not in oracles:
                oracles[key] = (app.rw if r['suite'] == 'realworld' else app.compare).oracle(r['case'], r['n'], 37)
            app.validate(r['process'], r['suite'], r['case'], r['n'], 37,
                         data['arguments']['warmups'], data['arguments']['samples'], oracles[key])
        if r.get('allocations'):
            app.gc.validate_allocations(r['allocations'])
        if r.get('gc'):
            app.gc.validate_sweep(r['gc'])
    for binary in data['binaries'].values():
        assert app.rw.digest(Path(binary['path'])) == binary['sha256'], (name, binary['path'])
    for source, expected in data['sources'].items():
        if app.rw.digest(ROOT / source) != expected:
            # Main can advance after a fixed-revision measurement. Check the
            # recorded source object; never silently substitute current main.
            pinned = subprocess.check_output(['git', 'show', f"{data['revision']}:{source}"], cwd=ROOT)
            assert hashlib.sha256(pinned).hexdigest() == expected, (name, 'source drift', source)
    for program in data.get('effective_programs', {}).values():
        assert app.rw.digest(ROOT / program['path']) == program['sha256'], (name, 'MIR drift')
    datasets.append(dict(path=str(path.relative_to(ROOT)), sha256=app.rw.digest(path), processes=count,
                         arguments=data['arguments'], binaries=data['binaries']))
    reports[name] = data
intervals.sort()
assert all(a[1] <= b[0] for a, b in zip(intervals, intervals[1:])), 'overlapping accepted measurements'

lines = ['# G8d corrected-main paired native evidence', '',
         'Identical saved MIR in fixed native runtimes. Negative changes are faster; all samples and ranges are retained. Controls are independent cohorts, not corrections or confidence intervals.', '']
for name in ['G8d-current-native', 'G8d-current-control']:
    data = reports[name]
    lines += [f'## {name}', '', '| Case | Workers | Paired median | Faster pairs | Min / max |',
              '|---|---:|---:|---:|---:|']
    for case, workers in sorted({(r['case'], r['workers']) for r in data['runs']}):
        rows = {(r['repetition'], r['variant']): r for r in data['runs'] if (r['case'], r['workers']) == (case, workers)}
        changes = [100 * (rows[i, 'candidate']['record']['elapsed_ns'] / rows[i, 'baseline']['record']['elapsed_ns'] - 1)
                   for i in range(data['arguments']['reps'])]
        lines.append(f'| {case} | {workers} | {statistics.median(changes):+.2f}% | {sum(v < 0 for v in changes)}/{len(changes)} | {min(changes):+.1f}% / {max(changes):+.1f}% |')
    lines += ['']
(BASE / 'G8d-paired-summary.md').write_text('\n'.join(lines) + '\n')

traffic = []
data = reports['G8d-current-census']
for case, workers in sorted({(r['case'], r['workers']) for r in data['runs']}):
    variants = {}
    for label in ['baseline', 'candidate']:
        rows = [r for r in data['runs'] if (r['case'], r['workers'], r['variant']) == (case, workers, label)]
        allocated = []
        for r in rows:
            by_type = defaultdict(Counter)
            for site in r['allocations']['sites']:
                by_type[site['type']].update(site['counts']['allocated'])
            allocated.append(by_type)
        variants[label] = dict(objects=sorted({r['allocations']['totals']['allocated']['objects'] for r in rows}),
            buffer_counts={k: sorted({a['Buffer'][k] for a in allocated}) for k in ['objects', 'cell_bytes', 'requested_bytes']},
            storage={k: sorted({(r['allocations']['buffer_storage']['events'][k]['calls'],
                                r['allocations']['buffer_storage']['events'][k]['bytes']) for r in rows})
                     for k in ['inline', 'inline_to_native', 'native_alloc', 'native_adopt', 'native_realloc_growth']})
    assert variants['baseline'] == variants['candidate'], (case, workers, variants)
    traffic.append(dict(case=case, workers=workers, variants=variants))

semantic = []
for name in ['fixed-semantic', 'fixed-semantic-diagnostic']:
    path = BASE / f'G8d-validation/{name}.json.gz'
    d = json.loads(gzip.decompress(path.read_bytes()))
    assert len(d['runs']) == 136 and len(set(d['programs'].values())) == 1
    for source, expected in d['sources'].items():
        assert app.rw.digest(ROOT / source) == expected, source
    semantic.append(dict(path=str(path.relative_to(ROOT)), sha256=app.rw.digest(path), checks=136))
manifest = dict(measured_baseline_revision='b7cf709', measured_candidate_revision='7c2fb36',
                accepted_measurements=sum(names.values()), buffer_measurements=sum(v for k, v in names.items() if k.startswith('G8d')),
                accepted_intervals_overlap=False, datasets=datasets, semantic=semantic, managed_and_storage_traffic=traffic,
                exclusions=['G8d-calibration-overlap: own compilation overlapped that initial calibration',
                            'G8d-baseline and G8d-trial: preliminary older main, before the list header correction',
                            'P1 smoke/fixture checks: correctness only; they overlap build/test work'],
                validation_artifacts={str(p.relative_to(ROOT)): app.rw.digest(p)
                                      for p in (BASE / 'G8d-validation').iterdir()
                                      if p.is_file() and p.name != 'audit.log'},
                audit_sha256=app.rw.digest(Path(__file__)))
(BASE / 'G8d-audit.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(f"Validated {sum(names.values()):,} disjoint measured processes, 272 focused semantic checks, unchanged buffer allocations and native storage allocation/growth counts")
