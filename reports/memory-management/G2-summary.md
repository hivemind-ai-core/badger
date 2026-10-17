# G2: common-size pooling on merged main

## Decision

Keep the 512-byte ordinary pooled-cell limit. Dense HAMT roots occupy 288 bytes,
so this moves frequent persistent path copies from system allocation/free into
existing worker-local 16-byte classes. Header layout, rooting, object addresses,
marking, GC thresholds, finalizer order and chunk sizes are unchanged.

This is a throughput/GC-cost improvement with a memory tradeoff. It does not
eliminate managed allocations or reduce collection frequency. Application RSS
medians increase by 1.7–16.4 MiB. CPU-heavy application throughput is essentially
flat in the follow-up; there is no general application tail-latency win.

## Provenance and method

- Latest committed main `09580597ddb62a2db732ec20b70325abacbf5cea` was merged at
  `8596e6d23b3774e71111d275edfa1ab77700d2b5` before these measurements.
- Baseline `e8f91e0a1b42d35816e9154023b016f9e7cfe572` differs from that candidate
  only in `SMALL_MAX` (256 versus 512). Compiler, standard library and other
  runtime fixes from main are identical. Source/binary hashes and the exact diff
  are in [G2-main-binaries.json](G2-main-binaries.json).
- Fixed ordinary release executables, no profiler/diagnostic feature, inherited
  `BADGER_*` cleared, one warmup/cell, shuffled fresh processes and independent
  checksum/payload oracles. Workload elapsed excludes frontend/JIT; process RSS
  includes them and is measured by the clean native launcher.
- Shared-host directional measurements. Keep raw ranges/outliers; a median
  difference alone does not establish statistical significance or attribution.
- Cargo initially reused the baseline executable across worktrees. That copy
  was rejected before measurement, runtime artifacts were cleaned and rebuilt.
  The [behavior probe](G2-main-validation/pool-limit-check.json) confirms
  0.25 versus 6.25 MiB pool reservation for identical retained boundary strings,
  with equal managed requested bytes and no collection.
- [Pre-main experiments](G2-pre-main/README.md) are retained separately. Their
  older source revisions and timing claims are not substituted for this pair.

| Matrix | Workers | Repetitions | Measured runs | Evidence |
|---|---|---:|---:|---|
| Repeated buffer/message controls | 1, 8 | 15 | 180 | [G2-main-controls/summary.md](G2-main-controls/summary.md) |
| Application profiles, 262,144 events each | 1, 2, 4, 8 | 7 | 224 | [G2-main-applications/summary.md](G2-main-applications/summary.md) |
| 16 map/snapshot/buffer/message cases | 1, 2, 4, 8 | 5 | 640 | [G2-main-coverage/summary.md](G2-main-coverage/summary.md) |
| CPU/fanout application follow-up | 1, 2 | 15 | 120 | [G2-main-followup/summary.md](G2-main-followup/summary.md) |
| Sustained Int message follow-up | 4 | 25 | 50 | [G2-main-message-control/summary.md](G2-main-message-control/summary.md) |

All 1,214 measured runs pass their independent oracles. Each directory retains
`measurements.json.gz` with exact arguments, toolchain/host, source/executable
hashes, raw elapsed/GC/RSS values and application latency histograms.

## Application results

Negative changes mean less elapsed or GC time. Values below compare medians.

| Profile | Workers | Work ms base / candidate | Work change | GC time change | RSS MiB base / candidate |
|---|---:|---:|---:|---:|---:|
| backpressure | 1 | 473.9 / 450.1 | -5.0% | -16.0% | 52.8 / 56.4 |
| backpressure | 2 | 340.5 / 321.9 | -5.5% | -14.5% | 59.1 / 63.5 |
| backpressure | 4 | 200.0 / 193.9 | -3.0% | -7.0% | 66.2 / 67.9 |
| backpressure | 8 | 142.5 / 145.4 | +2.1% | +0.4% | 70.9 / 80.3 |
| balanced | 1 | 489.4 / 460.2 | -6.0% | -15.4% | 82.8 / 85.1 |
| balanced | 2 | 387.4 / 334.9 | -13.6% | -14.4% | 92.0 / 101.4 |
| balanced | 4 | 209.7 / 194.4 | -7.3% | -10.4% | 90.8 / 96.4 |
| balanced | 8 | 157.6 / 149.4 | -5.3% | -5.4% | 93.7 / 103.5 |
| cpu | 1 | 923.1 / 927.9 | +0.5% | -11.9% | 79.1 / 80.8 |
| cpu | 2 | 520.5 / 543.8 | +4.5% | -6.8% | 87.2 / 89.4 |
| cpu | 4 | 298.1 / 301.1 | +1.0% | +3.7% | 90.8 / 99.8 |
| cpu | 8 | 330.6 / 333.2 | +0.8% | +0.5% | 123.8 / 128.3 |
| fanout | 1 | 1186.1 / 1291.5 | +8.9% | -6.6% | 83.2 / 88.0 |
| fanout | 2 | 1048.8 / 1039.9 | -0.8% | -10.1% | 99.1 / 109.4 |
| fanout | 4 | 768.3 / 744.7 | -3.1% | -6.1% | 110.7 / 127.1 |
| fanout | 8 | 849.3 / 818.8 | -3.6% | -9.8% | 96.3 / 105.8 |

Balanced elapsed medians improve 5.3–13.6%; backpressure improves 3.0–5.5% at
1/2/4 workers and is 2.1% slower at 8. GC pause medians decrease in 13/16 cells;
the other three are 0.4–3.7% higher. Collection counts are equal within each pair.

The first matrix showed fanout/1 +8.9% and CPU/2 +4.5%, with substantial timing
outliers. The separate 15-repeat follow-up retains those cases and their nearby
controls rather than discarding the original results:

| Profile | Workers | Work change | GC time change |
|---|---:|---:|---:|
| cpu | 1 | +0.4% | -12.6% |
| cpu | 2 | -0.1% | -8.3% |
| fanout | 1 | -0.1% | -12.0% |
| fanout | 2 | -7.2% | -12.3% |

The larger original elapsed regressions do not reproduce in this follow-up.
CPU remains essentially flat even when its GC time improves, consistent with
GC being only part of the workload. This comparison does not attribute the
earlier outliers to a particular host/runtime cause.

Pooled p99 histogram bounds are mixed in the application matrix; some candidate
runs contain long outliers. The follow-up has equal CPU p99 bounds; fanout/1 is
57.344 versus 59.392 ms, fanout/2 is 55.296 versus 53.248 ms (base/candidate).
These results do not establish a universal tail-latency improvement.
Full bucket bounds, sample counts and raw ranges remain in the linked summaries.

## Coverage and memory

- Persistent 64-key maps improve 17.7–27.7% across worker counts; 256-key maps
  improve 1.6–7.7%; 4,096-key maps improve 12.7–17.6%. These isolate ordinary
  path copying from string and checked numeric-conversion churn.
- Longer persistent cases include major collection: 64-key elapsed improves
  5.4–10.5%, 4,096-key improves 1.8–4.3%. They perform 14 and 23 collections,
  respectively; retained snapshots perform 18. These long-run gains are smaller
  than the short-case gains; short cases alone would overstate them here.
- Snapshot elapsed improves 5.8–12.6%; long snapshot elapsed improves 6.3–9.6%.
  Snapshot-major peak RSS is 1.0–1.5% lower, while persistent-map RSS is higher.
- At one worker, ending reserved pools in the long 64/4,096-key cases grow from
  6.25 MiB to 16.25/18.25 MiB. Long snapshots grow from 6.25 to 8.25 MiB.
  These are reserved/unswept snapshots, not live-object byte measurements.
- The 15-repeat one-worker controls are essentially flat: bytes +0.2%, words
  −0.1%, sustained Int messages −0.9%. An initial four-worker message +7.0%
  result was followed by 25 repetitions: −2.2%, GC −5.7%, RSS +0.3%. Small
  control differences remain workload/order sensitive; M3 stays open.

## Validation and next steps

Validation evidence is in [G2-main-validation/](G2-main-validation/). The merged
production candidate passes 7 ordinary runtime tests, 9 diagnostic runtime
tests, 68 distinct CLI/program/retention/builder tests, 9 harness tests, strict
runtime Clippy and formatting. The new boundary-handoff stress regression passes
separately after timing; its 16 combinations cover both collection modes,
1/2/4/8 workers and both language code-generation modes with GC stress, heap
verification and chaos.
Pool unit tests cover every payload through 1,040 bytes, zeroed reuse, native
boundaries, quarantine and removing free-list links before chunk retirement.

G2b is complete as a measured policy choice; G2 remains open. Follow-ups:

1. Bound larger-cell reserves/refill and compare more targeted classes/pages,
   including burst/idle/post-major memory measurements. The blanket size cutoff
   still leaves a substantial RSS cost.
2. Reduce G3 sweep scratch/helper-thread overhead without changing finalizer
   order or stop-world ownership. Measure phases again on the current revision.
3. Reduce allocation itself via proven I5 descendant ownership and G1b native
   buffer attribution before introducing request/message regions. Pooling alone
   provides neither escape analysis nor a safe producer/consumer arena protocol.
