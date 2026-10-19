# Persistent-map investigation after the October 19 main refresh

Finish and integrate G8d first. Baselines must include the merged MIR/runtime
work, rather than comparing a new prototype to the old October 18 compiler.

The current trie is already CHAMP: five hash bits per level, separate data and
child bitmaps, compact arrays, 24-byte cached-hash/key/value entries. Retaining
a map value shares its wrapper/root. Freezing an owned wrapper clears a token;
it does not copy or walk the trie. Updates copy shared paths, and scalar
ownership epochs reuse private paths between publications.

The existing compare snapshots and realworld state contracts also update maps
and enumerate retained versions. Keep those contracts unchanged. Add
`bench/gc/map_phases.bdg` and its independent Python oracle to separate update
and read time, cover scalar/pointer values, private/per-update/periodic retention,
and retaining unchanged maps at 64/1024/4096 keys. Measure ordinary binaries
separately from GC phases/allocation census, with identical saved MIR, adjacent
pairs and 1/2/4/8 workers. Single-task worker sweeps measure overhead, not
parallel speedup. Smoke runs during correctness builds are not timing evidence.

Before changing representation, establish current collection-phase shares and
managed traffic. Open collector costs include allocating mark-stack scratch,
discarding remembered-vector capacity, and sorting occupied cell addresses on
major trimming. Their source-level presence alone does not establish priority;
the old 85% sweep share is historical, and multiple sweep/allocator improvements
have already landed.

First compact-node experiment: a replacement that preserves entry/child counts
can copy one contiguous logical node payload and patch the changed entry/child,
instead of four prefix/suffix copies. Preserve pointer flags, strip old owner
tokens, stamp the new token, retain exact tracing/layout and old versions, and
copy only initialized logical bytes. Test collision buckets, mixed entries and
children, GC stress, barriers, removals and cursor traversal. Keep only if the
ordinary application/phase evidence justifies the added branch.

The corrected `7c2fb36` executable also exposes a cheaper first read-path
experiment: `hamt::get` contains software population-count sequences for bitmap
ranking, with no POPCNT instructions (`P1-get-baseline.asm`). The portable Rust
build does not enable that x86 feature globally. Test a whole-lookup specialization
guarded by runtime CPU detection, retaining the portable implementation and the
existing node representation. Verify generated instructions rather than assuming
the attribute affects outlined helpers. Cover existing lookup hit/miss, pointer
key/value and application contracts; directly exercise the fallback in unit
checks on this POPCNT-capable machine. A machine-local `target-cpu=native` build
alone is not an acceptable production fix.

Use [Rust target-feature rules](https://doc.rust-lang.org/reference/attributes/codegen.html#the-target_feature-attribute)
and [runtime CPU detection](https://doc.rust-lang.org/std/arch/macro.is_x86_feature_detected.html):
the specialized function must only execute on a supporting processor. Keep this
separate from node copying and collection changes so measurements identify the
cause of any improvement or regression.

Further candidates, not yet implemented: unchanged-update/absent-remove reuse
(including set duplicate insertion); pointer-bearing or parameter map epochs
with a publication/root proof; smaller cached-hash representations if occupancy
and hashing evidence supports an ABI change; shared-subtree-aware equality with
Float/NaN semantics preserved. Do not infer exclusive ownership from a single
root reference or introduce arenas without proving cross-task reachability.

Clojure references: [implementation](https://raw.githubusercontent.com/clojure/clojure/master/src/jvm/clojure/lang/PersistentHashMap.java),
[transients](https://clojure.org/reference/transients).
Our trie already has the 32-way branching and transient/epoch mechanisms;
equivalent techniques do not establish equivalent timings across runtimes.
