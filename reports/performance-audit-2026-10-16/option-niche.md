# Nullable local pointer options

Implemented in the isolated `audit/performance-2026-10-16` worktree after the
performance audit. The audit's measurements remain a pre-change snapshot.

The optimized MIR can represent a nonescaping local `Option<T>` with zero for
`None` and the payload pointer for `Some`, when `T` has a non-null managed
pointer representation. Copy-connected locals, branches and loop-carried
assignments can participate. Pattern tags become null checks, payload loads
become pointer copies, and recognized wrapper allocations/stores disappear.
The pointer remains `Repr::Ptr`, so existing GC liveness and root stores protect
the actual payload. This runs after inlining/receive splitting and before MIR
verification, dumping and saved-MIR encoding.

This is a local representation optimization. Returned, stored, captured,
passed-to-opaque-call, compared, displayed and otherwise escaping wrappers keep
the boxed ABI. Inlined helpers can expose more eligible local uses. Live mode
keeps its current behavior. Scalar payloads, raw nullable pointers, `Dynamic`
and nested options are excluded. `Some("")` remains present; `Some(0)` and
`Some(None)` retain their distinct meanings and boxed layouts.

The pass validates all producers and consumers of each copy group. It requires
the exact lowered `some` constructor's adjacent tag/payload stores and rejects
unrecognized definitions or observations. It does not introduce a new MIR
operation, runtime descriptor kind or saved-artifact format.

Global nullable-option layout across function/native/codec boundaries remains
separate work. Current enum readers discover the wrapper's type and tag through
its own header; a bare payload pointer cannot supply that wrapper identity.
Extending the representation globally requires descriptor-aware readers,
construction, comparisons, codecs/native conversion, tracing, nested-niche
rules and artifact compatibility to agree on the representation.

Source changes:

- [MIR pass](../../crates/lang_mir/src/niche.rs)
- [Pass registration](../../crates/lang_mir/src/lib.rs)
- [Release pipeline integration](../../crates/lang_driver/src/lib.rs)
- [Structural and GC/suspension integration tests](../../crates/lang_cli/tests/option_niche.rs)
- [Concurrent language regression](../../tests/run/option_niche.lang)

Validation includes a local nullable/merged option whose optimized and saved
MIR contain no wrapper allocation or tag load, a boxed control with MIR passes
disabled, scalar/nested/returned guards, and four concurrent tasks retaining
payloads across allocations and suspension under forced collection, heap
verification and chaos scheduling in debug/release on one/four workers.

Performance evidence is captured with immutable before/after CLI copies, five
shuffled measurements after warmup per binary and worker count (1/2/4/8), and
an asserted result. The before executable hash matches the audit's baseline.
This is a microbenchmark, not a whole-language speedup claim.

| Workers enabled | Before, ms | After, ms | Ratio |
|---:|---:|---:|---:|
| 1 | 13.448 | 2.380 | 5.65× |
| 2 | 13.195 | 2.617 | 5.04× |
| 4 | 13.095 | 2.594 | 5.05× |
| 8 | 13.834 | 2.649 | 5.22× |

This single-task loop uses a shared record payload, selecting `Some` or `None`
over two million iterations. Worker counts measure enabling each runtime mode,
not parallel execution of this loop. The work function goes from two static
allocation sites to one: its record remains, while the wrapper allocation that
executed one million times disappears. Its two tag-load sites become zero.
Raw ranges and executable/source hashes are in the linked JSON.

The full `cargo test --release --offline --workspace` suite passes, including
release/saved-MIR corpus runs and the existing stress tier. Its first sandboxed
attempt stopped because the MCP test could not reserve a loopback port; the
rerun with loopback access passes. The three focused option tests, rustfmt,
language formatting and `cargo clippy --offline -p lang_mir -p lang_driver --
-D warnings` also pass. No long randomized soak was requested or run.

```sh
python3 reports/performance-audit-2026-10-16/compare_options.py \
  --before /tmp/language-before-option-niche-lang \
  --after /tmp/language-after-option-niche-lang
cargo test --release --offline -p lang_cli --test option_niche
```

Raw results and pre/post MIR: [comparison](evidence/option-niche-comparison.json),
[before](evidence/pointer-options-before-w1.mir),
[after](evidence/pointer-options-after-w1.mir).
