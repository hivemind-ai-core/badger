#!/usr/bin/env python3
"""Summarize an interleaved opportunity run without discarding raw ranges."""
import argparse
import json
from pathlib import Path


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("measurement", type=Path)
    args = ap.parse_args()
    report = json.loads(args.measurement.read_text())
    rows = []
    for case, metric in [("compute", "options"), ("compute", "sequence"),
                         ("compute", "array_reads"), ("arrays", "array_iterate"),
                         ("arrays", "byte_iterate"), ("text", "slice-é-1000000"),
                         ("text", "slice-a-1000000")]:
        for w in [1, 2, 4, 8]:
            old = report["summaries"][f"baseline-{case}-w{w}"][metric]
            new = report["summaries"][f"current-{case}-w{w}"][metric]
            rows.append(dict(case=metric, workers=w, before=old, after=new,
                             speedup=old["median"] / new["median"],
                             reduction_percent=100 * (1 - new["median"] / old["median"])))
    print("| Case | Workers | Before median ms [range] | After median ms [range] | Speedup |")
    print("|---|---:|---:|---:|---:|")
    for r in rows:
        def value(k):
            x = r[k]
            return f"{x['median']/1e6:.4f} [{x['min']/1e6:.4f}, {x['max']/1e6:.4f}]"
        print(f"| {r['case']} | {r['workers']} | {value('before')} | {value('after')} | {r['speedup']:.2f}x |")
    args.measurement.with_name("comparison.json").write_text(json.dumps(rows, indent=2) + "\n")


if __name__ == "__main__":
    main()
