# MIR escape analysis and stack allocation (§3.2): not adopted

Measured 2026-10-20 against `3334ae2`. Status: **not adopted**. The item
asks for a compositional escape analysis in `crates/badger_mir/src/sra.rs`
whose non-escaping records and enums are stack-allocated, and expects
5-15 % on `bench/records`. Before writing the analysis, every allocation
the release pipeline leaves behind was classified by where its value goes.
On `bench/records` there is nothing for stack allocation to take: the only
allocation in a timed loop escapes into a heap cell, by design. On the
other benchmarks every hot allocation escapes for real. The candidates
that exist are in code that runs once or on error paths, and most of them
would need the runtime to accept a non-heap object, which this item cannot
change.

## Method

`badger run --release --dump-mir` prints the MIR handed to code generation,
after `sra`, `enums` and `niche` have already removed the allocations
that are used only through their fields and tags. For each remaining
`Alloc`, [`escape-analysis-survey/survey.py`](escape-analysis-survey/survey.py)
follows the value through its copies, including copies into locals that
are assigned more than once (the merges a phi would make). It then
classifies every other use. A site escapes if any use returns it, stores it
into another object or a global, branches on or converts the pointer,
calls an unknown closure with it, or passes it to a runtime function that
keeps its argument (list and builder appends, map inserts, atoms, task
spawn, channel send, cleanup and deferral registration, `err_origin`).
Every other site is counted by what a stack-allocation pass would have to
prove before it could take that site. The classification is an
**upper bound**: it treats every runtime call it does not recognise as one
that does not keep its argument.

## `bench/records`

The program has nine `Alloc` sites in optimised MIR. Eight escape. The
ninth is `describe_error`'s, on the path that reports a failed `main`. The
only allocation inside a timed loop is the record update in the last loop:

```
Assign(230, Alloc { .. size: 16 })                 // { ..small, w: i }
...
Call { callee: Intrinsic { name: "badger_rt_i_atom_set", .. }, args: [Local(378), Local(230)] }
```

The record is stored in an `Atom`, so it escapes to the heap. That is what
the loop measures ("record update (escaping, allocates)"). The loop before
it, "record update (local, scalar-replaced)", already allocates nothing
because `sra` removes the copy. The other three loops read records
allocated once before the loop.

With the `gc-profile` build, the objects allocated between collections
come to **19 998 710**, which is the loop's 20 000 000 iterations less the
young objects still unswept at exit. A perfect escape analysis leaves this
count unchanged, so the expected win on this benchmark is **0 %**, not
5-15 %. For the record, the wall clock of the baseline at four workers
(`--release --workers 4`, five runs) is 2189 / 2362 / 2509 / 2547 / 2850 ms,
median **2509 ms**, of which the escaping loop is about 1.9 s
(95 ns/op × 20 M).

## The other benchmarks

| Program | `Alloc` sites | escape | left over |
|---|---:|---:|---|
| `bench/collections` | 9 | 8 | `describe_error` (error report) |
| `bench/messages` | 72 | 70 | `describe_error`; a record passed once from `main` |
| `bench/parallel_alloc` | 10 | 9 | `describe_error` |
| `bench/heap` | 30 | 29 | `describe_error` |
| `bench/parallel` | 8 | 7 | `describe_error` |
| `bench/runtime` | 147 | 145 | `describe_error`; a closure built once per spawned task |
| `bench/telemetry` | 53 | 48 | `describe_error`; two `display_value` arguments; one `codec.encode` argument; one merge |
| `bench/realworld` workload | 26 | 24 | `describe_error`; one `codec.encode_text` argument |

On these programs, every allocation in a hot loop escapes for real. Each
one is returned, appended to a list or builder, inserted into a map,
spawned, sent, or registered for cleanup. The one leftover that does run
per iteration is in the realworld `json_orders`:
`codec.encode_text({ ..o, amount: .. })` allocates one record per input
line, out of about ten allocations per line. Its only use is as the
argument of a runtime intrinsic, which is where stack allocation runs out.
Under `BADGER_GC_VERIFY=1`, the collector aborts on any root or temporary
root that is neither a heap object nor a registered immortal
(the `check` in `gc.rs`'s collection). The runtime's codec, display and equality functions
read their arguments through the object header and may hold them in
temporary roots while they allocate. So a stack object passed to them
needs a runtime change, and this item rules that out.

## The corpus

Over the 242 programs in `tests/run`, there are 10 279 `Alloc` sites and
9 207 of them escape. The other 1 072 (10 %) break down as follows:

| What the site flows into | Sites | What stack-allocating it would need |
|---|---:|---|
| a merge (a local assigned more than once), used only by field and tag reads | 360 | a loop-reuse check (the slot is reused each iteration) and, for enums, the merge `enums.rs` already rejects (one cause: the null initializer lowering emits) |
| a direct call | 260 | a per-function summary that the parameter neither escapes nor is written with a pointer |
| a call of a static closure | 201 | the same summary, after resolving the closure constant to its function |
| runtime functions that only read (codec, display, `eq`, `cmp`, tracing) | ~220 | runtime cooperation, which is out of scope |
| a pointer comparison | 38 | nothing (the address is never observed except for identity) |

These are test programs, so none of the sites is in a measured hot loop.
They are mostly in `main`, in error-reporting paths, and in setup code that
runs once per task.

## What implementing it would have cost

A sound implementation needs all of the following:

- a new MIR rvalue for a stack allocation, carried through
  `serial`, `verify`, `validate`, the dump and code generation;
- root-slot exclusion in `roots.rs`, since a stack object in a shadow-stack
  slot aborts the verifier and is otherwise skipped as immortal without
  being traced;
- publishing the stack object's pointer fields as shadow-stack roots at
  every safe point while it is live, so the heap objects it holds stay
  alive;
- a liveness check so that a site in a loop does not reuse its slot while
  the previous iteration's object is still reachable;
- for the 461 call sites, a fixpoint over per-function parameter summaries
  that also rules out the callee storing a heap pointer into the object.

A false "does not escape" in any of these is a use-after-free. Against a
measured 0 % on the item's gate benchmark, and no hot-path candidate on the
others, that risk does not pay. This follows the repository's practice of
recording an item as not adopted, with the measurement, rather than
landing a change that does not pay.

## What would change the answer

- **Runtime cooperation.** If the runtime accepts stack objects, the
  read-only runtime arguments become candidates (`codec.encode*`,
  `display`, `eq`). The verifier would need to accept them, and so would
  every runtime function that roots its arguments. The realworld
  `json_orders` site is the only measured per-iteration case.
- **§3.3 region annotations.** These would catch "records passed to a
  function that escapes them", the case the item says 3.2 misses. That is
  the category this survey finds is the common one (`escapes`).
- **The enum merge.** Teaching `enums.rs` to accept the null initializer at
  a merge would take some of the 360 merge sites without stack allocation
  or any collector change. That is a scalar-replacement fix, not escape
  analysis. It is worth doing when a workload shows such a merge in a loop,
  and none of the benchmarks above does.
