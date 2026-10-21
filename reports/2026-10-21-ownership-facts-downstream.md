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

---

# Part 2 — can the facts side-step the collector, and do they pick an arena?

Added 2026-10-21. The collector is a precise, non-moving, generational
mark-sweep heap (`crates/badger_runtime/src/gc.rs:1-20`), one global heap
(`static HEAP`, `gc.rs:111`) with per-**worker** pools and object lists.
Worker is a thread-local (`task.rs:528`), and tasks migrate between
workers, so the pools are per-thread, not per-task.

## 2.1 Immediate free of known-dead objects — not available, and not the shape to build

There is no path that frees one managed object outside a sweep
(`grep -rn 'free_now\|fn free_object' crates/badger_runtime/src` is empty;
`free_cell`, `gc.rs:559-564`, is called only from the sweep closures and
`dealloc`s only cells with payload > 512 bytes). A small cell's space is
reclaimed by its lines staying clear at the next collection; `mark_lines`
(`gc.rs:269-290`) ORs bits and has **no inverse**, and only
`Pool::clear_lines` (`gc.rs:484`) clears a bitmap, wholesale, at a full
collection. So a small object cannot be individually returned to the
allocator between collections at all.

An immediate free would additionally have to:

- remove the object from its owning worker's list, `OBJECTS` (`gc.rs:136`)
  — there is no back-pointer from an object to its worker, and removing an
  element from the middle invalidates `YOUNG_START[w]` (`gc.rs:887`) and
  the `gc-profile` census, which walks list and side table in lockstep
  (`gc/sweep.rs:167-169`, `allocations.rs:243,265`);
- drop it from the remembered set (`REMEMBER`, `gc.rs:883`), `globals`
  (`gc.rs:719`), `pinned` (`gc.rs:747`), the task temp roots and the
  shadow stack (`task::for_each_root`, `task.rs:970-1040`);
- poison the payload and quarantine it under `BADGER_GC_VERIFY`
  (`gc.rs:1200-1215`, `obj` at `gc.rs:83`);
- run its finalizer exactly once (`FINALIZING`, `gc.rs:1534`);
- correct `live` / `OLD_BYTES`, which are snapshots taken at the end of a
  collection (`gc.rs:1280,1289`), not per-allocation counters.

That is a lot of invariants for a mechanism the immix pool already
replaces: reuse is by line bitmap and hole size class, and it is cheap.

## 2.2 Allocating the region's objects outside the heap — the implementable shape

The design that fits the runtime is not "free now inside the heap" but
"allocate the region's objects outside it, and release them in one action
at the end of the block". Both halves already exist:

- **Allocation outside the heap.** `alloc_immortal` (`gc.rs:706-716`)
  takes system `alloc_zeroed`, sets `IMMORTAL`, and puts the object in no
  worker list, so the sweep never sees it. `Buffer` element storage
  (`buffer.rs:79-93`) is native `malloc`, charged to the collector with
  `external_alloc` (`gc.rs:682`) and released by the object's finalizer
  (`buffer.rs:166-175, 212-221`).
- **Bulk release.** `Pool::rebuild(trim)` already `dealloc`s whole chunks
  whose lines are all clear (`gc.rs:447-455`), which is exactly the
  "release the region's storage at once" step.

**Soundness.** The property a region needs is "no pointer from outside the
region reaches a region object", and the region brand is precisely that
guarantee (`TypeDesc::region`, `desc.rs:173-177`, is documented as "the
type checker proved that none of its values outlives the region" — the
flag is wired but has no setter yet, see Part 1). Two obligations remain:

1. A region object may point *into* the heap (and does: a region-scoped
   record holds heap strings, atoms, buffers). The collector must trace
   those children while the region is live, so region objects need a real
   `Header` + descriptor and must be **rooted** while live. They already
   are: their locals sit in the shadow stack
   (`codegen/src/lib.rs:723-785` builds and links the frame), so the
   `header`-based `trace` (`gc.rs:1570`) will follow them.
2. They must be **excluded from the sweep lists** so the collector never
   claims them, and released only by the region's exit — i.e. an
   `alloc_immortal`-shaped allocation plus an explicit release, not the
   `alloc` path.

This is `MEMORY_MAN_TODO.md` R1–R6 and L1–L2 verbatim, and the doc's own
first constraint is the reason ownership modes alone do not buy it:
**"R0: Transfer is not proof of graph uniqueness."** A move or a transfer
says a value has one owner at the source level, not that the object graph
is uniquely reachable in the MIR (every copy in Part 1 is an alias). Only
the *region brand* carries non-escape, and it is erased before the MIR
(`ground_resolved`, `program.rs:2648`).

## 2.3 Stack allocation — blocked on the root/tracing model

A stack object that holds managed pointers must be traced like any other,
and the collector's verification requires every address it traces to be a
heap object or a registered immortal (`gc.rs:1036`; `freed_use`,
`gc.rs:592`, aborts otherwise). `MEMORY_MAN_TODO.md` L2 states the
prerequisite exactly: "Support stack/region objects containing pointers in
the root/tracing model before redirecting existing heap-object allocations
there." Pointer-free (leaf) stack objects are not blocked by that, but the
escape survey found no candidate in a measured hot loop
(`reports/2026-10-20-mir-escape-analysis.md`), so there is nothing to
stack-allocate that pays.

## 2.4 Choosing an arena (task-local vs shared vs handoff buffer)

There is no arena to choose between today: one global heap, and the only
split is by **size class** (payload ≤ 512 → the calling worker's immix
pool; else system `alloc_zeroed`, `gc.rs:501-505`). "Task-local" is not
expressible: the pool, the object list and the young boundary all follow
the **thread** (`worker_index()`, `task.rs:528`, reads a `Cell`), and
tasks migrate. `MEMORY_MAN_TODO.md` H1 names the prerequisite ("ownership
follows migrating tasks, and task completion cannot invalidate pending
join results").

Shared-versus-task-local is decided today by *type capability*, not by a
heap arena: `Share` / `Transfer` / affine in `solve.rs` / `env.rs`. A
`Buffer` is affine and movable, and its native element block is owned by
the managed object, so handing the buffer to another task transfers the
malloc block with the pointer and it is freed once by the finalizer
(`buffer.rs:166, 212`) — the "buffer with several objects, handed to
another task" pattern **already exists for buffer element storage**. To
generalize it to arbitrary objects is R1–R6; the doc's R3 is explicit that
reclamation must follow *completed processing*, not dequeue, and that a
borrowed message must not escape its lease.

What the checker's facts would supply is exactly an arena chooser's two
inputs — the region brand says "dies at the end of this block" (scratch /
region arena), `@move_only` and affinity say "one owner, movable across a
task boundary" (transfer arena). `MEMORY_MAN_TODO.md` L1 states the same
goal ("choose register, stack, region or GC allocation"). Both inputs are
available at the type level and neither reaches the MIR.

## 2.5 Can the backlog items be done now?

| Item | Status | Blocked on |
|---|---|---|
| §3.2 escape / stack alloc | built, measured, not adopted | 0 % on the gate benchmark; the ~220 read-only runtime-call args need the runtime to accept non-heap objects (L2) |
| §3.3 region brand / arena | landed as a static guarantee; perf half did not land | a runtime arena (R1–R6, unimplemented) + the brand erased by `ground_resolved` + `TypeDesc::region` never set |
| §3.4 linear resources | landed (E0413) | nothing; the `~3–5 %` on I3/I5 was **never measured**, and finalizer elision is unsound while error exits are unchecked |
| §3.5 `@move_only` | marker landed; `sra` half not adopted | §3.2, not adopted |
| §4.3 translation validation | cheap form landed | a full SMT model of MIR is open |
| §4.4 lifetime → inliner / DAE | measured, not adopted | nothing missing but the *value*: an oracle of the facts changes 0 decisions; what would pay is effect info, not lifetimes |

The region/arena items are not blocked on the type checker — the brand is
already computed and is the right fact. They are blocked on **runtime
mechanisms that do not exist** (a non-heap, traced-while-live object
domain; a root/tracing model for pointer-bearing non-heap objects) and on
**measurement**: `MEMORY_MAN_TODO.md` G1b ("measure allocation counts by
source/type and survival/escape class") is stated as needed before any
arena claim, and the one arena-shaped benchmark, `bench/parallel_alloc`,
measured 0 % for the escape half.

If arena work is wanted, the first slice the doc itself defines is **R1**:
a bounded FIFO for *pointer-free* message data between one producer and
one consumer, with wraparound, oversize, and full/empty tests. It is the
only arena slice that does not need the cross-heap rooting model (R6) and
it has a stated benchmark; everything with pointers depends on R6/L2
first.
