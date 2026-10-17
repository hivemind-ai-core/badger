# G1a: managed allocation attribution

The completed memory-management/I1 work is on main at `0d34cee`. G1a continues
on `perf/memory-management-2026-10-17`. This slice adds measurement and benchmark
coverage; it does not change the default collector or language representation.

## Reproduction and evidence

- [Binary provenance](G1-binaries.json): ordinary compiler from clean `0d34cee`;
  diagnostic compilers from `82e409e` and `a015467`, copied before rebuilding.
- [Original coverage](G1-coverage/summary.md) and
  [allocation attribution](G1-coverage/allocation-summary.md): eight cases,
  1/2/4/8 workers, three measured repetitions and one warmup per cell. Four
  variants separate ordinary execution, diagnostic-disabled, phase-only and
  allocation-census execution. All 384 measured runs pass their independent
  checksum oracle and census conservation checks.
- [Matched message controls](G1-message-controls/allocation-summary.md): another
  384 measured runs across the same variants, worker counts and repetitions.
- [Application measurements](G1-telemetry/summary.md) and
  [application allocations](G1-telemetry/allocation-summary.md): 65,536 events
  in each of four profiles, 1/2/4/8 workers, three repetitions and one warmup
  per cell (192 measured runs). This attribution matrix supplements the earlier
  larger application baselines; it does not replace them.
- [Sustained message measurements](G1-sustained/summary.md) and
  [allocation attribution](G1-sustained/allocation-summary.md): 262,144 messages
  in four conversion-light topologies, all worker counts and variants, another
  192 measured runs. Every ordinary cell performs five collections, unlike the
  smaller controls. Total retained evidence: 1,152 measured oracle-checked runs.
- `G1-validation/` retains runtime unit tests, strict Clippy output, the CLI
  retention/transient-map suites and black-box census checks. Census checks use
  stress, quarantine/heap verification, both collection modes and 1/2/4/8 workers.
  The ordinary binary ignores `BADGER_ALLOC_PROFILE`; the diagnostic census also
  works without `BADGER_GC_PROFILE` or the sampling profiler.

The benchmark runner records hashes of its source, workloads and executables.
The original coverage used the workload before the Int controls were added;
subsequent reports pin the expanded workload. No retained timing run overlaps
our compilation/tests. An initial overlapping trial was discarded. Shared-host
variation remains; raw ranges and all repetitions are retained.

## What the original census shows

Process-wide managed requested bytes include headers, startup and output
encoding. These are not all native allocations or peak resident/live heap bytes.

| Case | One-worker requested MiB | Largest managed allocation family |
|---|---:|---|
| U64 messages | 48.40 | Result: 99.2% |
| U64 message fanout | 80.42 | Result: 99.5% |
| Byte buffers | 5.51 | Result: 72.6%; Buffer: 27.2% |
| Persistent snapshots | 72.87 | MapNode: 84.1%; String: 10.4% |

The message workloads use `U64.try_from(...).unwrap_or(...)` during generation
and use checked conversions again for per-word validation and checksum addition.
This makes them useful conversion/enum benchmarks, but a poor sole baseline for
measuring payload arenas. Matched `messages-int*` cases keep the same 64-bit
payload width, topology and checksums without those conversions. The original
workloads remain available. This is a benchmark comparison, not a compiler
optimization or a claim of application speedup.

At one worker, 90.5% of original message allocation bytes and 94.0% of fanout
bytes are reclaimed before their first survival. These fractions are censored:
unswept tail objects are still counted as remaining, even if already unreachable.
Repeated full-survivor observations do not count distinct objects. No lifetime,
escape or unique-ownership proof follows from these observations.

Most original message bytes use the existing 32-byte small-pool class; cell
rounding adds only about 0.3% to requested managed bytes. Pool-size tuning alone
cannot remove the boxed results. This does not measure native payload-buffer
capacity, allocator bookkeeping, or idle/reserved pool memory.

## Matched controls and sustained traffic

One-worker managed requested MiB, using the same ordinary compiler and matched
message counts/payload widths:

| Topology | Original U64 checks | Int checks | Ordinary collections original / Int |
|---|---:|---:|---:|
| FIFO | 48.40 | 0.40 | 3 / 0 |
| Suspended consumer | 48.38 | 0.38 | 3 / 0 |
| Retained tail | 48.71 | 0.46 | 3 / 0 |
| Fanout | 80.42 | 0.42 | 5 / 0 |

The sustained Int cases restore collection pressure without numeric-conversion
churn. In a one-worker phase-only FIFO run, five collection snapshots account
for 5.89 MiB managed pressure and 74.31 MiB external pressure. These are sums
at collection boundaries, not complete native allocation totals: the final tail
is excluded. Native payload-buffer reuse therefore remains relevant even after
boxed conversion results are removed. One-worker ordinary FIFO takes about
92.9 ms with 8.6 ms of GC pause accounting; sustained fanout takes 113.1 ms with
8.0 ms. At eight workers, fanout takes 289.5 ms with 12.5 ms of GC accounting.
The worsening multi-worker fanout throughput cannot be explained by GC pause
time alone. Source inspection suggests handoff/coordination and allocation costs
as further candidates; this slice does not attribute native CPU time or locks.

## Application attribution

One-worker process-wide census; GC fractions use ordinary workload time and
pause medians. Detailed ranges, p99 bucket bounds and all worker counts are in
the linked application report. These shared-host measurements are directional.

| Profile | Requested MiB | MapNode share | Early reclaimed bytes | GC / elapsed, 1 worker | GC / elapsed, 8 workers |
|---|---:|---:|---:|---:|---:|
| Balanced | 78.39 | 62.5% | 92.2% | 17.2% | 28.2% |
| Backpressure | 78.32 | 62.5% | 95.6% | 16.3% | 26.3% |
| Fanout | 161.93 | 66.5% | 91.9% | 18.4% | 18.7% |
| CPU | 81.04 | 61.9% | 89.7% | 9.1% | 15.8% |

Most node allocation bytes come from `hamt.rs:159` (the persistent path-copy
helper). In balanced one-worker runs, about 96% of that site's bytes are
reclaimed before their first survival. The cache actor updates its persistent
map for every request. I1's fresh-builder optimization leaves these ordinary
persistent inserts unchanged, which explains why its isolated builder gains
need not improve this application's main allocation source. This is a
source-and-census inference, not a before/after persistent-map optimization.

GC remains a material application cost, especially as other work scales out.
The pause fractions exclude mutator allocation/initialization overhead, so they
do not bound the benefit of eliminating allocations. They also do not establish
GC as the sole scaling or latency limit.

## Consequences for the next changes

1. Investigate why checked conversion Results remain boxed across helper calls
   and branches. The current enum replacement requires complete local
   constructors and rejects parameter/escaping groups; inlining is bounded.
   Allocation-site attribution is still needed to connect ABI allocations to
   exact language locations. Do not assume every Result came from a conversion.
2. Prioritize I5's safe reuse of consumed persistent maps, including proof that
   descendants are not shared with snapshots. Combine this with G3's cheaper
   sweep/reclamation path for short-lived nodes; do not infer ownership from a
   low observed survival rate. Keep snapshot and pointer-value correctness tests.
3. Use conversion-light, sustained message traffic as the arena/pool baseline,
   alongside the original cases and the larger application workload. Small
   traffic runs may perform no collections and cannot establish GC improvements.
4. Add accounted native-buffer churn and task/escape observations (G1b) before
   choosing between payload-buffer reuse, task regions and shared segments.
   The current census intentionally excludes those native allocations.
5. Keep ordinary timing comparisons separate from census runs. Diagnostic
   per-allocation locks, counters and metadata can roughly double original
   message-workload time; the raw calibration records that overhead. Diagnostic
   RSS includes side-vector capacity and additional native profiler storage.

G1 remains open overall. G1a is complete; native accounting, language allocation
sites, workload-boundary snapshots and escape classification remain G1b. Ring
arenas and a different collector are not implemented by this slice.
