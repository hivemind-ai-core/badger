"""Regenerate application allocation-path tables from retained diagnostic data."""
import collections
import gzip
import json
from pathlib import Path
import statistics
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'bench/gc'))
from run import validate_allocations
from analyze_allocations import allocation_paths

path = ROOT / 'reports/memory-management/G8b-applications'
data = json.loads(gzip.decompress((path / 'measurements.json.gz').read_bytes()))
groups = collections.defaultdict(list)
for row in data['runs']:
    validate_allocations(row['allocations'])
    groups[(row['suite'] + '/' + row['case'], row['workers'])].append(row['allocations'])
lines = ['# G8b application allocation paths', '',
         'Diagnostic census, three process repetitions per cell, three warmups/five measured inner samples plus startup/output. Eight fixed logical consumers for messaging. These perturbed timings are not an ordinary-build speed comparison.']
lines += allocation_paths(groups)
lines += ['', '| Case | Workers | Allocated objects | Managed cell MiB | System cell byte share | Top runtime source by requested bytes | Source share |',
          '|---|---:|---:|---:|---:|---|---:|']
for (case, workers), profiles in sorted(groups.items()):
    sites = collections.Counter()
    for profile in profiles:
        for site in profile['sites']:
            sites[(site['type'], site['runtime_source']['file'], site['runtime_source']['line'])] += site['counts']['allocated']['requested_bytes']
    top, count = sites.most_common(1)[0]
    total = sum(p['totals']['allocated']['requested_bytes'] for p in profiles)
    objects = statistics.median(p['totals']['allocated']['objects'] for p in profiles)
    cells = statistics.median(p['totals']['allocated']['cell_bytes'] for p in profiles)
    system_share = statistics.median(p['cell_sources']['system']['cell_bytes'] / p['totals']['allocated']['cell_bytes'] * 100 for p in profiles)
    lines.append(f'| {case} | {workers} | {objects:,.0f} | {cells / 2**20:.2f} | {system_share:.1f}% | {top[0]} {Path(top[1]).name}:{top[2]} | {count / total * 100:.1f}% |')
(path / 'allocation-summary.md').write_text('\n'.join(lines) + '\n')
