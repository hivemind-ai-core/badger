"""Audit fixed-revision map-helper timing, allocation and semantic evidence."""
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

gc = app.gc
sha = app.rw.digest
NAMES = ['I2c-baseline','I2c-coverage','I2c-applications','I2c-census',
         'I2c-native-mir','I2c-native-control','I2c-native-census',
         'I2c-app-repeat','I2c-app-control','I2c-record-repeat','I2c-record-control']
reports, datasets, intervals, oracles = {}, [], [], {}
for name in NAMES:
    path = BASE / name / 'measurements.json.gz'
    data = json.loads(gzip.decompress(path.read_bytes()))
    assert data.get('status','complete') == 'complete', name
    groups = Counter((r.get('suite'),r['case'],r['workers'],r['variant']) for r in data['runs'])
    assert all(n == data['arguments']['reps'] for n in groups.values()), name
    for row in data['runs']:
        intervals.append((row['host_before']['wall_time_ns'],row['host_after']['wall_time_ns'],name))
        if 'record' in row:
            config = row['record']['config']
            key = tuple(sorted(config.items()))
            if key not in oracles:
                oracles[key] = gc.expected_coverage(config)
            assert row['record']['checksum'] == oracles[key], (name,row['case'])
            assert row['record']['elapsed_ns'] > 0
        else:
            key = (row['suite'],row['case'],row['n'])
            if key not in oracles:
                oracles[key] = (app.rw if row['suite']=='realworld' else app.compare).oracle(row['case'],row['n'],37)
            app.validate(row['process'],row['suite'],row['case'],row['n'],37,
                         data['arguments']['warmups'],data['arguments']['samples'],oracles[key])
        if row.get('allocations'):
            gc.validate_allocations(row['allocations'])
        if row.get('gc'):
            gc.validate_sweep(row['gc'])
    for binary in data['binaries'].values():
        assert sha(Path(binary['path'])) == binary['sha256'], (name,binary['path'])
    for source, expected in data['sources'].items():
        assert sha(ROOT / source) == expected, ('harness drift',name,source)
    datasets.append(dict(path=str(path.relative_to(ROOT)),sha256=sha(path),runs=len(data['runs']),
                         arguments=data['arguments'],binaries=data['binaries'],effective_programs=data.get('effective_programs')))
    reports[name] = data
intervals.sort()
assert len(intervals) == 2212
assert all(a[1] <= b[0] for a,b in zip(intervals,intervals[1:])), 'overlapping measured processes'

lines = ['# I2c paired coverage and attribution', '',
         'Median of adjacent candidate/baseline process ratios. Parentheses count faster pairs; ranges and every sample remain in the raw reports. These are not confidence bounds.', '']
for name in ['I2c-coverage','I2c-native-mir','I2c-native-control','I2c-record-repeat','I2c-record-control']:
    data = reports[name]
    lines += [f'## {name}', '', '| Case | Workers | Paired median | Faster pairs | Min / max |', '|---|---:|---:|---:|---:|']
    for case,workers in sorted({(r['case'],r['workers']) for r in data['runs']}):
        selected = {(r['repetition'],r['variant']):r for r in data['runs'] if r['case']==case and r['workers']==workers}
        changes = [(selected[(i,'candidate')]['record']['elapsed_ns']/selected[(i,'baseline')]['record']['elapsed_ns']-1)*100 for i in range(data['arguments']['reps'])]
        lines.append(f'| {case} | {workers} | {statistics.median(changes):+.1f}% | {sum(d<0 for d in changes)}/{len(changes)} | {min(changes):+.1f}% / {max(changes):+.1f}% |')
    lines += ['']
(BASE / 'I2c-paired-summary.md').write_text('\n'.join(lines).rstrip()+'\n')

traffic = []
for case in sorted({r['case'] for r in reports['I2c-native-census']['runs']}):
    variants = {}
    for variant in ['baseline','candidate']:
        profiles = [r['allocations'] for r in reports['I2c-native-census']['runs'] if r['case']==case and r['variant']==variant]
        totals = {k:sorted({p['totals']['allocated'][k] for p in profiles}) for k in ['objects','cell_bytes','requested_bytes']}
        tuples = {sum(s['counts']['allocated']['objects'] for s in p['sites'] if s['type']=='Tuple') for p in profiles}
        assert len(tuples)==1
        variants[variant] = dict(totals=totals,tuple_objects=tuples.pop())
    config = gc.COVERAGE[case]
    n,r = config['n'],config['rounds']
    if case in ['map-api-parallel','map-api-composed']:
        if case=='map-api-parallel':
            r = 8*((r+7)//8)
        expected_tuples = (4*n-(n+2)//3)*r
    else:
        expected_tuples = n*r*(2 if case in ['map-api-projections','map-api-merge'] else 1)
    assert variants['baseline']['tuple_objects'] == expected_tuples, (case,variants,expected_tuples)
    assert variants['candidate']['tuple_objects'] == (expected_tuples if case=='map-api-entry-control' else 0)
    if case=='map-api-entry-control':
        assert variants['baseline']['totals'] == variants['candidate']['totals']
    traffic.append(dict(case=case,variants=variants))

checks = json.loads(gzip.decompress((BASE / 'I2c-validation/map-helpers.json.gz').read_bytes()))
assert len(checks['probes'])==12 and len(checks['runs'])==136
for path,expected in checks['sources'].items():
    assert sha(ROOT / path)==expected, ('fixture/checker drift',path)
for row in checks['probes']:
    expected = (2 if row['case']=='merge_with' else 1) if row['label']=='baseline' else int(row['case']=='stored_entries')
    assert row['entries'] == expected
prod = 'crates/badger_hir/std/map.bdg'
old = subprocess.check_output(['git','show',f'9b23d40:{prod}'],cwd=ROOT)
new = subprocess.check_output(['git','show',f'2bed9af:{prod}'],cwd=ROOT)
assert (ROOT / prod).read_bytes()==new
changed = subprocess.check_output(['git','diff','--name-only','9b23d40','2bed9af','--','crates','Cargo.lock','Cargo.toml'],cwd=ROOT,text=True).splitlines()
assert changed==[prod], changed
bindir = Path('/home/dan/orca/projects/language/target/memory-binaries-2026-10-18')
report = dict(baseline_revision='9b23d40',candidate_revision='2bed9af',coverage_revision='94768bf',
              measured_processes=len(intervals),measured_intervals_overlap=False,datasets=datasets,
              managed_traffic_same_native=traffic,
              only_production_change=prod,standard_map_source_sha256={'baseline':hashlib.sha256(old).hexdigest(),'candidate':hashlib.sha256(new).hexdigest()},
              binaries={n:dict(path=str(bindir/n),sha256=sha(bindir/n)) for n in ['map-helpers-baseline','map-helpers-baseline-diagnostic','map-helpers-candidate','map-helpers-candidate-diagnostic']},
              validation_artifacts={str(p.relative_to(ROOT)):sha(p) for p in sorted((BASE/'I2c-validation').glob('*')) if p.is_file()},
              runtime_policy_changed=False,object_abi_changed=False,saved_mir_format_changed=False)
(BASE / 'I2c-binaries.json').write_text(json.dumps(report,indent=2)+'\n')
print('Validated 2,212 disjoint measured processes, 136 GC/semantic checks, 12 MIR probes and exact same-native tuple elimination')
