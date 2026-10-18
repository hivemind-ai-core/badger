"""Recheck retained datasets, exact managed traffic and measured intervals."""
from pathlib import Path
import gzip, hashlib, json, subprocess, sys
from collections import Counter
ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'bench/gc'))
import applications as app
import profile_applications as cpu
BASE = ROOT / 'reports/memory-management'
ORDINARY = ['C1-baseline', 'C1-applications', 'C1-coverage', 'C1-census',
            'C1-messaging-repeat', 'C1-messaging-control', 'I5b-native-mir',
            'I5b-native-mir-control', 'I5b-messaging-source-repeat',
            'W1-fixed-smoke', 'W1-fixed-baseline']
PROFILES = ['next-profile-baseline', 'next-profile-callers', 'C1-profile',
            'I5b-messaging-profile', 'W1-fixed-profile']
sha = app.rw.digest
intervals, datasets = [], []
for name in ORDINARY + PROFILES:
    profile = name in PROFILES
    path = BASE / name / ('profiles.json.gz' if profile else 'measurements.json.gz')
    data = json.loads(gzip.decompress(path.read_bytes()))
    assert data.get('status', 'complete') == 'complete', name
    groups = Counter((r.get('case'), r['workers'], r.get('variant')) for r in data['runs'])
    assert all(n == data['arguments']['reps'] for n in groups.values()), (name, groups)
    for r in data['runs']:
        if data['arguments'].get('consumers') and not profile:
            command = r['process']['command']
            assert int(command[command.index('--workers') + 1]) == r['workers']
            assert int(command[command.index('--') + 4]) == data['arguments']['consumers']
        intervals.append((r['host_before']['wall_time_ns'], r['host_after']['wall_time_ns'], name))
        if profile:
            folded = path.parent / r['folded']
            assert sha(folded) == r['folded_sha256']
            assert cpu.counts(folded.read_text()) == r['cpu']
        if r.get('allocations'):
            app.gc.validate_allocations(r['allocations'])
        if r.get('gc'):
            app.gc.validate_sweep(r['gc'])
    datasets.append(dict(path=str(path.relative_to(ROOT)), sha256=sha(path),
                         runs=len(data['runs']), arguments=data['arguments'],
                         binaries=data.get('binaries'), binary_sha256=data.get('binary_sha256')))
intervals.sort()
assert all(a[1] <= b[0] for a,b in zip(intervals, intervals[1:])), 'overlapping measured datasets'
census = json.loads(gzip.decompress((BASE/'C1-census/measurements.json.gz').read_bytes()))
traffic = []
for case in ['json', 'records', 'reconcile']:
    rs = [r for r in census['runs'] if r['case'] == case]
    totals = [r['allocations']['totals']['allocated'] for r in rs]
    for field in ['objects', 'cell_bytes']:
        assert len({t[field] for t in totals}) == 1, (case, field, totals)
    traffic.append(dict(case=case, runs=len(rs), allocated_objects=totals[0]['objects'],
                        allocated_cell_bytes=totals[0]['cell_bytes'],
                        requested_bytes_range=[min(t['requested_bytes'] for t in totals),
                                               max(t['requested_bytes'] for t in totals)]))
bindir = Path('/home/dan/orca/projects/language/target/memory-binaries-2026-10-18')
names = ['snapshot-epochs-specialized-candidate', 'snapshot-epochs-specialized-diagnostic',
         'prefetch32-candidate', 'codec-temporaries-candidate', 'codec-temporaries-diagnostic',
         'profile-beb0139-diagnostic', 'profile-b080078-diagnostic']
production = ['crates/badger_runtime/src/codec.rs', 'crates/badger_runtime/src/intrinsics_codec.rs',
              'crates/badger_runtime/src/intrinsics_io.rs', 'crates/badger_runtime/src/gc.rs',
              'crates/badger_runtime/src/map.rs', 'crates/badger_runtime/src/hamt.rs']
sources = {rev:{p:hashlib.sha256(subprocess.check_output(['git','show',f'{rev}:{p}'],cwd=ROOT)).hexdigest()
                for p in production} for rev in ['beb0139','b080078']}
for p in production:
    assert sha(ROOT/p) == sources['b080078'][p], ('production drift',p)
report = dict(candidate_revision='b080078', baseline_revision='49f281c',
              binaries={n:dict(path=str(bindir/n),sha256=sha(bindir/n)) for n in names},
              production_sha256=sources, datasets=datasets, measured_processes=len(intervals),
              measured_intervals_overlap=False, managed_traffic=traffic,
              profiling_flags=dict(CARGO_PROFILE_RELEASE_DEBUG='1', RUSTFLAGS='-C force-frame-pointers=yes',
                                   feature='badger_runtime/gc-profile'),
              validation_artifacts={str(p.relative_to(ROOT)):sha(p) for p in sorted((BASE/'C1-validation').glob('*'))
                                    if p.is_file() and p.suffix != '.py'},
              validation_scripts={str(p.relative_to(ROOT)):sha(p) for p in sorted((BASE/'C1-validation').glob('*.py'))},
              harness_sha256={str(p.relative_to(ROOT)):sha(p) for p in [ROOT/'bench/gc/check_codecs.py',
                              ROOT/'bench/gc/profile_applications.py', ROOT/'bench/gc/applications.py']},
              audit_sha256=sha(__file__))
(BASE/'C1-binaries.json').write_text(json.dumps(report,indent=2)+'\n')
print(len(intervals),'measured processes across',len(datasets),'disjoint datasets; managed objects/cell bytes unchanged')
