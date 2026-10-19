# Standard-library transformation costs — 2026-10-19

Status: implemented and measured on `main` after the JSON decode work
(`f1b1737`). Scope: runtime and standard library, excluding the garbage
collector.

## Result

`list.group_by` built one record per item and sorted them; it now appends each
item to its group's list as it arrives. `chunks_of`/`slice` (and any bulk
append) size the first page to the run they know, instead of growing a small
page by copying. `min_by`/`max_by` keep their best key and item in separate
locals instead of rebuilding a `(K, T)` record per item. `sort_with` merges
into one contiguous buffer per pass instead of rebuilding a paged list.

Measured per item over a 200,000-item `List<Int>` (String rows are the same
list of strings), interleaved fresh `--release` runs of the same program
against the revision before these changes:

| Operation | Before | After | Change |
|---|---:|---:|---:|
| `group_by` by String key (1,000 groups) | 611 ns | 87 ns | −85.8% |
| `group_by` by Int key (1,000 groups) | 344 ns | 186 ns | −45.9% |
| `chunks_of` (64) | 95 ns | 2 ns | −97.9% |
| `min_by` (Int key) | 11 ns | 2 ns | −81.8% |
| `max_by` (Int key) | 13 ns | 1 ns | −92.3% |
| `min_by` (String key) | 18 ns | 8 ns | −55.6% |
| `sort_with` (Int) | 399 ns | 278 ns | −30.3% |
| `slice`, `concat`, `to_array` | 3–5 ns | 0–1 ns | page sizing |
| `unique` (Int, few values) | 24 ns | 20 ns | −16.7% |

Minimums over 5 interleaved repetitions; values of 0–3 ns/op are quantised by
the integer nanosecond-per-op division and move within noise
(`map`, `filter`, `fold`, `zip`, `enumerate`, `dedup`, `partition`, `sum`
are unchanged). Host: AMD Ryzen 7 8745HS, x86-64 Linux.

## What changed

- **`group_by`** (`std/list.bdg`): a map of key to list, appended per item via
  `map_insert_mut` and shareable-list `append`. Groups keep their items'
  order; the map is unordered as before. `ListBuilder` is affine and cannot
  be stored in a `Map`, so per-group accumulators must be shareable lists.
- **Bulk first-page sizing** (`list::push_raw`): when a bulk append creates
  the first page it knows how many elements remain, so it allocates exactly
  that many slots (at least the small-page minimum) instead of the
  grow-by-quadrupling copy path that a one-element-at-a-time `push` needs.
  This is what makes small slices and `chunks_of` cheap.
- **`min_by`/`max_by`** (`std/list.bdg`): best key and best item are separate
  `Option` locals, so no `(K, T)` record is allocated per item.
- **`sort_with`** (`std/list.bdg`): each bottom-up merge pass writes into a
  fresh capacity-`n` buffer (`push` per element) and freezes it into an array
  for the next pass; reads are contiguous array indexing rather than paged
  list reads, and no paged list is rebuilt per pass. Stability and comparison
  order are unchanged.

## Verified

- `tests/run/collections_everyday.bdg` (which exercises `group_by`,
  `sort_by` stability, `sort_with`, `chunks_of`, `dedup`, `unique`,
  `min_by`/`max_by`, `partition`, `zip`, `scan`) and `map_helpers`,
  `codec_paths` golden outputs are byte-identical.
- `cargo test -p badger_cli --release --test programs` passes (76 tests),
  including `run_programs`/`run_programs_release`/`run_programs_multicore`.

## Remaining opportunities (measured, not done)

- `sort_by` 295 ns/op and `sort_with` 278 ns/op, against 15 ns/op for
  `list.sort` (Rust sort over a packed `Vec`). Both compute one key per item
  and then pay for language-level comparisons; a runtime `sort_by` intrinsic
  that calls the key closure once per item, sorts indices with
  `value::cmp` on the key descriptor, and then reorders could approach the
  packed sort's cost.
- `zip` (41) and `enumerate` (13) allocate one pair record per item, which is
  inherent to their result type.
- `unique` on 200,000 distinct Ints is 118 ns/op; almost all of it is HAMT
  insert cost (`map_insert_mut` on an existing key measured at ~45 ns/op),
  which belongs to the memory-management/GC owner.
