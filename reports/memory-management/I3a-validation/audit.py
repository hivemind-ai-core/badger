"""Recheck stored-sequence raw evidence, provenance, pairing and GC traffic."""
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
import applications as app

gc = app.gc
sha = app.rw.digest
names = ['I3a-baseline', 'I3a-coverage', 'I3a-census', 'I3a-native-mir', 'I3a-native-control', 'I3a-native-census', 'I3a-applications', 'I3a-isolated-source', 'I3a-isolated-native', 'I3a-isolated-cross-native']
for optional in ['I3a-app-repeat', 'I3a-app-control']:
    if (BASE / optional / 'measurements.json.gz').exists():
        names.append(optional)
reports, datasets, intervals, oracles = {}, [], [], {}
for name in names:
    path = BASE / name / 'measurements.json.gz'
    data = json.loads(gzip.decompress(path.read_bytes()))
    assert data.get('status', 'complete') == 'complete', name
    cells = Counter((r.get('suite'), r['case'], r['workers'], r['variant']) for r in data['runs'])
    assert all(n == data['arguments']['reps'] for n in cells.values()), name
    for row in data['runs']:
        intervals.append((row['host_before']['wall_time_ns'], row['host_after']['wall_time_ns'], name))
        if 'record' in row:
            config = row['record']['config']
            key = tuple(sorted(config.items()))
            if key not in oracles:
                oracles[key] = gc.expected_coverage(config)
            assert row['record']['checksum'] == oracles[key], (name, row['case'])
            assert row['record']['elapsed_ns'] > 0
        else:
            key = (row['suite'], row['case'], row['n'])
            if key not in oracles:
                oracles[key] = (app.rw if row['suite'] == 'realworld' else app.compare).oracle(row['case'], row['n'], 37)
            app.validate(row['process'], row['suite'], row['case'], row['n'], 37,
                         data['arguments']['warmups'], data['arguments']['samples'], oracles[key])
        if row.get('allocations'):
            gc.validate_allocations(row['allocations'])
        if row.get('gc'):
            gc.validate_sweep(row['gc'])
    for binary in data['binaries'].values():
        assert sha(Path(binary['path'])) == binary['sha256'], (name, binary['path'])
    for source, expected in data['sources'].items():
        assert sha(ROOT / source) == expected, ('harness drift', name, source)
    for program in data.get('effective_programs', {}).values():
        assert sha(ROOT / program['path']) == program['sha256']
    datasets.append(dict(path=str(path.relative_to(ROOT)), sha256=sha(path), runs=len(data['runs']),
                         arguments=data['arguments'], binaries=data['binaries'], effective_programs=data.get('effective_programs')))
    reports[name] = data
intervals.sort()
assert all(a[1] <= b[0] for a, b in zip(intervals, intervals[1:])), 'overlapping measurements'

lines = ['# I3a paired timing evidence', '', 'Median adjacent candidate/baseline elapsed ratios; negative is faster. All samples retained. Ranges are not confidence intervals.', '']
for name in ['I3a-coverage', 'I3a-native-mir', 'I3a-native-control', 'I3a-isolated-source', 'I3a-isolated-native', 'I3a-isolated-cross-native']:
    data = reports[name]
    lines += [f'## {name}', '', '| Case | Workers | Paired median | Faster pairs | Min / max |', '|---|---:|---:|---:|---:|']
    for case, workers in sorted({(r['case'], r['workers']) for r in data['runs']}):
        selected = {(r['repetition'], r['variant']): r for r in data['runs'] if r['case'] == case and r['workers'] == workers}
        changes = [(selected[(i, 'candidate')]['record']['elapsed_ns'] / selected[(i, 'baseline')]['record']['elapsed_ns'] - 1) * 100 for i in range(data['arguments']['reps'])]
        lines.append(f'| {case} | {workers} | {statistics.median(changes):+.1f}% | {sum(d < 0 for d in changes)}/{len(changes)} | {min(changes):+.1f}% / {max(changes):+.1f}% |')
    lines += ['']
(BASE / 'I3a-paired-summary.md').write_text('\n'.join(lines).rstrip() + '\n')
traffic = []
for case in sorted({r['case'] for r in reports['I3a-native-census']['runs']}):
    variants = {}
    for variant in ['baseline', 'candidate']:
        profiles = [r['allocations'] for r in reports['I3a-native-census']['runs'] if r['case'] == case and r['variant'] == variant]
        totals = {k: sorted({p['totals']['allocated'][k] for p in profiles}) for k in ['objects', 'cell_bytes', 'requested_bytes']}
        options = {sum(s['counts']['allocated']['objects'] for s in p['sites'] if s['type'] == 'Option') for p in profiles}
        types = sorted({s['type'] for p in profiles for s in p['sites'] if s['counts']['allocated']['objects']})
        variants[variant] = dict(totals=totals, option_objects=sorted(options), allocated_types=types)
    if case in ['stored', 'stored-reused', 'stored-list', 'stored-pointers']:
        config = gc.COVERAGE[case]
        expected = 2 * config['n'] * config['rounds'] * (2 if case == 'stored-reused' else 1)
        assert variants['baseline']['option_objects'] == [expected + 1], (case, variants)
        assert variants['candidate']['option_objects'] == [1], (case, variants)
    else:
        assert variants['baseline']['option_objects'] == variants['candidate']['option_objects'], (case, variants)
        assert variants['baseline']['totals']['objects'] == variants['candidate']['totals']['objects'], (case, variants)
    traffic.append(dict(case=case, variants=variants))
checks = {}
for label in ['semantic', 'semantic-diagnostic']:
    path = BASE / f'I3a-validation/{label}.json.gz'
    check = json.loads(gzip.decompress(path.read_bytes()))
    assert len(check['probes']) == 24 and len(check['runs']) == 152
    for source, expected in check['sources'].items():
        assert sha(ROOT / source) == expected, ('semantic source drift', source)
    checks[label] = dict(path=str(path.relative_to(ROOT)), sha256=sha(path), probes=24, runs=152)
production = ['crates/badger_mir/src/lower.rs', 'crates/badger_mir/src/lower/fusion.rs']
changed = subprocess.check_output(['git', 'diff', '--name-only', '78cc5a4', '10dde6a', '--', 'crates', 'Cargo.toml', 'Cargo.lock'], cwd=ROOT, text=True).splitlines()
assert [p for p in changed if '/src/' in p or '/std/' in p or p in ['Cargo.toml', 'Cargo.lock']] == production
assert not subprocess.check_output(['git', 'diff', '10dde6a', '--', *production], cwd=ROOT)
profiles_path = BASE / 'I3a-byte-profile/profiles.json.gz'
profiles = json.loads(gzip.decompress(profiles_path.read_bytes()))
assert len(profiles['runs']) == 12
for row in profiles['runs']:
    assert sha(ROOT / row['folded']) == row['folded_sha256']
    gc.validate_coverage(row['record'], profiles['config'])
    intervals.append((row['host_before']['wall_time_ns'], row['host_after']['wall_time_ns'], 'profile-perturbed'))
intervals.sort()
assert all(a[1] <= b[0] for a, b in zip(intervals, intervals[1:])), 'profile/ordinary overlap'
instructions = json.loads((BASE / 'I3a-validation/runtime-instructions.json').read_text())
assert all(value['matching_normalized_instructions'] for value in instructions.values())
validation = BASE / 'I3a-validation'
import re
initial = (validation / 'tests.log').read_text()
remaining = (validation / 'tests-remaining.log').read_text()
assert 'cannot reserve a local port' in initial
assert 'FAILED' not in remaining
count = lambda text: sum(map(int, re.findall(r'test result: ok\. (\d+) passed;', text)))
assert count(initial) + count(remaining) == 121
assert '14 passed; 0 failed' in (validation / 'tests-mir.log').read_text()
assert 'Ran 27 tests' in (validation / 'harness.log').read_text() and 'OK' in (validation / 'harness.log').read_text()
assert not (validation / 'format.log').read_text()
for name in ['clippy-ordinary', 'clippy-diagnostic', 'build-ordinary', 'build-diagnostic']:
    log = (validation / f'{name}.log').read_text()
    assert 'Finished' in log and 'error:' not in log, name
artifacts = {str(p.relative_to(ROOT)): sha(p) for p in validation.iterdir() if p.is_file()}
manifest = dict(baseline_revision='78cc5a4', candidate_revision='10dde6a', measured_processes=len(intervals)-12, profile_processes=12, measured_intervals_overlap=False,
                datasets=datasets, profile_artifact=dict(path=str(profiles_path.relative_to(ROOT)), sha256=sha(profiles_path)), semantic_checks=checks, managed_traffic_same_native=traffic,
                production_sources={p: sha(ROOT / p) for p in production}, validation_artifacts=artifacts, audit_sha256=sha(Path(__file__)))
(BASE / 'I3a-binaries.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(f'Validated {len(intervals)-12:,} disjoint measured processes plus 12 perturbed profiles, 48 MIR probes and 304 GC/semantic checks')
