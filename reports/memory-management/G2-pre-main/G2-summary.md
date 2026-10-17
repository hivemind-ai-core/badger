# G2: pooled cells for common HAMT nodes

Work remains on `perf/memory-management-2026-10-17`. Main and its ongoing edits
are untouched by this slice. [Binary provenance](G2-binaries.json) pins immutable
ordinary baseline/candidate compilers and the separate diagnostic experiment.

## Evidence and implementation

G1's phase data puts sweep/finalization at about 64–94% of collection wall time
in the application cells inspected. HAMT nodes dominate application managed
allocations. A dense CHAMP root has a 16-byte GC header, 16-byte node header and
32 child pointers: 288 bytes, just above the previous 256-byte pool cutoff.

A validated one-worker balanced census (65,536 events; diagnostic counters only)
is retained in `G2-validation/balanced-size-census.json.gz`. Requested-byte shares:

| Requested cell bucket / proposed cutoff | Share of managed requested bytes |
|---|---:|
| 288-byte rounded bucket alone | 24.8% |
| At most 256 bytes | 44.5% |
| At most 512 bytes | 97.7% |
| At most 1,024 bytes | 99.8% |

These include startup/output allocations and exclude external buffers and other
native allocations. Requested-size buckets preserve actual allocation counts and
bytes while showing how much traffic each pool cutoff would cover.

The candidate raises the constant pool cutoff to 512 bytes, retaining 16-byte
classes in the existing worker-local pools. It changes allocation/reclamation
placement, not map copying, object/header layouts, rooting, collection budgets,
marking, finalizer order or chunk size. Descriptor/header addresses remain stable.
Free-list cells are cleared before reuse; chunks remain eligible for retirement
only when no live or quarantined cell occupies them. Larger classes can retain
more reserved memory between major collections, so RSS and long-run retention
must be evaluated alongside elapsed time and pauses.

Diagnostic builds additionally support fixed process-wide 256/512/1024-byte
limits, independently of the chosen production constant. Those settings and
census hooks do not exist in ordinary builds. `G2-policies/` retains all 240
oracle-checked diagnostic policy measurements (five repetitions, one warmup,
four profiles, 1/2/4/8 workers, 65,536 events). Some ranges are wide and outliers
are retained. These policy timings select a candidate; ordinary comparisons
decide whether to keep it.

## Coverage and correctness

- `G2-baseline/`: before-change persistent integer maps at 64/256/4096 keys;
  ordinary baseline `0d34cee`, harness `55d0a43`, 60 measured runs, five
  repetitions and one warmup at every worker count.
- New persistent integer-map cases separate path-copy allocation from strings
  and numeric conversion results. Their oracle checks every final key/value.
- Extended persistent-map and retained-snapshot cases exercise major collection
  and chunk retirement; short cases can miss both.
- The pool unit test runs in isolated child processes, tests every payload
  size through 1,040 bytes, poisons storage before reuse, and verifies zeroing,
  system/pool boundary handling, quarantine retention and retiring chunks with
  free-list links. Diagnostic limits 256/512/1024 and verification 0/1 pass.
- String boundary regression values cross tasks and remain live while further
  allocations/collections occur. The complete string contents are checked.
- Default runtime tests (7) and diagnostic runtime tests (9) pass, as does strict
  Clippy. CLI programs (62), GC retention (1), and transient/saved-MIR maps (2)
  pass. The corpus includes source, optimized, saved-MIR and multi-worker paths.
- Both 512- and 1024-byte diagnostic policies pass allocation conservation and
  boundary/map stress fixtures in both collector modes at 1/2/4/8 workers.

Validation logs and counters are in `G2-validation/`. No retained throughput
comparison overlaps our compiler builds or correctness test processes.

## Ordinary application results

262,144 events/profile, seven measured repetitions and one warmup per cell,
workers 1/2/4/8. All 224 measured runs pass the independent application oracle.
Negative percentages mean less elapsed/pause time. Peak RSS includes frontend/JIT;
ending pool reservation is an unswept heap snapshot, not a live-byte census.

| Profile | Workers | Elapsed change | GC pause change | Peak RSS MiB baseline / candidate | Ending pool MiB baseline / candidate |
|---|---:|---:|---:|---:|---:|
| backpressure | 1 | +3.2% | -6.7% | 53.1 / 57.5 | 12.2 / 24.2 |
| backpressure | 2 | -0.7% | -4.8% | 55.3 / 61.9 | 12.5 / 24.5 |
| backpressure | 4 | -5.5% | -6.9% | 69.2 / 70.9 | 17.0 / 27.0 |
| backpressure | 8 | -7.1% | -11.3% | 76.2 / 83.6 | 18.0 / 34.0 |
| balanced | 1 | -1.4% | -15.3% | 81.5 / 86.8 | 16.2 / 30.2 |
| balanced | 2 | +5.3% | -7.8% | 87.5 / 99.6 | 18.5 / 36.5 |
| balanced | 4 | -2.2% | -7.4% | 90.5 / 98.7 | 19.0 / 33.0 |
| balanced | 8 | -5.2% | -2.7% | 94.5 / 105.0 | 20.0 / 36.0 |
| cpu | 1 | +0.6% | -10.8% | 78.0 / 80.7 | 14.2 / 24.2 |
| cpu | 2 | +2.1% | -4.2% | 84.8 / 89.9 | 16.5 / 28.5 |
| cpu | 4 | +0.5% | -5.2% | 93.6 / 97.6 | 17.0 / 33.0 |
| cpu | 8 | -8.9% | -14.2% | 123.4 / 130.7 | 30.0 / 56.0 |
| fanout | 1 | -1.9% | -11.5% | 81.3 / 86.8 | 16.2 / 30.2 |
| fanout | 2 | -1.0% | -6.7% | 98.4 / 107.1 | 22.5 / 42.5 |
| fanout | 4 | -7.6% | -9.3% | 111.5 / 121.9 | 27.0 / 51.0 |
| fanout | 8 | -5.1% | -11.4% | 99.5 / 110.7 | 20.0 / 34.0 |

GC pause medians fall in every cell (2.7–15.3%). At eight workers all four
elapsed medians improve by 5.1–8.9%. Other elapsed results are mixed: 11 of 16
cells improve, while five are 0.5–5.3% slower. These shared-host medians do not
establish every small difference as a reproducible effect; ranges are retained.
Peak RSS increases by 1.7–12.1 MiB, making this a latency/allocation tradeoff,
not a memory-saving change. See [full timing/latency ranges](G2-applications/summary.md).
