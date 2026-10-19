# Sequence fusion and pre-sized builders

Follow-up on sequence/iterator operations. `Seq` already provided the lazy
adapter model (`map`, `filter`, `take`, `flat_map`, ...), and `for`, `fold`,
and `count` already fused supported sources/stages into one loop. Two gaps
remained for common pipelines:

1. `collect` was not a fusion terminal. `xs.seq().map(f).filter(g).collect()`
   still built the lazy adapter closures, `Cell`s, and iterator objects and
   called them per item before the generic `list.collect` loop.
2. Both eager `List.map` and collected lists grew their page directory and
   allocated pages while iterating, even when the result size was known.

## Changes

- `collect` and `sum` are fusion terminals in `lower/fusion.rs`.
  Supported `Seq` sources/stages now lower to one loop that appends to a
  list or accumulates an integer; stored `Seq` locals are eligible too, so
  a reusable view fuses on every terminal. `collect` sizes its builder from
  the source count (`range` bounds or list length) when no `filter` stage
  can drop items; `take` is still an upper bound and `finish` trims the
  directory.
- New `list_builder_capacity` intrinsic and `list::with_capacity`: the
  final directory and one page per page-capacity elements are allocated up
  front, and appends fill those pages in place. No directory copy or page
  allocation happens during the loop. Reserved pages are GC roots while
  the object is a builder (`BUILDING` bit in the capacity word), so a
  collection between allocation and filling cannot free them. `finish`
  clears the flag, trims the directory to the visible pages, and hands the
  storage to the result list.
- Only scalar element types reserve pages. For managed element types
  (records, tuples, strings, lists) every collection would scan the empty
  reserved pages; those builders grow incrementally, keeping their old
  behavior. This avoided regressions in `enumerate`/`zip` (tuple outputs).
- Eager `List.map` and `List.scan` use the sized builder; `List.zip` caches
  the two lengths instead of calling `list_len` repeatedly. `enumerate`
  keeps a plain builder (tuple elements).
- No public API was added. The intrinsic is standard-library internal.

## Measurements

Host shared with unrelated builds; values are medians of 8 alternating
pre/post process pairs, each over 8 warm iterations of one million items
(`list.range(0, 1_000_000)`).

| operation | before | after | delta |
|---|---:|---:|---:|
| `list.map` (eager) | 8.11 ms | 7.88 ms | −2.8% |
| `Seq.map.collect` | 49.96 ms | 11.88 ms | **−76.2%** |
| `Seq.filter.collect` | 39.38 ms | 12.42 ms | **−68.5%** |
| `Seq.map.filter.map.collect` | 64.21 ms | 12.91 ms | **−79.9%** |
| `Seq.map.sum` | 33.56 ms | 10.04 ms | **−70.1%** |
| `list.enumerate` | 21.31 ms | 22.51 ms | neutral (noise; tuple output) |
| `list.scan` (Int state) | 7.97 ms | 5.91 ms | −25.8% |
| `list.zip` | 54.84 ms | 40.22 ms | −26.7% |

The collection wins come from removing the lazy adapter chain entirely:
the fused loop calls the callbacks directly and writes into one builder.
The pre-sized builder removes page/directory growth inside the loop; the
eager-map and scan gains are smaller because their loop body is the cost.

## GC correctness

A pre-sized builder publishes pages before any element is written, so a
full collection must consider pages below `dir_used`, not only pages below
`len` (which is still zero for most of them). The `BUILDING` capacity-word
flag makes `list::trace` mark those pages; tracing their zeroed element
counts is safe. `finish` clears the flag and trims the directory, so a
finished list traces only visible pages. `BADGER_HAMT_SLACK`-style A/B is
not needed here; `BADGER_LIST_PREALLOC=0` forces plain builders for
diagnostics.

Validated with `BADGER_GC_STRESS=1 BADGER_GC_VERIFY=1` on empty, one-item,
page-boundary, large, filtered, `take`n, pointer-element, and stored-Seq
collects; `cargo test ... --test programs` (76), `mir_optimization` (7),
`docs` (3), and the full `tests/run` corpus at `release`, `aggressive`, and
`superopt` all match expected output and exit codes.

## Remaining ideas

A direct fill loop for known-size scalar collects could skip the per-item
page-index arithmetic and capacity check that `list_builder_append` still
performs; it would need careful per-page count/barrier publication at
safepoints. Pre-sizing filtered collects would need a selectivity estimate,
which the language does not provide. Both are measurable next steps, not
required for the wins above.
