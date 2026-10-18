# G8a experiment: specialize clearing reused small cells

Baseline production is C1 b080078, now main 494b178 (the intervening main change
adds an example/report, without compiler/runtime changes). Immutable ordinary
and diagnostic C1 binaries and fresh before-edit coverage are saved.

Release disassembly of gc::alloc shows the recycled-cell path calling memset
with the whole rounded cell size, then overwriting both header words. Test
constant-size full clears for 32/48/64-byte cells, leaving other sizes, initial
zeroed chunks, allocation accounting, GC policy and all object layouts unchanged.
LLVM may turn these constants into stores and remove the overwritten header
zeroes itself. Keep raw allocator semantics fully zeroed, including padding;
do not skip initialization or assume application-level ownership.

Validate native lowering and the existing exhaustive pool reuse/trim test over
payload sizes 0..1040, both verification modes, diagnostic limits/refills and
cross-worker returns. Then compare ordinary record/reconcile, pointer-map,
snapshot, buffer and cold-allocation controls, with same-binary repeats if needed.
Retain only reproducible application gains. Diagnostic/profile changes are
separate from ordinary speed claims. Mark this experiment incomplete until its
correctness and measured tradeoffs are recorded.
