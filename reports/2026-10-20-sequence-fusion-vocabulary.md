# Sequence fusion: the kind vocabulary, and what it does not yet cover

Reviewed 2026-10-20 on branch `feature/fusion-params` (worktree
`~/orca/projects/language-search`), merged with `main` at `f708d9c`.

This is the follow-up to the sequence/collection audit: what the fusion
recognizer's vocabulary is now, what the compiler can and cannot infer, and
the one remaining structural gap (`chunks`/`windows`/`flat_map`).

## The vocabulary

`@fusion(role = ..., template = ..., <parameter> = ...)` marks a sequence
function. Any module may annotate; the recognizer still requires a matching
shape and receiver type, so an unsuited marking simply does not fuse.

| role | kind | covers |
|---|---|---|
| source | `range`, `from_range` | `seq.range`, `seq.from_range` |
| source | `list` | `list.of`/`list.seq` (counted, indexable) |
| source | `entries` | `map.to_list` |
| stage | `call` | `map`, `filter`, `filter_map` (classified from the callback's return type) |
| stage | `cursor(mode)` | `take`, `skip`, `step_by` |
| stage | `scan`, `enumerate` | 1:1, with state |
| stage | `dedup`, `distinct` | state-carrying selections |
| terminal | `fold` | a user callback |
| terminal | `accumulate(combine)` | `count`, `sum`, `product` |
| terminal | `collect` | a builder plus `list_finish` |
| terminal | `any`, `all`, `find`, `find_map`, `first`, `position` | short-circuiting searches |

Lowering checks an annotation against the function it is written on: the
role, the arity, and the parameters the kind cannot be honoured without
(`E0200`), and the values the compiler knows (`E0194`). Invalid markings are
warnings in user code and errors in the standard library.

## Findings

**A kind names semantics, not a shape — so the type cannot choose one.** A
search that returns the *last* match has `find`'s signature; a filter that
*drops* what its predicate accepts has `filter`'s. Deriving the kind from the
signature would therefore mis-fuse semantically different functions, so the
annotation is the claim and the shape check is the part the compiler
verifies. `call`'s choice of `map`/`filter`/`filter_map` from the callback's
return type is the exception, and is sound only because those three differ by
nothing else.

**Cardinality is a property of the plan, not a fold.** `count` over a source
that knows its length (a list, map, or range) through stages that neither
drop nor re-shape items is that length — `(end - start) + inclusive`, clamped
— so no loop is emitted. The gate is `callback_pure_infallible`: skipping a
callback that could defect or have an effect is observable. **The practical
consequence is larger than it looks:** Badger's integer arithmetic is
*checked*, so `fn(x) { x * 2 }` can defect and is refused — the shortcut
mostly covers callback-free pipelines (`coll.seq().count()`, `range(..).count()`,
`.enumerate().count()`), which is where the O(1) win matters most anyway.
`scan` is excluded outright because its `initial` is a construction-time
argument. `filter`, `filter_map`, `flat_map`, `dedup`, `distinct`, and the
cursor stages lose the count.

**Fusion runs at HIR; inlining runs at MIR.** So "inline first, then fuse" is
impossible in that order — a MIR inliner's result cannot feed an HIR
recognizer. What exists is `fuse(HIR) → inline(MIR)`, and the MIR inliner
already turns *known* closures into loops (`MAX_CALLEE`/`MAX_HIGHER_ORDER`),
which is why a non-fused `map`/`filter`/`fold` chain is not as expensive as
the recognizer's absence suggests. Point-free callbacks (`map(xs, double)`)
are invisible to the recognizer today; the cheapest fix is to let it read a
named function's body directly (it already has whole-program HIR and extracts
λ bodies), not to add an HIR inliner, which could inline the *stage* itself
and destroy the shape being matched.

**`collect`, `sum`, `count`, and `product` are folds with a fixed combine.**
They are one `accumulate` shape with a combine step (`add` the item, `add`
one, multiply) and an identity (0, or 1 for `product`); `collect` is the
exception that needs a builder and a `finish`. `min`/`max` cannot join them
without a nullable accumulator, since they have no identity.

## What this does not cover: `chunks`, `windows`, `flat_map`

The library has these, and they work — but unfused, so each chunk costs a
builder, a `Cell`, and a `link` adapter per item. Fusing them is not another
kind; it is a change to the loop's shape, because a batch stage consumes *n*
upstream items per item it emits:

- **`chunks(n)`** — the loop body gathers into a builder and, when the chunk
  is full, finishes it and runs the *downstream* stages and the terminal on
  the chunk. The last, short chunk must still be emitted, so the downstream
  emission has to be reachable from the flush as well.
- The way to do that without duplicating the emission (MIR has no
  subroutines, and a stage after the batch may `goto step` to drop a chunk):
  emit the downstream stages and the terminal **once**, into a `tail` block
  entered from two places — the body when a chunk completes, and the
  end-of-input path when the builder still holds items — with a `resume` flag
  choosing `step` or `exit` after it. A stage-after that drops a chunk may
  then `goto step` harmlessly: the head sees the input exhausted, the builder
  is empty, and it exits.
- **`windows(n)`** needs the same gather with an overlapping buffer (drop the
  oldest, keep the last `n - 1`), which is the same structure with different
  bookkeeping.
- **`flat_map`** is weaker than it looks: the inner sequence is a *runtime*
  `Seq` produced by the callback, so only the outer adapter can be removed —
  the inner iteration stays a `start`/`next` loop. Fusing the inner would
  need fusion to run after inlining (a MIR-level recognizer), which is a much
  larger change.

Estimated at ~150–200 lines in `lower_fused` plus per-opcode logic and tests,
with the risk concentrated in the flush/partial-chunk paths — a project in
the same area as the loop's core, not another table entry. It is recorded
here rather than half-done.
