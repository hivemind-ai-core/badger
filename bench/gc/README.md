GC/application attribution and coverage
=====================================

Managed allocation census (G1a)
------------------------------

The diagnostic build also accepts `BADGER_ALLOC_PROFILE=path.json`, independently
of phase profiling. It records process-wide managed allocations by type descriptor
and Rust runtime call site, plus small-pool size classes. Requested bytes include
the header; cell bytes include small-pool rounding or the large allocation layout,
but exclude allocator bookkeeping and reserved unused pool space. Counts separate
reclamation before any survival, first survival, later reclamation, and remaining
unswept objects. Minor collections leave the old prefix unexamined. Repeated full
survivor observations are not distinct objects. Quarantined objects count as
reclaimed when removed from the object list, even while their cells remain reserved.

```
python3 bench/gc/run.py --baseline /tmp/base/badger --diagnostic /tmp/diag/badger --allocations --suite coverage --workers 1 2 4 8 --reps 3 --out /tmp/allocation-census
```

This adds `diagnostic-allocations` beside ordinary, diagnostic-disabled and
phase-only runs. Every census must conserve objects, requested bytes and cell
bytes across sites, size classes, reclamation and remaining metadata. The gzip
report retains the full per-site census. Attribution covers startup and output
encoding as well as the workload; it cannot be filtered using GC collection
indices. It excludes immortals, external buffers, task stacks and other native
allocations. Runtime call sites are not language source locations. Survival does
not prove task escape or unique ownership. Side metadata, counters and mutexes
perturb both timing and RSS: use the ordinary variant for performance claims.
`object_metadata_capacity_bytes` reports just the side-vector capacity; site tables,
output encoding and other diagnostic native storage add further overhead.
`type_id` is process-local: generic instantiations can share a display name, so
use the id to distinguish descriptors within a report and names for family-level
summaries across processes.
Counters flush at the first profiler finish, one worker at a time. Early process
exit while workers still run does not produce a coherent global heap snapshot.

To repeat the stress/verification checks and summarize saved measurements:

```
python3 bench/gc/check_allocations.py --ordinary /tmp/base/badger --diagnostic /tmp/diag/badger --out /tmp/census-checks.json.gz
python3 bench/gc/analyze.py /tmp/allocation-census
python3 bench/gc/analyze_allocations.py /tmp/allocation-census
```

`coverage.bdg` adds construction-inclusive byte/word buffer churn, a larger
survivor window, persistent-map snapshot histories, and equivalent visible,
stored, collected and eager pipelines. Checksums include retained data. The
Python oracle independently checks every workload; telemetry reuses its full
payload/routing/cache oracle. Timings include setup, mutation and consumption,
so they complement the earlier read-only microbenchmarks.

Build an ordinary fixed-revision compiler and copy it to an immutable path.
Build a separate diagnostic compiler (frame pointers and symbols matter):

```
CARGO_TARGET_DIR=/tmp/gc-diagnostic-target CARGO_PROFILE_RELEASE_DEBUG=1 RUSTFLAGS='-C force-frame-pointers=yes' cargo build --offline --release -p badger_cli -p badger_runtime --features badger_runtime/gc-profile
```

`gc-profile` adds per-collection phase timings with `BADGER_GC_PROFILE=path.json`.
It records stop-world rendezvous, preparation, root enumeration, tracing,
weak-page clearing, verification, sweep/finalization, accounting, pool trimming
and finishing. Counts include examined objects, survivors, remembered objects,
external pressure/live bytes and pool reservation. Phase timings are disjoint
wall-clock intervals; parallel sweep CPU can exceed its wall time.

Only that feature accepts experimental `BADGER_GC_MINOR_BYTES`,
`BADGER_GC_MAX_MINORS` and `BADGER_GC_TRIM=0`. They are attribution tools, not proposed
defaults. The young-budget override applies after the first collection. Normal
builds contain neither instrumentation nor these policy controls. Diagnostics
are buffered and written at exit. `BADGER_PROFILE_SCOPE=runtime` excludes frontend
and JIT samples; its default retains existing whole-process profiling. With
sampling active, diagnostic sweep helper threads are sampled too.

`BADGER_GC_POOL_MAX=256|512|1024` is another diagnostic-only experiment. It fixes
the pooled cell limit for the process, including headers and size-class rounding.
Allocation and reclamation always use the same limit. Compare it with
`--suite policy --policies pool256 pool512 pool1024`; the runner verifies the
reported policy to reject a binary that does not support this setting.
Allocation censuses also include requested-size buckets through 1,024 bytes,
independent of which limit is pooled. Histogram volumes conserve the same
objects, requested bytes and actual cell-layout bytes as the other counters.

`BADGER_GC_SWEEP_REUSE=0|1` compares scoped and cached native sweep helpers in a
diagnostic build (`--suite policy --policies sweep-scoped sweep-reuse`). The
profile records per-worker work, dispatched helper work, and newly created native
threads separately. The runner checks conservation; `analyze.py` summarizes skew
and small jobs over workload collections. These counters do not directly time
thread creation. All helper results are joined/received before serial finalizers
or worker resumption; profiler/process finish closes the cache and joins helpers.
Later collections use scoped helpers. The cache is bounded by `MAX_WORKERS` and
created lazily, but idle native stacks/channels consume resources until finish.
Ordinary builds retain scoped helpers. Reuse was rejected as a default after
mixed application results and a repeated CPU-heavy slowdown; the cache and its
shutdown hook compile only for diagnostics/unit tests. See `G3-summary.md` in
`reports/memory-management/` for the full decision and retained measurements.

Before timing an ordinary scoped/cached comparison on Linux, verify actual policy:

```
python3 bench/gc/check_sweep_reuse.py --baseline /tmp/base/badger --candidate /tmp/candidate/badger --out /tmp/sweep-check.json
```

This validates independent final-map contents and GC activity, then counts named
cached helpers through procfs while still inside the language program. It detects
accidental Cargo artifact reuse independently of executable labels/hashes.
Use `--candidate-policy scoped` when checking the restored ordinary default.

```
python3 bench/gc/run.py --baseline /tmp/base/lang --diagnostic /tmp/diag/lang --suite telemetry --out /tmp/gc-telemetry
python3 bench/gc/run.py --baseline /tmp/base/lang --suite coverage --out /tmp/gc-coverage
python3 bench/gc/run.py --baseline /tmp/base/lang --diagnostic /tmp/diag/lang --suite policy --out /tmp/gc-policy
python3 bench/gc/run.py --baseline /tmp/base/lang --diagnostic /tmp/diag/lang --suite samples --workers 1 4 --reps 1 --out /tmp/gc-samples
python3 bench/gc/run.py --baseline /tmp/base/lang --diagnostic /tmp/diag/lang --suite smoke --workers 1 4 --reps 1 --out /tmp/gc-smoke
python3 -m unittest discover -s bench/gc -p 'test_*.py'
```

Each cell gets a warmup followed by shuffled fresh processes; the runner clears
inherited BADGER settings, fixes parser topology, saves binary/source hashes and
raw runs in gzip JSON, and fails on oracle errors or timeout. Oracle outputs
are cached beside output directories by input and oracle-source hash; delete
the `oracles` directory to regenerate them. `baseline` and
`diagnostic-off` calibrate diagnostic build overhead; `diagnostic-off` versus
`diagnostic` additionally calibrate phase counters. Sampling plus lock statistics
is a separate perturbed diagnostic suite, not a throughput comparison. The
ordinary timing matrix contains no sampler or lock-stat overhead.

CLI CPU/RSS includes compilation; language elapsed time excludes it. GC phase
files span runtime execution, including configuration and output encoding;
filter collection indices to `(heap_before.collections, heap_after.collections]`
for workload attribution. CPU samples sum CPU across workers, whereas pauses
are wall time. They must not be added or treated as interchangeable percentages.
Worker sweeps keep eight parser tasks fixed. Shared-host runs are directional;
publish ranges and memory costs alongside policy comparisons.

Executable hashing streams through a bounded buffer. Reading the whole debug
binary into Python can inflate fork/exec RSS high-water marks; such runs must
not be used for memory comparisons.

`rss_spawn.c` is built with the system C compiler into the output parent. It
forks the language from a small clean executable and reports that child's
`wait4` RSS, excluding the Python runner's memory high-water mark. The original
launcher-tree value is retained separately. This matters once large debug
binaries or accumulated JSON reports have grown the Python process.

Memory-management progress also covers fresh-key and replacement-heavy map
builders, pointer-valued maps, set builders, and producer/consumer payloads.
The message cases include capacity-one backpressure with a consumer that yields
while holding the payload, bounded retention past dequeue, and immutable fanout.
Every payload word contributes to the independent oracle. These use ordinary
channels as the baseline; no ring arena or topology specialization is implied.
Map cases execute on one task even when the runtime has several workers.
`parallel-maps` and `parallel-snapshots` add eight fixed independent allocating
tasks across every worker count. The snapshot case checks every retained string
against the insertion step of its captured version after all churn, so a correct
root length cannot hide prematurely reclaimed descendants. Setup, task joining
and retained-data validation are included in workload time.
`persistent-64`, `persistent-256` and `persistent-4096` replace integer keys in
ordinary persistent maps. They complement builder-only and string/snapshot
cases and isolate path-copy churn at different trie shapes. Final validation
checks every key and its last inserted value.
The `persistent-64-major`, `persistent-4096-major` and `snapshots-major` cases
extend allocation volume to exercise major collection and pool retirement, which
short runs can miss. Retained snapshots also expose pool fragmentation costs.

`messages-int`, `messages-int-slow`, `messages-int-retained` and
`messages-int-fanout` mirror the original `U64` message workloads using signed
machine words. Payload width, topology, retention, suspension and checksum are
the same; the new cases avoid per-word checked numeric conversions in generation
and validation. Keep both families: conversion-result boxing can dominate the
managed allocation count in the original cases and obscure payload-storage gains.
The four `messages-int*-sustained` cases send 262,144 messages, versus 16,384 in
the matched controls. Use these to exercise collections after removing conversion
churn; the smaller controls can finish without collecting at all.

Use `--cases map-new map-replace map-strings set-builder` to select coverage.
For before/after comparisons, copy binaries before rebuilding and run:

```
python3 bench/gc/run.py --baseline /tmp/base/lang --candidate /tmp/candidate/lang --baseline-revision BASE_SHA --candidate-revision CANDIDATE_SHA --suite coverage --workers 1 2 4 8 --reps 5 --out /tmp/memory-comparison
```

The candidate is an ordinary build, separate from the diagnostic variants.
Revision labels describe the binaries; the report also records the harness
revision, source hashes, host/toolchain, raw ranges, and executable hashes.
Capture a baseline-only run before editing production code, then compare both
binaries interleaved after each slice. Peak RSS includes frontend/JIT and is
not a live-heap or allocation-volume measurement. Never reuse output directories
for a different experiment.

When sharing a Cargo target between worktrees, force a rebuild of changed packages
before copying each executable. Older source mtimes can otherwise reuse another
worktree's artifacts. Record the build log and compare executable hashes. For the
256/512-byte pool experiment, also verify allocator behavior before timing:

```
python3 bench/gc/check_pool_limit.py --baseline /tmp/base/badger --candidate /tmp/candidate/badger --out /tmp/pool-check.json
```

This retains strings with 288/304-byte cells below the first collection threshold
and checks their pool reservation, independently of binary labels or hashes.
