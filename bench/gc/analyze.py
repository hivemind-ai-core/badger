#!/usr/bin/env python3
"""Summarize retained measurements and collapsed CPU profiles without rerunning."""
import argparse
import collections
import gzip
import json
from pathlib import Path
import statistics

from run import t


def workload_collections(run):
    before, after = ((run["record"]["before"], run["record"]["after"]) if "before" in run["record"]
                     else (run["record"]["heap_before"], run["record"]["heap_after"]))
    if run["gc"]:
        yield from (c for c in run["gc"]["collections"]
                    if before["collections"] < c["heap"]["collection"] <= after["collections"])


def phase_totals(run):
    result = collections.Counter()
    for c in workload_collections(run):
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
    if d["arguments"].get("paired") and "candidate" in d["binaries"]:
        pairs = collections.defaultdict(dict)
        for run in d["runs"]:
            pairs[(run["case"], run["workers"], run["repetition"])][run["variant"]] = run
        changes = collections.defaultdict(list)
        for (case, workers, _), pair in pairs.items():
            if "baseline" in pair and "candidate" in pair:
                before = pair["baseline"]["record"]["elapsed_ns"]
                after = pair["candidate"]["record"]["elapsed_ns"]
                changes[(case, workers)].append(100 * (after / before - 1))
        lines += ["", "Adjacent randomized pairs: per-pair elapsed ratios, without dropping outliers.",
                  "Pairs reduce time separation; these counts are not a significance test.", "",
                  "| Case | Workers | Pairs | Work change median [min, max] | Pairs with lower candidate time |",
                  "|---|---:|---:|---:|---:|"]
        for (case, workers), values in sorted(changes.items()):
            lines.append(f"| {case} | {workers} | {len(values)} | {statistics.median(values):+.1f}% [{min(values):+.1f}%, {max(values):+.1f}%] | {sum(v < 0 for v in values)} / {len(values)} |")
    checkpoints = collections.defaultdict(list)
    for run in d["runs"]:
        stages = ["tasks done", "after churn", "idle"] if run["case"].startswith("tasks-") else ["burst", "after churn", "idle"]
        for stage, snapshot in zip(stages, run["record"].get("checkpoints", [])):
            checkpoints[(run["case"], run["workers"], run["variant"], stage)].append(snapshot)
    if checkpoints:
        lines += ["", "Lifetime checkpoints: pool reservation is distinct from live objects and process peak RSS.",
                  "Idle yields without allocating intentionally or forcing a collection.", "",
                  "| Case | Workers | Variant | Stage | Pool MiB median [min, max] | Managed live MiB median | Native external MiB median | Collection count median |",
                  "|---|---:|---|---|---:|---:|---:|---:|"]
        for (case, workers, variant, stage), snapshots in sorted(checkpoints.items()):
            pool = [s["pool_reserved_bytes"] / (1 << 20) for s in snapshots]
            live = statistics.median(s["managed_live_bytes"] for s in snapshots) / (1 << 20)
            external = statistics.median(s["external_live_bytes"] for s in snapshots) / (1 << 20)
            count = statistics.median(s["collections"] for s in snapshots)
            lines.append(f"| {case} | {workers} | {variant} | {stage} | {statistics.median(pool):.2f} [{min(pool):.2f}, {max(pool):.2f}] | {live:.2f} | {external:.2f} | {count} |")
    histograms = collections.defaultdict(lambda: [0] * 512)
    for measurement in d["runs"]:
        record = measurement["record"]
        if record.get("workload") != "telemetry":
            continue
        histogram = histograms[(measurement["case"], measurement["workers"], measurement["variant"])]
        for shard in record["result"]["shards"]:
            for i, count in enumerate(shard["histogram"]):
                histogram[i] += count
    if histograms:
        lines += ["", "End-to-end latency: pooled histogram bucket upper bounds across repetitions.",
                  "These are sampled batch/event latencies, not GC pauses. Shared-host variation applies;",
                  "p99.9 is not reported here; fewer than 100,000 samples per cell is insufficient for it.", "",
                  "| Profile | Workers | Variant | Samples | p50 <= ms | p99 <= ms |",
                  "|---|---:|---|---:|---:|---:|"]
        for (case, workers, variant), histogram in sorted(histograms.items()):
            quantiles = [t.percentile(histogram, q) for q in [.5, .99]]
            values = [f"{value / 1e6:.3f}" if value is not None else "overflow" for value in quantiles]
            lines.append(f"| {case} | {workers} | {variant} | {sum(histogram)} | " + " | ".join(values) + " |")
    lines += ["", "Diagnostic GC phases summed across measured repetitions (wall time):", ""]
    groups = collections.defaultdict(collections.Counter)
    for run in d["runs"]:
        groups[(run["case"], run["workers"], run["variant"])].update(phase_totals(run))
    for key, counts in sorted(groups.items()):
        if counts:
            lines.append(f"- {key}: " + ", ".join(f"{k} {v/1e6:.3f} ms" for k, v in counts.most_common()))
    sweep_groups = collections.defaultdict(list)
    for run in d["runs"]:
        for c in workload_collections(run):
            if "sweep_work_per_worker" in c["heap"]:
                sweep_groups[(run["case"], run["workers"], run["variant"])].append(c["heap"])
    if sweep_groups:
        lines += ["", "Sweep work distribution (workload collections only):", "",
                  "Small helper jobs contain fewer than 4,096 objects; native thread creation is counted separately.",
                  "these counts alone do not measure thread-startup cost.", "",
                  "| Case | Workers | Variant | Collections / parallel | Helper jobs | New native threads | Small helper jobs | Median helper objects | Median largest-list share in parallel collections |",
                  "|---|---:|---|---:|---:|---:|---:|---:|---:|"]
        for (case, workers, variant), heaps in sorted(sweep_groups.items()):
            parallel = [h for h in heaps if h["sweep_helpers"]]
            jobs = [n for h in heaps for n in h["sweep_work_per_helper"]]
            shares = [max(h["sweep_work_per_worker"]) / h["swept"] for h in parallel]
            median = f"{statistics.median(jobs):.0f}" if jobs else "n/a"
            share = f"{100 * statistics.median(shares):.1f}%" if shares else "n/a"
            created = sum(h.get("sweep_helpers_created", h["sweep_helpers"]) for h in heaps)
            lines.append(f"| {case} | {workers} | {variant} | {len(heaps)} / {len(parallel)} | {len(jobs)} | {created} | {sum(n < 4096 for n in jobs)} | {median} | {share} |")
    for path in sorted(args.directory.glob("*.folded")):
        if path.stem.endswith("--1"):
            continue
        n, own, total = profile(path)
        lines += ["", f"CPU profile {path.name}: {n} samples", "", "Self:"]
        lines.extend(f"- {name}: {c/n*100:.1f}% ({c})" for name,c in own.most_common(15))
        lines.append("Inclusive (overlapping):")
        lines.extend(f"- {name}: {c/n*100:.1f}% ({c})" for name,c in total.most_common(15))
    (args.directory / "summary.md").write_text("\n".join(lines).rstrip() + "\n")


if __name__ == "__main__":
    main()
