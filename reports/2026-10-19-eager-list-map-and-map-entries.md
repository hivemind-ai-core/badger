# Eager list fusion and map entry traversal

Reviewed 2026-10-19 on branch `perf/eager-map-snapshots-2026-10-19`,
based on `main` at `8675a4d`. This report covers the two structural costs
called out by the release MIR report: eager `List.map`/`List.filter`
materialization and persistent-map snapshot enumeration.

## Eager list fusion

`xs.filter(p).map(f)` and `xs.map(f).filter(p)` are lowered as one
traversal (`list.filter_then_map`, `list.map_then_filter`) instead of two
passes over an intermediate list. The fused loop interleaves the stages,
so it is only used when the **earlier** callback is side-effect free and
cannot fail:

- no calls, allocation, mutation, `try`, loops, indexing, string
  interpolation, or unsafe blocks;
- no arithmetic that can defect (checked `+ - * / %`, shifts, negation,
  lossy checked conversions);
- reads of concrete record/struct fields, local values, literals, total
  comparisons (`== != < <= > >=`), boolean `&& ||`, and bitwise
  operations are allowed.

Under those conditions the earlier callback's calls are unobservable, and
the later callback still runs on the same elements in the same order, so
its effects and defects are unchanged. An effectful or possibly failing
earlier callback keeps the original two-pass lowering.
`BADGER_NO_LIST_FUSION=1` selects the control form.

## Map entries and snapshots

- `m.entries().map(f)`, `.filter(p)`, and `.fold(init,f)` lower to
  `map.entries_map`, `.entries_filter`, and `.entries_fold`: one trie walk,
  no intermediate `List<(K,V)>`. Entry order is the same trie order that
  `map_entries` uses. `BADGER_NO_MAP_FUSION=1` selects the control form.
- `for (k, v) in m` already walked the trie directly; that path now stores
  the current node entry's address in the cursor, and generated code loads
  key and value directly from the node. This removes two stores per entry
  from the cursor. The cursor's trie nodes rooted on its stack keep the
  entry alive; the interior address is not traced.
- Persistent insertion remains copy-on-write: an epoch-owned wrapper is
  edited in place until a retained snapshot requires freezing and a fresh
  ownership epoch. That is the semantic minimum for sharing versions (a
  mutable-in-place snapshot would corrupt retained versions), so no
  representation change was made there.

## Measurements

Host: AMD Ryzen 7 8745HS, 16 logical CPUs, Linux 7.2.2, shared with two
unrelated `rustc` processes (load average 5–8). Runs alternated between
the `main` binary and this branch in both orders; kernel medians are over
20 warm iterations per process and the reported value is the median of
interleaved process samples. Treat small deltas as directional.

| case | main | branch | delta |
|---|---:|---:|---:|
| realworld `reconcile` (filter→map chain), kernel | 5.753 ms | 5.191 ms | −9.8% |
| same binary, fusion vs `BADGER_NO_LIST_FUSION=1` | 6.547 ms | 6.334 ms | −3.3% |
| realworld `state` (snapshot iterate), kernel | 1.170 ms | 1.148 ms | −1.9% |
| `bench/collections` run time | 85.19 ms | 84.96 ms | −0.3% |
| `bench/runtime` run time | 28.74 ms | 20.82 ms | −27.5% (from the release MIR work) |
| `bench/records` run time | 440.5 ms | 444.2 ms | +0.8% |

Structural checks on a filter→map program: the control MIR contains two
list-builder creations for the chain; the fused form contains one.
`m.entries().map(..)` plus `.fold(..)` produces zero `map_entries` calls in
the fused MIR and three in the control, with identical output.
`BADGER_NO_LIST_FUSION` and `BADGER_NO_MAP_FUSION` reproduce the original
lowering.

Compilation cost is unchanged apart from the inliner's higher-order
budget (96 statements instead of 80, so the fused helper can be inlined):
realworld startup/compile stayed within the previous +2–12% range of the
release MIR pipeline, and binary MIR for the 800-function synthetic
program remains smaller than main (1.79 MB vs 2.59 MB).

## Validation

`cargo test -p badger_cli --release --test programs` (76 passed),
`mir_optimization` (7 passed), `option_niche`, `scalar_enums`,
`sequence_fusion`, `docs` (API reference regenerated), and the full
`tests/run` corpus at `release`, `aggressive`, and `superopt` all match
their expected output and exit codes.

## Remaining persistent-map cost

Profiling `state` shows `hamt::insert_at` (~19%), `map::insert_owned`
(~11%), the digest loop (~15%), cursor iteration (~10%), and GC
allocation/collection (~12%). Insertion is dominated by CHAMP path copies
and node edits required to keep retained snapshots immutable; hash and
equality are small. No safe compiler change was found that removes those
copies without changing observable snapshot semantics. Future work should
target range analysis (to remove checked digest arithmetic) or a
versioned/epoch representation with cheaper node stamps, not more eager
materialization.
