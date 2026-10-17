# G2c evaluation: smaller global pool refills

Keep the ordinary 2 MiB post-first refill default. Smaller global chunks lower
reservation but introduce broad directional throughput regressions in one-binary
diagnostic comparisons. No smaller-refill ordinary candidate is selected or shipped.
Targeted size-class/page approaches remain unevaluated; G2c remains open.

Committed main `56f5983` was integrated at `1b47a02` before the ordinary baseline.
The normal binary is frozen at `/tmp/badger-G2c-binaries/baseline-1b47a02`.
Benchmark coverage/baselines were committed at `d1c25a2`; diagnostic policies
and unchanged ordinary default at `3a74557`. Raw reports pin binary/workload hashes.
No builds or tests ran concurrently with our benchmark processes.

## Coverage and boundaries

- Fresh ordinary baselines: 24 burst runs and 32 application runs, plus warmups.
- Diagnostic evaluation: 90 coverage runs and 72 application runs, plus warmups;
  one executable, 256/512/2048 KiB refills, workers 1/8, three repetitions.
- Independent checks validate full retained strings after churn, all final map
  entries, every checksum, and the telemetry routing/payload oracle.
- All 54 full-size diagnostic burst runs include a major collection after
  dropping the full burst and before the after-churn checkpoint; see
  [major checks](G2c-validation/provenance-and-major-checks.json).
- After-churn snapshots include subsequent nursery allocation/free-list slack;
  they are not an immediate-after-major resident-memory measurement. Managed
  live bytes are last collection accounting; reserved bytes are not process RSS.
- Idle yields do not force collection. Peak RSS includes frontend/JIT.
- The native allocator test passes all three refill policies, cell cutoffs
  256/512/1024, verification on/off, zeroed reuse and quarantined-cell trimming.
- Diagnostic timing/host noise prevents treating these as ordinary performance
  or tail-latency claims. Keep ranges and outliers. A rejection here avoids
  spending an ordinary candidate comparison on an already unattractive policy.

## Sparse survivors

With 16 survivors out of 65,536 strings, the one-worker after-churn reservation
is 34.25 MiB for 2 MiB refills, 24.25 MiB for 512 KiB and 20.25 MiB for 256 KiB.
Last-accounted managed live data is approximately 0.02 MiB. Peak RSS changes
far less because all variants first hold the full burst. Smaller reservation
does not establish lower GC allocation pressure; cell layouts/counts are unchanged.

## Application policy comparison

Negative elapsed/RSS differences are better; diagnostic medians, three repetitions.
Full ranges are in [coverage](G2c-refill-coverage/summary.md) and
[applications](G2c-refill-app/summary.md).

| Profile | Workers | Refill KiB | Work change vs 2048 | RSS MiB change |
|---|---:|---:|---:|---:|
| backpressure | 1 | 256 | +10.9% | -1.7 |
| backpressure | 1 | 512 | +3.1% | -1.6 |
| backpressure | 8 | 256 | -3.9% | -8.7 |
| backpressure | 8 | 512 | -2.3% | -12.1 |
| balanced | 1 | 256 | +8.1% | -0.7 |
| balanced | 1 | 512 | +9.0% | -0.4 |
| balanced | 8 | 256 | +3.9% | -9.7 |
| balanced | 8 | 512 | +3.3% | -2.8 |
| cpu | 1 | 256 | +8.8% | -1.9 |
| cpu | 1 | 512 | +5.7% | -1.7 |
| cpu | 8 | 256 | +14.0% | -0.0 |
| cpu | 8 | 512 | +15.8% | -2.5 |
| fanout | 1 | 256 | +3.8% | +0.6 |
| fanout | 1 | 512 | +3.4% | +0.6 |
| fanout | 8 | 256 | +2.7% | -3.4 |
| fanout | 8 | 512 | +3.4% | -1.7 |

The backpressure/8 refill256 cell has especially severe variation; no causal
claim is made from it. Broader allocation-heavy controls also slow down. The
2 MiB policy retains huge-page alignment/advice, which smaller allocations lose;
counters here do not isolate page-fault or THP causality.

## Next selection

G5a removes native Buffer/Array metadata allocation rather than changing pool
refill geometry. Every buffer currently owns a managed cell, a separate BufData
Box, and element storage. Embed BufData in the stable cell and preserve the
existing pointer/field offsets; test growing, frozen, pointer-valued, handed-off
and reclaimed buffers. G1b full native/language-site attribution remains open.
