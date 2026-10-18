# G8b: diagnostic allocation paths and pool churn

Retain diagnostic counters (`ec0ad88`) and optional report columns. Ordinary
allocation policy and initialization are unchanged. G8a's clear specialization
was reverted before these builds. This slice measures allocation sources; it
claims no ordinary-build speed or memory improvement.

## Scope and validation

With `gc-profile` and `BADGER_ALLOC_PROFILE`, classify each successful managed
request immediately before cell allocation: worker-local free-list reuse,
fresh pooled bump space, or the system allocator. The worker owns its pool and
no safepoint or intervening allocation separates classification from allocation.
Count actual successful pool refills and retired chunks, with reserved bytes as
the difference. Reusing one cell ten times counts ten allocation requests.

All object/requested-byte/cell-byte source totals conserve the existing census;
the system group must exactly match large-system-cell accounting. Checks reject
inconsistent partitions and pool event totals, while accepting older censuses
without optional fields. Native buffers, task stacks and other malloc calls are
outside these new counters. Pool reservation is neither RSS nor live bytes.
Survival does not prove escape, uniqueness or safe arena ownership.

Validation: 76 program tests (including multicore/saved MIR), 13 diagnostic and
11 ordinary runtime unit tests, three runtime integration tests in each mode,
strict runtime/CLI all-target Clippy in both modes, formatting and 25 Python
harness tests pass. Sixteen retained census stress checks cover generational
modes 0/1, workers 1/2/4/8, verification/quarantine, chaos, map updates and string
pool boundaries. Ordinary builds emit no census. Optional reporting reproduces
the older G1 coverage report byte-for-byte. A first program-test invocation
selected the wrong Cargo package set and failed before tests; its log is kept
alongside the corrected passing invocation, not counted as test execution.

## Measurements

[Coverage paths, lifetime census and calibration](G8b-paths/allocation-summary.md):
336 process observations, seven cases, workers 1/2/4/8, three repetitions of
ordinary, diagnostic-off, phase-only and phase-plus-census variants. The harness
also performs discarded warmups, retained neither as measured observations nor
in these totals. [Timing ranges](G8b-paths/summary.md) and compressed raw report
remain. No outlier removal. Diagnostic code layout and census storage/locking
perturb time: group-median diagnostic-off changes versus ordinary range from
-24.8% to +10.8%; census versus phase-only from -8.0% to +91.7%. Do not use
instrumented speedups or CPU fractions as ordinary performance claims.

[Broader application census](G8b-applications/allocation-summary.md): 30
processes, five cases, workers 1/8, three repetitions, three warmups/five measured
inner iterations plus startup/output; messaging uses eight fixed consumers.
All independent result checks pass. New ordinary/diagnostic immutable binaries,
source hashes, commands, intervals and validation artifacts are recorded in
[G8b-binaries.json](G8b-binaries.json). The 366 measured process intervals are
disjoint from each other and the preceding G8a measurements; other actors on
the shared host are uncontrolled.

| Shape | Pooled reuse, objects | System allocation, cell bytes | Interpretation |
|---|---:|---:|---|
| Record-window coverage | 87.3% | 10.7% | Reuse/initialization is relevant; allocation elimination could save more. |
| Parallel snapshots | 64.0–70.4% | 56.1% | Substantial native allocator traffic remains alongside recycling. |
| Compare records | 54.2% | 14.5% | Managed records dominate requested bytes (83.0%). |
| Compare snapshots | 0.0% | 0.4% | Short block uses almost entirely fresh bump space. |
| Reconcile | 67.9% | 16.9% | Managed Order records dominate requested bytes (78.7%). |
| Realworld state | 0.0% | 76.2% | MapNode dominates requested bytes (98.3%); mostly native allocator traffic. |
| Messaging | 0.0% | 0.0% | Few allocations and no recycling in this block; allocator changes cannot explain all worker ceilings. |

Compare snapshots allocate 12.14 MiB and state 14.77 MiB in this short census;
no observed collection/reuse is not proof of long-lived data. Parallel snapshots
allocate about 602.5 MiB and do collect, covering a gap in short application
blocks. Worker count does not parallelize serial state/record loops. Parallel
snapshot ending pooled reservation varies substantially (median 18.25–54 MiB
across worker settings), so benchmark any expanded pool against retained memory,
not only allocator throughput.

## Next allocator experiment

Try an explicit unsafe fully-initialized allocation contract for HAMT nodes.
Both node construction and splice paths write the entire declared payload
without another allocation/safepoint. Audit all call sites, tracing/publication,
padding, failure behavior and builder capacity first. Preserve the public
zeroed allocator contract and all other object types. This could avoid redundant
clearing on both recycled and system cells without expanding pool reservation.
Measure ordinary state, snapshots, records, byte-heavy controls and parallel
coverage before deciding whether to retain it. Larger pools remain an option,
with an explicit memory tradeoff; no ownership/region inference is added here.
