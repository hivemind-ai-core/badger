# Map builder node headroom

Follow-up to the eager list/map work: a final check for slowdowns showed the
`bench/collections` persistent-map insert phase reallocating a trie node on
almost every insertion. Nodes were allocated at their exact current size;
the next insertion into the same node (the normal case for a bulk
`map.builder`/`from_list`/`collect`, or a compiler-proved private update
loop) did not fit, so `edit` fell back to `splice`, allocating and copying
the node again. For a node growing to a few dozen entries this is one
allocation and copy per entry.

## Change

`hamt::splice_owned` and `hamt::alloc_owned` now reserve three entries and
three child slots of spare capacity when the mutation mode is **Builder**
(a bulk builder owns every node outright):

- `Builder`: exact node edits plus slack on fresh/spliced nodes.
- `Epoch` (retained-map copy-on-write) and `Persistent`: unchanged exact
  sizes. Enlarging epoch nodes made the `state` snapshot workload's cache
  footprint worse, and its copies are already minimized by in-place
  edits within an ownership window.

The spare capacity is invisible to tracing and iteration: node headers
record the logical entry/child counts, and only those are visited.
`BADGER_HAMT_SLACK=0` selects exact-size allocation for A/B control.

## Measurements

Same binary, alternating `BADGER_HAMT_SLACK` default vs `0`; 14 process
pairs, median of each side. Host shared with unrelated builds; treat small
deltas as directional.

| case | headroom | exact | delta |
|---|---:|---:|---:|
| `bench/collections` run time | 73.89 ms | 91.58 ms | **−19.3%** |
| realworld `reconcile`, kernel | 5.381 ms | 5.424 ms | −0.8% |
| realworld `cpu`, kernel | 53.52 ms | 53.57 ms | −0.1% |
| realworld `messaging`, kernel | 1.279 ms | 1.254 ms | +2.0% (no maps; noise) |
| realworld `pipeline`, kernel | 55.23 ms | 55.17 ms | +0.1% |
| realworld `state` (epoch maps), kernel | 2.477 ms | 2.565 ms | −3.4% (noise; mode unchanged) |

The `collections` per-operation output shows the mechanism directly:
`persistent map insert` went from 519 ns/op without headroom to 137 ns/op
with it in the same sampling round; `map lookup` was unchanged. Peak RSS
did not increase (the fewer allocations outweighed the spare bytes).

## Validation

`cargo test -p badger_cli --release --test programs` (76 passed),
`mir_optimization` (7), `option_niche`, `scalar_enums`, `sequence_fusion`,
and the full `tests/run` corpus at `release`, `aggressive`, and `superopt`
all match their expected output and exit codes.
