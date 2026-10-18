# Channel fixes benchmark comparison

Previous: `eacd48c51c8582fdc38ff971042c8d3e0eec9274`. Current committed main snapshot: `ec2b13e9e319b87d86e2ecc7a3412809311d3085`.

Measured on AMD Ryzen 7 8745HS w/ Radeon 780M Graphics; completed 2026-10-16T10:00:51.915686+00:00.

Same machine; sequential fresh processes, one warmup and three measured runs per case/worker count. Optimized host and JIT. --bench excludes compilation. LANG_* variables cleared; unpinned, frequency and background activity uncontrolled. Instrumented runs separate. Built isolated git archive of committed main with independent target directory; immutable executable copy and unchanged source snapshot checked.

All nine local workspace packages were rebuilt in an isolated archive of committed main, with an independent target directory. Uncommitted working-tree changes were excluded. No production source was changed by this benchmark task.

Each run contains 132 successful timing processes (11 cases × 3 worker counts × 1 warmup plus 3 measured repetitions), followed by five separate instrumented runs. Inputs, CPU and affinity were checked to match. Lower times are better; negative percentage changes mean less elapsed time. Ranges are the minimum and maximum of three measured repetitions, not confidence intervals. Machine background activity and frequency were uncontrolled; small changes should not be interpreted as established improvements.

[Previous raw results](channel-topology-investigation/rerun-main-eacd48c-093108.json) and [current raw results](channel-topology-investigation/rerun-main-ec2b13e-100035.json) include hashes, commands, environment details, process output and sampled instrumentation.

## Interleaved confirmation and interpretation

Two further optimizations landed after the previous measured `eacd48c` snapshot: `6fc4e88` enables channel fast paths on one worker and `c86417f` eliminates matched receive result allocations. This rerun measures committed `ec2b13e`, which contains both. It excludes the uncommitted scheduler/stealing experiment in the shared checkout. Automatic topology inference, specialized queues, producer-owned batch credits and bulk receive remain future work.

The initial matrix contained wide control-workload outliers, including a four-worker CPU sample of 321 ms and one-worker allocation samples of 62–105 ms. Therefore both retained executables were rerun sequentially in seed-shuffled interleaved order, with one warmup and seven measured runs per version/case/worker count. Six cases on workers 1 and 4 produced 192 successful processes. These current-condition medians are the preferred before/after comparison for those cases. The initial matrix is retained below rather than overwritten.

| Workload | Workers | Old remeasured ms | New remeasured ms | Time change | Old range | New range |
| --- | ---: | ---: | ---: | ---: | --- | --- |
| 1:1, capacity 1024 | 1 | 10.257 | 4.179 | -59.3% | 10.128–10.605 | 4.024–4.294 |
| 1:1, capacity 1024 | 4 | 10.318 | 5.228 | -49.3% | 9.431–11.180 | 4.914–6.125 |
| 1:1, 512-byte budget | 1 | 11.580 | 5.257 | -54.6% | 11.465–12.443 | 5.025–7.415 |
| 1:1, 512-byte budget | 4 | 11.905 | 6.358 | -46.6% | 11.238–12.740 | 5.800–8.931 |
| 4 producers / 4 consumers | 1 | 10.342 | 4.291 | -58.5% | 10.226–12.213 | 4.230–4.390 |
| 4 producers / 4 consumers | 4 | 15.154 | 10.474 | -30.9% | 10.137–16.341 | 10.043–11.218 |
| Messaging benchmark | 1 | 25.425 | 15.599 | -38.6% | 24.694–27.095 | 15.470–15.962 |
| Messaging benchmark | 4 | 24.666 | 18.534 | -24.9% | 22.858–25.158 | 16.860–23.299 |
| Independent channel pairs | 1 | 10.573 | 4.278 | -59.5% | 10.180–11.012 | 4.204–5.629 |
| Independent channel pairs | 4 | 5.402 | 2.745 | -49.2% | 5.108–6.274 | 2.119–3.209 |
| Allocation/list updates | 1 | 34.100 | 33.477 | -1.8% | 33.575–34.911 | 33.045–33.777 |
| Allocation/list updates | 4 | 23.718 | 26.075 | +9.9% | 22.530–25.133 | 25.017–29.701 |

Channel gains repeat: at four workers, buffered 1:1 takes 49.3% less time, byte-budget 46.6% less, N:M 30.9% less, messaging 24.9% less and independent pairs 49.2% less. Allocation/list updates measured 1.8% faster on one worker but 9.9% slower on four, with narrowly overlapping ranges; preserve this as a follow-up signal rather than claim universal improvement.

Multicore overhead still exists: the confirmed N:M case takes 4.291 ms on one worker and 10.474 ms on four. The improved one-worker fast path makes that relative penalty more visible even while both worker counts improve substantially against the old executable.

[All confirmation samples and commands](channel-topology-investigation/confirmation-eacd48c-ec2b13e.json) are retained separately. No production source was changed by this benchmark task.

## Initial three-repeat four-worker comparison

| Workload | Previous ms | Current ms | Time change | Previous range | Current range |
| --- | ---: | ---: | ---: | --- | --- |
| 1:1, capacity 1 | 26.038 | 18.370 | -29.4% | 24.487–26.316 | 17.995–18.384 |
| 1:1, capacity 64 | 12.439 | 6.030 | -51.5% | 11.651–12.769 | 5.351–6.307 |
| 1:1, capacity 1024 | 10.882 | 5.379 | -50.6% | 9.996–11.716 | 5.181–5.728 |
| 1:1, 512-byte budget | 12.527 | 6.905 | -44.9% | 12.325–12.945 | 6.179–7.064 |
| 4 producers / 1 consumer | 10.711 | 5.494 | -48.7% | 10.496–11.145 | 5.147–6.634 |
| 1 producer / 4 consumers | 13.476 | 7.339 | -45.5% | 11.593–14.453 | 7.092–8.793 |
| 4 producers / 4 consumers | 15.310 | 11.328 | -26.0% | 15.265–15.389 | 9.875–11.356 |
| Messaging benchmark | 24.268 | 17.960 | -26.0% | 23.690–24.436 | 16.784–21.054 |
| CPU tasks | 172.256 | 172.120 | -0.1% | 171.538–172.768 | 171.313–321.140 |
| Four independent channel pairs | 5.917 | 8.956 | +51.4% | 5.173–5.981 | 5.447–10.071 |
| Allocation/list updates | 26.750 | 68.815 | +157.3% | 24.613–27.312 | 68.809–80.703 |

## All worker counts

| Workload | Workers | Previous ms | Current ms | Time change | Previous range | Current range |
| --- | ---: | ---: | ---: | ---: | --- | --- |
| 1:1, capacity 1 | 1 | 21.778 | 14.857 | -31.8% | 21.131–22.932 | 14.671–15.347 |
| 1:1, capacity 1 | 2 | 24.707 | 17.793 | -28.0% | 23.432–25.582 | 17.389–18.043 |
| 1:1, capacity 1 | 4 | 26.038 | 18.370 | -29.4% | 24.487–26.316 | 17.995–18.384 |
| 1:1, capacity 64 | 1 | 11.422 | 4.764 | -58.3% | 11.366–12.306 | 4.759–4.878 |
| 1:1, capacity 64 | 2 | 11.769 | 5.556 | -52.8% | 11.252–12.145 | 5.331–5.767 |
| 1:1, capacity 64 | 4 | 12.439 | 6.030 | -51.5% | 11.651–12.769 | 5.351–6.307 |
| 1:1, capacity 1024 | 1 | 10.826 | 4.284 | -60.4% | 10.529–11.158 | 4.200–4.445 |
| 1:1, capacity 1024 | 2 | 11.473 | 5.304 | -53.8% | 10.125–11.476 | 5.084–5.451 |
| 1:1, capacity 1024 | 4 | 10.882 | 5.379 | -50.6% | 9.996–11.716 | 5.181–5.728 |
| 1:1, 512-byte budget | 1 | 12.182 | 5.464 | -55.1% | 12.179–12.821 | 5.323–5.666 |
| 1:1, 512-byte budget | 2 | 12.948 | 6.159 | -52.4% | 12.113–13.209 | 6.155–6.584 |
| 1:1, 512-byte budget | 4 | 12.527 | 6.905 | -44.9% | 12.325–12.945 | 6.179–7.064 |
| 4 producers / 1 consumer | 1 | 10.925 | 4.475 | -59.0% | 10.762–10.990 | 4.429–4.985 |
| 4 producers / 1 consumer | 2 | 12.856 | 5.308 | -58.7% | 10.943–13.813 | 5.143–7.696 |
| 4 producers / 1 consumer | 4 | 10.711 | 5.494 | -48.7% | 10.496–11.145 | 5.147–6.634 |
| 1 producer / 4 consumers | 1 | 11.037 | 4.323 | -60.8% | 10.982–11.111 | 4.310–4.434 |
| 1 producer / 4 consumers | 2 | 12.950 | 9.583 | -26.0% | 12.148–13.456 | 7.857–10.209 |
| 1 producer / 4 consumers | 4 | 13.476 | 7.339 | -45.5% | 11.593–14.453 | 7.092–8.793 |
| 4 producers / 4 consumers | 1 | 10.901 | 4.510 | -58.6% | 10.717–10.910 | 4.393–4.517 |
| 4 producers / 4 consumers | 2 | 13.085 | 9.835 | -24.8% | 12.415–13.213 | 8.695–9.946 |
| 4 producers / 4 consumers | 4 | 15.310 | 11.328 | -26.0% | 15.265–15.389 | 9.875–11.356 |
| Messaging benchmark | 1 | 26.277 | 16.606 | -36.8% | 25.373–27.259 | 15.753–16.902 |
| Messaging benchmark | 2 | 22.222 | 17.537 | -21.1% | 21.727–23.291 | 16.614–17.561 |
| Messaging benchmark | 4 | 24.268 | 17.960 | -26.0% | 23.690–24.436 | 16.784–21.054 |
| CPU tasks | 1 | 676.545 | 673.002 | -0.5% | 675.267–678.577 | 672.992–679.636 |
| CPU tasks | 2 | 340.761 | 339.621 | -0.3% | 334.575–344.452 | 339.555–343.211 |
| CPU tasks | 4 | 172.256 | 172.120 | -0.1% | 171.538–172.768 | 171.313–321.140 |
| Four independent channel pairs | 1 | 11.062 | 8.053 | -27.2% | 10.813–12.934 | 7.889–8.482 |
| Four independent channel pairs | 2 | 7.350 | 10.911 | +48.4% | 7.007–7.518 | 10.288–11.449 |
| Four independent channel pairs | 4 | 5.917 | 8.956 | +51.4% | 5.173–5.981 | 5.447–10.071 |
| Allocation/list updates | 1 | 35.719 | 63.407 | +77.5% | 35.323–35.775 | 61.762–104.620 |
| Allocation/list updates | 2 | 30.445 | 67.484 | +121.7% | 29.955–30.482 | 64.860–71.295 |
| Allocation/list updates | 4 | 26.750 | 68.815 | +157.3% | 24.613–27.312 | 68.809–80.703 |

## Messaging components

Prompt capacity is 256. Batch capacity is 4096 with maximum batch 256. The final component is a request/reply round despite the fixture's `ns/message` label. Values are medians of printed integer nanoseconds.

| Operation | Workers | Previous ns | Current ns | Time change | Previous range | Current range |
| --- | ---: | ---: | ---: | ---: | --- | --- |
| prompt channel (capacity 256) | 1 | 52 | 20 | -61.5% | 52–54 | 20–20 |
| batch channel (max_batch 256) | 1 | 50 | 41 | -18.0% | 48–55 | 38–43 |
| prompt round trip | 1 | 258 | 193 | -25.2% | 258–278 | 190–203 |
| prompt channel (capacity 256) | 2 | 48 | 23 | -52.1% | 47–55 | 22–24 |
| batch channel (max_batch 256) | 2 | 32 | 37 | +15.6% | 31–33 | 32–41 |
| prompt round trip | 2 | 289 | 254 | -12.1% | 280–293 | 229–260 |
| prompt channel (capacity 256) | 4 | 51 | 24 | -52.9% | 50–51 | 24–26 |
| batch channel (max_batch 256) | 4 | 38 | 39 | +2.6% | 38–39 | 35–50 |
| prompt round trip | 4 | 300 | 236 | -21.3% | 282–308 | 222–295 |

## Global lock instrumentation

Counts below are from one separate instrumented four-worker run per case. Scheduling can change counts; sampled hold totals are not interpreted as percentages of elapsed time.

| Case | Previous global acquisitions | Current global acquisitions | Previous channel-slow acquisitions | Current channel-slow acquisitions |
| --- | ---: | ---: | ---: | ---: |
| 1:1, capacity 1 | 133,369 | 133,372 | 133,336 | 133,336 |
| 1:1, capacity 1024 | 421 | 424 | 391 | 390 |
| 1:1, 512-byte budget | 6,091 | 6,094 | 6,061 | 6,060 |
| 4 producers / 4 consumers | 825 | 702 | 768 | 650 |
| Messaging benchmark | 45,665 | 45,630 | 41,553 | 41,551 |

## Current scaling

| Workload | 1 worker ms | 2 workers ms | 4 workers ms | Speedup, 1 to 4 workers |
| --- | ---: | ---: | ---: | ---: |
| CPU tasks | 673.002 | 339.621 | 172.120 | 3.91× |
| Four independent channel pairs | 8.053 | 10.911 | 8.956 | 0.90× |
| Allocation/list updates | 63.407 | 67.484 | 68.815 | 0.92× |
| 4 producers / 4 consumers | 4.510 | 9.835 | 11.328 | 0.40× |

Main at end of current measurement: `ec2b13e9e319b87d86e2ecc7a3412809311d3085`. Production topology specializations and queue-only native prototypes were not changed or benchmarked by this task.
