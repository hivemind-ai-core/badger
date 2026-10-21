# Collections: choosing and costs

Which collection to use, what each operation costs, and the idioms that
keep transformations within clear memory bounds. The representations are
specified in [memory and collections](../spec/04-memory-and-collections.md);
every function is listed in the [API reference](api/README.md).

## Which collection

| Need | Use | Why |
|---|---|---|
| An immutable sequence you index, iterate, share | `List<T>` | Paged and frozen: constant-time indexing, cheap to share between tasks, structural sharing on `append`/`concat` |
| Building a list item by item | `list.build` / `ListBuilder<T>` | One allocation path, no intermediate snapshots; `finish` freezes in place |
| A fixed-length contiguous block | `Array<T>` | Contiguous, immutable; what FFI and bulk numeric code want |
| Contiguous mutable storage | `Buffer<T>` | Affine, grows by relocation; freeze into an `Array` (or `Bytes`) when done |
| Lookup by key | `Map<K, V>` | Persistent: `insert`/`remove` give new maps sharing structure with the old |
| Membership | `Set<T>` | A map without values |
| A pipeline over items, possibly large or infinite | `Seq<T>` | Lazy: nothing runs until consumed; early exits close scoped sources |
| Items arriving over time from other tasks | `Stream<T>`, channels | Suspending: a consumer parks until the next item |
| Keys in order: first/last, floor/ceiling, ranges | `sorted.SortedMap<K, V>` | A persistent AVL tree keyed by `<`: O(log n) lookups and updates, entries in key order |
| Key membership in order | `sorted.SortedSet<K>` | The same tree without values: ordered iteration, ranges, and set algebra |
| The smallest item next (priority queue) | `heap.Heap<T>` | A persistent leftist heap: O(log n) `push`/`pop`/`merge`, O(1) `peek` |
| A queue or stack used at both ends | `deque.Deque<T>` | Amortized O(1) `push_front`/`push_back`/`pop_front`/`pop_back` |
| A bounded cache that evicts the least recently used | `lru.Cache<K, V>` | `get` records a use and returns the updated cache; O(log n) per operation |
| Sets of small non-negative integers | `bits.BitSet` | 63 per word: O(1) `contains`, word-wise `union`/`intersection`/`difference` |

Reshaping, in one call each (see each function's entry in the
[API reference](api/README.md)):

- **Count**: `list.frequencies` (occurrences by value), `list.count`,
  `map.count`, `seq.frequencies`.
- **Key**: `list.index_by`/`seq.index_by` (one item per key),
  `list.group_by` (a list per key), `map.map_keys`/`invert`.
- **Split**: `list.split_at`, `list.split_while`, `list.take_last`/
  `drop_last`.
- **Window**: `list.chunks_of`, `list.windows`/`seq.windows`
  (overlapping), `seq.chunks`.
- **Runs**: `list.chunk_by`/`seq.chunk_by` (consecutive equal keys),
  `list.dedup`/`dedup_by`/`unique`, `seq.dedup`/`distinct`.
- **Sides**: `list.interpose`/`seq.interpose`, `list.flatten`,
  `seq.chain`, `list.zip_with`/`seq.zip_with`, `list.unzip`.
- **Maps and sets**: `map.select_keys`/`omit_keys`/`put_new`/
  `update_with`, `set.is_superset`/`is_disjoint`/`symmetric_difference`/
  `map`.

Rules of thumb:

- Build with a builder (or `list.build`, `map.build`, `set.build`), not by
  repeated `append`/`insert` on snapshots. A loop of `xs = xs.append(x)`
  is correct but may copy a partial page each time; a loop of
  `xs = list.concat(xs, [x])` copies the whole list each time (quadratic;
  the compiler warns, E0454).
- Keep pipelines lazy (`Seq`) until you need a list, then `collect()`
  once. A `Seq` over a scoped resource must be consumed inside the
  resource's scope.
- Iteration order of `Map` and `Set` is unspecified (stable for one
  value, not across inserts). Sort explicitly (`keys().sort()`,
  `sort_by`), or keep a `sorted.SortedMap`, for stable output.
- `SortedMap`, `Heap`, `Deque`, `Cache`, and `BitSet` are persistent like
  the others: each operation returns a new value. Keep the current one in
  a `var` (or an `Atom` when tasks share it).

## Costs

`n` is the collection's length, `k` the size of the other argument.

| Operation | Cost | Notes |
|---|---|---|
| `list.get(i)`, `len`, `first`, `last` | O(1) | A page lookup plus an offset |
| `append` (snapshots) | O(page) amortized | Shares complete pages; copies a partial tail page |
| `concat` | O(n + k) | Copies both lists into a new one |
| builder `append` | O(1) amortized | Pages never move; the directory may grow |
| `map`, `filter`, `filter_map`, `flat_map`, `enumerate`, `scan`, `zip` (lists) | O(n) | One pass into a builder |
| `take`, `skip`, `slice`, `take_while`, `skip_while` (lists) | O(result) | A new list of the kept items |
| `find`, `position`, `any`, `all`, `find_map` | O(position found) | Stop at the first match |
| `count` | O(1) on a counted source, else O(n) | See [counting](#counting) |
| `sum`, `product`, `fold`, `min_by`, `max_by` | O(n) | `min_by`/`max_by` keep the first item on ties |
| `sort`, `sort_by`, `sort_with` | O(n log n) | Stable; `sort_by` computes each key once |
| `dedup` | O(n) | Consecutive duplicates only |
| `unique`, `group_by` | O(n) expected | Hash-based; each group keeps input order |
| `chunks_of`, `partition` | O(n) | |
| `try_map`, `try_fold`, `try_each` | O(position of the first error) | Stop at the first error |
| `validate` | O(n) | Visits every item, collecting every error |
| `map.get`, `contains`, `insert`, `remove`, `update` | O(log n) | Persistent hash trie; `insert` and `remove` share structure |
| `map.keys`, `values`, `entries`, `map_values`, `filter` | O(n) | |
| `merge`, `merge_with` | O(n + k) | Copies both maps' entries |
| `set.union` | O(n + k) | |
| `set.intersection`, `difference`, `is_subset` | O(n) lookups in the other set | |
| `Seq` adapters (`map`, `filter`, `skip`, `chunks`, ...) | O(1) to build | The work happens during iteration |

### Counting

A `list`, `map`, or `range` source knows how many items it has, so `count`
is that span — no pass. A pipeline keeps the count when its stages neither
drop nor re-shape items (`map`, `enumerate`) **and** those callbacks are
pure and infallible: skipping work that could defect or have an effect
would be observable, so a fallible or effectful callback, or a `filter`,
`filter_map`, `flat_map`, `dedup`, `take`, `skip`, or `step_by` stage,
counts the items as they pass instead. Both paths return the same number;
only the work differs.

### Text and bytes

`String` is UTF-8. Operations counted in **characters** (`len`, `slice`,
`index_of`, `char_at`, `pad_start`) walk the text: O(n). Byte operations
(`byte_len`, `bytes.slice`, `bytes.index_of`) are O(1) or O(n) in bytes
without decoding. `split`, `lines`, `words`, `replace`, and `join` are one
pass. Repeated `$"{acc}{piece}"` concatenation in a loop is quadratic
(the compiler warns, E0453): collect the pieces and `text.join` them once.

## Ordering and equality

`<`, `sort`, `sort_by`, `min_by`, and `max_by` use one order: numbers,
characters, text (by bytes, so by code point), durations, instants, and
timestamps as you expect; `false < true`; tuples by position; records
field by field in label order; lists and arrays lexicographically
(a prefix first); `Option` and `Result` by variant, then payload.
`unique`, `dedup`, set membership, and map keys use structural equality
(`Hash` for maps, sets, `unique`, and `group_by`).

## Fallible processing

Fail fast with `try_map`, `try_fold`, and `try_each` (lists and
sequences) or `all_ok`; the first error stops the pass and, for a
sequence over a scoped source, closes it. Collect every problem instead
with `validate`, which returns all the errors in order. Do not drop
failures with `filter_map` when they matter.
