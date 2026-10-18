# G7c experiment: finalizer classification in descriptors

Do not retain production prototype `b1fffcf`. It avoids G7b's per-object cost,
but its broader throughput and phase evidence is not strong enough to justify
cache/backpatch changes. Retain the baseline, checks and every result. Next test
sweep memory prefetching, which can address latency without changing type facts.

The prototype initializes finalizer presence once per TypeDesc in spare bits of
its existing leaf-cache byte. Marking masks the low two bits; sweep reads the
cached finalizer bit. A shared reset_kind_caches method refreshes finalizer and
float/leaf facts after recursive-layout/saved-MIR kind backpatching. No object
header, managed allocation path, native buffer transition, GC policy or MIR
format changes. Callback ordering and actual freeing/quarantine are unchanged.

## Measurements and decision

**996 measured processes across six datasets**, all measured intervals disjoint,
with immutable binaries and no removed outliers. The production baseline is
`fc759e3`; earlier baseline samples were saved before either prototype, and
reverting G7b restored those exact production crates. [Manifest, hashes,
commands and interval audit](G7c-binaries.json), [baseline](G7c-baseline.md).

Coverage has five adjacent randomized pairs per case at all four worker counts:
[workers 1/8 pilot](G7c-pilot/summary.md), [workers 2/4 extension](G7c-workers24/summary.md).
Scalar snapshot pilot work is -9.7%/-4.5% at workers 1/8 and GC pause -23.7%/-5.0%;
record-window work -1.4%/+0.6%, and buffers/parallel snapshot effects vary.
No allocation-count reduction or broad memory/scaling improvement is claimed.

[All nine application contracts](G7c-applications/summary.md) use seven pairs
at workers 1/2/4/8. One-worker group/paired changes: compare records -2.1%/-2.1%,
snapshots -0.1%/-2.5%, state -3.8%/-7.6%, batches +9.2%/+4.1%. Batches have
no collections in phase probes; the collector fast path cannot directly explain
those timings. Reconciliation and other application cells also remain mixed.

A [15-pair one-worker repeat](G7c-followup/summary.md) gives snapshots -6.9%
group/-5.1% paired (10/15 faster), state +3.0%/+0.1% (7/15 faster), batches
+6.4%/+1.2% (4/15 faster). [Same-binary controls](G7c-self-control/summary.md)
are snapshots -3.0%/-4.9% (11/15 faster), state +7.2%/+7.2% (6/15 faster),
batches +1.3%/+2.2% (4/15 faster). Process medians and pair estimators differ;
none is substituted or treated as an isolated significance test.

[Diagnostic whole-process phases](G7c-phases/summary.md), three paired processes
at workers 1/8: compare snapshots sweep -6.9%/+4.5%, records +4.8%/+3.4%, state
-0.7%/-2.3%, JSON +17.2%/-1.0%. Instrumentation includes all eight inner iterations
and output; it does not establish normal kernel speed. Together these do not
support keeping this more involved type-cache change for a broad collector win.

## Validation

76 program tests, 12 MIR tests and 11 ordinary/13 diagnostic runtime tests pass,
including the new trace/finalizer independence and descriptor-reset test.
72 source/old/new saved-MIR stress checks, 16 diagnostic allocation conservation
checks plus the ordinary-profile-absence probe, strict runtime/MIR Clippy in
both modes and scoped formatting pass. [Logs and captures](G7c-validation/).
Saved programs use existing format 6; runtime-defined callback-bearing descriptors
are constructed by the runtime, not serialized as native callback addresses.
The shared 22-test Python harness was unchanged from its passing G7b run.

Revert the production prototype before main integration. Compiler allocation
elimination (I2b/I5a) remains the clear application win; these collector
classification micro-optimizations have not earned publication.
