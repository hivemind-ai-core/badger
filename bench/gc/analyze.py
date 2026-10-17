#!/usr/bin/env python3
"""Summarize retained measurements and collapsed CPU profiles without rerunning."""
import argparse
import collections
import gzip
import json
from pathlib import Path


def phase_totals(run):
    before, after = ((run["record"]["before"], run["record"]["after"]) if "before" in run["record"]
                     else (run["record"]["heap_before"], run["record"]["heap_after"]))
    result = collections.Counter()
    if run["gc"]:
        for c in run["gc"]["collections"]:
            if before["collections"] < c["heap"]["collection"] <= after["collections"]:
                result.update(c["phases_ns"])
    return result


def profile(path):
    own, total = collections.Counter(), collections.Counter()
    count = 0
    for line in path.read_text().splitlines():
        frames, n = line.rsplit(" ", 1)
        n = int(n)
        stack = frames.split(";")
        own[stack[-1]] += n
        for name in set(stack):
            total[name] += n
        count += n
    return count, own, total


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("directory", type=Path)
    args = ap.parse_args()
    d = json.loads(gzip.decompress((args.directory / "measurements.json.gz").read_bytes()))
    lines = ["| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |",
             "|---|---:|---|---:|---:|---:|"]
    for s in d["summary"]:
        v = s["elapsed_ms"]
        lines.append(f"| {s['case']} | {s['workers']} | {s['variant']} | {v['median']:.3f} [{v['min']:.3f}, {v['max']:.3f}] | {s['gc_pause_ms']['median']:.3f} | {s['peak_rss_kib']['median']/1024:.1f} |")
    paired = collections.defaultdict(dict)
    for summary in d["summary"]:
        paired[(summary["case"], summary["workers"])][summary["variant"]] = summary
    if "candidate" in d["binaries"]:
        lines += ["", "Ordinary candidate versus baseline (negative time/RSS change is better):", "",
                  "| Case | Workers | Work time change | GC time change | RSS change | Collections base / candidate |",
                  "|---|---:|---:|---:|---:|---:|"]
        def change(base, candidate):
            if base == 0:
                return "n/a"
            return f"{100 * (candidate / base - 1):+.1f}%"
        for (case, workers), variants in sorted(paired.items()):
            if "baseline" not in variants or "candidate" not in variants:
                raise ValueError(f"incomplete comparison: {case} w{workers}")
            base, candidate = variants["baseline"], variants["candidate"]
            changes = [change(base[key]["median"], candidate[key]["median"])
                       for key in ["elapsed_ms", "gc_pause_ms", "peak_rss_kib"]]
            lines.append(f"| {case} | {workers} | " + " | ".join(changes)
                         + f" | {base['collections']['median']} / {candidate['collections']['median']} |")
    lines += ["", "Diagnostic GC phases summed across measured repetitions (wall time):", ""]
    groups = collections.defaultdict(collections.Counter)
    for run in d["runs"]:
        groups[(run["case"], run["workers"], run["variant"])].update(phase_totals(run))
    for key, counts in sorted(groups.items()):
        if counts:
            lines.append(f"- {key}: " + ", ".join(f"{k} {v/1e6:.3f} ms" for k, v in counts.most_common()))
    for path in sorted(args.directory.glob("*.folded")):
        if path.stem.endswith("--1"):
            continue
        n, own, total = profile(path)
        lines += ["", f"CPU profile {path.name}: {n} samples", "", "Self:"]
        lines.extend(f"- {name}: {c/n*100:.1f}% ({c})" for name,c in own.most_common(15))
        lines.append("Inclusive (overlapping):")
        lines.extend(f"- {name}: {c/n*100:.1f}% ({c})" for name,c in total.most_common(15))
    (args.directory / "summary.md").write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
