# I3a — stored finite sequence fusion (implemented)

Implemented in `10dde6a`; [results and retained regressions](I3a-summary.md).

Baseline: main `78cc5a4`. Work is isolated in `perf/gc-reclaim-2026-10-18`.

Extend existing range/list fusion to immutable local sequence descriptions used
only by supported count/fold/for consumers. Evaluate source bounds/list and
adapter construction arguments once at the original declaration; snapshot
mutable values and retain typed roots. Each traversal resets take counters and
source position. Keep escaped/captured/reassigned sequences, unsupported
consumers and scoped/unknown sources on the ordinary iterator protocol.

First slice does not elide eager list transformations or merge map stages.
Composed stored declarations and collect terminals remain follow-up work.

Before-edit calibration completed: 216 measured processes, nine cases,
workers 1/8, three repetitions and ordinary/diagnostic-off/phase/census modes.
The old binary passes the new semantic fixture before production changes.

Fresh before-edit baseline uses immutable ordinary/diagnostic binaries from the
previous slice: only benchmark/docs changed since their production source.
Record hashes, paired randomized repetitions and all raw results. Measure
existing visible/stored/collected/eager pipelines plus repeated stored iteration,
scalar/pointer list sources, an escaping call boundary and stored collection.
Run 1/2/4/8 worker comparisons; finite scalar work does not prove worker scaling.

Acceptance: exact callback/construction order; mutable snapshots; reusable take;
early break/continue/return; nesting; suspension and pointer GC roots; inclusive
Int maximum; escaping/scoped/custom-method fallbacks; source/no-inline/unoptimized
and bidirectional saved-MIR checks. Preserve broad application controls and
allocation evidence separately from diagnostic timing. Keep negative outcomes.

After the full matrix, repeat compare batches/snapshots at workers 2/4 with
fifteen pairs and same-binary controls. Add standalone visible/byte/escaping
programs to separate changes in the combined coverage program from native
compiler/runtime differences; save and compare both MIR payloads. All original
and follow-up samples remain in the evidence, including regressions.
