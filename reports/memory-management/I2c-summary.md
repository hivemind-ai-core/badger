# I2c retained: remove standard map-helper intermediate entry lists

Keep `2bed9af`. `std.map.keys`, `values`, `map_values`, `filter` and `merge_with`
now traverse the existing map cursor directly. Projections still build their
public result lists; transformations still build their public result maps.
Explicit stored `entries()` lists remain materialized. The only production
change is 20 lines in `crates/badger_hir/std/map.bdg`; allocator/collector policy,
object layouts, compiler implementation and saved-MIR format are unchanged.

## Results

Fresh ordinary binaries, seven randomized adjacent pairs per case/worker.
Percentages below are medians of paired elapsed-time ratios; negative is better.
Every target helper pair is faster at each worker setting. Raw ranges and all
samples are retained in [paired tables](I2c-paired-summary.md) and the linked
reports. These are shared-host measurements, not confidence intervals.

| Coverage | w1 | w2 | w4 | w8 |
|---|---:|---:|---:|---:|
| Stored key/value projections | -67.9% | -63.9% | -62.5% | -63.1% |
| Map values | -26.3% | -25.1% | -26.1% | -23.5% |
| Filter | -33.1% | -32.4% | -33.3% | -33.9% |
| Overlapping merge | -23.2% | -22.2% | -22.3% | -21.2% |
| Values/filter/merge composition | -21.8% | -21.7% | -22.2% | -21.0% |
| Eight fixed composition tasks | -21.8% | -21.3% | -20.2% | -13.5% |

Each helper workload uses 1,024 keys and 256 transformation rounds; output lists
or maps survive across eight later rounds and are read again. The parallel case
uses eight fixed tasks, each running 32 rounds, independent of worker count.
Independent Python oracles check all checksums, including overlapping merge
semantics, odd-size/small configurations, and an explicit stored-entry control.
Original compare/realworld workloads and peer-language contracts are unchanged.
Composition still builds three eager result maps; I3 pipeline fusion remains open.

[Same native runtime, old/new saved MIR](I2c-native-mir/summary.md) reproduces the
wins: projection elapsed -68.4%/-61.7%, serial composition -19.9%/-22.9%, and
parallel composition -20.0%/-8.9% at workers 1/8; every pair is faster. Other
individual transforms improve 21.3–35.9%. The effect survives removal of native
binary/compiler-source differences. Saved MIR still changes generated code and
its layout, so this is not a proof that every timing delta is solely GC time.

The untouched stored-entry control improves 1.7–4.2% in the source matrix and
1.0–6.0% in the same-native comparison, despite identical allocation traffic.
Keep this as evidence of other code/layout/timing effects. [Identical-runtime,
identical-MIR controls](I2c-native-control/summary.md) are mostly small/mixed;
the eight-worker byte control is +5.1% paired median with only three of seven
pairs faster. Do not subtract separate cohorts as a causal noise correction.

## Managed traffic, GC and memory

[Same-native diagnostic census](I2c-native-census/summary.md), three repetitions
at workers 1/8, exactly removes the following tuple allocations in every process:
524,288 for projections and merge; 262,144 for values and filter; 961,024 for
serial/parallel composition. Candidate tuple count is zero in those workloads.
The stored-entry control keeps all 262,144 tuples and exactly unchanged object,
requested-byte and cell-byte totals. Diagnostic timings include census overhead
and are not ordinary-build speed claims.

| Shape | Managed objects, base → candidate (w1) | Objects change | Cell bytes change |
|---|---:|---:|---:|
| Projections | 539,710 → 10,814 | -98.0% | -76.6% |
| Values | 531,526 → 267,078 | -49.8% | -10.3% |
| Filter | 443,974 → 179,526 | -59.6% | -17.7% |
| Merge | 1,055,046 → 525,894 | -50.2% | -15.6% |
| Composition | 1,976,390 → 1,005,894 | -49.1% | -11.5% |
| Eight tasks | 2,005,158 → 1,034,662 | -48.4% | -11.3% |
| Stored-entry control | 275,006 → 275,006 | 0.0% | 0.0% |

Counts are process-wide, including startup/output. Cell bytes include managed
headers and rounding; native buffers, allocator metadata and unused pool
reservations are excluded. These are allocation totals, not live bytes or escape
proof. Full metric ranges, type/source sites and exact checks remain in the
[manifest](I2c-binaries.json) and censuses.

Ordinary projection peak RSS falls about 12.2–12.4 MiB at workers 1/8
(49.0→36.6 and 49.5→37.2 MiB). Its one kernel collection disappears. Serial
composition GC counter time falls 16.22→13.19 ms at one worker, with 21→19
collections; parallel/eight falls 20.54→16.30 ms, also 21→19. These are whole
kernel counter deltas, not per-collection latency or p99 estimates.

Memory improvement is not universal: ordinary filter peak RSS rises about
1–2 MiB, despite lower total allocation. Pool reservations, unswept tails,
frontend/JIT/native storage and graph retention contribute to process RSS.
Cursor traversal can also retain input trie nodes through callbacks where an
eager entry list could release trie nodes earlier. That is a lifetime tradeoff,
not a newly proven region/escape property.

At one/eight workers the fixed-task parallel kernel medians change
196.20/64.81→153.47/55.40 ms. Absolute time improves at both, while its scaling
ratio changes 3.03x→2.77x. This does not fix scheduler/GC scaling limits.

## Broader controls and validation

[Original application matrix](I2c-applications/summary.md): nine unchanged cases,
workers 1/2/4/8, seven pairs, 15 warmups/15 measured inner samples. Most changes
are small/mixed; no general application improvement is claimed for code that
rarely calls these helpers. Initial messaging/four and state/eight block-mean
regressions are +6.7%/+8.6%. [Fifteen-pair repeats](I2c-app-repeat/summary.md)
are -2.4%/-2.1%; JSON/eight's initial +3.9% becomes -1.0%.
[Same-binary controls](I2c-app-control/summary.md) remain variable, including messaging/four +6.7% and
state/eight +2.5% on paired block means. Retain all initial and repeated samples.
Record-window/two's initial +3.4% coverage median becomes +0.4% in fifteen pairs,
versus +2.3% for its [same-binary control](I2c-record-control/summary.md).

Validation passes: 76 full program tests (source, release, saved MIR, multicore),
11 ordinary/13 diagnostic runtime unit tests plus three integrations in each
mode, strict runtime/CLI all-target Clippy in both modes, cargo fmt, and 26
Python harness tests. The new helper checker passes 12 MIR probes and 136
semantic/GC checks: generations 0/1, reuse/quarantine, chaos, workers 1/2/4/8,
unoptimized/no-inline modes, old/new saved MIR, and new MIR in the old runtime.
Fixtures cover retained results/source versions, projection and callback order,
reentry, suspension, pointer keys/values, packed U8/F32, empty maps, temporary
inputs and callback failure. Explicit materialized-entry probes stay strict.

All 2,212 measured process intervals across 11 datasets are disjoint. Own
builds/tests and MIR compilation precede their timing blocks; other actors on
the shared host are uncontrolled. A fresh 168-process before-edit calibration
is in [I2c-baseline](I2c-baseline/summary.md); candidate calibration is in
[I2c-census](I2c-census/allocation-summary.md). Executable/source hashes, exact
commands, raw data and validation artifacts are in [I2c-binaries.json](I2c-binaries.json).
I2c removes intermediate GC allocations; record packing, stack/region proofs,
shared arenas and broader collector/scheduler work remain open.
