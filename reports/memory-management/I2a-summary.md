# I2a: local Map/List lookup results

## Decision and scope

Keep final candidate `fd333a5`. It removes Option boxes from Map/List get results whose
entire use is local tag tests, payload reads and proven single-definition copies.
Stored, returned, compared, reassigned and otherwise escaping results retain the
boxed ABI. Optimization requires the existing optimized/inlined pipeline and is
disabled for live dispatch. The array control already avoids result boxes in
baseline MIR; no new array optimization is claimed.

The split call returns the standard Option tag and publishes its payload in
the current task's traced auxiliary slot. The compiler immediately takes and
clears that slot into a typed local, before any other call/poll. Pointer payloads
remain pointer roots; packed values keep their bit representations. A separate
tag preserves some(0), dynamic null and nested Options. Misses return tag zero
without task lookup/publication; the slot is already empty from the preceding
split take. Standard Option tags 0/1 are the same contract used by array lowering.
No GC policy, write barrier, loop polling or general boxed enum ABI changes.

Runtime helpers join the existing unlocked native-entry list, preserving the
original getters' synchronization rules. MIR format remains 6; actual baseline
saved programs run with the new runtime. Newly optimized programs require the
new helper bindings. Old runtimes cannot supply those new symbols.

Candidate traversal now runs backward within blocks: inserting call/take
statements previously shifted later stored positions. The regression test uses
two eligible calls with different arguments and an intervening unrelated getter,
so it catches accidental rewriting of the wrong call as well as skipped sites.
Other tests check pointer roots across polls, packed representations, escaping
results and disabled live transformation.

## Baseline, refinement and evidence

Completed memory work was first published to local main at `fab8a02`, integrating
`adf29aa` with main `40291dc`. All unrelated dirty files/index changes were
preserved. This slice uses fresh ordinary and diagnostic binaries built from
that combined committed production tree. Prior memory timing claims stay pinned
to their original revisions. No configured remote was available for a network
push. [Main integration validation](main-integration-2026-10-17/README.md).

Before production changes: 10 smoke, 60 lookup coverage, 32 application and
10 diagnostic census runs. New mixed hit/miss cases include negative indices,
independent checksums and full pointer-string comparisons against prebuilt data.
Stored-Option and already-eliminated array controls prevent attributing all
lookup changes to this optimization.

The first candidate `0c0feda` has strong mixed-lookup/application results, but
added miss-only coverage exposes list misses +35.3% at one worker (+28.9% at
eight). None is already shared; publishing and clearing an empty payload wastes
work. Keep this evidence as an intermediate result, not the accepted default.

The refinement avoids publication on misses and repeated standard-tag name
lookup. Its independent nine-pair miss probe removes the large regression;
final seven-case coverage includes misses at every worker count. One-worker
list-only misses still measure +3.0% (roughly 0.2 ns per lookup); the probe is
roughly flat there, and other worker settings improve. Record this residual
cost; do not claim universal improvement in every lookup pattern.
[Initial miss regression](I2a-misses/summary.md),
[refined miss probe](I2a-fast-miss-probe/summary.md).

The saved evidence contains **1,630 measured runs across 20 datasets**. All
measured wall-clock intervals are nonoverlapping; every raw run, including
outliers, is retained. No own compilation/tests overlapped accepted timings.
[Binary/source hashes, arguments and interval audit](I2a-binaries.json) pin
ordinary baseline `fab8a02` and final candidate `fd333a5`; superseded results
remain explicitly separate. The exploratory fast-miss probe was built before
its source commit but uses the identical final executable hash. Same-binary
controls use the exact baseline executable under both labels.

Main's subsequent `f11f33d` commit changes only benchmark indentation. The
final integration retains that formatting after measurement. Raw source hashes
retain the measured form; later main integration is validated separately below.

| Dataset | Measured runs |
|---|---:|
| [I2a-app-baseline](I2a-app-baseline/summary.md) | 32 |
| [I2a-app-census-after](I2a-app-census-after/measurements.json.gz) | 4 |
| [I2a-app-census-before](I2a-app-census-before/measurements.json.gz) | 4 |
| [I2a-applications](I2a-applications/summary.md) | 224 |
| [I2a-census-after](I2a-census-after/summary.md) | 10 |
| [I2a-census-before](I2a-census-before/summary.md) | 10 |
| [I2a-coverage](I2a-coverage/summary.md) | 280 |
| [I2a-coverage-baseline](I2a-coverage-baseline/summary.md) | 60 |
| [I2a-fast-miss-probe](I2a-fast-miss-probe/summary.md) | 72 |
| [I2a-final-app-census-after](I2a-final-app-census-after/measurements.json.gz) | 4 |
| [I2a-final-app-census-before](I2a-final-app-census-before/measurements.json.gz) | 4 |
| [I2a-final-applications](I2a-final-applications/summary.md) | 224 |
| [I2a-final-census-after](I2a-final-census-after/measurements.json.gz) | 14 |
| [I2a-final-census-before](I2a-final-census-before/measurements.json.gz) | 14 |
| [I2a-final-coverage](I2a-final-coverage/summary.md) | 392 |
| [I2a-final-followup](I2a-final-followup/summary.md) | 120 |
| [I2a-final-self-control](I2a-final-self-control/summary.md) | 40 |
| [I2a-misses](I2a-misses/summary.md) | 72 |
| [I2a-self-control](I2a-self-control/summary.md) | 40 |
| [I2a-smoke-baseline](I2a-smoke-baseline/measurements.json.gz) | 10 |

## Final lookup results

Seven randomized adjacent pairs per cell, one million lookups with setup and
checks included. Negative work/RSS changes are improvements. These are single
language-task workloads: changing runtime workers adds scheduling/GC context,
not parallel lookup scaling.

| Case | Work change, 1 worker | Work change, 8 workers |
|---|---:|---:|
| Map<Int, Int>, mixed hits/misses | -36.9% | -34.3% |
| Map<Int, String>, mixed hits/misses | -30.3% | -20.4% |
| List<String>, mixed hits/misses | -33.0% | -21.8% |
| Map, only misses | -9.9% | -10.5% |
| List, only misses | +3.0% | -4.5% |
| Array control | -0.5% | -2.7% |
| Stored-Option control | -2.7% | +1.9% |

Mixed cases' peak RSS falls about 39–44%; measured workload GC count goes from
one to zero. This does not disable GC or remove collection requirements from
other workloads. Stored/array counts stay unchanged. Their small timing changes
and outliers show why code layout/host variation cannot all be attributed to
lookup splitting. [Every 1/2/4/8 cell and paired range](I2a-final-coverage/summary.md).

The separate diagnostic census proves that each mixed Map/List case removes
**524,288 Option objects and exactly 16 MiB of Option cell allocation traffic**
at both one and eight workers. The one setup Option remains. Array, stored
Option and all-miss controls remove zero Option objects. This is allocation
traffic, not a claim about equal live-memory/RSS reduction; unrelated process
allocation totals can differ by a few setup bytes.

Application census (262,144 events): balanced Option objects fall from
439,989/440,055 to 73 at one/eight workers, avoiding about 13.43 MiB of cell
traffic. Fanout falls from 897,949/897,988 to 73, avoiding about 27.40 MiB.
Remaining setup/escaping Options keep their ABI. These are diagnostic process
counts, not ordinary performance timings or a complete source-site attribution.
[Exact family-level allocation proof](I2a-validation/option-allocation-proof.json).

## Final applications and limits

Seven adjacent randomized pairs per cell; work time changes compare group
medians. Negative changes are improvements.

| Profile | 1 worker | 2 workers | 4 workers | 8 workers |
|---|---:|---:|---:|---:|
| backpressure | -3.1% | -5.0% | -5.6% | -2.5% |
| balanced | -4.3% | -5.0% | -4.6% | +2.0% |
| cpu | -2.4% | -5.3% | -6.8% | -11.1% |
| fanout | -5.0% | -12.7% | +6.4% | -3.0% |

The first final application matrix is mostly faster, but group medians show
balanced/eight +2.0% and fanout/four +6.4%. The corresponding adjacent-pair
median changes are -4.1% (6/7 faster) and -2.5% (5/7 faster). Neither estimator
justifies discarding the conflicting evidence. Same-binary controls move from
-1.8% to +1.9% in group medians, with wider individual paired ranges.

A separate **15-pair follow-up** repeats balanced and fanout at four/eight
workers. Balanced improves -8.1%/-6.8% in group medians (-7.9%/-5.5% paired;
12/15 and 14/15 faster). Fanout improves -5.9%/-5.2% in group medians, but its
paired medians are smaller: -1.9%/-3.2%, with 11/15 and 10/15 faster and broad
outliers. Both runs and all extremes remain saved; this supports a useful
application improvement without establishing a universal percentage.

Reduced allocation does not translate directly into less peak memory or every
pause improving. In the follow-up, RSS changes balanced/four -3.0%, balanced/
eight +1.8%, fanout/four +0.2%, fanout/eight +3.9%. Fanout/four GC time is
+1.2% despite fewer collections (42→40). Sampled p99 bucket bounds improve at
four workers but worsen at eight in both profiles. Shared-host interference,
scheduling, remaining immutable-node allocation, and pool retention can affect
these observations; the timings do not identify their individual contributions.
Keep the allocation reduction with these limits, rather than claiming a broad
memory or tail-latency win.
[Full matrix](I2a-final-applications/summary.md),
[same-binary controls](I2a-final-self-control/summary.md),
[independent follow-up and latency buckets](I2a-final-followup/summary.md).

All work timings exclude frontend/JIT, while process peak RSS includes them.
Every run uses independent output checks, immutable binaries, cleared inherited
BADGER settings, source/executable hashes and host pressure context. Final
ordinary comparisons use randomized adjacent pairs. No own builds/tests overlap accepted timings. Shared-host
noise remains; pair counts/ranges are not significance tests. Diagnostic census
builds perturb execution and supply counts rather than ordinary speed claims.
No general p99 or universal RSS improvement is claimed.

## Validation and follow-ups

Final code passes 10 ordinary/12 diagnostic runtime tests, all 66 CLI/program
tests, six MIR tests (same MIR source as the first candidate), strict runtime/
MIR/codegen Clippy with all targets in both modes, scoped Rust formatting and
17 harness tests. The focused checker passes 88 source/saved-MIR stress checks
and two optimized/unoptimized MIR probes, including actual freeing/quarantine,
both GC generations, workers 1/4/8, scheduler perturbation, packed scalars,
dynamic null, nested Options, stored/compared results and pointer results handed
to another task. Real baseline format-6 programs still run correctly.
[All commands, MIR captures, hashes and validation logs](I2a-validation/).

I2a is a bounded slice of I2; broader enum/record/closure scalar replacement
remains open. Next evaluate safe multiple-producer/copy groups and tag-only
lookup predicates, preserving copy snapshots, escaping ABI, roots and scheduler
semantics. Use these new miss-heavy cases to prevent protocol overhead from
being hidden by successful lookups. Immutable-node/record allocation and targeted
pool reservation remain candidates; arena/ring ownership work is unimplemented.

## Integration with newer main (2026-10-18)

Evidence was committed at `c1eed2b`; merge `1c4963c` integrates current main
`1c8d910` without conflicts. This includes newer JIT relocation/PIC handling,
frontend/library and application benchmark work, so the historical timings above
remain specific to `fab8a02` versus `fd333a5`. They are not measurements of this
newer combined tree. The lookup protocol itself is unchanged. Integration checks
and combined-source hashes are saved separately in
[main-integration-2026-10-18](main-integration-2026-10-18/).
