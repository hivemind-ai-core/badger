"""Render compact tables from retained raw measurements without rerunning workloads."""
from pathlib import Path
import gzip
import json
import statistics

BASE = Path(__file__).resolve().parents[1]
for path in sorted(BASE.glob('I3a-*/measurements.json.gz')):
    data = json.loads(gzip.decompress(path.read_bytes()))
    rows = data['runs']
    lines = [f'# {path.parent.name}', '', f'{len(rows):,} measured processes. [Raw data](measurements.json.gz). Negative paired changes are faster; ranges and all samples remain in the raw artifact.', '']
    application = 'record' not in rows[0]
    if application:
        lines += ['| Suite/case | Workers | Paired median | Paired block mean | Faster median pairs |', '|---|---:|---:|---:|---:|']
        for summary in data['summary']:
            lines.append(f"| {summary['suite']}/{summary['case']} | {summary['workers']} | {100*summary['paired_work_change']['median']:+.2f}% | {100*summary['paired_block_mean_change']['median']:+.2f}% | {summary['faster_pairs']}/{data['arguments']['reps']} |")
        (path.parent / 'derived-summary.json').write_text(json.dumps(data['summary'], indent=2)+'\n')
    else:
        lines += ['| Case | Workers | Variant | Kernel median ms | Collections | Pause ms | RSS MiB | Managed objects (census) |', '|---|---:|---|---:|---:|---:|---:|---:|']
        for case, workers, variant in sorted({(r['case'], r['workers'], r['variant']) for r in rows}):
            group = [r for r in rows if r['case'] == case and r['workers'] == workers and r['variant'] == variant]
            med = lambda values: statistics.median(values)
            objects = f"{med(r['allocations']['totals']['allocated']['objects'] for r in group):,.0f}" if group[0].get('allocations') else '—'
            lines.append(f"| {case} | {workers} | {variant} | {med(r['record']['elapsed_ns']/1e6 for r in group):.3f} | {med(r['heap_collections'] for r in group):g} | {med(r['gc_pause_ns']/1e6 for r in group):.3f} | {med(r['process']['peak_rss_kib']/1024 for r in group):.3f} | {objects} |")
    (path.parent / 'summary.md').write_text('\n'.join(lines).rstrip()+'\n')
