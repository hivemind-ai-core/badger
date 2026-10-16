# Telemetry benchmark measurements

These measurements use the committed runtime at `9a2d68e` and the telemetry
workload added on `bench/telemetry-indexer`. They exclude the ongoing, uncommitted
runtime work in the main checkout. Read [the benchmark design](../../bench/telemetry.md)
for topology, correctness checks, metric definitions, and reproduction commands.

* [Default sampling baseline](baseline.md): four profiles, workers 1/2/4/8,
  65,536 events, one fresh-process warmup and three measured runs per cell,
  sampling every 16th event.
* [Every-event latency run](tails.md): the same experiment with every event
  sampled; 196,608 pooled latency observations per cell. Compare this with the
  baseline to assess instrumentation sensitivity.
* [64k memory run](memory-64k.md), [256k memory run](memory-256k.md),
  [1m memory run](memory-1m.md): balanced topology, workers 1/4, sampling every
  16th event, one warmup and two measured runs per cell.

Raw records are stored alongside each report as `.json.gz` files. They retain
all configs, binary/workload/runner hashes, process and heap counters, per-run
histograms, correctness totals, and run ordering. Python's `gzip` and `json`
modules can read them directly. The harness itself emits uncompressed JSON.
The output prefix in the tails raw record reflects its original baseline name;
the report was renamed to distinguish the sampling settings.

The balanced workload's median throughput was about 505k events/s with one worker
and 487k with four (0.96×). Increasing useful CPU work to eight hash passes reached
about 522k events/s with four workers, a 2.68× speedup relative to that profile's
one-worker run. Per-event enrichment stayed roughly flat and regressed with more
workers. These are observations on this host, not performance gates or proof of
a particular scheduler cause. The single cache actor is a deliberate serial
service, and scheduler/collector costs are also present.

| Events | Peak RSS, 1 worker | Peak RSS, 4 workers |
|---:|---:|---:|
| 65,536 | 50.8 MiB | 87.1 MiB |
| 262,144 | 49.4 MiB | 98.4 MiB |
| 1,048,576 | 51.5 MiB | 110.0 MiB |

Memory growth was much smaller than event-count growth. These three sizes do not
establish an exact RSS plateau: cache population, collector thresholds, native
pools, and compiler/JIT memory all contribute. RSS covers the whole process.

Validation passed the independent oracle in all 108 measured baseline/tail/memory
runs and their 38 warmups, plus nine edge-case smoke runs on workers 1/2/4.
Nine Python tests cover oracle validation failures, histogram boundaries/tails,
perf counter parsing, subprocess failure/timeout cleanup, and large output.
`cargo test --release --offline -p lang_cli --test programs bench_programs_check`
passed, as did language type and formatting checks. A separate 8,192-event
experiment verified `--runtime-stats` on workers 1/4, including the extra diagnostic
pass. Hardware perf counters were not collected because `perf` is unavailable on
this host; parsing is unit tested. Actual allocator calls remain unavailable
through the public runtime inspection API and are recorded as null.
