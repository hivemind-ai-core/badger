# G8c experiment: fully initialized HAMT allocation (evaluated, rejected)

Completed: [results and exact validation boundaries](G8c-summary.md).
Prototype 5b91953 passes its correctness checks, but application repeats remain
small/mixed and byte-heavy coverage regresses 8–14%. Revert a9d51d7 restores
production code. Keep the benchmark and test improvements. The original
hypothesis and acceptance criteria follow.

Baseline main: 27e9653, after the rejected G8a prototype was reverted. Immutable
ordinary `allocation-paths-ordinary` and diagnostic `allocation-paths-diagnostic`
binaries remain, with G8b path coverage and fresh pre-edit application baseline.

G8b found system cells account for 76.2% of managed allocation bytes in realworld
state and 56.1% in collection-heavy parallel snapshots. HAMT MapNode dominates
these shapes. Rather than increase pooled reservation, test avoiding redundant
zeroing only when the entire declared node payload is immediately overwritten.

Audit: `alloc_node` writes all four u32 header fields, every three-word entry
and every child word. `splice_owned` writes all fields, copies/inserts every
entry and child in the new layout, and never calls another allocator, callback
or safepoint during initialization. Both allocate an exact 16+24*entries+
8*children payload. Their source references remain rooted across allocation as
before. Builder/epoch in-place edits retain original capacity and do not use the
new allocation API. Node tracing only reads initialized logical entries/children.
No public zero-initialization contract changes.

Prototype an internal unsafe API requiring complete payload initialization
before another safepoint, reference publication or read. Keep descriptor/meta
initialization, GC pressure/root publication, object accounting, OOM abort,
quarantine and pool ownership identical. Preserve zeroed chunks because pooled
bump storage is also used by ordinary zeroed allocations. Skip recycled-cell
clears and use system malloc instead of calloc only for this explicit contract.
All rounded padding remains outside the declared payload and may not be read.

Regression checks must poison/reuse pooled cells, initialize/check every declared
byte, then verify the ordinary zeroed path still clears them; also exercise
system cells without reading uninitialized bytes. Existing randomized HAMT,
collisions, builders/epochs, source/saved-MIR maps, GC stress/quarantine/chaos and
cross-worker publication must pass. Benchmark ordinary applications and parallel
coverage at 1/2/4/8 workers, including records/bytes/JSON controls. Census counts
should stay identical; no allocation-count/GC-frequency/RSS saving is promised.
Retain only after broad measured tradeoffs are recorded.
