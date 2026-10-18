# G8c evaluated: fully initialized HAMT allocation rejected

Do not retain production prototype `5b91953`. Revert `a9d51d7` restores the
allocator, sweep and production HAMT source exactly to main `27e9653`.
Retain the modern-compiler compatibility checker and Miri-only model sizing
from `9f96361`, plus all measurements and validation evidence. The initialized
allocation contract looked valid in the audited paths, but this implementation
did not establish a broad performance win.

## Mechanism and correctness boundary

An internal unsafe allocator initialized descriptor/meta and left payload bytes
unspecified. Two HAMT constructors then wrote every declared payload word before
any subsequent safepoint, callback or publication. Pooled reuse skipped memset;
large nodes used malloc instead of calloc. Fresh pooled chunks remained zeroed
because other allocation sites require zeroed bump storage. Headers, object
counts, GC debt, thresholds, pool sizes, tracing and object layouts stayed the
same. No ownership, stack allocation, region or GC bypass was implemented.

[Candidate uninitialized-path assembly](G8c-validation/uninit-alloc.asm) confirms
malloc for system cells and no recycled-cell clearing. Its remaining memset is
for a fresh 2 MiB chunk. [Zeroed path](G8c-validation/zeroed-alloc.asm) retains
clearing; [baseline](G8c-validation/baseline-alloc.asm) remains. The ordinary
zeroed allocator grows 1,992→2,005 bytes; the new entry point is 1,955 bytes.
Generic specialization/wrapper attributes and code layout also changed. The
control regression below has not been isolated to one cause; this experiment
does not prove that avoiding redundant clearing inherently slows execution.

Prototype validation passes: 11 ordinary/13 diagnostic runtime unit tests,
three integration tests in each configuration, strict runtime/CLI all-target
Clippy in both modes, 76 program tests and 25 Python harness tests. Poisoned-cell
checks cover payloads 0..1040, mixed zeroed/uninitialized reuse, pool limits and
refills, native cells, quarantine and retirement. An extra checker passes 136
source/saved-MIR GC checks, 28 MIR probes and 36 differential cases, covering
aliases, pointer values, snapshot epochs, generations 0/1, verification 0/1,
chaos and multiple workers.

The first checker invocation assumed the comparison compiler predates snapshot
epochs and failed on its MIR capability expectation, before stress checks. The
retained fix calibrates that compiler on a two-site snapshot probe; candidate
expectations remain strict. Both the initial failure and passing report remain.

The first full Miri collision model was interrupted after about ten minutes;
it is not counted as passing. The revised interpreter-only model retains 24
saved epochs, 128 equal-hash keys (still exceeding the 64-entry metadata boundary)
and maximum owner tokens. Native tests retain 512 keys/3,072 edits. Miri then
passes the complete selected versioned-map model in 441.64 seconds, with its
existing integer/exposed-provenance warnings. This checks executed paths for
uninitialized reads; it is not a strict-provenance or universal safety proof.

## Ordinary measurements

Fixed revision-pinned ordinary binaries, shuffled adjacent pairs, shared host,
15 warmups/15 measured inner application iterations. All independent checksum
checks pass. Each fresh process contributes a median and a block mean; no
outliers are removed. Ranges are retained, not interpreted as confidence bounds.
Application A/B pairs use identical graphs at each worker count; the original
messaging/pipeline suite varies logical consumers with workers and does not
isolate scheduler scaling. Dedicated parallel coverage keeps eight tasks fixed.

| Dataset | Measured processes | Scope |
|---|---:|---|
| [Before-edit baseline](G8c-baseline/summary.md) | 36 | Six applications, workers 1/8, three repetitions |
| [Application matrix](G8c-applications/summary.md) | 504 | Nine applications, workers 1/2/4/8, seven pairs |
| [Coverage](G8c-coverage/paired-summary.md) | 392 | Seven cases, workers 1/2/4/8, seven pairs |
| [Separate diagnostic census](G8c-census/summary.md) | 48 | Four cases, workers 1/8, three pairs; three warmups/five samples |
| [Targeted repeat](G8c-repeat/summary.md) | 240 | Four applications, workers 1/8, fifteen pairs |
| [Same-binary control](G8c-control/summary.md) | 240 | Same repeat contract and candidate on both sides |

All 1,460 measured intervals are disjoint; no own build/test/profile runs overlap
these timings. Other actors on the shared host are uncontrolled. Commands,
source/executable hashes, allocation totals and validation artifacts remain in
[G8c-binaries.json](G8c-binaries.json). Native prototype binaries were built at
`5b91953`; `9f96361` changes only test/checker code. Do not use diagnostic census
timings as ordinary-build speed evidence.

| Repeat | Workers | Paired median | Paired block mean |
|---|---:|---:|---:|
| Records | 1 | +1.0% | +2.1% |
| Records | 8 | -0.5% | -0.4% |
| Compare snapshots | 1 | -1.3% | -1.4% |
| Compare snapshots | 8 | -2.9% | -1.4% |
| JSON | 1 | +0.7% | +0.7% |
| JSON | 8 | +2.5% | +2.2% |
| State | 1 | -1.0% | +0.1% |
| State | 8 | -0.1% | -1.3% |

The initial state medians improve 1.4–3.5%, but repeats stay small/mixed.
Same-binary control paired medians range from -3.0% to +3.5% across these cells;
block means also vary. Do not subtract separate cohorts as a causal correction.
JSON/eight is slower in 13/15 median pairs and 14/15 block-mean pairs; its initial
matrix also regresses. Initial reconcile/eight's -11.7% paired median becomes
only -1.0% on block means, illustrating why costly GC iterations must remain.

Collection-heavy parallel snapshots improve 1.0–3.0% on paired coverage medians,
but scalar parallel snapshots and parallel maps regress 8.9%/8.3% at eight
workers. The transient 256-element Array<U8> control regresses 8.2–14.4% across
workers, slower in all seven pairs at 1/4/8 and six at 2. Buffer and record-window
controls remain small/mixed. These regressions outweigh the narrow wins.

## Allocation and next priorities

Across all 48 diagnostic census processes, baseline/candidate object and cell-byte
totals match exactly for each case: records 1,603,420/89,865,136; snapshots
57,628/12,725,696; state 47,455/15,490,432; reconcile 902,239/70,586,320.
Requested-byte differences are a few bytes from printed timing-number lengths.
This was cheaper initialization, not allocation elimination. Repeat process RSS
medians are modestly lower (0.4–1.8% depending on case/worker); no production
memory reduction or universal latency improvement is retained.

A large system-allocation byte fraction does not establish that clearing
accounts for much CPU time. Before another allocator micro-optimization, obtain
ordinary allocation-call CPU attribution. Prioritize existing I2/I4/I9 work to
remove materialized records/node copies or pack pointer-free values, and G7e
trace-cardinality attribution for collection-heavy workloads. Regions and rings
still require ownership/lifetime proofs. No architectural arena or GC bypass
has been completed by these experiments.

After the production revert, the retained code passes ordinary/diagnostic runtime
tests and strict all-target runtime/CLI Clippy, formatting, 25 Python harness
tests, and the complete 136/28/36 map checker again. The post-revert checker uses
two immutable compilers with the restored production behavior. The 76 program
tests and Miri result above refer to the prototype; they are not relabeled as
new post-revert executions.
