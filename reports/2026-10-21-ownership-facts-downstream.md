# Ownership modes and region brands downstream: produced, consumed by nothing — verified; no optimisation follows

Checked 2026-10-21 against `3e2e20b`, on branch `opt/ownership-modes-consumed`.
Status: **the premise holds; no optimisation can be built on these facts
today.** This report records the verification, the reason, and the exact
condition that would change it. It does not re-measure what three existing
reports already measured — it names them instead.

## The premise, verified

The claim under test: the ownership modes (affine / linear / `@move_only`)
and the region brands are produced by the type checker and consumed by
nothing downstream; the MIR and the code generator contain no reference to
them. That is exactly true.

| Fact | Produced | Where it lives | Downstream consumer |
|---|---|---|---|
| affinity (use at most once) | `ownership.rs` | `Affine` enum, private; `Own.affine` map, private, per-`check_def` | none |
| linearity (use exactly once) | `ownership.rs` + `env::LINEAR_STD` | `Linear` mode, private | none |
| `@move_only` | `hir/lower.rs` (attribute) → `env.move_only: FxHashSet<DefId>` | typeck `TypeEnv` | `program::report_stored_state` (a *typeck* diagnostic); not MIR |
| region brand | `infer.rs` / `solve.rs` region skolems; `hir.region_blocks` | `TyKind::Skolem` with `info.region` | typeck escape checks only |

Evidence, by crate:

- `crates/badger_typeck/src/ownership.rs`: `enum Mode`, `enum Affine` and
  `struct Own` are private. `Own` is built at the top of each of
  `check_def` (l.151), `generic_pass` (l.772), `check_module_closures`
  (l.844) and dropped at the end of it. Its fields are `affine`,
  `reported`, `consuming`, `last_use`, `loans`, `blocks`, `params` — none
  is written back into `Infer`, the HIR, or any global. The pass's only
  output is `Diagnostic`s.
- `grep -rc move_only crates/badger_mir/src` → 0 files;
  `crates/badger_codegen/src` → 0 files. `Affine`/`Linear` appear nowhere
  in `badger_mir` or `badger_codegen`.
- `crates/badger_runtime/src/desc.rs` has the region flag
  (`TypeDesc::region`, `with_region`), but `with_region` has **no caller**
  outside its own unit test, and the brand is erased before layout:
  `program::ground_resolved` maps every `TyKind::Skolem` to `unit()`
  (`program.rs:2648`), and a region brand travels as a `Con` type argument,
  so the MIR's `Ty` never carries it. `layout.rs` (which builds the
  descriptors) sees only ground types; it cannot set the flag.
- The MIR opcode set has no ownership operation at all. `mir.rs`'s `Stmt`
  is `Assign | Store | GlobalStore | Poll | Loc`; `Rvalue` is arithmetic,
  loads, `Alloc`, `Call`, `Concat`, `Tag`, addresses. There is no drop,
  free, retain/release, or move instruction. A move and an ordinary copy
  are the same `Assign(dst, Use(src))`, and a borrow is erased to the
  value it borrows (`lower.rs:1557`, `UnOp::Ref | UnOp::RefMut => Some(v)`).

### Demonstration

`@move_only pub struct Token { pub id: Int }`, `let u = t` (a binding move),
`let r = { a: u, b: { id: 2 } }` (a move into an aggregate), with
`--dump-mir` on `3e2e20b`:

```
Assign(2, Alloc { desc: .., size: 8 })        // Token is one 8-byte cell
Store { base: Local(2), offset: 0, repr: I64, value: Local(1) }
Assign(3, Use(Local(2)))                       // let t = ..
Assign(4, Use(Local(3)))                       // let u = t: a move is a word copy
Assign(5, Use(Local(4)))                       // move into r.a: another word copy
Assign(27, Use(Local(5)))                      // take(r.a): pass by value, a word copy
```

The source binding `t` is left valid; nothing invalidates it and nothing
frees it. The checker's E0415 for a *second* use of `t` is a source-level
diagnostic with no representation in the MIR. A borrow (`&t`, `t.id`,
`t.peek()`) lowers to the same value as the move.

## Why no optimisation follows

An optimisation consumes a fact only if the fact changes an operation that
has a cost. The three costs a "one owning use" fact could remove are: a
copy the move performs, a free/drop the value needs, and a data race an
alias could cause. None exists here.

- **Moves are pointer copies.** Objects are GC-managed cells; every value
  in a verdict local is one machine word (`repr: Ptr` or an `I64`). Moving
  it is `Assign(dst, Use(src))`, which `simplify` already removes when the
  source is dead. There is no deep copy, no reference count to transfer,
  and no destructor. A `@move_only` value costs exactly what an ordinary
  value costs.
- **There is nothing to drop.** The runtime is a tracing collector; the
  only per-type hook is `Kind::Opaque { finalize, .. }`, and it is set to
  `None` for every descriptor built by `layout.rs`. Linearity guarantees a
  resource is *consumed* on every normal exit, not that its finalizer can
  be removed: error exits (`?`, `return err(..)`) are deliberately not
  checked (see E0413's text in `catalog.rs`), so the finalizer is still the
  safety net there.
- **The MIR already derives the use facts.** `reports/2026-10-20-lifetime-driven-optimisation.md`
  built an oracle for the strongest fact the checker holds (a binding is
  never used again) and ran it over the 256 programs of `tests/run`: it
  changes **0** inlining decisions and removes **0** assignments. Liveness
  on the MIR — on the program the optimiser actually has, after inlining
  and closure specialisation — already subsumes it, more precisely.
- **No region arena exists.** The region brand's intended win is
  allocating region-scoped values in an arena that is freed at block exit.
  `reports/2026-10-20-mir-escape-analysis.md` and the §3.3 status in
  `OPTIMIZATION_BACKLOG.md` show the allocation side is unimplemented and
  the escape side does not pay: on `bench/records` a perfect escape
  analysis leaves the timed loop's allocation count unchanged (19 998 710
  objects) because the loop's object escapes into an `Atom` by design, and
  every other hot allocation on the benchmarks escapes for real. Regions
  would only narrow "records passed to a function that escapes them",
  which the survey already counts as the common case and not a candidate.

The region brand is also not cheaply reachable from the MIR: it is erased
by `ground_resolved` before lowering, so using it needs plumbing the
ungrounded type (or a side table) from the type checker to `layout.rs`, and
then a runtime that can allocate into a region. Both are the arena project
the §3.3 status already blocks on.

## What would change the answer

The precondition is a copy, a free, or an alias with a price:

- **Reference counting or Perceus-style reuse** (§3.5, and the "what would
  change the answer" note of the lifetime report). With a retain/release
  pair, "the caller never uses this argument again" becomes a fact that
  removes a decrement, and in-place update of a uniquely owned value
  becomes possible. Today the collector owns the cell and a "dead" object
  is simply unswept, so there is nothing to elide. The fact to carry then
  is the move-only marker (a *type-level* bit), not the checker's internal
  `last_use`/`loans` state.
- **A region arena.** Give `region R { .. }` a runtime arena, mark the
  descriptor of a region-branded type (`TypeDesc::region`, the wired-but-
  unset flag), allocate its values there, and free the arena at block exit.
  That is the §3.3 win, and it needs the runtime, the verifier, and the
  root handling the escape report enumerates — not a type-checker change.
- **A region brand that reaches the MIR.** Stop erasing the brand in
  `ground_resolved` (or pass a side table) so `layout.rs` can call
  `with_region`. Necessary for the arena; neither sufficient nor useful
  without it.

Until one of these lands, the ownership modes and the region brands are
soundness checks, and the right amount of downstream consumption is none.

## Reproducing

```sh
# premise
grep -rc 'move_only' crates/badger_mir/src crates/badger_codegen/src     # 0
grep -rn 'with_region' crates | grep -v 'desc.rs'                        # no callers
grep -n 'Skolem(_)' crates/badger_typeck/src/program.rs                  # erased to unit()

# demonstration (the MIR a move lowers to)
cat > /tmp/mv.bdg <<'EOF'
import std.io
@move_only
pub struct Token { pub id: Int }
fn take(t: Token) -> Int { t.id }
pub fn main() -> Result<Unit, io.Error> {
    let t = { id: 1 }
    let u = t
    io.println(take(u))?
    ok(())
}
EOF
cargo build --release -p badger_cli
target/release/badger run --dump-mir /tmp/mv.bdg 2>&1 >/dev/null | sed -n '/fn 0 /,/^fn 1/p'
```

## Existing measurements this relies on

- `reports/2026-10-20-lifetime-driven-optimisation.md` — the checker's use
  facts vs. MIR liveness: 0 inlining decisions, 0 assignments.
- `reports/2026-10-20-mir-escape-analysis.md` — stack allocation of
  non-escaping records: 0 % on the gate benchmark, no hot-path candidate.
- `OPTIMIZATION_BACKLOG.md` §3.3 (region brand), §3.4 (linear), §3.5
  (`@move_only`) — all landed as static guarantees with the MIR, codegen
  and runtime unchanged.
