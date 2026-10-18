"""Render a side-by-side comparison of two retained main benchmark runs."""
import argparse
import json
import os
from pathlib import Path
import re
import statistics

LABELS = {
    "spsc_cap1": "1:1, capacity 1",
    "spsc_cap64": "1:1, capacity 64",
    "spsc_cap1024": "1:1, capacity 1024",
    "spsc_byte_budget": "1:1, 512-byte budget",
    "mpsc": "4 producers / 1 consumer",
    "spmc": "1 producer / 4 consumers",
    "mpmc": "4 producers / 4 consumers",
    "messages": "Messaging benchmark",
    "parallel": "CPU tasks",
    "independent_channels": "Four independent channel pairs",
    "parallel_alloc": "Allocation/list updates",
}


def components(data, workers):
    samples = {}
    for row in data["runs"]:
        if row["case"] == "messages" and row["workers"] == workers and not row["warmup"]:
            for line in row["stdout"].splitlines():
                label, value = line.rsplit(": ", 1)
                samples.setdefault(label, []).append(int(value.split()[0]))
    return {name: (statistics.median(values), min(values), max(values)) for name, values in samples.items()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--before", required=True, type=Path)
    parser.add_argument("--after", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    before = json.loads(args.before.read_text())
    after = json.loads(args.after.read_text())
    for data in (before, after):
        assert len(data["runs"]) == 132
        assert all(row["exit"] == 0 for row in data["runs"])
        assert len(data["stats"]) == 5
    assert before["fixture_sha256"] == after["fixture_sha256"], "Benchmark fixtures differ"
    assert before["cpu"] == after["cpu"] and before["affinity"] == after["affinity"]
    old = {(row["case"], row["workers"]): row for row in before["summary"]}
    new = {(row["case"], row["workers"]): row for row in after["summary"]}
    assert old.keys() == new.keys()
    lines = [
        "# Channel fixes benchmark comparison", "",
        f"Previous: `{before['revision']}`. Current committed main snapshot: `{after['revision']}`.", "",
        f"Measured on {after['cpu']}; completed {after['completed_utc']}.", "",
        after["method"], "",
        "All nine local workspace packages were rebuilt in an isolated archive of committed main, with an independent target directory. Uncommitted working-tree changes were excluded. No production source was changed by this benchmark task.", "",
        "Each run contains 132 successful timing processes (11 cases × 3 worker counts × 1 warmup plus 3 measured repetitions), followed by five separate instrumented runs. Inputs, CPU and affinity were checked to match. Lower times are better; negative percentage changes mean less elapsed time. Ranges are the minimum and maximum of three measured repetitions, not confidence intervals. Machine background activity and frequency were uncontrolled; small changes should not be interpreted as established improvements.", "",
        f"[Previous raw results]({os.path.relpath(args.before, args.out.parent)}) and [current raw results]({os.path.relpath(args.after, args.out.parent)}) include hashes, commands, environment details, process output and sampled instrumentation.", "",
        "## Four-worker comparison", "",
        "| Workload | Previous ms | Current ms | Time change | Previous range | Current range |",
        "| --- | ---: | ---: | ---: | --- | --- |",
    ]
    for name, label in LABELS.items():
        a, b = old[name, 4], new[name, 4]
        change = (b["median_ms"] / a["median_ms"] - 1) * 100
        row = f"| {label} | {a['median_ms']:.3f} | {b['median_ms']:.3f} | {change:+.1f}% | {a['min_ms']:.3f}–{a['max_ms']:.3f} | {b['min_ms']:.3f}–{b['max_ms']:.3f} |"
        lines.append(row)
        print(row)
    lines += ["", "## All worker counts", "", "| Workload | Workers | Previous ms | Current ms | Time change | Previous range | Current range |", "| --- | ---: | ---: | ---: | ---: | --- | --- |"]
    for name, label in LABELS.items():
        for workers in (1, 2, 4):
            a, b = old[name, workers], new[name, workers]
            change = (b["median_ms"] / a["median_ms"] - 1) * 100
            lines.append(f"| {label} | {workers} | {a['median_ms']:.3f} | {b['median_ms']:.3f} | {change:+.1f}% | {a['min_ms']:.3f}–{a['max_ms']:.3f} | {b['min_ms']:.3f}–{b['max_ms']:.3f} |")
    lines += ["", "## Messaging components", "", "Prompt capacity is 256. Batch capacity is 4096 with maximum batch 256. The final component is a request/reply round despite the fixture's `ns/message` label. Values are medians of printed integer nanoseconds.", "", "| Operation | Workers | Previous ns | Current ns | Time change | Previous range | Current range |", "| --- | ---: | ---: | ---: | ---: | --- | --- |"]
    for workers in (1, 2, 4):
        a, b = components(before, workers), components(after, workers)
        for label, previous in a.items():
            current = b[label]
            change = (current[0] / previous[0] - 1) * 100
            row = f"| {label} | {workers} | {previous[0]:g} | {current[0]:g} | {change:+.1f}% | {previous[1]}–{previous[2]} | {current[1]}–{current[2]} |"
            lines.append(row)
            if workers == 4:
                print(row)
    lines += ["", "## Global lock instrumentation", "", "Counts below are from one separate instrumented four-worker run per case. Scheduling can change counts; sampled hold totals are not interpreted as percentages of elapsed time.", "", "| Case | Previous global acquisitions | Current global acquisitions | Previous channel-slow acquisitions | Current channel-slow acquisitions |", "| --- | ---: | ---: | ---: | ---: |"]
    def counters(row):
        match = re.search(r"lock: (\d+) fast, (\d+) after spinning, (\d+) after parking", row["stderr"])
        total = sum(map(int, match.groups()))
        match = re.search(r"\[runtime stats\]\s+(\d+) x channel slow path", row["stderr"])
        return total, int(match[1]) if match else None
    for row in after["stats"]:
        previous = next(r for r in before["stats"] if r["case"] == row["case"])
        a, b = counters(previous), counters(row)
        old_slow = f"{a[1]:,}" if a[1] is not None else "not listed"
        new_slow = f"{b[1]:,}" if b[1] is not None else "not listed"
        line = f"| {LABELS[row['case']]} | {a[0]:,} | {b[0]:,} | {old_slow} | {new_slow} |"
        lines.append(line)
        print(line)
    lines += ["", "## Current scaling", "", "| Workload | 1 worker ms | 2 workers ms | 4 workers ms | Speedup, 1 to 4 workers |", "| --- | ---: | ---: | ---: | ---: |"]
    for name in ("parallel", "independent_channels", "parallel_alloc", "mpmc"):
        vals = [new[name, w]["median_ms"] for w in (1, 2, 4)]
        lines.append(f"| {LABELS[name]} | {vals[0]:.3f} | {vals[1]:.3f} | {vals[2]:.3f} | {vals[0] / vals[2]:.2f}× |")
    lines += ["", f"Main at end of current measurement: `{after['main_at_end']}`. Production topology specializations and queue-only native prototypes were not changed or benchmarked by this task.", ""]
    args.out.write_text("\n".join(lines))
    print(args.out)


if __name__ == "__main__":
    main()
