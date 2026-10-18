# C1 retained: remove codec temporary allocations

Keep production `b080078`, on top of the retained I5b map epochs. The ordinary
baseline is the immutable `49f281c` executable; the starting main `beb0139` has
identical production code. [Binary/source hashes and dataset audit](C1-binaries.json).

## Fresh profiles and change

Fresh frame-pointer diagnostic profiles show different costs by workload.
In reconciliation, managed allocation accounts for about 33% inclusive CPU and
map reads about 12%; record comparison spends about 65–67% in checked allocation,
including about 35–36% in collection. Those inclusive fractions overlap. About
99.8% of newly marked objects in the record comparison are descriptor-classified
leaves; this is a mark count, not an allocation/escape census.

JSON instead has considerable native work: accepted-name construction is
6.7–8.1% inclusive CPU, lookup 8.5–9.6%, and `alloc::fmt::format` 10.3–12.6%.
GC collection is about 2.9% inclusive in this diagnostic cohort. These are
sampled, instrumented attribution figures, not ordinary-build speed ceilings.
[Original caller stacks](next-profile-callers/profiles.json.gz).

The codec now borrows its rooted String/Bytes input, looks up borrowed current
names and aliases in precedence order, builds paths only while propagating an
error, reserves record/variant scratch vectors once, and appends quoted strings
and integers directly to the output buffer. UTF-8 replacement behavior, escaping,
field/variant order, wire names/alias priority, missing Option fields, validation
order and exact error paths remain. Existing `json_quote` callers keep their API.
Successful decode no longer formats a path for every field/element. The owned
serde_json intermediate tree and managed result objects remain.

[Post-change profiles](C1-profile/profiles.json.gz) contain no accepted-name or
successful-path `alloc::fmt::format` samples; lookup is about 0.5% inclusive.
The remaining serde JSON parse/tree/drop work merits investigation. Optimizer
inlining affects individual symbol attribution; use ordinary timings below for
the size of the improvement, not differences in profile percentages.

## Ordinary results

Seven randomized adjacent process pairs per cell; 15 warmup and 15 measured
kernels per process in the unchanged realworld/compare application contracts.
Each process contributes one median AND one block mean. All outputs are checked
against independent oracles; inner iterations are correlated.

| JSON | Workers 1 | 2 | 4 | 8 |
|---|---:|---:|---:|---:|
| Baseline / candidate median ms | 17.473 / 10.585 | 18.080 / 10.932 | 17.974 / 10.880 | 18.018 / 10.894 |
| Group median change | -39.4% | -39.5% | -39.5% | -39.5% |
| Paired median change | -39.0% | -39.8% | -39.1% | -39.7% |
| Paired block-mean change | -39.8% | -40.4% | -39.6% | -39.0% |

All seven JSON pairs are faster at each worker count, by both statistics.
Other application controls are mostly small/mixed; preserve all 504 processes
and the [full application tables](C1-applications/summary.md).

New checked coverage, measured before edits and again in 224 ordinary paired
processes at workers 1/2/4/8:

| Coverage | Group median reduction across workers | Faster pairs per cell |
|---|---:|---:|
| Flat record decode | 30.4–36.8% | 7/7 |
| Nested lists/maps/tuples/packed arrays/Option | 46.1–49.9% | 7/7 |
| Renamed field with legacy alias | 46.5–47.7% | 7/7 |
| Invalid nested record, exact error path | 22.7–24.4% | 7/7 |

[Before-edit baseline](C1-baseline/summary.md), [full coverage and paired ranges](C1-coverage/summary.md).
These are serial kernels; varying runtime workers does not make them parallel.

## Allocation, memory and controls

Across all three census repetitions and workers one/eight, baseline and candidate
have exactly the same process-wide managed object and cell-byte totals:

| Workload (eight inner iterations plus startup/output) | Objects | Cell bytes |
|---|---:|---:|
| JSON | 576,265 | 40,132,939 |
| Reconcile | 902,239 | 70,586,320 |
| Compare records | 1,603,420 | 89,865,136 |

[36 perturbed census processes](C1-census/measurements.json.gz). Requested-byte
counts can differ by a few bytes as printed timing integers change length;
object and rounded cell totals are invariant. No native-allocation count or peak
live-heap reduction is measured. The source removes temporary native allocations;
this is **not a managed-GC allocation reduction or GC bypass**. Codec coverage has
0→0 collections (nested 1→1); JSON peak process RSS is about +0.4–0.5%, and most
other controls have small increases too. Keep those costs, including compiler/JIT
memory, rather than claiming lower RSS.

Messaging/four in the initial matrix is +18.5% paired median/+6.3% block mean;
eight is +8.9%/+4.0%. The [15-pair repeat](C1-messaging-repeat/summary.md) shrinks
these to +2.2%/-1.7% at four and +2.4%/+3.3% at eight. Its
[same-binary control](C1-messaging-control/summary.md) is +4.7%/+4.6% at four and
-3.8%/-2.7% at eight, with wide ranges. The large four-worker slowdown does not
repeat, but small code/layout tradeoffs remain possible, especially at eight.
No universal no-regression or memory improvement is claimed.

## Competitive check and worker followup

A separate fresh official JSON cohort uses five processes/language, 15 warmups
and 15 measured iterations. All 20 performance and 16 validation processes pass.

| Language | Kernel median ms [process-median range] | CV |
|---|---:|---:|
| Badger | 10.507 [10.281, 10.591] | 1.1% |
| Go | 13.980 [13.938, 14.621] | 1.8% |
| Node | 6.511 [6.460, 6.532] | 0.5% |
| Java | 10.592 [10.211, 11.664] | 4.9% |

Badger is close to Java in this kernel; the small difference is not a reliable
rank. Go's kernel is slower here; Node remains substantially faster. This is not
a general language ranking, cold-start result or proof of JIT convergence.
Badger's process RSS is 61.4 MB versus Go's 11.8 MB, so faster does not mean lower
memory. [Complete tool versions, commands, builds and samples](C1-realworld/2026-10-18T19-03-34.411447Z-b080078a8e55.json).

The prior I5b two-worker source-run regression remains unresolved: a fresh
15-pair comparison reproduces +13.3% paired median/+11.1% block mean. Holding
old saved MIR fixed across native binaries gives -3.2%/-2.8%, while the prior
same-native old/new-MIR study was close to its control. These separate cohorts
do not isolate source/JIT layout or frontend heap-state effects. Keep the
unfavorable source result. [Detailed attribution and fixed-task baseline](W1-followup.md).

## Validation, provenance and next work

All 76 program tests pass, including source/release/multicore/saved-MIR corpora.
Ordinary runtime: 11 unit tests and three integration tests; diagnostic: 13 plus
three. Strict runtime/CLI all-target Clippy in both modes, workspace formatting,
25 Python harness tests and 224 focused codec checks pass. The latter check
source and old/new MIR with collection before allocation, generational/full GC,
verification/quarantine and ordinary reuse, chaos and workers 1/2/4/8. Include
retained nested pointer values, dynamic text/bytes inputs, lossy UTF-8, packed
scalars, aliases and exact error paths. [Checker and logs](C1-validation/).

The audit retains **1,168 measured processes in 16 nonoverlapping datasets**,
including diagnostic profiles, source/native attribution, same-binary controls
and fixed-topology baselines. No outliers are removed. Ordinary measurements
follow completed builds/tests. The separate official cohort lacks absolute child
intervals and is excluded from that interval audit. Initial sandbox profiles are
leaf-only because self `process_vm_readv` returned EPERM; retain them separately.
Caller profiles were rerun with that permission, and the harness now checks it
before sampling. One failed short fixture frontend check overlapped the initial
caller-profile cohort; those perturbed samples are retained. No ordinary paired
measurement overlaps that check. Sampling is 1 kHz of per-thread CPU, with native
frame pointers, including fixtures/warmups/main execution but excluding frontend/JIT.

Next prioritize G8 allocation/leaf-object costs in record workloads and W1
channel spin/worker wake/park costs, using the new fixed-eight-consumer baseline.
Do not loosen publication/GC synchronization or skip initialization without a
proof. Generic scalar map read specialization, packed records, bounded marking,
interprocedural ownership and task/transfer arenas remain open. No arena or
collector-algorithm change is implemented by C1.
