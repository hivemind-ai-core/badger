# I5b initial application evidence — refinement needed

Candidate `8ffce23`, baseline `38855c7`: 908 measured processes, three disjoint
datasets, seven pairs per ordinary cell, workers 1/2/4/8; no outliers removed.
All hashes/raw samples remain in I5b-binaries.json and the linked directories.
Own builds/tests completed before timing. This is an intermediate candidate,
not the retention decision.

Scalar snapshot coverage improves 60–66%; eight-task scalar snapshots 63–69%,
with 16→2 collections and 11–25% lower RSS. Serial scalar snapshots go 2→0
collections and ~28% lower RSS. Realworld state improves 37–49%; compare
snapshots 66–69%, all seven process pairs faster at every worker count.

However private controls regress: private eight-task maps at eight workers show
+22.6% group/+11.4% paired, only 1/7 faster and a wide paired range. Single-task
64-key private maps show +2.5–5.8% groups. Reconcile/eight is +7.1%, 0/7 faster;
compare batches/eight +32.8%. Other controls vary, so don't claim a universal
improvement or dismiss non-GC slowdowns simply because GC didn't run.

The initial HAMT implementation passes a dynamic token even to the old builder
path, with token-zero checks during insertion. Next specialize immutable,
builder/private, and epoch mutation into distinct compile-time modes. Keep
ownership data/branches confined to the epoch mode, then repeat ordinary pairs
and census. Preserve this entire initial matrix for comparison.

Allocation census (eight inner iterations, including warmups):

| Workload | Map objects before / after | MapNode objects before / after | Map+MapNode cell bytes before / after |
|---|---:|---:|---:|
| state | 131080 / 520 | 285200 / 46304 | 131093248 / 15322304 |
| compare snapshots | 131080 / 2056 | 261888 / 53392 | 71311616 / 12106368 |
| reconcile control | 8 / 8 | 7848 / 7848 | 2835968 / 2835968 |

State diagnostic collections fall 7→0, snapshots 4→0; reconcile stays 3.
Counts match exactly at workers one/eight. This is real garbage reduction,
not GC bypass, live-memory measurement, request latency, or a general worker
scaling win. The slower initial private path has unchanged allocation counts.

Validation: 76 program tests at the compiler integration, 14 MIR tests including
the subsequent conservative ABI mutation rejection, 11 ordinary/13 diagnostic
runtime tests, strict four-crate Clippy both modes, 136 source/old/new saved-MIR
GC checks, 28 MIR probes, 36 generated differential checks, 72 native-container
checks, allocation conservation/ordinary absence, and 23 harness tests. The
sandboxed program attempt fails local network/debugger permissions; the permitted
rerun passes all 76. Final guarded ordinary/diagnostic binaries rerun ownership
and allocation checks; no code generation behavior changed for legacy literals.
