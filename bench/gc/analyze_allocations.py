#!/usr/bin/env python3
"""Summarize allocation census and its calibration without rerunning workloads."""
import argparse
import collections
import gzip
import json
from pathlib import Path
import statistics

from run import validate_allocations


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    report = json.loads(gzip.decompress((args.directory / "measurements.json.gz").read_bytes()))
    groups = collections.defaultdict(list)
    for row in report["runs"]:
        if row.get("allocations"):
            validate_allocations(row["allocations"])
            groups[(row["case"], row["workers"])].append(row["allocations"])
    summary = {(s["case"], s["workers"], s["variant"]): s for s in report["summary"]}
    lines = [
        "Process-wide managed allocations, including startup and output encoding. Three repetitions unless noted in the raw report.",
        "Requested bytes include headers. Cell bytes include pool rounding but exclude native buffers, allocator metadata and unused pool capacity.",
        "Early reclamation and first survival are observed fractions of all allocations; unswept tail objects censor lifetime measurements.",
        "Survival does not establish escape or unique ownership. Diagnostic timings/RSS include profiler overhead.",
        "", "| Case | Workers | Objects | Requested MiB | Cell MiB | Early reclaimed bytes | First-survived bytes | Remaining bytes | Side-vector capacity MiB |",
        "|---|---:|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for (case, workers), profiles in sorted(groups.items()):
        def median(category, metric="requested_bytes"):
            return statistics.median(p["totals"][category][metric] for p in profiles)
        def fraction(category):
            return statistics.median(p["totals"][category]["requested_bytes"] / p["totals"]["allocated"]["requested_bytes"] * 100 for p in profiles)
        capacity = statistics.median(p["object_metadata_capacity_bytes"] for p in profiles) / 2**20
        lines.append(f"| {case} | {workers} | {median('allocated', 'objects'):,.0f} | {median('allocated') / 2**20:.2f} | {median('allocated', 'cell_bytes') / 2**20:.2f} | {fraction('reclaimed_before_survival'):.1f}% | {fraction('first_survival'):.1f}% | {fraction('remaining'):.1f}% | {capacity:.2f} |")
    lines += ["", "Calibration: workload time medians; positive change means slower. Shared-host ranges are in summary.md.",
              "", "| Case | Workers | Phase-off vs ordinary | Phase-only vs phase-off | Census vs phase-only |",
              "|---|---:|---:|---:|---:|"]
    for case, workers in sorted(groups):
        times = {v: summary[(case, workers, v)]["elapsed_ms"]["median"] for v in
                 ["baseline", "diagnostic-off", "diagnostic", "diagnostic-allocations"]}
        changes = [times[a] / times[b] * 100 - 100 for a, b in
                   [("diagnostic-off", "baseline"), ("diagnostic", "diagnostic-off"), ("diagnostic-allocations", "diagnostic")]]
        lines.append(f"| {case} | {workers} | " + " | ".join(f"{c:+.1f}%" for c in changes) + " |")
    lines += ["", "Largest runtime allocation sites (byte shares pooled over repetitions):", "",
              "| Case | Workers | Type | Runtime source | Share of requested bytes |",
              "|---|---:|---|---|---:|"]
    for (case, workers), profiles in sorted(groups.items()):
        sites = collections.Counter()
        total = sum(p["totals"]["allocated"]["requested_bytes"] for p in profiles)
        for p in profiles:
            for s in p["sites"]:
                loc = s["runtime_source"]
                source = f"{Path(loc['file']).name}:{loc['line']}"
                sites[(s["type"], source)] += s["counts"]["allocated"]["requested_bytes"]
        for (name, source), count in sites.most_common(4):
            lines.append(f"| {case} | {workers} | {name} | {source} | {count / total * 100:.1f}% |")
    (args.directory / "allocation-summary.md").write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
