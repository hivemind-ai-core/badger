# Map and collector baseline after the main refresh

Initial baseline runtime: **7c2fb36**, including G8d and corrected main b7cf709.
Measurements use identical saved MIR, fixed executables, independent Python
oracles, clean inherited runtime settings and 1/2/4/8 workers. The subsequently
merged builder headroom optimization is not attributed to these measurements;
future production comparisons need a common refreshed base.

The trie is already **32-way CHAMP**, with cached-hash/key/value inline triples,
compact child arrays, persistent path copies, private builders and scalar-map
ownership epochs. Retaining a version shares its wrapper/root. Freeze clears an
owner token and never walks/copies the trie. Clojure's [32-way trie implementation](https://raw.githubusercontent.com/clojure/clojure/master/src/jvm/clojure/lang/PersistentHashMap.java)
and [transients](https://clojure.org/reference/transients) provide useful design
comparisons; adopting that branching factor is not new work here.

## Where time and allocation go

`bench/gc/map_phases.bdg` separates update/publication from reading retained
versions. It supplements, and does not change, the realworld/compare contracts.
Reading includes its digest/value work, not just cursor advancement. The
"private" configuration retains only the final version, but shares a source
branch with periodic publication and can use epochs rather than the cheaper
all-private builder lowering. Existing coverage provides that other control.

At one worker, medians across three fresh processes:

| Case | Updates / retain ms | Read ms |
|---|---:|---:|
| 131,072 unchanged-map references, 64 keys | 0.610 | 2.715 |
| same reference count, 1,024 keys | 0.684 | 2.708 |
| same reference count, 4,096 keys | 0.728 | 3.320 |
| 65,536 scalar updates, 1,024 keys, no intermediate retention | 2.003 | 0.007 |
| same, retain every 64 updates | 15.933 | 6.356 |
| same, retain every update | 49.894 | 352.300 |
| string-valued updates, 1,024 keys, no intermediate retention | 16.741 | 0.032 |
| same, retain every 64 updates | 31.755 | 18.825 |
| same, retain every update | 55.148 | 876.376 |

Unchanged retention includes allocating/writing the reference list. Its cost is
roughly independent of map size; it is not deep copying. Reading every retained
version scales with the total entries visited. Strings in pointer cases come
from a bounded prebuilt pool, so update differences are not new string churn.
[Full ordinary table](P1-baseline/summary.md).

Separate diagnostic allocation counts (startup/output included) for the same
65,536 updates at 1,024 keys:

| Case | MapNode objects | Map wrappers |
|---|---:|---:|
| scalar, no intermediate retention | 1,017 | 6 |
| string values, no intermediate retention | 143,010 | 65,541 |
| scalar, retain every 64 | 38,390 | 1,029 |
| string values, retain every 64 | 143,010 | 65,541 |

This supports extending ownership reuse to **immutable leaf** keys/values
(String/Bytes) while keeping unknown/composite pointer graphs excluded. Preserve
the existing fresh-origin, alias, publication, return/capture/cursor and live-mode
rules. Root pointer arguments before allocation and retain node/wrapper barriers.
It is not evidence that arbitrary pointer graphs are uniquely owned, or that
parameter maps/arenas can be mutated safely.

Pointer read time nearly doubles when an otherwise idle second worker is added:
the per-update-retention/1,024-key case is 876 ms at one versus 1,638 ms at two.
GC is only about 24 ms in the separately perturbed census. Source inspection
shows that `text_len`, `text_byte_len` and `text_is_empty` are allocation-free
immutable reads, but are absent from codegen's unlocked-entry list. Confirm lock
counts and add a read-only control before blaming trie representation. Audit and
measure a narrowly scoped unlock separately from ownership or node-copy changes.

Disassembly also shows software popcount sequences in `hamt::get`, with no
POPCNT instruction. Runtime CPU specialization with a portable fallback and
dense-bitmap shortcuts are separate candidates; inspect actual generated code
and measure them, rather than globally requiring a newer CPU.
[Baseline disassembly](P1-get-baseline.asm), [experiment notes](P1-plan.md).

## Remaining GC priority

Phase-only diagnostic runs, separate from ordinary timings and allocation census:

| Case / one worker | GC ms / workload ms | Largest GC phases |
|---|---:|---|
| records-window | 23.32 / 132.56 | sweep/finalize 86.3%; trim 5.3%; prepare 3.3% |
| snapshots-major | 14.15 / 84.40 | sweep/finalize 73.4%; trace 16.9%; trim 3.9% |
| bytes-survivors | 0.64 / 92.05 | sweep/finalize 56.8%; trace 38.9% |
| pool-burst-256 | 0.58 / 20.74 | trace 76.8%; sweep/finalize 18.9% |

Sweep remains important for collection-heavy allocation. The old 85% figure
still resembles records, but is a fraction of collection time, not the whole
application. Small scratch/remembered-vector reuse and occupied-address sorting
are real costs; even eliminating prepare plus trim entirely would save only
about 1.5% of records here. That does not eliminate the case for GC work: larger
leaf/slab/page reclamation or allocation elimination can avoid per-object sweep
work, whereas merely changing mark-stack allocation cannot.

Prioritize bounded leaf-map reuse and the measured read-call overhead first.
Then evaluate contiguous replacement copies/bitmap ranking, and investigate
batched leaf reclamation with allocation/occupancy evidence before redesigning
pool ownership. Keep general parameter ownership, regions, transferable arenas,
compact entries and shared-subtree equality as explicit unimplemented followups.
Single-task worker sweeps establish overhead, not parallel speedup; the separate
eight-task map matrix is saved in `P1-parallel-baseline` for subsequent comparisons.
