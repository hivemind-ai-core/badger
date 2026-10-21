# Purity-gated dead-call removal (the §4.4 follow-up): built, measured, not adopted

Measured 2026-10-21 on `perf/alloc-and-dead-calls` against `2d358d1`. Status:
**not adopted — the population is empty.** `reports/2026-10-20-lifetime-driven-optimisation.md`
ends by naming the one thing that would make the type checker's facts pay:
"the type checker's effect rows (`!{Io}`) and a runtime-function purity table
could prove some of them removable: a total, effect-free callee whose result
is unused … It needs 'cannot diverge or raise a defect', which effect rows do
not state today." This builds exactly that and measures it over the corpus.
Every dead-result call the optimiser sees is either an effect or *is* the
effect; there is nothing to remove.

## What was built

- `opt::simplify`: `removable` keeps its rvalue rules; a new
  `pure_total_callee` table admits a `Rvalue::Call` whose callee is proven
  **total** (cannot fail, diverge, or raise a defect) and **effect free**
  (no effect, no allocation, no write to managed state). A `Call` with a dead
  destination in that table is dropped by `dead_assignments`, exactly as a
  dead `Load` is.
- `opt::tv`: the translation validator models an uninterpreted call as a
  world *effect* (`effect(&format!("call.r.{name}")…)`), so removing one would
  be reported as a refutation. A call in the same table is modelled as
  `pure_op` — a pure operation of its arguments, no world change — so the
  `BADGER_MIR_TV=1` gate can still prove the pass. The table is the single
  source of truth for both.
- `BADGER_MIR_DEADCALLS=1`: a per-callee histogram of calls whose
  destination is dead — `(seen, removed)` — printed by the pipeline, to size
  the table from data rather than from guesses.

## The measurement

`tests/run`, 301 programs, `badger compile` each under
`BADGER_MIR_DEADCALLS=1`. A program is counted once per dead call per
`simplify` invocation (the pipeline runs `simplify` up to four times), so the
sightings below rank the callees; they are ~4–5× the statement count.

96 409 sightings over 86 distinct callees. The table is ordered by the
population, and every head of it is an effect:

| callee | sightings | why it is not removable |
|---|---:|---|
| `badger_rt_defect` | 35 266 | it *is* the defect |
| `badger_rt_err_origin` | 15 853 | records the error origin |
| `badger_rt_list_push` | 13 285 | mutates the list |
| `badger_rt_i_list_builder_append` | 6 073 | mutates the builder |
| `badger_rt_barrier` | 4 240 | GC write barrier |
| `badger_rt_i_write_out` | 3 728 | writes to the sink |
| `badger_rt_i_scope_close` | 1 578 | closes a scope |
| `badger_rt_panic` | 1 538 | raises |
| `badger_rt_i_cleanup_remove` | 1 473 | mutates the cleanup list |
| `badger_rt_map_insert_mut` | 1 320 | mutates the map |
| `badger_rt_i_iter_close` | 1 289 | closes an iterator |
| `badger_rt_global_done` | 1 175 | releases an initializer |
| … | | (74 more, all effects or unprovable) |

The pure readers the item hoped to find are **absent or empty**:

- `badger_rt_list_len`, `badger_rt_list_page_cap` — **0 dead results**: both
  are produced as loop bounds and always used.
- `badger_rt_list_page` (4), `badger_rt_list_get` (30),
  `badger_rt_map_index` — can defect on an out-of-range index, so not total:
  dropping one would swallow the defect.
- `badger_rt_eq`, `badger_rt_cmp` — pure, but never dead.
- `badger_rt_i_text_len` — pure and total, **2 sightings** (~0.4 statements
  over the whole corpus).
- `badger_rt_i_sys_stopping` (626) — a *false* candidate: it calls
  `inspect::system_stopping` (`crates/badger_runtime/src/http.rs:100-103`),
  so it mutates inspection state.
- `badger_rt_caller_loc` (173), `badger_rt_contract_site` (118),
  `badger_rt_take_aux` (168) — all mutate task state.
- `direct` (456) and `closure` (304) — a `Direct` callee could be proven
  pure only by an interprocedural body analysis, and `opt::tv` has no program
  at `validate(src, tgt)` to model it, so the gate's independence would have
  to be weakened or plumbed. At ~90 real dead `direct` calls over 301
  programs, it is not worth either.

So the removable population is ~0: the only table entry with any occurrence
is `badger_rt_i_text_len`, at 0.4 statements per corpus.

## Why the item's premise does not hold

A dead-result call survives `dead_assignments` **because its effect is the
only reason it exists**. The compiler lowers a defect, an error origin, a
scope close, a cleanup, a sleep, a write, a channel send, a push, and an
`Atom.set` each as a call with an unused result — the value is the vehicle,
the effect is the point. Removing the call removes the program. The one
category the item names — a total, effect-free callee whose result is unused
— is a *compiler-introduced* dead computation, and the earlier passes
(`cse`, `simplify`'s own propagation, `dead_assignments` for rvalues) already
remove those before this rule could see them.

## What would change the answer

- **A different fact, not purity**: `badger_rt_list_push` and
  `badger_rt_i_list_builder_append` together are 19 358 sightings — a fifth
  of the population — and they are dead *because the list they build is
  dead*. Removing them needs escape analysis of the built list, which is the
  §3.2 item measured at 0 % on its gate benchmark
  (`reports/2026-10-20-mir-escape-analysis.md`).
- **`defect` and `panic` (36 804 sightings)**: dead-result because the
  surrounding branch is known to be unreachable; that is `cfg` work, not
  purity.

## Reproducing

The numbers above come from a scratch probe, not from code in the tree: the
mechanism, its validator carve-out and the histogram were removed after the
measurement, so the branch is exactly the allocator change plus this report.
The probe was `BADGER_MIR_DEADCALLS=1`: `dead_assignments`
(`crates/badger_mir/src/opt/simplify.rs`) tallied, per callee name, every
`Rvalue::Call` whose destination was dead as `(seen, removed)`, and the
pipeline printed the histogram. Reinstating that tally alone re-measures the
population without adopting the rule, and the run behind the table was:

```sh
cargo build --release -p badger_cli
for f in tests/run/*.bdg; do BADGER_MIR_DEADCALLS=1 target/release/badger compile "$f" -o /tmp/x.out; done 2>&1 | grep deadcall | sort | uniq -c
```
