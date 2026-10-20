# Lifetime-driven inlining and dead-assignment elimination (§4.4): not adopted

Measured 2026-10-20 against `6b96579`. Status: **not adopted**. The item
asks the inliner's cost model (`crates/badger_mir/src/inline.rs`) and the
dead-assignment eliminator (`crates/badger_mir/src/opt/simplify.rs`) to use
the type checker's lifetime information, and expects ~3–5 % more inlining
decisions and ~2 % more removed assignments. Before building a bridge from
the type checker to the MIR, both passes were instrumented with an
**oracle**: the most any lifetime fact could tell each pass, computed
exactly on the MIR itself. Over the 256 programs of `tests/run`, the oracle
changes **no** inlining decision and removes **no** assignment. The facts
the item names are either ones the MIR already has, or facts that make no
difference to it.

## What the type checker knows

`ownership.rs` is a checker. It walks each definition, tracks per binding
whether its type is affine, where it is moved, its last use, and the loans
on it, and reports diagnostics. Nothing it computes outlives the check.
The facts are about **affine and linear values only**: list builders,
buffers, batch leases, `once` closures, resources, and aggregates holding
them. Ordinary values are freely copied and never moved. `regions.rs`
tracks which values derive from a region-scoped one, to reject escapes
from a `for<S>` scope. That is a soundness check with nothing to say about
how a value is used inside its function.

The MIR erases the distinctions these checks make. `&x` and `&mut x`
lower to the value of `x` (`lower.rs`, `UnOp::Ref | UnOp::RefMut =>
Some(v)`). A by-value, `&T` or `&mut T` parameter is therefore the same
pointer or word in the MIR, and "moving" a value is copying it. There is no
reference count, drop, or aggregate copy whose cost depends on ownership.

## Method

[`lifetime-oracle-survey/oracle.patch`](lifetime-oracle-survey/oracle.patch)
adds a `BADGER_ORACLE=1` report to the release pipeline;
[`aggregate.py`](lifetime-oracle-survey/aggregate.py) sums it over the
corpus (its docstring has the commands).

- **Inlining.** Before each inlining round, the oracle visits every direct
  call site. It records whether the call's result is dead after the call
  and whether every argument is dead after it, using the same liveness
  dataflow that `simplify` uses. Liveness on the MIR is the strongest form
  of "the caller never uses this again". Any sound type-checker fact about
  a binding implies it: a fact that disagreed would be wrong about the
  program the optimiser runs. The oracle repeats the inliner's own
  candidate test (`inlinable`, `fits`, the size limit). For each candidate
  the size limit rejects with a dead result, it then tries the most a
  "result unused" fact could buy. It clones the callee, drops every
  returned value, runs `dead_assignments` on the clone, and checks whether
  the clone fits the limit.
- **Dead assignments.** After `simplify`, `simplify2` and `reach`, the
  oracle counts every assignment whose destination is dead after it and
  that is still in the program, by right-hand side. A "this binding is
  dead" fact from the type checker can do no more than mark these
  destinations dead, and liveness already does that.

## Inlining

| Round | Direct calls | Candidates | Accepted | Rejected (size) | Rejected, result dead | Rejected, all arguments dead | Accepted with a dead-result discount |
|---|---:|---:|---:|---:|---:|---:|---:|
| 0 | 21 862 | 21 527 | 21 089 | 438 | 52 | 324 | **0** |
| 1 | 1 358 | 574 | 217 | 357 | 8 | 321 | **0** |

The release pipeline inlines 37 446 calls over the corpus. That is more
than the accepted counts above, because closure calls and calls inside
bodies inlined in the same round also count.

- **The result is unused** at 1 394 of 21 862 direct calls (6 %). Of the
  rejected candidates, 52 have a dead result in round 0 and 8 in round 1.
  None of them fits the size budget even with the whole return slice
  removed: a callee too large to inline is large because of what it does,
  not because of the value it returns.
- **The arguments are never used again** at 18 930 direct calls (87 %).
  That holds for accepted and rejected candidates alike (87 % and 74 %),
  so it does not discriminate between them. It also has nothing to change
  in the MIR. Inlining binds each parameter with a copy of the argument
  (`inline_at`), which is correct whether or not the caller reads the
  argument afterwards, and `simplify` propagates and removes the copies.
  For a value with no ownership cost, whether the callee takes it by move
  or by borrow changes neither the correctness nor the size of the
  inlined code.

So there is no class of false-positive candidates for the lifetime
information to rule out. The candidates the inliner takes are already
valid. The ones it declines fail on size, and no ownership fact makes a
body smaller.

## Dead-assignment elimination

| After | Assignments | Dead destination, still present | of which direct, closure and intrinsic calls | runtime calls | checked arithmetic | `Concat` | removable |
|---|---:|---:|---:|---:|---:|---:|---:|
| `simplify` | 245 894 | 17 518 | 4 153 | 13 358 | 5 | 2 | **0** |
| `simplify2` | 213 398 | 15 161 | 3 766 | 11 394 | 1 | 0 | **0** |
| `reach` (to codegen) | 155 206 | 12 147 | 2 628 | 9 513 | 5 | 1 | **0** |

Every assignment left with a dead destination is kept because evaluating
it has an effect. A call can print, allocate, raise a defect, or not return.
Checked arithmetic can trap, and a string concatenation allocates.
`dead_assignments` already removes every dead assignment whose right-hand
side is removable. It iterates liveness to a fixpoint, so the definitions
that feed a removed one go too. None of the dead destinations still present
is a named source binding. A `let` lowers to a temporary and a named copy
of it, and the copy is the one removed. Lifetime information says whether a
value is used again, not whether computing it has effects, so it adds
**0** removals over the corpus.

The one place where the type checker could know more than liveness on
locals is memory. If an affine aggregate is dropped without being consumed,
the stores into it are dead. But stores into heap objects are not
assignments, `dead_assignments` never removes them, and removing them
needs the allocation removed too. That is escape analysis, recorded as not
adopted in
[`2026-10-20-mir-escape-analysis.md`](2026-10-20-mir-escape-analysis.md).
Linear values must be consumed on every path, so they have no dead tail.

## The bridge, and why it was not built

A sound bridge would have to record a map from HIR `LocalId` to MIR
`Local` at lowering. It would attach each fact to a MIR position and apply
the facts only before the first pass that changes the function: `inline`
renumbers locals and splits blocks, and `simplify` deletes definitions. It
would also have to handle generic instances, closure environments, and
the fact that `ownership.rs` skips the trusted standard modules entirely.
Every fact it carried would be one that liveness on the MIR already
derives, more precisely and on the program the optimiser actually has.
The bridge would add compile time for zero changed decisions, and it would
add a new place where a stale fact becomes a miscompilation.

## Gates

No code changed, so the gates hold by construction:

- `BADGER_TIMINGS=2` prints the type checker's slowest binding groups. It
  has no per-pass MIR timing distribution. `BADGER_TIMINGS=1` prints the
  `optimize` phase time and each pass's change count. Neither changes.
- `bench/messages.bdg` is unchanged, since the compiler is unchanged.
- The corpus output is the same with "the new decisions on and off",
  because the oracle shows there are no new decisions.

## What would change the answer

- **Effect information, not lifetimes.** The removals that remain are
  12 141 calls whose result is dead (2 628 direct, closure and intrinsic
  calls and 9 513 core runtime calls, after `reach`). The type checker's effect rows
  (`!{Io}`) and a runtime-function purity table could prove some of them
  removable: a total, effect-free callee whose result is unused. That is
  a different item. It needs "cannot diverge or raise a defect", which
  effect rows do not state today.
- **Ownership with a runtime cost.** If badger gains reference counting,
  in-place update of uniquely owned values (Perceus-style reuse), or
  move-only types whose copy is not free (§3.5), then "the caller never
  uses this argument again" becomes a fact with a price attached. It would
  then be worth carrying to the MIR, through the move-only marker rather
  than through the checker's internal state.
