#!/usr/bin/env python3
"""Recompute process-level median/block-mean reports without modifying raw runs."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path

import applications as app


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('directory', type=Path)
    args = ap.parse_args()
    raw = args.directory / 'measurements.json.gz'
    data = json.loads(gzip.decompress(raw.read_bytes()))
    if data.get('status') != 'complete':
        raise ValueError('application measurements are incomplete')
    rows = app.summarize(data['runs'])
    report = dict(raw_sha256=hashlib.sha256(raw.read_bytes()).hexdigest(),
                  analysis_sha256={str(Path(p).name): hashlib.sha256(Path(p).read_bytes()).hexdigest()
                                   for p in [__file__, app.__file__]}, summary=rows)
    (args.directory / 'derived-summary.json').write_text(json.dumps(report, indent=2) + '\n')
    lines = ['# Application kernel medians', '',
             'Each fresh process contributes one median; all inner outputs are checked. Negative changes are faster/smaller. Paired ranges are not confidence intervals.', '']
    baselines = [r for r in rows if len(r['variants']) == 1]
    comparisons = [r for r in rows if len(r['variants']) == 2]
    if baselines:
        lines += ['## Single-variant baselines', '',
                  '| Suite/case | Workers | Variant | Kernel ms median [min, max] | Block mean ms | Peak RSS MiB | Processes |',
                  '|---|---:|---|---:|---:|---:|---:|']
        for row in baselines:
            variant, values = next(iter(row['variants'].items()))
            k = values['kernel_ns']
            lines.append(f"| {row['suite']}/{row['case']} | {row['workers']} | {variant} | {k['median']/1e6:.3f} [{k['min']/1e6:.3f}, {k['max']/1e6:.3f}] | {values['kernel_block_mean_ns']['median']/1e6:.3f} | {values['peak_rss_bytes']['median']/2**20:.1f} | {k['count']} |")
        lines += ['']
    for metric, paired, faster, title in [
        ('kernel_ns', 'paired_work_change', 'faster_pairs', None),
        ('kernel_block_mean_ns', 'paired_block_mean_change', 'block_mean_faster_pairs', 'Block means retain expensive iterations'),
    ]:
        if not comparisons:
            break
        if title:
            lines += ['', '## ' + title, '',
                      'One arithmetic mean of measured kernel iterations per process. This includes costly GC iterations but excludes fixture/I/O time as in the original contract. Inner iterations remain correlated.', '']
        lines += ['| Suite/case | Workers | Base / candidate ms | Group change | Paired median [min, max] | Faster pairs | RSS change |',
                  '|---|---:|---:|---:|---:|---:|---:|']
        for s in comparisons:
            a, b = s['variants']['baseline'], s['variants']['candidate']
            x, y, q = a[metric]['median'], b[metric]['median'], s[paired]
            rss = (b['peak_rss_bytes']['median'] / a['peak_rss_bytes']['median'] - 1) * 100
            lines.append(f"| {s['suite']}/{s['case']} | {s['workers']} | {x/1e6:.3f} / {y/1e6:.3f} | {(y/x-1)*100:+.1f}% | {q['median']*100:+.1f}% [{q['min']*100:+.1f}%, {q['max']*100:+.1f}%] | {s[faster]}/{q['count']} | {rss:+.1f}% |")
    (args.directory / 'summary.md').write_text('\n'.join(lines) + '\n')
    print(f"{args.directory}: {len(data['runs'])} process observations; raw measurements unchanged")


if __name__ == '__main__':
    main()
