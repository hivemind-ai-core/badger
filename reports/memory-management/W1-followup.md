# W1 baseline: fixed task topology and messaging attribution

No scheduler, channel-lock or GC policy change is implemented here. Production
is C1 `b080078`. The new `--consumers 8` option in the ordinary application and
CPU-profile runners fixes eight consumer tasks plus producer/collector while
runtime workers vary. Original defaults and published application contracts stay
unchanged. The checks independently validate checksums and record the actual
runtime-worker and consumer arguments.

## I5b source-run regression remains open

The prior I5b comparison is `38855c7` versus `49f281c`; C1 is not part of it.
All rows below use 15 randomized adjacent process pairs, five warmups and fifteen
measured kernels at two workers. Median and mean are one observation per process.
Keep cohorts separate; ratios cannot be subtracted to infer a causal effect.

| Cohort | Paired median change | Paired block-mean change | Faster median pairs |
|---|---:|---:|---:|
| Identical pre-I5b MIR, old/new native executable | -3.2% | -2.8% | 12/15 |
| Identical pre-I5b MIR, same new executable control | -0.5% | -0.1% | 10/15 |
| Fresh source execution, old/new compiler/runtime | +13.3% | +11.1% | 2/15 |

[Native comparison](I5b-native-mir/summary.md), [control](I5b-native-mir-control/summary.md),
[source repeat](I5b-messaging-source-repeat/summary.md). The original same-native
old/new saved-MIR comparison also stayed near its control. Neither saved-MIR
study reproduces the full source regression; neither proves it absent. Generated
code/data layout, frontend/JIT heap state and scheduling sensitivity remain
hypotheses. Preserve the unfavorable source result and investigate source versus
loaded-MIR execution before blaming ownership epochs or declaring no regression.
The exact commands, frozen program hash, binary hashes and all outputs remain.

C1's separate four/eight-worker control repeats are recorded in
[C1-summary.md](C1-summary.md); they concern a different native change and longer
warmup. Do not merge the two attribution questions.

## Fixed-eight-consumer ordinary baseline

Seven fresh processes per cell, 15 warmups/15 measured kernels; no candidate
variant is compared in this baseline. Messaging processes 65,539 IDs in batches
of 64 with two score rounds; pipeline processes 120,003 IDs with 128 rounds.
Input and logical task graph stay constant within each worker sweep.

| Case | w1 median ms | w2 | w4 | w8 |
|---|---:|---:|---:|---:|
| Messaging | 1.323 | 0.861 | 1.253 | 1.420 |
| CPU-heavy pipeline | 50.104 | 25.338 | 13.045 | 7.454 |

Pipeline scales 6.72x at eight; short-message processing improves at two, then
loses that gain at four/eight. This rules out a universal runtime worker ceiling,
while exposing coordination costs for the cheap kernel. It does not attribute
all lost throughput to one lock. [All 56 checked process observations, means,
ranges and RSS](W1-fixed-baseline/summary.md). A separate four-process smoke
checks fixed consumers at runtime workers one/eight.

## CPU evidence for the next experiment

Fixed-eight-consumer diagnostic profiles: three processes per worker setting,
15 warmups and 1,000 kernels, per-thread CPU sampling at 1 kHz, native frame
pointers and collector diagnostics. Includes startup within runtime/warmups;
excludes frontend/JIT. Profiling perturbs execution; the following are CPU sample
shares, not blocked wall time or ordinary-build speedups. Inclusive shares overlap.

| Runtime workers | Median CPU samples/process | Worker loop inclusive | Condvar wait_timeout inclusive | syscall self | PAUSE self | GC collect inclusive |
|---|---:|---:|---:|---:|---:|---:|
| 1 | 1,038 | 0.0% | 0.0% | 0.0% | 0.0% | 2.6% |
| 2 | 1,278 | 14.5% | 1.5% | 2.5% | 2.6% | 2.4% |
| 4 | 2,728 | 30.6% | 15.6% | 22.5% | 15.3% | 1.3% |
| 8 | 4,441 | 49.8% | 31.2% | 35.5% | 10.9% | 0.8% |

[Fixed topology stacks and counts](W1-fixed-profile/profiles.json.gz). The earlier
[variable-consumer profiles](I5b-messaging-profile/profiles.json.gz) show the same
qualitative pattern, but vary logical task count as well. Samples in condvar
frames include entering/leaving waits and associated kernel work; sleeping time
does not advance a thread's CPU timer. Do not interpret 31% as time blocked.

In the variable-consumer eight-worker stacks, most PAUSE self samples occur
under `chan_recv_split`; most syscall samples occur under worker condvar waits
or `task::deliver`. Source inspection identifies bounded channel fast-path
backoff, ready-queue locks, a 50-us reserved-next-slot steal delay, worker idle
waits and wake delivery as candidates. The runtime already uses notify_one plus
a WAKING gate on ordinary ready notifications; this is not evidence for a simple
notify_all removal. GC uses a small CPU share in this messaging shape, unlike
record-heavy allocation workloads.

Next add diagnostic-only counts for channel fast-path attempts/spins/fallbacks,
ready notifications, waits returning due to work/timeout, steals and per-worker
work. Evaluate bounded backoff and wake/steal policies with these fixed-topology
cases plus backpressure, fanout, I/O, cancellation and GC rendezvous controls.
Preserve wait generations, parking/wake atomicity, publication, timer deadlines
and progress; do not split locks or lengthen spinning merely from CPU samples.
No GC/arena claim follows from scheduler profiles. Record/leaf-object allocation
and collection remain the next memory-management target alongside this followup.
