# Runtime benchmark update after receive optimization

Measured committed main `ec2b13e9e319b87d86e2ecc7a3412809311d3085` using an isolated archive and independently rebuilt local packages. The shared checkout had an uncommitted scheduler/stealing experiment; it was excluded. Main was still at the measured revision at final verification. No production source was changed by this task.

## What landed since the preceding run

The preceding measured compiler/runtime was `eacd48c` (also equivalent for those inputs to the later telemetry merge `fa0ae0d`). Two newer optimizations were absent from that run:

- `6fc4e88`: use channel fast paths on one worker and cache shared-unit construction decisions.
- `c86417f`: an optimized MIR pass removes `Option`/`TryRecv` result allocation when a receive result is only inspected/matched. Pointer payloads remain rooted through the split return.

These are now measured. The report commit `ec2b13e` changes documentation. Topology inference, specialized SPSC/MPSC queues, producer-owned batch credits and bulk receive remain future work; this is not completion of every slice in LOCK_OPTIMIZATION.md.

## Comparable microbenchmark rerun

Repeated the same 11 workloads on workers 1/2/4, then used seven shuffled/interleaved repetitions of the old and new executables to confirm six cases on workers 1/4. The initial matrix contained large control outliers, so the interleaved medians are preferred for those cases. All 137 matrix/diagnostic processes and 192 confirmation processes succeeded. Lower milliseconds are better.

| Workload | Workers | Previous remeasured ms | New remeasured ms | Time change |
| --- | ---: | ---: | ---: | ---: |
| 1:1, capacity 1024 | 4 | 10.318 | 5.228 | -49.3% |
| 1:1, 512-byte budget | 4 | 11.905 | 6.358 | -46.6% |
| 4 producers / 4 consumers | 4 | 15.154 | 10.474 | -30.9% |
| Messaging benchmark | 4 | 24.666 | 18.534 | -24.9% |
| Independent channel pairs | 4 | 5.402 | 2.745 | -49.2% |
| Allocation/list updates | 4 | 23.718 | 26.075 | +9.9% |

Allocation/list updates measured about 10% slower on four workers in confirmation (narrowly overlapping ranges), despite being roughly unchanged on one worker. This remains a follow-up signal. Channel improvements do not imply that every workload improved. A current one-worker N:M run is 4.291 ms versus 10.474 ms on four, so single-channel multicore overhead also remains.

[Full old/new comparison and ranges](2026-10-16-channel-latest-comparison.md) and [all confirmation samples](channel-topology-investigation/confirmation-eacd48c-ec2b13e.json).

## New realistic-workload baseline

Ran the existing telemetry suite unchanged with the same isolated executable: all four profiles, fixed 65,536 events, 8 parsers, sampling every 16th event, workers 1/2/4/8, one warmup and three measured fresh processes per cell. Case order is reproducibly shuffled. There are 48 measured and 16 warmup runs; every run passed the independent payload/hash/routing/cache oracle. Nine edge-case smoke runs and all nine Python harness tests also passed. Benchmark execution did not overlap the build, microbenchmarks or harness unit tests. External machine activity/frequency and CPU placement were uncontrolled.

Throughput below is median events per second; higher is better. Speedup compares each profile against its own one-worker result.

| Profile | 1 worker | 2 workers | 4 workers | 8 workers | 4-worker speedup | 8-worker speedup |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| balanced | 561,257 | 523,698 | 529,612 | 482,062 | 0.94× | 0.86× |
| backpressure | 533,677 | 655,673 | 629,314 | 590,583 | 1.18× | 1.11× |
| fanout | 145,582 | 158,273 | 149,652 | 147,966 | 1.03× | 1.02× |
| cpu | 208,091 | 356,703 | 558,085 | 617,993 | 2.68× | 2.97× |

The CPU-heavy profile scales to 2.68× on four workers and 2.97× on eight. Balanced is 0.94× on four and 0.86× on eight, so adding workers still reduces throughput there. Backpressure peaks at two workers; fan-out is roughly flat with more workers. The workload deliberately includes a single cache actor and sequential service work; these results do not attribute all limits to channel locking.

## Comparison with the existing telemetry reference

The repository already retained an older telemetry baseline using runtime `9a2d68e`. Its workload and runner hashes, default configs, workers and sample settings match this new baseline. These older measurements were collected at a different time and are reference comparisons, not an interleaved optimization attribution.

| Profile | Workers | Older reference events/s | New baseline events/s | Throughput change |
| --- | ---: | ---: | ---: | ---: |
| balanced | 1 | 505,007 | 561,257 | +11.1% |
| balanced | 4 | 486,801 | 529,612 | +8.8% |
| balanced | 8 | 445,055 | 482,062 | +8.3% |
| backpressure | 1 | 520,444 | 533,677 | +2.5% |
| backpressure | 4 | 644,693 | 629,314 | -2.4% |
| backpressure | 8 | 588,985 | 590,583 | +0.3% |
| fanout | 1 | 156,480 | 145,582 | -7.0% |
| fanout | 4 | 147,615 | 149,652 | +1.4% |
| fanout | 8 | 137,241 | 147,966 | +7.8% |
| cpu | 1 | 195,084 | 208,091 | +6.7% |
| cpu | 4 | 522,035 | 558,085 | +6.9% |
| cpu | 8 | 523,944 | 617,993 | +18.0% |

## Retained evidence and limits

- [New telemetry baseline](telemetry-indexer/2026-10-16-ec2b13e-baseline.md) with all worker counts, latency and memory metrics.
- [Raw baseline](telemetry-indexer/2026-10-16-ec2b13e-baseline.json), including every measurement, config, source/executable hashes and process counters.
- [External executable build provenance](telemetry-indexer/2026-10-16-ec2b13e-provenance.json). The runner revision identifies the harness checkout; this file establishes the external executable revision.
- [Smoke results](telemetry-indexer/2026-10-16-ec2b13e-smoke.md) and [raw smoke records](telemetry-indexer/2026-10-16-ec2b13e-smoke.json).

The default run pools 12,288 sampled latency observations per cell. Histogram quantiles are bucket upper bounds; this is not a high-confidence p99.9 study. Peak RSS includes compilation/JIT and native pools, while pipeline throughput excludes compilation. Allocation-call counts remain unavailable; semantic counts are recorded. No perf-counter, long-duration or memory-scaling run was added here. Preserve the older reference and these new files; do not overwrite them when measuring the next revision.
