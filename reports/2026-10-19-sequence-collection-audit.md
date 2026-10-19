# Sequence and collection audit: gaps, additions, and optimizations

Reviewed 2026-10-19 against the working tree (the draft-0 language and
runtime, `cargo build` current). Scope: the sequence/collection layers of
the standard library (`std.list`, `std.seq`, `std.stream`, `std.map`,
`std.set`, `std.sorted`, `std.deque`, `std.heap`, `std.bits`, `std.lru`,
`std.array`, `std.buffer`, `std.text`) and the typed record operations
(`std.records`). Precedents consulted: Clojure's `clojure.core` sequence
library and transducers, Elixir's `Enum`/`Stream`/`Map`/`MapSet`, Rust's
`Iterator`/`BTreeMap`/`BTreeSet`, and Swift/Kotlin sequence APIs.

The goal of this pass is that

- records of collections and collections of records are first-class:
  mapping, filtering, grouping, joining, indexing, and reshaping are one
  call each and stay lazy until a boundary;
- every operation that is O(n) in another language is available here and
  does not silently become O(n²); and
- the compiler/runtime work that would make the new surface efficient is
  written down, with the extension points named.

Sections 1–3 record the findings; section 4 records what this change
implements; section 5 is the optimization backlog; section 6 is the
validation.

## 1. Current surface

Inventory of the public functions/types, by module (source of truth
`crates/badger_hir/std/*.bdg`; docs generated into `docs/api/`).

- **`std.list`** — `map`, `filter`, `filter_map`, `flat_map`, `fold`,
  `find`, `position`, `find_map`, `any`, `all`, `count`, `enumerate`,
  `take`, `skip`, `take_while`, `skip_while`, `scan`, `zip`, `min_by`,
  `max_by`, `partition`, `sort`, `sort_by`, `sort_with`, `dedup`,
  `unique`, `chunks_of`, `group_by`, `try_map`, `try_fold`, `try_each`,
  `all_ok`, `validate`, `partition_results`, `sum`, `range`, `len`,
  `get`, `chunks`, `first`, `last`, `contains`, `reverse`, `slice`,
  `append`, `concat`, `is_empty`, `to_array`, `seq`, `element_ref`,
  `builder`/`build`/`empty`/`collect`, `filter_then_map`,
  `map_then_filter` (fusion targets).
- **`std.seq`** — `map`, `filter`, `filter_map`, `flat_map`, `zip`,
  `take`, `skip`, `take_while`, `skip_while`, `scan`, `chunks`,
  `enumerate`, `fold`, `try_fold`, `try_each`, `try_map`, `try_collect`,
  `count`, `find`, `find_map`, `any`, `all`, `first`, `last`, `min_by`,
  `max_by`, `partition`, `sum`, `group_by`, `collect`, `range`,
  `from_range`, `from_fn`, `scoped`, `close_iter`.
- **`std.stream`** — `from_fn`, `from_receiver`, `from_list`, `close`,
  `map`, `filter`, `filter_map`, `enumerate`, `scan`, `find`, `take`,
  `take_while`, `batches`, `try_map`, `each`, `try_each`, `fold`,
  `drain`, `collect`, `collect_at_most`, `try_collect`.
- **`std.map`** — `empty`/`insert`/`remove`/`get`/`contains`/`len`/
  `to_list`/`entries`/`keys`/`values`/`get_or`/`update`/`merge`/
  `merge_with`/`map_values`/`filter`/`is_empty`, builders,
  `from_list`/`collect`, `entries_map`/`entries_filter`/`entries_fold`
  (fusion targets).
- **`std.set`** — `empty`/`insert`/`remove`/`contains`/`len`/`to_list`/
  `union`/`intersection`/`difference`/`is_subset`/`filter`/`is_empty`,
  builders, `from_list`/`collect`.
- **`std.sorted`** — `SortedMap`: `empty`/`of`/`get`/`contains`/`insert`/
  `remove`/`first`/`last`/`floor`/`ceiling`/`range`/`to_list`/`keys`/
  `values`. No `SortedSet`.
- **`std.deque`** — `empty`/`of`/`len`/`is_empty`/`push_front`/
  `push_back`/`pop_front`/`pop_back`/`peek_front`/`peek_back`/`to_list`.
- **`std.heap`** — `empty`/`of`/`len`/`is_empty`/`push`/`peek`/`pop`/
  `merge`/`to_list`.
- **`std.bits`** — `empty`/`of`/`contains`/`insert`/`remove`/`union`/
  `intersection`/`difference`/`is_empty`/`len`/`to_list`.
- **`std.records`** — `Extends`, `Projects`, `project`, `remove`,
  `merge`.

## 2. Gaps against Clojure / Elixir / Rust

Only operations that a strong data-processing language provides as
library, that this language can type-check, and that are absent, are
listed. "→" marks the precedent (Clojure `clojure.core`, Elixir `Enum`/
`Stream`/`Map`/`MapSet`, Rust std).

| Missing | Precedent | Why it matters here |
|---|---|---|
| `each` / `for_each` on list & seq | Clojure `run!`, Elixir `Enum.each`, Rust `for_each` | side-effecting traversals (logging, enqueue, counters) are common and currently need a `for` loop or a `fold` returning `Unit` |
| `min` / `max` on plain order | Clojure `min`/`max`, Elixir `Enum.min`/`max` | today only `min_by`/`max_by` exist; `list.min()` needs `min_by(id)` gymnastics |
| `sum` over `Float`/`sum_by`; `product` | Elixir `Enum.sum`/`product`, Rust `Iterator::product` | `sum` is `Int`-only; products/float sums are missing |
| `frequencies` (count by value) | Clojure `frequencies`, Elixir `Enum.frequencies` | the canonical "count occurrences" step in data extraction; today a manual `group_by` + `len` |
| `index_by` (build a map by key) | Clojure `group-by`/`into {}`, Elixir `Enum.into` | keying a collection of records by an id is the most common reshape; `group_by` always builds lists |
| `chunk_by` / consecutive grouping | Clojure `partition-by`, Elixir `Enum.chunk_by` | `group_by` groups globally; runs/sessions/segments need consecutive grouping |
| `windows` / sliding windows | Clojure `partition n 1`, Elixir `Enum.chunk_every/…`, Rust `windows` | time-series/lag features; `chunks_of` only gives non-overlapping chunks |
| `interpose` / `intersperse` | Clojure `interpose`, Elixir `Enum.intersperse` | building delimited output without `map_join` |
| `split_at`, `split_while` | Clojure `split-at`/`split-with`, Elixir `Enum.split_while` | one-pass split; today `take_while` + `skip_while` scan twice |
| `dedup_by` | Elixir `Enum.dedup_by` | consecutive de-duplication by a key (adjacent duplicate records) |
| `flatten` | Clojure `flatten`, Rust `Iterator::flatten` | `flat_map` exists but a plain `List<List<T>> -> List<T>` is missing |
| `zip_with`, `unzip` | Elixir `Enum.zip_with`/`unzip`, Clojure `map`+`unzip` | pairwise combination and splitting a list of pairs |
| `fold_while` / `reduce_while` | Clojure `reduced`, Elixir `Enum.reduce_while` | folds that stop early without an error channel |
| `take_last` / `drop_last` | Elixir `Enum.take/2` negative, Clojure `take-last` | tail slices |
| `repeat`, `iterate` (lazy) | Clojure `repeat`/`iterate`, Elixir `Stream.repeatedly`/`iterate` | generating lazy inputs without hand-writing `from_fn` |
| `step_by` | Rust `Iterator::step_by`, Elixir `Stream.take_every` | sampling/downsampling |
| `chain` / `concat` of sequences | Clojure `concat`, Elixir `Stream.concat` | append one lazy source to another |
| `dedup`, `distinct` (lazy) | Clojure `distinct`, Elixir `Stream.uniq` | adjacent/all de-duplication inside a pipeline |
| `position`/`index_of` on seq | Clojure `keep-indexed`, Elixir `Enum.find_index` | index of first match without collecting |
| `map_keys` | Clojure `map-keys`, Elixir `Map.new` | reshape key spaces (symbol/string normalization) |
| `select_keys` / `omit_keys` | Clojure `select-keys`/`dissoc`, Elixir `Map.take`/`drop` | pick/drop a key set in one call |
| `put_new` / `update_with` | Elixir `Map.put_new`/`update/4`, Rust `entry().or_insert` | defaults and counters without a `get`+`insert` dance |
| `find`/`any`/`all`/`count`/`each`/`fold` over map entries | Clojure/Elixir map APIs | only `entries_fold` (fusion target) exists publicly today |
| `invert` | Clojure `map-invert`, Elixir `Map.new` | invert a map's values to keys |
| lazy `keys`/`values`/`entries` views | Elixir `Map.keys` streams, Rust `Iterator` | eager `List`s force materialization; see §5 |
| `set.map`, `is_superset`, `is_disjoint`, `symmetric_difference` | Rust `BTreeSet`, Clojure | set algebra is incomplete (only union/intersection/difference/subset) |
| `SortedSet` | Clojure `sorted-set`, Rust `BTreeSet` | ordered membership/range queries have no type |
| `SortedMap` `update`/`get_or`/`filter`/`map_values`/`merge`/`pop_first`/`pop_last`/`range_inclusive` | Rust `BTreeMap` | only point/range reads today |
| deque `get`/`contains`/`each`/`fold`/`reverse` | Rust `VecDeque` | deque is write/scan-only |
| heap `by_key` (documented, absent), `of` alias | Rust `BinaryHeap` with key | `heap.bdg`'s own header promises `by_key`; it does not exist |
| bitset `is_subset`/`symmetric_difference`/`toggle`/`min`/`max` | Rust `HashSet`-like | bit-set algebra is incomplete |
| stream `skip`/`skip_while`/`zip`/`count`/`last`/`any`/`all`/`min_by`/`max_by` | Elixir `Stream` | `Stream` adapters lag `Seq` badly, so channel pipelines are limited |
| `records.merge_left` (left-biased) | Clojure `merge` variants | only right-biased `merge` exists |
| `records.omit` (several labels) / `records.rename` | Clojure `dissoc`/`rename-keys` | reshaping records in a collection needs one-copy ops |
| `records.field_count`/`labels` (compile-time reflection) | Clojure `keys`/`(count m)` | row-generic code cannot ask a shape for its labels |

Two documentation defects fell out of the audit: `std.heap`'s header
promises `by_key` (never implemented; corrected in this change) and
`std.list.sort_by`'s header comment is accurate but `map`'s fusion note
(`lower.rs:2432`) claims a named-function callback can fuse while the
implementation accepts only an `ExprKind::Lambda` (`lower.rs:2469`).

## 3. What is already strong

- **Eager list fusion** (`filter∘map`, `map∘filter`) and **map entry
  fusion** (`entries().map/filter/fold`) remove intermediate
  collections for the hottest two-stage shapes (`ARCHITECTURE.md` §"Eager
  list fusion", "Map entry fusion").
- **Seq pipeline fusion** (`badger_mir/src/lower/fusion.rs`) flattens
  arbitrary-length `map`/`filter`/`take` chains over finite sources into a
  counted loop, preserving callback order (including take suppressing
  upstream effects). Gated to release+optimized builds. This change adds
  `skip`/`step_by` stages and a `collect` terminal, so the default
  `.map().filter().collect()` idiom fuses too (§4).
- **Closure devirtualization / higher-order inlining**
  (`opt/specialize.rs`, `opt/inline.rs`) make the callbacks of any helper
  (fused or not) direct calls.
- **Builder append inlining** (`lower.rs:2181`) and **list index inlining**
  (`lower.rs:3477`) keep the hot loops free of runtime calls.
- **Transient/owned map reuse** (`badger_mir/src/maps.rs`) edits a
  private fresh map in place during construction.
- **Fallible processing** (`try_map`/`try_fold`/`try_each`/`validate`/
  `partition_results`/`all_ok`) and **scoped iterators** (`seq.scoped`
  closing on early exit) are unusually complete for a young language.
- **Persistent structures** (`Map`/`Set` HAMT, AVL `SortedMap`, leftist
  `Heap`, banker's `Deque`, `BitSet`) with value semantics.

## 4. Implemented in this change

Pure-library additions to `crates/badger_hir/std/*.bdg` (no compiler
change needed; std is compiled as ordinary language modules), each with a
free function and where applicable a method wrapper:

- **`std.list`**: `each`, `min`, `max`, `sum_by`, `product`, `interpose`,
  `flatten`, `split_at`, `split_while`, `chunk_by`, `dedup_by`, `windows`,
  `frequencies`, `index_by`, `zip_with`, `unzip`, `fold_while`,
  `take_last`, `drop_last`.
- **`std.seq`**: `chain`, `dedup`, `distinct`, `interpose`, `chunk_by`,
  `windows`, `zip_with`, `step_by`, `frequencies`, `index_by`,
  `fold_while`, `each`, `position`, `min`, `max`, `product`, `repeat`,
  `iterate`.
- **`std.stream`**: `skip`, `skip_while`, `zip`, `count`, `last`, `first`,
  `any`, `all`, `min_by`, `max_by`.
- **`std.map`**: `map_keys`, `select_keys`, `omit_keys`, `put_new`,
  `update_with`, `find`, `any`, `all`, `count`, `each`, `fold`, `invert`.
- **`std.set`**: `is_superset`, `is_disjoint`, `symmetric_difference`,
  `map`, `each`, `fold`, `any`, `all`.
- **`std.sorted`**: `SortedMap` gains `get_or`, `update`, `filter`,
  `map_values`, `each`, `fold`, `merge`, `pop_first`, `pop_last`,
  `range_inclusive`; new **`SortedSet<K>`** with
  `empty`/`of`/`insert`/`remove`/`contains`/`len`/`is_empty`/`first`/
  `last`/`floor`/`ceiling`/`range`/`range_inclusive`/`to_list`/`union`/
  `intersection`/`difference`/`is_subset`/`filter`.
- **`std.deque`**: `from_list`, `get`, `contains`, `each`, `fold`,
  `reverse`.
- **`std.heap`**: `from_list`; the stale `by_key` header promise is
  corrected (a comparator-carrying heap is deferred; §5 O10).
- **`std.bits`**: `from_list`, `is_subset`, `symmetric_difference`,
  `toggle`, `min`, `max`, `each`, `fold`.
- **`std.records`**: `merge_left`; `omit` (drop several literal labels in
  one copy); `rename` (one-copy relabel); `field_count` (compile-time
  field count for row-generic code).

### Compiler optimization applied: sequence fusion terminals and stages

The added lazy adapters are only useful if ordinary pipelines over them are
cheap. Sequence fusion previously recognized `map`/`filter`/`take` stages
but only `fold`/`count`/`for` terminals, so the default
`.map(...).filter(...).collect()` built per-stage iterator closures and
`Cell` counters instead of one loop. This change extends
`crates/badger_mir/src/lower/fusion.rs`:

- a **`collect` terminal** (`std.list.collect(seq)` and the
  `std.seq.collect()` method) builds the result list inside the loop
  (`badger_rt_list_new`/`badger_rt_list_push`) instead of calling the
  helper; and
- **`skip` and `step_by` stages** (counter stages mirroring `take`), so
  `range.filter(p).skip(k).map(f).collect()` and
  `range.step_by(k).map(f).collect()` fuse, with the skipped items still
  running earlier stages and not later ones, exactly as the lazy library
  does.

Both are gated by the existing `Mono::fuse_sequences` (release+optimized
non-live builds). New coverage: `tests/run/seq_collect_fusion.bdg` and
`tests/run/seq_skip_step_fusion.bdg` pin results and callback order across
debug/release/aggressive/superopt, and
`crates/badger_cli/tests/mir_optimization.rs::a_collect_pipeline_is_one_loop`
checks the fused MIR appends in place and does not call the collect helper.

Remaining fusion work (stages for the stateful adapters `enumerate`,
`filter_map`, `dedup`, `interpose`, `chunk_by`, `windows`, `zip`, and the
`last`/`find`/`any`/`all`/`try_fold` terminals) is O3 below.

The record operations that depend on the record's *shape*
(`omit`/`rename`/`field_count`) required compiler work, because standard
library files are ordinary modules and the result type is computed from
the operand row — they are not expressible as plain generic functions.
The changes are the minimal shape-dependent path used by the existing
`project`/`remove`/`merge`:

- `crates/badger_hir/std/records.bdg` — three `extern "intrinsic"`
  declarations.
- `crates/badger_typeck/src/infer.rs` — two label-literal special cases
  (`omit` reads a literal label list; `rename` reads two literal labels),
  following the `remove` template (`infer.rs:1388-1417`).
- `crates/badger_mir/src/lower.rs` `fn intrinsic` — `omit` reuses the
  existing `project | remove` alloc-and-copy arm; `rename` is a new ~15
  line arm that copies each target field from its source label; and
  `field_count` folds to an `Int` constant. No codegen or runtime change
  (the scout confirmed record ops are `Rvalue::Alloc` + per-field
  `Store`).

## 5. Optimization backlog (compiler / runtime)

Findings only; not all are implemented here. Each names the extension
point. Ordered by expected impact on data processing.

### O1 — Sub-list sharing for `take`/`skip`/`slice`/`take_while`/`skip_while`
`std.list.take`/`skip` are `list_slice`, and `list_slice`
(`badger_runtime/src/intrinsics_std.rs:171`) copies every element into a
fresh list (`push_run`). A `List<T>` is a length plus a page directory
whose pages are already shared immutably by persistent `append`
(`list.rs:1-12`, `:660`). A sub-range whose ends fall inside pages can
share every complete page and copy only the two boundary pages, making
`take(k)` on a large list O(pages) instead of O(k). Same for
`list_concat`, which copies both operands (`intrinsics_std.rs:153`) though
it could share `a`'s complete pages. `list.chunks()` likewise copies each
page into an `Array` (`intrinsics_std.rs:289`; the spec already lists
zero-copy page views as proposed).

### O2 — Extend eager list fusion
`try_fused_list_map` (`badger_mir/src/lower.rs:2428`) fuses exactly two
stages and only `map∘filter`/`filter∘map`, and only when the earlier
callback is an `ExprKind::Lambda` (`:2469`). High-value extensions, with
the recipe in the report's scout notes:
- `map∘map` (a single loop; both callbacks run once per element in order,
  so no purity guard is needed) and `filter∘filter`
  (`filter_then_filter`).
- recursive stage collection so chains longer than two fuse (`a().b().c()`
  today fuses at most one pair).
- `filter_map` as a stage, and `flat_map` inner stages.
- `map(...).sort_by(key)` → a single `list.map_sort_by(xs, f, key)` (the
  earlier `map` must be pure/infallible, as today).
`try_fused_*` hooks live at `lower.rs:1443` (method form) and `:2546`
(free-call form); add the fused loop in `std/list.bdg` and a kill switch
(`BADGER_NO_*_FUSION`) plus a control test, as the existing fusion does.

### O3 — Extend sequence fusion (`badger_mir/src/lower/fusion.rs`)
**Partially applied.** Sources `range`/`from_range`/`list.of`/`list.seq`/
eligible stored `Seq`; stages `map`/`filter`/`take` **+ `skip`/`step_by`**;
terminals `fold`/`count`/`for` **+ `collect`**; `Mono::fuse_sequences` is on
only for release+optimized non-live builds
(`badger_driver/src/lib.rs:1196`). Remaining extensions, each an `enum`
variant plus arms in `sequence_plan`/`prepare_sequence`/`lower_fused`:
- stages: `enumerate`, `zip`, `filter_map`, `scan`, `dedup`, `interpose`,
  `zip_with`, `chunk_by`, `windows` (the last three bind a buffer — add
  them to `sequence_local_use`/`eligible_sequence_locals` so escaping
  pipelines are not fused; `enumerate`/`filter_map` need tuple/Option
  construction).
- terminals: `last`, `find`, `any`, `all`,
  `try_fold`/`try_each` (need an early-exit phi).
- allow the common `list.collect()` terminal at `run`/`-O0` (the pipeline
  `.map().filter().collect()` is the default idiom and currently builds
  per-stage iterator closures and `Cell`s outside optimized builds).
Any new counter stage must keep the take-style pre-request check
(`fusion.rs:308`) so upstream effects are not run one extra time.

### O4 — `sort_by` decorate/sort/undecorate allocation
`std.list.sort_by` (`list.bdg:492`) builds a `List<(key, i, x)>` for every
element, sorts, then maps out — `n` tuple allocations and two passes. A
`list_sort_by_key` intrinsic (or MIR rewrite) that hoists keys into a
contiguous buffer and sorts `(key, payload)` pairs with the comparator
avoids the tuple list. This is the heaviest remaining list operation for
data processing. `sort_with` is already a buffer-based merge sort.

### O5 — `group_by` / `merge_with` accumulation
`group_by` (`list.bdg:600`) does a persistent `list_append` per item; each
append copies a partial tail page. In practice the old version is dead
the moment it is replaced, so the runtime can claim the same page slot in
place (O(1) amortized), but a `Map<K, ListBuilder<T>>` accumulator
(builder per group, finished once) would make that explicit and O(n) in
all cases — it is blocked only because a `Map` value must be shareable and
a `ListBuilder` is affine. Enabling affine map/set values, or a dedicated
`map_group_by` intrinsic, is the change to make. `merge_with`/`merge`
(`map.bdg:125`) rebuild both maps into a fresh one; starting from the
larger map as a transient and inserting only the other's entries would
share untouched subtrees (the `badger_mir/src/maps.rs` ownership machinery
already distinguishes private/owned/snapshot maps).

### O6 — Lazy map/set views and iteration terminals
`map.keys`/`values`/`entries` (`map.bdg:94`) materialize `List`s, so a
pipeline over a large map allocates an intermediate list unless the
`entries()` fusion fires. A cursor-backed lazy `Seq` over a map (or
extending the fusion to `keys().map`/`values().map`) avoids that. `invert`,
`map_keys`, and `frequencies` should route through the transient-map path
where the result map is private.

### O7 — `deque` element-per-node allocation
`std.deque` boxes every element (`deque.bdg:14-18`, `cons`). A chunked
representation (arrays of ~32 elements per node, as Clojure's
`PersistentQueue`) cuts allocations ~32× and improves cache behaviour for
the sliding-window/worklist uses the new `windows` and queue APIs
encourage. Same consideration for `sorted`'s per-node `Box` and its
`range` up-stack (`sorted.bdg:254`), which allocates a `Box` per step; an
explicit preallocated `Buffer` stack removes that.

### O8 — `bits` word-list rebuild
`bits.with_word` (`bits.bdg:29`) copies the whole word list to change one
word, so `insert` is O(n/63) and building a set of k bits is O(k·n/63).
An owned/builder word array for bulk construction, plus a word-level
insert path, makes construction O(n/63 + k).

### O9 — `list_contains`-family page scans
`list_contains` (`intrinsics_std.rs:194`) calls `list::get` per index,
re-resolving the page each time; `find`/`position`/`any`/`all`/`count`
written in `list.bdg` iterate pages already, but the intrinsic-backed
`contains` should scan page-by-page. Small but free.

### O10 — Heap `by_key` ordering
`by_key` cannot store a bare key with the element without making the
element comparable (ordering a `(K, T)` pair compares `T` too, and
`PartialOrd` for a key-only wrapper is not user-implementable today).
Either a user-implementable `PartialOrd`/mirror protocol, a comparator-
generic `Heap`, or a runtime comparator value is needed. The stale header
promise was removed rather than shipped half-working; the operations that
exist (`of`/`push`/`pop`/`peek`/`merge`/`to_list`) cover the current uses.

## 6. Validation

The additions were exercised end to end; every command below passed.

- `cargo test -p badger_cli --test programs -- run_programs --exact`
  (all `tests/run` programs, default flags), `-- run_programs_release
  --exact` (release MIR pipeline, sequence fusion on),
  `-- run_programs_compiled --exact` (binary MIR), and
  `-- run_programs_multicore --exact` (4 workers): each compares stdout and
  exit code. The new cases are
  `tests/run/collections_extra.bdg`, `seq_extra.bdg`,
  `collections_misc.bdg`, `records_extra.bdg`, `seq_collect_fusion.bdg`,
  and `seq_skip_step_fusion.bdg` (with their `.out`), each also run at
  `aggressive`/`superopt` during development and shown byte-identical to
  the unfused form.
- `cargo test -p badger_cli --test programs -- reject` — the four new
  `tests/reject` cases (`records_omit_label`, `records_omit_missing`,
  `records_rename_label`, `records_rename_missing`) report the expected
  codes `E0344` / `E0301`.
- `cargo test -p badger_cli --test docs` — `api_reference_current`
  (`badger doc std --out docs/api --check`, the reference regenerated here)
  and `doc_examples_check` (every checked `badger` block in the docs) pass.
- `cargo test -p badger_cli --test mir_optimization` (8 passed) — the eager
  list/map fusion control and enabled paths are unchanged, and the new
  `a_collect_pipeline_is_one_loop` shows the `collect` terminal fuses
  (fused MIR appends in the loop; the control still calls the helper).
- `cargo test -p badger_cli --test programs -- examples_run --exact` — the
  numbered examples still run to their expected output.

Direct smoke runs of the new operations (`badger run` on the four new
programs) show `merge_left`/`merge` differing only on the overlapping
field, `rename` copying the source label into the target while keeping the
rest, `omit` dropping several labels in one copy, and `field_count`
matching the layout (`4` for a four-field record, `3` for a three-field
struct).
