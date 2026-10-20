# §3.1: the type checker's allocations, and why not an arena

Branch `danielytics/opt-arena`, based on `main` at `b168512`.
`OPTIMIZATION_BACKLOG.md` §3.1 asks for a bump arena in the runtime
(`badger_runtime/src/arena.rs`) for the short-lived nodes the type checker
allocates, with `ownership.rs` extended to decide which nodes cannot outlive
the check. The malloc traffic it targets is real. The arena is not the way to
remove it. `090efbc` removes most of the traffic by not allocating.

| commit | what |
|--------|------|
| `090efbc` | `perf(typeck)`: share unchanged type subtrees instead of copying them |

## Why the arena half is not adopted

- **The premise does not hold.** `badger_typeck` does not depend on
  `badger_runtime` and never touches the collector's managed heap. Its nodes
  are Rust values on the system allocator. The HIR is already
  `la_arena`-indexed, and types are `Rc<TyKind>`. Nothing about the collector's
  invariants is involved, in either direction.
- **The nodes escape by design.** The types the check builds go into
  `Checked` (read by MIR lowering), and `Session`'s `StdCache` keeps them
  across checks. Short-lived types are mixed with long-lived ones through the
  same `Rc`, so per-node placement would need lifetimes through `Infer`,
  `Checked`, the cache and the MIR lowering.
- **`ownership.rs` analyses the wrong program for this.** It tracks the
  modes of the *Badger program's* bindings (affine moves, borrows). It knows
  nothing about which compiler values outlive the check. In Rust, the borrow
  checker already does that escape analysis: an arena reference that
  outlives its arena does not compile.
- **Stable Rust cannot point `Rc`/`Vec`/`HashMap` at an arena.**
  `allocator_api` is unstable, and the toolchain is 1.98 stable. A runtime
  `arena.rs` would be dead code to the type checker, or would need a
  typeck→runtime dependency and new `unsafe`.

## Where the allocations came from

A scratch counting global allocator (not committed) with backtraces sampled
every 331st allocation, on `badger check tests/run/select_and_budget.bdg`:
831K allocations in the typecheck phase, averaging 46 bytes.
- **`InferCtxt::resolve` and `ty_map`** rebuilt the whole type tree on every
  call, even when no variable in it was bound and nothing was substituted.
  These were the largest source.
- **`ExprKind::children()`** allocated a `Vec` per visited node in
  `walk_exprs` and in the ownership and region walks. About 15% of the samples.
- **`strip_effects`** copied every function type.

The fix returns the same `Rc` when no child changed (`map_shared`,
`rebuild1`). The children are still visited in the same order, so the
union-find's side effects are unchanged. The child lists are now inline
(`SmallVec`, `for_each_child`). Typecheck allocations drop from 831K to 386K.

## Gate: `badger check` is identical

The sweep runs `badger check FILE` (from the repository root) over `tests/run/*.bdg`,
`tests/reject/*.bdg`, `examples/**/*.bdg` and `tests/projects/**/*.bdg`. These
are the 2026-10-19 sweep's globs. They matched 559 files then and match 610
now. The sweep compared stdout, stderr and exit status between `main` and this
branch, in plain and `--json` modes: **610 files, 0 differences** (298 exit
0, 312 exit 1; 234 KB of diagnostics). The ownership-mode and region
diagnostics are part of that output (`tests/reject`).

## Measurements

`badger check` over the 610 files, one process per file, `BADGER_TIMINGS=1`.
Each file runs with both binaries back to back, order alternating. Running
each binary over the whole corpus in turn did not work on this shared
16-core host (load 5–40): the unchanged phases drifted by up to 33%
between blocks. Per-file pairing keeps them within 0.3 s.

| 5 rounds, paired | `main` | this branch | change |
|---|---|---|---|
| typecheck phase (median) | 26.52 s | 22.06 s | **−16.8 %** |
| everything else (median) | 19.30 s | 18.99 s | — |
| wall (median) | 45.83 s | 41.06 s | **−10.4 %** |

The two quietest rounds give typecheck −16.6% and −16.8%, wall −9.3% and
−9.8%. That is within the item's expected 10–20% on the typecheck half.

## What the remaining allocations cost

On top of `090efbc`, two scratch builds (not committed) bound what
allocation still costs. Both used the same paired method.

| 3 rounds, paired | typecheck | rest | wall |
|---|---|---|---|
| system malloc (counting allocator, idle) | 17.66 s | 16.46 s | 34.12 s |
| never-free bump allocator, whole process | 15.09 s | 11.20 s | 26.29 s |
| system malloc (this branch) | 32.10 s | 26.22 s | 58.33 s |
| jemalloc as `#[global_allocator]` | 27.84 s | 21.36 s | 49.20 s |

The first pair and the second pair are separate runs under different load.
Compare within a pair only.

- **A bump allocator bounds any arena from above.** It allocates by bumping a
  pointer and never frees, and it bought another 14.6% of typecheck. A typeck
  arena would get less, because most types escape and the `Rc` drops remain.
- **jemalloc gets most of that ceiling without any analysis**: typecheck
  −13.3%, wall −15.7%, and parse/resolve gain too. Its 610-file sweep output
  was also identical.

It is a process-wide change: the JIT-run program's runtime shares the
process. So it belongs to the CLI and runtime owners, with the runtime suites
(stress, `gc_retention`, the corpus at four workers) and an RSS comparison as
its gates. This is the jemalloc/mimalloc line of
`research/13-type-system-memory-management.md` §6.9–§6.10. It is recorded
here as the recommended follow-up, not done.

## Verification

- `cargo build --workspace`: no new warnings. The one warning, `affinity`
  unused in `badger_runtime`, comes from `main`'s NUMA commit.
- `cargo test -p badger_typeck`: 7 passed, including the new
  `resolve_and_map_share_unchanged_subtrees`.
- `cargo test -p badger_cli --test programs -- run_programs`: the corpus in
  interpreted, compiled, `--workers 4` and release modes, 4 passed.
- `cargo test --workspace --no-fail-fast`: three failures, none from this
  change:
  - `docs::api_reference_current`: `docs/api/compiler.md` is stale after
    `7ca370b` made `ApiItem` and `Symbol` parse.
  - `programs::main_error_report`: it expects `main_error.bdg:14:16`, but
    the `@derive` migration (`3d8f5c6`) moved the line to 16.
  - `programs::run_programs_multicore`: a `cancel_matrix` race under load 30+.
    It passed on re-run.
