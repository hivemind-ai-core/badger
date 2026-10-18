# G7e0: trace-work baseline

Diagnostic counters at `4814c06`; ordinary collection algorithm remains G7d.
108 measured fresh processes: three cases, workers 1/2/4/8, three repetitions,
ordinary / instrumented-off / instrumented-on, randomized adjacent groups.
All independent checks and per-collection counter conservation checks pass.
Raw samples and provenance are in G7e-trace-baseline; binary hashes are in
G7e-binaries.json. The reusable analyzer writes the full calibration/count tables.
No measured process overlaps another in this dataset; own builds/tests preceded it.

For eight fixed pointer-snapshot tasks, diagnostic medians are:

| Workers | Collections | Pointer candidates | Newly marked | Traced objects | Trace ms |
|---:|---:|---:|---:|---:|---:|
| 1 | 37 | 2,692,422 | 460,053 | 145,560 | 23.317 |
| 2 | 37 | 2,618,841 | 439,871 | 140,555 | 23.069 |
| 4 | 36 | 3,710,277 | 629,063 | 199,010 | 36.555 |
| 8 | 33 | 5,460,062 | 797,674 | 284,404 | 66.348 |

The eight-worker schedule does about twice the candidate/trace work, despite fewer
collections: more overlapping live graphs affect collection cost. A ratio of
phase time to counts cannot isolate memory latency, lock contention, or per-object
cost; graph/locality/collection differences and instrumentation remain. Roots
and remembered tracing are separate phases; trace counts include remembered
parents. Counts are per-process totals, not unique objects across collections.

Records-window has identical 51,229 candidates and 24,202 traced objects at every
worker count, about 0.41–0.45 ms trace. Scalar snapshots similarly have 33,357
candidates and 25,574 traces, about 0.57–0.68 ms. Those serial workloads do not
supply worker-scaling evidence.

Instrumentation is material: ordinary vs diagnostic-off/on pointer snapshot work
is 246.1/258.9/259.8 ms at one worker and 397.2/398.7/402.4 ms at eight. Record
work spans 121–124 ms ordinary vs 126–134 ms diagnostic-off; scalar snapshots
10.9–11.7 vs 11.3–11.6 ms. Three repetitions are calibration, not a precise
estimate of instrumentation overhead. Use ordinary paired binaries for wins.

Validation before measurements: 12 diagnostic runtime tests, 23 Python harness
tests, strict ordinary/diagnostic runtime Clippy. Full application/GC stress suites
were run for G7d; these additional counters do not change ordinary marking.

Batched marking is still only a proposal. Next prioritize I5b allocation reduction
on real scalar snapshots; the longer-warm comparison still shows state at 4.264 ms
versus Go 2.085, Node 2.170 and Java 1.126. See the separately saved G7d-realworld
report for the protocol and limitations; do not compare unlike warmup cohorts as
an optimization delta.
