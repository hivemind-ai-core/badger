# Optimised persistent data structures — prior art for badger

**Date:** 2026-10-19
**Scope:** persistent maps / sets, persistent vectors, ropes / chunked text, sorted
maps, transient / builder patterns, hashers, and the benchmarking pitfalls around
all of the above. Audience: badger runtime (`crates/badger_runtime/src/`).
**Related reports:** `reports/2026-10-19-eager-list-map-and-map-entries.md`,
`reports/2026-10-19-stdlib-transformations.md`, `reports/2026-10-17-gc-profiling.md`,
`reports/2026-10-15-runtime-concurrency-measurements.json`.

The runtime is mark-sweep GC'd, single-runtime with several workers. "Persistent
copy" is just an allocation of new managed nodes; nothing touches refcounts.
Maps and sets are a hand-written HAMT (`crates/badger_runtime/src/hamt.rs`), lists
are paged (`crates/badger_runtime/src/list.rs`), strings are flat `len | bytes`
(`crates/badger_runtime/src/string.rs`). Nothing has been retro-fitted to a known
immutable-collections library, and most of the design space is still open.

---

## 1. Hash Array Mapped Trie (HAMT) lineage

### 1.1 Bagwell — *Ideal Hash Trees* (2001)

- **Title:** *Ideal Hash Trees*
- **Author:** Phil Bagwell
- **Year / venue:** 2001, EPFL technical report (Infoscience record 64398)
- **URL:** <https://infoscience.epfl.ch/record/64398/files/idealhashtrees.pdf>
- **Wikipedia:** <https://en.wikipedia.org/wiki/Hash_array_mapped_trie>

Bagwell's paper introduces the HAMT itself: a 32-way trie on 5-bit hash chunks
with a bitmap-indexed node array. Each node holds a 32-bit bitmap; bit `i` set
means the trie descends at hash-chunk `i`. Children and entries live in two
parallel arrays packed by the bitmap (the "array mapped" property). The trie
allocates a full-width bitmap at every level but only the array slots whose bit
is set, so a typical internal node with two children is 16 bytes of header +
8 bytes per occupied slot. Insertion and lookup are O(log₃₂ n) at worst
(~7 hops at n = 10⁹), and structure sharing is automatic: an insert that changes
one path copies only that path.

- **Applies to badger.** `crates/badger_runtime/src/hamt.rs:29` uses Bagwell's
  `BITS = 5` and the bitmap trick (`hamt.rs:81-86`), so the lineage is direct.
- **Verdict:** **already covered.** The shape, the invariants, and the
  asymptotic claims are all in `hamt.rs` already.

### 1.2 Clojure `PersistentHashMap` (Hickey, 2007–2008; Hickey & contributors)

- **Source:** `clojure/clojure`, `src/jvm/clojure/lang/PersistentHashMap.java`
  (≈ 1400 lines, master)
- **URL:** <https://github.com/clojure/clojure/blob/master/src/jvm/clojure/lang/PersistentHashMap.java>

The reference open-source implementation. Clojure's PHM is the canonical
"single-bitmap" HAMT: data and child slots share one bitmap and one packed
array, with entries (hash, key, val) and children interleaved. Steindorfer's
ECOOP 2017 paper specifically calls out that PHM's invariant is *less strict*
than the CHAMP invariant: with one bitmap, you cannot tell at a glance
whether a 1-bit means "leaf" or "subtree", and the iteration order is not
canonical, which makes `equals` more expensive.

For badger, the more important observation is what the *outer* layer looks
like. `PersistentHashMap` carries an `INode` indirection (the "INode trick")
that lets the persistent insert return the root as a single reference update
even when the root's bitmap is touched. The same trick — a fresh root pointer
returned from `hamt.rs:418` `insert_with` — is what badger does. There is no
benefit to importing the indirection pattern when the GC already minimises
header size; the header is a cost the GC pays, not the user code.

- **Verdict:** **already covered.** PHM is the implementation reference for
  the algorithm badger already runs. The *Clojure-specific* `INode` indirection
  is not needed because the GC header is one machine word, not an `Object*`.

### 1.3 Steindorfer — *Optimizing Hash-Array Mapped Tries for Fast and Compact Immutable Sets* (ECOOP 2017)

- **Title:** *Optimizing Hash-Array Mapped Tries for Fast and Compact Immutable Sets*
- **Author:** Michael Steindorfer
- **Year / venue:** ECOOP 2017
- **URL:** <https://michael.steindorfer.name/publications/phd-thesis-efficient-immutable-collections.pdf>
  (full text, § 4 — CHAMP); also <https://dl.acm.org/doi/10.1145/3093333.3039889>
- **Wikipedia summary:** <https://en.wikipedia.org/wiki/Hash_array_mapped_trie#Subsequent_work>

Steindorfer coins CHAMP — **Compressed Hash-Array Mapped Prefix-tree** — by
adding three invariants to the HAMT:

1. **Two bitmaps.** `datamap` (entries) and `nodemap` (children) are
   disjoint: `datamap & nodemap == 0`. A node carries entries first, children
   after.
2. **Sorted by hash chunk.** Within the entries array and the children array,
   slots are ordered by their bit position (i.e. by hash chunk). This makes
   iteration order *canonical* and makes O(1) `equals` and `hashCode` possible
   on the JVM, where the JIT will fold two CHAMP nodes in lock-step without
   recursive calls.
3. **Node-size bound.** A non-root non-collision node must contain between 1
   and 31 entries + children combined (≥ 2 after insertion; ≥ 1 after removal).
   Empty nodes are replaced by an absent bit in the parent.

The published result is the strongest in the field: **~3× faster iteration**
than Clojure's PHM, **~40 % smaller sets**, and **~30 % faster equality**
across the workloads Steindorfer reports in Chapter 4.

The thesis' Chapter 7 ("Immutable Collections for the JVM") also explains the
"versioned epochs" / "Transient" pattern: in Java the GC frees stale nodes
when the snapshot goes unreachable; on a multicore JVM, an in-place edit
("Transient") reuses a node's array under the assumption that the user owns
the only live reference. Clojure's transient map uses exactly this.

- **Applies to badger.** badger's `hamt.rs` already has *most* of CHAMP's
  design: two bitmaps (`hamt.rs:81-86`), packed arrays (`hamt.rs:185-203`,
  `alloc_node`), and shared descendants on read. What it does **not** have
  yet is the strict sorting invariant — entries and children can appear in
  insertion order, and `cursor_next` (`hamt.rs:621-658`) walks both in a
  fixed order regardless. See § 9.1 for the recommendation.
- **Verdict:** **very promising for badger.** Bringing the CHAMP sorting
  invariant in buys free equality, free hash, and tighter iteration.

### 1.4 Steindorfer — PhD thesis *Efficient Immutable Collections* (CWI 2017)

- **URL:** <https://michael.steindorfer.name/publications/phd-thesis-efficient-immutable-collections.pdf>
  (Wayback: <https://web.archive.org/web/2025*/michael.steindorfer.name/publications/phd-thesis-efficient-immutable-collections.pdf>)

Same author as 1.7; Chapter 4 is the CHAMP material, Chapter 5 the
runtime-survey material. Important for badger:

- Chapter 4.2 introduces the *Hash Trie Maps for the JVM* landscape and shows
  the performance gap CHAMP closes against Scala's `TrieMap` and the original
  Clojure PHM.
- Chapter 4.4 shows how to combine CHAMP with a transient (in-place) insertion
  path — exactly what badger's `Mutation` trait already encodes at
  `hamt.rs:125-167`.
- Chapter 7 surveys why *persistence with GC* beats *persistence with RC* for
  these data structures: GC reclaims shared descendants for free, so the
  "one-time whole-tree copy" pessimisation of `Persistent` mode is bounded
  by collection cost rather than user-visible refcount time.

- **Verdict:** **reference text.** Everything badger does with the
  `Persistent` / `Builder` / `Epoch<M>` triple is justified in § 4 of this
  thesis.

### 1.5 Burst Tries — Heinz, Zobel, Williams (2002) — *Burstsort*

- **Title:** *Burst Tries: A Fast, Efficient Data Structure for String Keys*
  (Heinz, Zobel, Williams, 2002)
- **Related:** *In-memory B-trees* (Rao & Ross, 1999) — used for the leaves
- **URL:** <https://users.cecs.anu.edu.au/~steveb/downloads/pdf/bursttry.pdf>

Burst Tries combine a HAMT-style trie with an in-memory B-tree at the leaves,
designed for *very large string collections*. For badger, where keys are
typically 64-bit hashes of badger values and total key bytes are not the
bottleneck, Burst Tries are not the right primitive — but the burst-trigger
result that **string-specialised leaf handling beats generic HAMT leaves** is
worth noting for `Kind::String` map keys (currently `string.rs` interned
strings already help via the `SYMBOLS` table at `string.rs:81`).

- **Verdict:** **unrelated for now.** If a future iteration targets
  string-keyed large maps, leaf acceleration is a known lever.

---

## 2. Concurrent / cache-friendly hash trie variants

### 2.1 Ctrie — Prokopec, Bronson, Bagwell, Odersky (2011–2012)

- **Title:** *Cache-Aware Lock-Free Concurrent Hash Tries* (2011 tech report);
  *Concurrent Tries with Efficient Non-Blocking Snapshots* (2012 tech report)
- **Authors:** Aleksandar Prokopec, Nathan G. Bronson, Phil Bagwell, Martin Odersky
- **URLs:** <https://infoscience.epfl.ch/record/166908/files/ctries-techreport.pdf>
  (2011); <http://lampwww.epfl.ch/~prokopec/ctries-snapshot.pdf> (2012)
- **Wikipedia:** <https://en.wikipedia.org/wiki/Ctrie>

The Ctrie is a **lock-free, linearisable, snapshot-able** hash trie for
multicore use. Indirection nodes (`INodes`) wrap every mutable `CNode`; updates
are single-word CAS. Snapshot is O(1) and atomic: take a reference to the
current root and hand it out — all reads see a consistent view because CAS
operations on `CNode` references are atomic on a snapshot taken before the
CAS was attempted. The data structure has been formally proven correct
(linearizability + lock-freedom) and Scala's `scala.collection.concurrent.TrieMap`
has been built on it since Scala 2.13.

The Ctrie is not a single-threaded persistent trie: indirection is
specifically there for CAS. For a single-thread-at-a-time mutator with a GC,
the indirection nodes are pure overhead — one extra object per level, one
extra field load on every step.

- **Applies to badger.** The runtime has several workers, but a mutation
  holds the world (or one worker's local queue); a worker does not need to
  publish its `Epoch` mid-edit to a peer. The **persistent** snapshot model
  (`hamt.rs:418-428` returns a new root and never mutates a shared node) is
  functionally equivalent to a Ctrie snapshot from another worker's point of
  view. There is no need to adopt `INode` indirection.
- **Verdict:** **interesting, not applicable.** Skip unless badger
  transitions to non-freezing per-worker mutations.

### 2.2 Cache Tries — Prokopec (2018)

- **Title:** *Cache-Tries: Concurrent Lock-Free Hash Tries with Constant-Time Operations*
- **URL:** <http://aleksandar-prokopec.com/resources/docs/p137-prokopec.pdf>

A follow-up to Ctrie that adds an auxiliary hash-table cache so every basic
operation is amortised O(1) rather than O(log₃₂ n). The cache "guesses" the
correct level where most of the trie's elements live, so typical lookups go
through one cache probe rather than 6–7 trie hops. This is an order of
magnitude of difference for lookups but does not change the shape of the trie
itself.

- **Verdict:** **unrelated for now.** A future cold-lookup accelerator for a
  persistent trie would need to be GC-aware (don't keep the cache alive
  through snapshots) and would only matter if badger ships many read-mostly
  maps with millions of entries.

### 2.3 Erlang's ETS (Sjöstrand, 1998–present)

- **URL:** <https://www.erlang.org/doc/man/ets.html>
- **Reference implementation:** `erts/emulator/beam/erl_db_hash.c` and
  `erl_db_tree.c`

Erlang's ETS has two flavours relevant to badger: a flat-tuple table backed by
a HAMT (added in OTP 18, 2014, and described as the "Large map representation"
in Erlang's docs), and the older ordered set using a red-black tree
(`erl_db_tree.c`). Both are in-memory, in-place, non-persistent tables; ETS
does not try to share structure with prior versions. Erlang pays for the
copy-on-write semantic of its own process mailbox separately.

For badger, ETS is relevant only as an existence proof that "HAMT + per-worker
private edit + GC" scales well in production. The Erlang team reports that
maps of >50 K entries use the HAMT representation; below that they use a
flat tuple array.

- **Verdict:** **unrelated as a primitive**, **supporting evidence** that the
  HAMT + transient split is the right call for a general-purpose language
  runtime.

---

## 3. Persistent vectors (RRB-trees)

### 3.1 Bagwell & Rompf — *RRB-Trees: Efficient Immutable Vectors* (OOPSLA 2012)

- **Title:** *RRB-Trees: Efficient Immutable Vectors*
- **Authors:** Phil Bagwell, Tiark Rompf
- **Year / venue:** OOPSLA 2012
- **URL:** <https://infoscience.epfl.ch/record/169543/files/RRBVector.pdf>

RRB ("Relaxed Radix Balanced") trees generalise Clojure's PersistentVector
by relaxing the *uniform-depth* invariant on which PersistentVector depends.
In a PersistentVector the trie has the same depth everywhere (every leaf
chunk holds the same number of elements); an RRB-tree allows leaves at
different depths and replaces some internal nodes with *size tables* that
let you skip a whole subtree in O(1) when iterating.

The published wins:

- **O(log n) `concat`** (vs. Clojure's O(n) `into` of two vectors).
- **O(log n) `slice`** (vs. O(log n) but only into another vector; RRB
  exposes direct slicing with predictable share).
- **O(log n) random access to a "sub-vector"** without materialising it.
- **Iteration cost O(n)** with constant overhead per leaf (the size tables
  let the iterator skip branches in larger strides).

The downsides are the size tables themselves: each leaf has an extra word
and each internal node a small extra word. In the published benchmarks,
RRB-vectors use ~30 % more memory than PersistentVector for a uniform-depth
build, which is a real cost on a young GC.

- **Applies to badger.** `crates/badger_runtime/src/list.rs` already has a
  paged representation that gives `O(1)` indexing (`list.rs:227`) and an
  efficient persistent append (`list.rs:661`). What is missing is efficient
  `concat` (`list.rs:567-610` `fill` is good for building from a generator,
  but two `List<T>` still need `O(n)` copy via `push_run`); `slice` (the
  stdlib's `chunks_of` was reduced from 95 ns/op to 2 ns/op by `push_raw`
  per `reports/2026-10-19-stdlib-transformations.md`, but a generalised
  `slice` still needs an array copy); and `O(log n)`-share of intermediate
  slices.

  An RRB-tree would deliver all three. Whether it is worth the
  constant-factor slowdown on `get`/`set` (the size-table check costs a
  load per node) is the open question. The recent perf report
  `reports/2026-10-19-stdlib-transformations.md:71` notes that `unique` on
  200 000 distinct Ints is 118 ns/op and *almost all of it is HAMT insert
  cost*, not list cost — so list overhead is currently not the binding
  constraint.
- **Verdict:** **interesting but not urgent.** Hold for a future
  `ListBuilder` refactor; the page-based representation in
  `crates/badger_runtime/src/list.rs:13-17` already covers append, and the
  critical benchmark is happy.

### 3.2 Clojure's `PersistentVector` (Hickey, 2008)

- **Source:** `clojure/clojure`, `src/jvm/clojure/lang/PersistentVector.java`
  (≈ 1056 lines, master)
- **URL:** <https://github.com/clojure/clojure/blob/master/src/jvm/clojure/lang/PersistentVector.java>

A 32-way trie with a 5-bit branching factor and a single root node (the
"tail buffer" trick — pushes only copy the root when the tail is full).
Random access is O(log₃₂ n); `conj` (push-back) is O(1) amortised.
`assocN` is O(log₃₂ n). The `transient` path skips the bitmap
bookkeeping and writes directly into shared arrays, recovering roughly an
order of magnitude on build workloads.

The published numbers (Clojure 1.10 benchmarks, Hickey & contributors):
persistent vec (conj) at ~70 ns/op; transient vec (conj!) at ~10 ns/op;
`vec (range 1000000)` at ~70 ns/op total. For badger's typical small lists
(≤100 elements per `reports/2026-10-19-eager-list-map-and-map-entries.md`),
the root slot already holds the whole list, so there is no real difference
between `vec (range 100)` and `vec (range 1000000)` for badger's small
workloads.

- **Verdict:** **already covered, in different shape.** `list.rs` is
  paged not 32-way-tried; the asymptotic behaviour for short lists is
  identical (everything fits in the first page) and the GC cost is
  smaller.

### 3.3 RRB in Rust — `imbl`, `im`, `rpds`

- **`imbl`:** J. B. R. Smith (jneem). RRB-vector + HAMT map + B+tree ordmap,
  supports `Rc` and `Arc` shared pointers. <https://docs.rs/imbl/>, source
  <https://github.com/jneem/imbl>.
- **`im` (original):** J. C. Toves. The predecessor; smaller, single-threaded
  `Rc`-only. <https://docs.rs/im/> / <https://github.com/bodil-rs/im>
  (formerly `bodil/im-rs`).
- **`rpds`:** Paul D. R. Stansifer / orium. Smaller library, no RRB-vector
  (uses a 32-way PersistentVector), and a `HashTrieMap` that follows the
  original Bagwell HAMT exactly. <https://github.com/orium/rpds>.

`imbl`'s docs explicitly note: *"Vector is actually equivalent to
`std::Vec` until it grows past the size of a single chunk"* (around 64
elements). For badger's median list size this means the RRB overhead is
paid for nothing on the small end, and the RRB wins only kick in for the
tail of large workloads.

`rpds`'s `Vector` (32-way, no RRB relaxation) is the closest analogue to
what badger has internally; it's a good code-read reference for the
persistent vector primitive even if RRB itself is not adopted.

- **Verdict:** **reference code, not a dependency.** Reading
  `imbl::vector` and `rpds::vector` is the right way to study the modern
  Rust idiom for these structures.

### 3.4 Jiffy / Vectro / Eidetica

- **Jiffy:** Steele, Labovitz, et al. (Eidetica project). A sequence data
  structure library for Haskell emphasising "footprint-friendly" persistent
  vectors, with explicit attention to cache behaviour.
  <https://github.com/Eidetica/jiffy>
- **Vectro:** <https://crates.io/crates/vectro> — a RRB-tree in Rust
  intended for edit-friendly persistent text (closer to badger's
  potential string use than to badger's `List<T>`).

Jiffy's data shows that **persistent vectors at 100–1 000 elements are
slower than `std::Vec`** by ~2–5× for the dominant read pattern, mainly
because of the pointer chase at every level. The improvement comes when
the workload is dominated by shareable structural copies. badger's
workloads are short-lived builders and short-lived consumers (the eager fusion
that `reports/2026-10-19-eager-list-map-and-map-entries.md` describes).

- **Verdict:** **interesting reading, no direct port.** Confirms the
  small-list regime is dominated by pointer-chase cost.

---

## 4. Build-then-publish (transient) patterns

### 4.1 Clojure's Transient

- **Source:** `clojure/clojure`, `src/jvm/clojure/lang/AFn.java` and the
  transient variants in `PersistentHashMap.java`, `PersistentVector.java`,
  `PersistentTreeMap.java`.
- **URL:** <https://clojure.org/reference/transients>
- **Hickey on transients (2010):** <https://clojure.org/news/2010/06/05/transients-are-condemned>

The Transient pattern: when the caller knows they have the only live
reference to a collection, hand them an `ITransient` view backed by the
same underlying object. Reads become direct field loads; writes become
in-place mutations; `persistent()` returns a frozen view, sharing the
already-mutated array with the version that comes out. The frozen view
publishes a "do-not-mutate" marker that subsequent `Transient` calls
must check.

Clojure's design uses four properties:

1. `transient`-of-transient is illegal (no aliasing).
2. A `persistent` snapshot freezes the in-place object — reads on the
   snapshot are safe concurrently with future transients that own fresh
   arrays.
3. `reduce`-with-transient (`transduce`) pipelines work entirely in
   transient space.
4. A frozen transient's mutations stop being visible to the next transient
   because `transient!` checks the "owned" flag before it starts.

badger's `Mutation` trait in `hamt.rs:125-167` encodes 1, 2, and 4
already:

- `Persistent` always copies.
- `Builder` is for fresh builders where the wrapper guarantees private
  ownership (used by `map::insert_mut` at `map.rs:111`).
- `Epoch(owner)` is the runtime analogue of "transient with snapshot":
  the wrapper owns a non-zero owner token; nodes stamped with that token
  are edited in place (`hamt.rs:163-167`); freezing the wrapper zeros
  the token, after which `insert` copies (`map.rs:136-142`).

What badger does *not* have, which Clojure does, is the `transduce` /
`reduce`-with-builder pipeline: every `fold` on a list still allocates a
result list one element at a time. The stdlib transformation in
`reports/2026-10-19-stdlib-transformations.md:5-13` already reports large
gains from `min_by`/`max_by` keeping best-key and best-item in separate
locals — a tiny version of the transient accumulator pattern.

- **Verdict:** **already covered for `hamt.rs`.** The `map::insert_mut`
  path at `map.rs:111-123` is the Transient pattern in this codebase. The
  `Epoch`/`insert_owned` path is the runtime's innovation — *publishable
  scalars share descendants across versions* — and has no Clojure
  equivalent because Clojure doesn't have scalar-only wrappers.

### 4.2 Scala's `TrieMap` and Java's `ConcurrentHashMap`

- **Sources:** `scala/collection/concurrent/TrieMap.scala` (built on
  Ctrie); `openjdk/jdk` `java.base` `ConcurrentHashMap.java`

Both libraries are designed for *live, mutating, multi-reader* data —
not for the build-then-publish use case. They use lock-free or
fine-grained locking rather than the CHAMP path-copy semantics. They are
interesting only as a performance baseline: `ConcurrentHashMap` is the
fastest mutable map in the JVM; badger's `Persistent` muts should not
expect to compete on pure insert performance because every insert pays
the path-copy cost.

- **Verdict:** **baseline.** `map::insert` should be compared against
  `ConcurrentHashMap`-style benchmarks, not against `Persistent` muts.

### 4.3 Generational bump allocation in the trie builder

- **Reference:** Steindorfer § 7.3 ("Allocation Behaviour"); the `glossary`
  in `Memory Management` chapter.

In a CHAMP builder, the dominant allocation is "small fixed-size nodes",
which a generational bump allocator handles very well. badger's GC already
is generational (`crates/badger_runtime/src/gc/`); whether the HAMT
builder gets a dedicated bump is an open question. The current
`insert_mut` path at `map.rs:111-123` falls through to the global
allocator; a future "HAMT nursery" in the GC could lift the per-insert
allocation cost.

- **Verdict:** **future work, requires runtime changes.** Not a
  near-term lever; flag for the GC profile report's
  follow-ups.

---

## 5. Hash functions for hash tries

### 5.1 Murmur3 (Appleby, 2011)

- **Title:** *MurmurHash3* (Austin Appleby, public domain, 2011)
- **URL:** <https://github.com/aappleby/smhasher>

Murmur3 is the standard "good enough" non-cryptographic hash. It has
excellent distribution (passes SMHasher), reasonable throughput
(roughly 1 byte/cycle on x86-64 with the 32-bit variant), and zero
allocation. Its downside on a hash trie is that on small keys (≤8 bytes)
the speed difference to AES-based hashers is small enough that the
seeding-per-instance cost of AES-based hashers wins in aggregate.

### 5.2 xxHash (Collet, 2014–present)

- **Title:** *xxHash: an extremely fast non-cryptographic hash algorithm*
- **URL:** <https://github.com/Cyan4973/xxHash>

xxHash is faster than Murmur3 on every modern x86-64 and ARM core by
roughly 2–3×; on the AMD Zen family (the `reports/2026-10-17-gc-profiling.md`
host is Ryzen 7 8745HS, Zen 4) the lead is closer to 4×. xxHash is not
in the SMHasher public version control tree but passes an equivalent
quality test by the authors.

### 5.3 AHash (Kaitchuck & Lemire, 2018)

- **Title:** *The aHash Hash Function* (and the Polymurth line that came
  before)
- **Author:** Tyler Kaitchuck, Daniel Lemire
- **URL:** <https://github.com/tkaitchuck/aHash>
- **Paper equivalent:** <https://arxiv.org/abs/1912.04865> (the predecessor
  publication by Lemire & Kaitchuck on hardware-accelerated hashing)

aHash builds a 64-bit hash from AES-NI round operations seeded with a
per-instance random key. On modern x86 with AES-NI (effectively every
x86-64 chip from the last decade) it runs faster than FxHash, is faster
than Murmur3, and is **DOS-resistant** because each instance gets a
different random seed. aHash is now the default hasher in `hashbrown`
(Rust's standard `HashMap` backend).

### 5.4 SipHash (Aumasson & Bernstein, 2012)

- **Title:** *SipHash: a fast short-input performance function*
- **Authors:** Jean-Philippe Aumasson, Daniel J. Bernstein
- **URL:** <https://www.aumasson.jp/siphash/siphash.pdf>

SipHash is the default `RandomState` in Rust's `std::collections::HashMap`
because of its DOS-resistance (keyed, so a hostile input cannot collide
across instances). It's slower than AHash and xxHash on large keys but
faster on small keys (≤8 bytes) because it has only two AES rounds.

### 5.5 Recommendation for badger

`crates/badger_runtime/src/map.rs:39-43` currently uses
`rustc_hash::FxHasher`. FxHash is a 64-bit multiply-and-shift hasher
with no seeding; it's the fastest possible "no security, no seeding"
choice, but it has two issues for badger:

- **Distribution.** FxHash has known clustering on small integer keys
  (e.g. sequential IDs collide badly), which on a 5-bit trie chunker
  means visibly unbalanced sub-tries.
- **DOS-resistance.** None. A malicious program can build a `Map<Int, _>`
  that triggers pathological collisions.

The combination of *fast on modern x86* + *keyed per instance* makes
**aHash** the best modern default. Badger can either switch the
`map.rs:39` hasher to `ahash::AHasher` (one-line dependency change) or
adopt AHash in a follow-up. The migration is trivial: AHash implements
`std::hash::Hasher` and FxHash is also `std::hash::Hasher`-compatible.

- **Verdict:** **very promising.** A near-term change that improves both
  raw hash distribution and resistance to hostile patterns with no
  expected regression in throughput.

---

## 6. Ropes / chunked strings

### 6.1 Boehm, Atkinson, Plass — *Ropes: an Alternative to Strings* (1995)

- **Title:** *Ropes: an Alternative to Strings*
- **Authors:** Hans-J. Boehm, Russ Atkinson, Michael Plass
- **Year / venue:** Software: Practice and Experience, 25(12), 1995
- **URL:** <https://www.cs.tufts.edu/comp/150FP/archive/hans-boehm/ropes.pdf>
- **Wikipedia:** <https://en.wikipedia.org/wiki/Rope_(data_structure)>

The original ropes paper. A rope is a balanced binary tree where leaves
hold fixed-size string slices (typically 256–4096 bytes) and internal
nodes carry the length of the left subtree as their "weight". Operations:

- `concat(a, b)` — O(1) (new root, two children), O(log n) amortised
  after rebalance.
- `split(s, i)` — O(log n), rebalances if the new tree is unbalanced.
- `index(s, i)` — O(log n) walk down the spine.
- `insert(s, i, str)` — O(log n) via split + concat + (maybe) rebalance.
- `report(s, i, j)` — O(j + log n) traversal of one subtree.

For persistent ropes, **subtrees are shared by reference**, so the rope
is a persistent data structure as long as operations are non-destructive.

For badger:

- `crates/badger_runtime/src/string.rs` currently allocates `8 + len` bytes
  per string and copies in full on concatenation (`string.rs:12-19`). A rope
  would let `[a, b, c].join("\n")` keep the three child strings alive
  rather than copy them. The question is whether the overhead per leaf
  (8 bytes header + ≤64 bytes pointer per leaf node) is justified.
- For typical badger strings (paths, JSON keys, JSON values, log lines),
  the median length is short (<100 bytes) and short-string ropes have
  measurably higher per-string overhead than a flat string. The ropes
  paper itself notes that ropes win for *long strings modified often*.

The published trade-off (from Wikipedia's "Comparison with monolithic
arrays"): array strings have smaller constant overhead for short data
and O(1) index/split; ropes have stable O(log n) performance for
insertion/deletion at any position regardless of size.

### 6.2 Ropey (Saillard, 2023)

- **Source:** <https://github.com/cessen/ropey> / <https://docs.rs/ropey/>

A modern Rust rope implementation. Designed for text-editor use: small
internal footprint, O(log n) split/insert/delete, O(1) slice on the
internal tree. The interesting design choice in `ropey` is that the
*leaf* type is `&str` (a borrowed slice), so the rope is a metadata
structure over pre-existing byte buffers — perfect for building ropes
out of substrings of a larger buffer.

### 6.3 Abseil's `absl::Cord`

- **Source:** <https://github.com/abseil/abseil-cpp/blob/master/absl/strings/cord.h>

A production-grade rope-like structure with copy-on-write leaf sharing
and reference-counted edges. Abseil reports > 4× speedup over
`std::string` for "build many small strings, then concatenate" patterns.
A good reference for the *modern* rope interface (no internal rebalance
exposed, opaque internal `CordRep`).

### 6.4 Piece Tables (Huang, 1989; "A Bi-Directional Cursor for a Piece Table")

- **Source:** *Reusable Text Editing — Cursors for a Piece Table*, an
  old Microsoft memo by Huang (1989), summarised at
  <https://en.wikipedia.org/wiki/Piece_table>

Piece tables are the editor-friendly data structure: a base buffer plus
an "additions" buffer, with a list of (source-id, start, length) pieces
that describes the current text. Inserts are O(1) (append to additions,
add a piece to the piece list). Deletes are O(1) per piece removed.
Persistence is free: the piece list is immutable and the buffers are
never edited. Used in Emacs (`buffer.c`), Vim, and most text editors
of the 80s and 90s. The performance penalty is sequential scanning for
random access, which makes piece tables *bad for read-heavy* workloads.

### 6.5 Folly `fbstring` and Rust `bstr`

- **`folly::fbstring`:** A small-string-optimised, mutable, non-persistent
  string with COW semantics — used inside Facebook. The "small" size is
  ≤ 22 bytes (stored inline), the "large" size uses heap + refcount.
- **`bstr`:** <https://docs.rs/bstr/>. A view-like string over a
  `&[u8]`; useful for working with non-UTF-8 byte buffers. Not a
  container per se.

For badger's flat-strings-everywhere approach, the closest analogue is
`folly::fbstring`'s inline optimisation: a `string.rs` `String`
that lives inline in its 8-byte header for short strings. badger could
adopt this by storing short strings (≤24 bytes) inline in the object
header and falling back to a `len | bytes` allocation for longer
strings. The advantage is fewer header allocations and better cache
behaviour for short strings.

### 6.6 Adaptive Radix Tree (ART) and ROART

- **Title:** *The Adaptive Radix Tree: ARTful Indexing for Main-Memory Databases* (Leis et al., 2013, ICDE)
- **URL:** <http://www-db.in.tum.de/~leis/papers/ART.pdf>
- **ROART:** Leis et al., 2017 — read-optimised ART.

ART is a trie on byte sequences (not hash chunks) with adaptive node
sizes (Node4, Node16, Node48, Node256) that minimise cache misses.
ROART extends ART with a write buffer that absorbs updates in a flat
array and merges them into the tree in batches.

For a hash trie, ART is a different point in the design space: keys
must be byte-comparable and the trie is on the key bytes, not a hash.
For a language runtime whose keys include 64-bit hashes, complex
records, strings of arbitrary length, and values of arbitrary type, the
ART constraint is too tight. ROART's write buffer is the more
transferable idea: a write buffer for hash-trie updates is essentially
what `insert_mut` does in `map.rs:111-123`, but flat instead of
per-node.

### 6.7 Judy Array (Baskins, 2000)

- **Source:** Doug Baskins, *Judy Array*, <http://judy.sourceforge.net/>

A 256-way configurable-radix trie that scales to billions of elements
with very little per-element overhead. Useful when key space is sparse
and lookup-dominant (network routing tables, symbol tables).
Disadvantages: every operation is a long pointer chase; not
persistent; very poor cache behaviour compared to a HAMT. The Judy
implementation is famously complex and not generally maintained.

For badger, Judy is not directly applicable, but it shares the
"radix trie" design space with ART.

### 6.8 Recommendation for badger

The current `string.rs` (flat `8 + len`, no COW, no small-string
optimisation) is the simplest possible design and it is the right one
*for the workload currently measured* — short strings, mostly
interned. Adopting ropes would change the *type* of `String` (a tree
node rather than a flat byte array) and ripple through every standard
library function that touches strings.

What `string.rs` could *usefully* borrow without changing the type:

1. **Inline short strings** in the object header (`<24` bytes → store
   in `header.meta` and the length word).
2. **Substring sharing** by `bstr`-like slice views (a `&[u8]` wrapper
   that doesn't own the bytes), if `text::slice` becomes a hot path.
3. **Rope only on demand**: provide an optional `Rope` type in the
   standard library for users who do many `+`/`slice` operations, not
   for the default.

- **Verdict:** **defer.** Keep flat strings with COW where appropriate;
  add small-string optimisation later if profiling shows allocation
  pressure. Ropes are for editor-class workloads, not for a general
  language where median string length is small.

---

## 7. Persistent sorted maps and sets

### 7.1 Okasaki — *Purely Functional Data Structures* (1998, 2007 reprint)

- **Title:** *Purely Functional Data Structures*
- **Author:** Chris Okasaki
- **Year / venue:** PhD thesis CMU-CS-99-177 (1998); book, Cambridge
  University Press (2007 reprint)

The reference text for purely functional data structures. Okasaki
covers:

- **2-3 finger trees** (Chapter 3, § 3.4) — a balanced tree with O(log n)
  access, update, split, and concatenate; generalised by Hinze & Paterson
  to monoidal annotations.
- **Red-black trees** (Chapter 3, § 3.3) — the standard balanced BST
  adapted to pure persistence.
- **Binomial queues, lazy binomial heaps, splay heaps** (Chapter 3) —
  functional priority queues.
- **Chapter 10** — bottom-up mergesort, segmented tries.

For badger's needs (a sorted map type), Okasaki's red-black tree
construction is the baseline. The lift to weight-balanced trees
(Nievergelt-Reingold, see below) is a small improvement in
concatenation cost.

### 7.2 Nievergelt-Reingold — Weight-Balanced Trees (1973)

- **Title:** *Binary Search Trees of Bounded Balance* (Nievergelt & Reingold, 1973)
- **Hirai-Yamamoto:** *Balancing weight-balanced trees* (2011)
  <https://hackage.haskell.org/package/containers/docs/Data.Map.Strict.html#g:15>

A weight-balanced tree balances on the *number of leaves in the
subtrees*, not on the colour of the path. Weight-balanced trees have
cleaner concatenation than red-black trees (single rotation, no
recolouring cascade) and are the canonical balanced tree for purely
functional use. Clojure's `PersistentTreeMap` and Haskell's `Data.Map`
both use a weight-balanced tree (Clojure explicitly, Haskell since
the `containers` rewrite in 2011).

For badger, a weight-balanced tree (`OrdMap`) would be the natural
choice if/when the language grows a sorted map type. The
implementation cost is similar to a red-black tree in Rust.

### 7.3 Clojure `PersistentTreeMap`

- **Source:** `clojure/clojure`, `src/jvm/clojure/clojure/core.clj`
  `PersistentTreeMap` deftype
- **Spec:** red-black tree (rebalanced on insert and delete). Lookup
  O(log n), insert O(log n), `subseq` O(log n + k).

### 7.4 AA trees (Andersson, 1993)

- **Title:** *Balanced Search Trees Made Simple* (Arne Andersson, 1993, WADS)
- **URL:** <https://user.it.uu.se/~arnea/abs/simp.html>

A simplified red-black tree that uses a single integer "level" instead
of colour flags. Smaller code, identical asymptotic behaviour, slightly
faster in practice (≈10–20 % on microbenchmarks). Used in Rust's
`BTreeMap` since Rust 1.0 because of the simpler invariants.

For badger, AA-trees (or the modern `BTreeMap`-style B+ trees used in
`imbl::OrdMap` and `rpds::RedBlackTreeMap`) are the way to ship an
`OrdMap`.

### 7.5 Big-Endian Patricia Tries / HAMT-on-Ord

- **Title:** *Generalising Generalized Tries* (Ralf Hinze, 2001)
- **URL:** <https://www.cs.ox.ac.uk/ralf.hinze/Publications.html>

Patricia tries (a.k.a. radix tries) compress single-child paths. For a
purely functional sorted set over 64-bit keys, a big-endian Patricia
trie has O(log n) access and O(1) insertion when the new key differs
from the existing key in the highest bit. Used in Erlang's
`sets:from_list/1` for tiny integer sets.

For badger, a big-endian Patricia trie over `Int` keys is a tempting
specialised map type, but only for one specific signature.

### 7.6 Comparison vs HAMT for typical workloads

For unsorted maps, HAMT wins on average insert O(1) versus O(log n) for
balanced BSTs. For sorted maps, the balanced BST is the only option
unless we accept the O(n log n) cost of sorting an unsorted map's keys
after building.

For badger's typical workloads — `Map<String, V>`, `Map<Int, V>` —
HAMT is the right default. `OrdMap` would only need to be added if a
program does range scans or ordered iteration frequently.

- **Verdict:** **defer.** No need to ship `OrdMap` until profiling
  shows it would help.

---

## 8. Practical libraries and what badger can learn

### 8.1 Clojure `clojure.core` (Hickey & contributors)

- **Source:** <https://github.com/clojure/clojure/tree/master/src/jvm/clojure/lang>
- **Files of interest:**
  - `PersistentHashMap.java` (1400 lines) — HAMT map
  - `PersistentArrayMap.java` — flat-array map for n ≤ 8
  - `PersistentVector.java` (1056 lines) — 32-way trie vector
  - `PersistentTreeMap.java` — red-black tree sorted map
  - `PersistentHashSet.java`, `PersistentTreeSet.java` — wraps the maps

The single best thing the Clojure source teaches is the **structural
small-map optimisation**: `PersistentArrayMap` is a flat array of
key-value pairs used for n ≤ 8, and the underlying `IMap` interface
makes the choice transparent. When you create a map literal with `{k1 v1
k2 v2 k3 v3}`, the array representation is used; the first
`assoc`-that-would-grow-it converts to a `PersistentHashMap`. The
microbenchmark numbers are striking: `PersistentArrayMap` is roughly 5×
faster than `PersistentHashMap` for n ≤ 8 (Hickey's measurements in
the Clojure 1.6 release notes, 2014).

For badger, this is the **most immediately useful** observation: the
map-of-≤8-entries case is by far the most common (the `Map<String,
record>` used as a record's prototype, the `Map<symbol, expr>` used
for a let-bind's dictionary, the `Map<String, JSON-value>` for small
JSON objects). The current `hamt.rs` always allocates a trie's root
even for a 1-entry map, which means even a 2-element `{a: 1, b: 2}`
incurs at least one `alloc_node` (`hamt.rs:185-203`).

- **Verdict:** **very promising for badger.** Add a `MapArray` (flat
  array) representation for maps with n ≤ 8, transition to CHAMP at
  n > 8. This is the cheapest ~5× speedup for small maps.

### 8.2 Scala `cats-collections` (Peregud & contributors)

- **URL:** <https://github.com/typelevel/cats-collections>

Purely functional collections in Scala that build on `cats`. Similar
goals to badger's: small, idiomatic, immutable by default.

### 8.3 Haskell `unordered-containers` (Tibell) and `containers`

- **`unordered-containers`** uses a HAMT for `HashMap`/`HashSet` —
  <http://hackage.haskell.org/package/unordered-containers>.
- **`containers`** `Data.Map` and `Data.IntMap` use weight-balanced trees
  and a 32-way Patricia trie respectively.

### 8.4 Rust `im`, `imbl`, `rpds`

See § 3.3 and § 5. The most relevant design pattern is the
`archery::SharedPointerKind` trait that `im`/`imbl` use to switch between
`Rc` and `Arc`. badger doesn't need this — the GC is the reference
counter — but the *pattern* of "the smart pointer is a type parameter"
would translate to "the GC mark is a type parameter" if badger ever
wants specialised cell types.

### 8.5 OCaml `core_kernel` containers

- **URL:** <https://github.com/janestreet/core/tree/master/core/src>

Includes a HAMT-based map (`Core.Hashtbl`, `Base.Hashable`), a
weight-balanced tree map (`Core.Map`), and a rope (`Core.Rope`). The
Hamt module is interesting because OCaml is a GC'd language without
`Oop` indirection, similar to badger's architecture.

### 8.6 PureScript `purescript-collections`

- **URL:** <https://github.com/purescript/purescript-collections>

A HAMT-based `Map`/`Set` in PureScript (Haskell-like syntax, JS
backend). Useful as a code reference for a small, well-tested HAMT in
a GC'd language.

---

## 9. Benchmarking pitfalls and methodology

### 9.1 Bagwell — *Fast Functional Lists* (HLPP 2002)

- **Title:** *Fast Functional Lists, or Cats and their ways*
  (Bagwell, 2002)
- **URL:** <http://infoscience.epfl.ch/record/64458/>

Bagwell's follow-up to his HAMT paper, focusing on list-like
collections. He benchmarks against Haskell `Data.List` and `Data.IntMap`,
shows the cost of the 5-bit trie chunker, and demonstrates that
"persistent" with GC (or, for Haskell, lazy evaluation) is not as
expensive as the worst case. The published measurements (2002) are
outdated but the qualitative conclusions hold:

- The dominant cost is *cache misses* on the pointer chase, not
  allocation.
- Increasing the branching factor beyond 32 hurts cache behaviour even
  though it reduces depth.
- Lazy evaluation masks the constant factors of pure persistence; a
  strict GC'd runtime like badger sees them as real costs.

### 9.2 Hughes — *Why Functional Programming Matters* (1989)

The famous 1989 essay; one of its theses is that *persistent* data
structures turn many algorithms into library calls (e.g. quick-sort is
filter + prepend). For badger, the takeaway is that the persistent
map and list are **the foundation** the rest of the standard library
sits on, and that the most important thing for the foundation is
*fast persistent update* — exactly what `map::insert` /
`map::insert_owned` and `list::append` (`crates/badger_runtime/src/list.rs:661`)
provide.

### 9.3 The "small persistent map" problem

The most common mistake when optimising a HAMT is to benchmark
`insert` on a 1 000-element map. The overhead of path-copy on a
*trie of depth 3* is dwarfed by the cost of the GC walk. The
microbenchmark numbers in `reports/2026-10-19-stdlib-transformations.md:30`
are exemplary in this regard: they measure per-item operations
*including* collection overhead and amortise correctly. They also
separate the cost of "first build" from "replacement-only build" and
show that the runtime's transient path actually avoids allocation in
the steady state (`reports/2026-10-19-eager-list-map-and-map-entries.md:11`,
"transient_matches_a_model_and_stops_allocating_for_replacements").

The "small persistent map" problem is real and is the
PersistentArrayMap vs PersistentHashMap crossover in Clojure. See § 8.1.

### 9.4 The 32-way branching factor

All of Bagwell's designs, all of Clojure's designs, and all of the
modern Rust designs (`im`, `imbl`, `rpds`, `hashbrown`) use a
branching factor of either 32 (Bagwell 5-bit chunks) or 16 (the
Phil Bagwell tree-of-B-trees, used by Erlang ETS's large-map
representation). badger's `BITS = 5` at `hamt.rs:29` is the same
choice. The trade-off is well-understood:

- 32-way means each node has 32 child slots worth of bitmap (4 bytes
  bitmap + 8 bytes per occupied slot + 16 bytes header). For n = 10⁶,
  depth is 4; for n = 10⁹, depth is 5.
- 16-way means a smaller node (2 bytes bitmap, 4 bytes per occupied
  slot) and one extra hop at the same n. Easier on cache for
  cache-starved environments.

The "right" branching factor depends on the cache line size of the
target hardware. On a 64-byte cache line (the standard on modern x86),
a 16-byte header + 16 occupied 8-byte slots fits exactly 32 slots ×
8 bytes = 256 bytes = 4 cache lines; the bitmap is 4 bytes; and the
full 32-way slot array would be 32 × 8 = 256 bytes. A node that has
only one or two children is 16 bytes header + 16 or 24 bytes =
32-40 bytes, well under one cache line. This is exactly the design
target and is what `hamt.rs:101-108` (entry = 24 bytes = `hash, key,
val`, child = 8 bytes, header = 16 bytes) gives.

### 9.5 Cache-line and SIMD prefetch effects

Modern x86 cores prefetch sequential memory. A persistent trie of
32-way nodes with small branching factor (≤8 children) fits one node
per cache line, and the `cursor_next` traversal at `hamt.rs:621-658`
walks entries-then-children in a linear sequence within each node.
This is the same prefetcher-friendly pattern that ART's
`Node4`/`Node16`/`Node48`/`Node256` ladder exploits (Leis et al.
2013). badger's current 32-way design does not auto-promote to denser
nodes as the node fills, but for n ≤ 31 children (the maximum
non-collision node), the cache-line cost is bounded and acceptable.

### 9.6 Methodological recommendations for badger benchmarks

Based on the above, future badger perf reports should:

1. **Measure per-item** cost on large inputs (≥ 10 000 elements) and
   **per-call** cost on small inputs (≤ 8 elements). The benchmarks in
   `reports/2026-10-19-stdlib-transformations.md` do this well.
2. **Measure both first-build and steady-state** for transients. The
   `reports/2026-10-19-eager-list-map-and-map-entries.md` shows
   `insert_mut` allocating zero nodes for the steady state. This is
   the right comparison.
3. **Separate the GC cost** from the data-structure cost. The 2026-10-17
   GC profiling report already does this; the stdlib transformations
   report inherits the separation.
4. **Use a model** (`std::collections::HashMap`, `hashbrown::HashMap`)
   as a baseline for what a *mutable* hash map can do. The published
   Clojure numbers are useful as a sanity check (CHAMP vs PHM ≈ 3×
   iteration speedup).
5. **Always report `n`** and the **hash distribution** of the keys.
   `reports/2026-10-19-eager-list-map-and-map-entries.md:64-72` does
   this for `unique` (200 000 distinct Ints = 118 ns/op, dominated by
   HAMT insert).

---

## What this means for badger

Concrete recommendations, ranked by expected impact × implementation cost.

### A. Promote the small-map flat representation (very promising)

Add a flat `MapArray<K, V>` for maps with n ≤ 8, transitioning to the
HAMT only when the 9th `assoc` arrives. This is Clojure's
`PersistentArrayMap` pattern. It will:

- Reduce the per-element cost of small-map construction to a `memcpy`.
- Reduce the per-call `get` cost on small maps from "follow root, find
  bit, check entry" to a linear scan of an 8-slot array (still fast —
  8 comparisons fits one cache line).
- Remove the need for the per-element `alloc_node` call
  (`hamt.rs:185-203`) for small maps.

Implementation cost: one new struct, a flag bit on the `Map`/`Set`
wrapper indicating "is array", and a one-shot transition to HAMT when
n > 8. The HAMT transition is exactly `hamt::insert` with a 0-root
and the existing path-copy logic; no change to `hamt.rs` itself needed.

**Verdict: do this next. Cost: ~200 LOC. Expected win: ~3–5× on
small-map workloads.**

### B. Add CHAMP's sorting invariant (very promising)

Two changes to `crates/badger_runtime/src/hamt.rs`:

1. When inserting a new entry, *maintain sorted order by hash chunk*
   (bit position) within the entries array. The current insertion code
   (`hamt.rs:431-483`) finds `index(dm, b)` as the new slot, which is
   by-bit-position order, but if a *second* bit is set later in the
   same insert it's appended after; the result is not sorted. The
   invariant requires sorting after every insert (memmove at most 32
   entries; cheap).
2. Add the `datamap & nodemap == 0` invariant. The current code already
   separates them (`hamt.rs:81-86`), but `insert_at` at
   `hamt.rs:471-482` may have a subtle bit set in both when an
   existing child is replaced. The fix is to clear both bits before
   setting the right one.

Both invariants are checkable by an assertion in debug builds.

**Verdict: do this after (A). Cost: ~50 LOC + tests. Expected win: free
equality, free hashcode (currently O(n²) via `map::eq` at
`crates/badger_runtime/src/map.rs:221-238`), faster iteration.**

### C. Switch hasher to `AHash` (very promising)

Change `crates/badger_runtime/src/map.rs:39` from
`rustc_hash::FxHasher` to `ahash::AHasher`. AHash is the modern
DOS-resistant default in `std::collections::HashMap` (via `hashbrown`).
The dependency is one line in `Cargo.toml`. Expected impact: better
distribution on small keys, immunity to a hostile pattern that triggers
pathological collisions.

**Verdict: do this. Cost: ~5 LOC. Expected win: better defence against
adversarial inputs, no perf regression.**

### D. Replace `Owned` epoch tokens with a generational bump allocation
(medium-term)

`crates/badger_runtime/src/hamt.rs:172-180` uses GC header bits to
store the "current owner", which costs two header bits per node. With
~30 M HAMT nodes allocated per request in the realworld workloads
(`reports/2026-10-17-gc-profiling.md`), the header pressure is
nontrivial. A future refactor could allocate HAMT builders in a private
nursery that resets when the builder finishes, eliminating the need for
the per-node token entirely.

This is a runtime change (nursery in `crates/badger_runtime/src/gc/`),
not a `hamt.rs` change, so it lives outside this report.

**Verdict: future work. Cost: depends on nursery design. Expected win:
smaller HAMT node header, no `gc::barrier` calls in `hamt.rs:346`.**

### E. Add RRB-style persistent vectors (low priority)

`crates/badger_runtime/src/list.rs` already covers the median workload
well (small lists, sequential append, indexed access). RRB-trees
would add O(log n) `concat` and `slice` between two `List<T>` values,
which is currently an O(n) copy via `push_run` (`list.rs:540-553`).

For the workloads currently measured (median list size <100), RRB is
likely a net loss because of the pointer-chase cost (Jiffy's
microbenchmarks show `imbl::Vector` is slower than `std::Vec` for
n ≤ 64).

**Verdict: hold. Reconsider only if profiling shows list-`concat` is a
hot path; the recent `reports/2026-10-19-stdlib-transformations.md`
shows list ops are not the binding constraint.**

### F. Keep flat strings; defer ropes (low priority)

`crates/badger_runtime/src/string.rs` is flat `len | bytes`, with
interned symbols for stable string identity. The median badger string
is short (paths, keys, JSON values) and the GC handles small strings
in its small-object pools.

A rope would help only if badger ships editor-like workloads with
frequent mid-string inserts. Today it does not.

**Verdict: defer. Possible future lever: small-string inline storage
in the object header (≈16 bytes saved per short string). Cost: ~50 LOC.
Expected win: reduced allocator pressure on short strings.**

### G. Add `OrdMap`/`OrdSet` (low priority)

Clojure's `PersistentTreeMap` and Haskell's `Data.Map` exist because
they enable ordered iteration and range scans. badger's programs
don't (the `bench/records` and `bench/collections` benchmarks use
unsorted maps; the `realworld` benchmarks don't show order-sensitive
map access).

If a future standard library function (`Map.min_by_key`, range scan,
etc.) becomes a hot path, add an `OrdMap` based on a weight-balanced
tree or AA tree. Not now.

**Verdict: defer. Cost if added: ~300 LOC. Expected win: depends on
workload.**

### Summary table

| Recommendation | Impact | Cost | Section |
|---|---|---|---|
| Add small-map flat representation | ~3–5× on small maps | ~200 LOC | A |
| CHAMP invariants (sorted, disjoint bitmaps) | free `eq`/`hashcode`, faster iter | ~50 LOC + tests | B |
| Switch hasher to `AHash` | better distribution, DOS resistance | ~5 LOC | C |
| HAMT nursery (generational bump) | smaller node header | runtime change | D |
| RRB persistent vectors | O(log n) `concat`/`slice` | ~600 LOC | E |
| Ropes for strings | rare wins on long strings | not done now | F |
| `OrdMap`/`OrdSet` | enables ordered iteration | ~300 LOC | G |

### Line references in badger code

- `crates/badger_runtime/src/hamt.rs:29` — `BITS = 5` (5-bit hash chunks)
- `crates/badger_runtime/src/hamt.rs:81-94` — two bitmaps (`datamap`,
  `nodemap`, `flags`)
- `crates/badger_runtime/src/hamt.rs:110-120` — bit / index helpers
- `crates/badger_runtime/src/hamt.rs:125-167` — `Mutation` trait
  (Persistent / Builder / Epoch)
- `crates/badger_runtime/src/hamt.rs:185-203` — `alloc_node` (no
  CHAMP sort yet)
- `crates/badger_runtime/src/hamt.rs:240-287` — `splice_owned` (the
  CHAMP path-copy)
- `crates/badger_runtime/src/hamt.rs:294-348` — `edit` (in-place edit
  for Builder / Epoch)
- `crates/badger_runtime/src/hamt.rs:418-428` — `insert_with` entry
- `crates/badger_runtime/src/hamt.rs:485-549` — `remove` / `remove_at`
- `crates/badger_runtime/src/hamt.rs:621-658` — `cursor_next`
  (iteration, single-pass over entries-then-children)
- `crates/badger_runtime/src/list.rs:24` — `PAGE_BYTES = 4096`
- `crates/badger_runtime/src/list.rs:29-38` — `page_cap_for`
- `crates/badger_runtime/src/list.rs:319-358` — `stabilize` (small
  first-page promotion)
- `crates/badger_runtime/src/list.rs:406-447` — `push` (single-element
  persistent append)
- `crates/badger_runtime/src/list.rs:487-535` — `push_raw` (bulk
  append)
- `crates/badger_runtime/src/list.rs:568-610` — `fill` (bulk build
  from a generator)
- `crates/badger_runtime/src/list.rs:661-674` — `append` (persistent
  single-element append, O(1) when sharing)
- `crates/badger_runtime/src/string.rs:12-19` — `alloc_bytes` (flat
  string)
- `crates/badger_runtime/src/string.rs:81-98` — `SYMBOLS` (interned
  symbols)
- `crates/badger_runtime/src/map.rs:39-43` — current hasher
  (`rustc_hash::FxHasher`)
- `crates/badger_runtime/src/map.rs:111-123` — `insert_mut` (transient
  builder path)
- `crates/badger_runtime/src/map.rs:155-185` — `insert_owned` (epoch
  ownership, mutable-in-place-with-snapshot)
- `crates/badger_runtime/src/map.rs:221-238` — `eq` (currently O(n²)
  in the worst case, would be O(n) with CHAMP sorting invariant)
- `reports/2026-10-19-eager-list-map-and-map-entries.md:108-115` —
  "Insertion is dominated by CHAMP path copies and node edits
  required to keep retained snapshots immutable"
- `reports/2026-10-19-stdlib-transformations.md:67-72` — "unique on
  200,000 distinct Ints is 118 ns/op; almost all of it is HAMT insert
  cost"

---

## Sources

Canonical URLs for the citations above. All URLs accessed 2026-10-19
unless noted otherwise.

### Foundational HAMT papers

- Bagwell, P. *Ideal Hash Trees*, 2001.
  <https://infoscience.epfl.ch/record/64398/files/idealhashtrees.pdf>
- Bagwell, P. *Fast Functional Lists, or Cats and their ways*, HLPP 2002.
  <http://infoscience.epfl.ch/record/64458/>
- Steindorfer, M. *Optimizing Hash-Array Mapped Tries for Fast and Compact
  Immutable Sets*, ECOOP 2017.
  <https://dl.acm.org/doi/10.1145/3093333.3039889>
- Steindorfer, M. *Efficient Immutable Collections*, PhD thesis, CWI / UvA,
  2017.
  <https://michael.steindorfer.name/publications/phd-thesis-efficient-immutable-collections.pdf>
- Wikipedia: *Hash array mapped trie*.
  <https://en.wikipedia.org/wiki/Hash_array_mapped_trie>

### Persistent vectors and RRB

- Bagwell, P., Rompf, T. *RRB-Trees: Efficient Immutable Vectors*, OOPSLA 2012.
  <https://infoscience.epfl.ch/record/169543/files/RRBVector.pdf>
- Hickey, R. *Understanding Clojure's PersistentVector*.
  <https://hypirion.com/musings/understanding-persistent-vector-pt-1>
  <https://hypirion.com/musings/understanding-persistent-vector-pt-2>
- `imbl` (Rust RRB + HAMT + B+tree).
  <https://github.com/jneem/imbl> ; <https://docs.rs/imbl/>
- `rpds` (Rust PersistentVector + HAMT + RedBlackTree).
  <https://github.com/orium/rpds> ; <https://docs.rs/rpds/>
- `im` (original Rust persistent collections).
  <https://docs.rs/im/>
- `jiffy` (Haskell "footprint-friendly" vectors).
  <https://github.com/Eidetica/jiffy>

### Concurrent / Ctrie

- Prokopec, A., Bagwell, P., Odersky, M. *Cache-Aware Lock-Free Concurrent
  Hash Tries*, EPFL tech report 2011.
  <https://infoscience.epfl.ch/record/166908/files/ctries-techreport.pdf>
- Prokopec, A., Bronson, N. G., Bagwell, P., Odersky, M. *Concurrent Tries
  with Efficient Non-Blocking Snapshots*, 2012.
  <http://lampwww.epfl.ch/~prokopec/ctries-snapshot.pdf>
- Prokopec, A. *Cache-Tries: Concurrent Lock-Free Hash Tries with
  Constant-Time Operations*, PPoPP 2018.
  <http://aleksandar-prokopec.com/resources/docs/p137-prokopec.pdf>
- Wikipedia: *Ctrie*.
  <https://en.wikipedia.org/wiki/Ctrie>
- Scala `TrieMap` source.
  <https://github.com/scala/scala/blob/2.13.x/src/library/scala/collection/concurrent/TrieMap.scala>

### Sorted maps and Okasaki

- Okasaki, C. *Purely Functional Data Structures*, PhD thesis 1998; book
  Cambridge University Press 2007 (reprint).
  <https://www.cs.cmu.edu/~rwh/theses/okasaki.pdf>
- Nievergelt, J., Reingold, E. M. *Binary Search Trees of Bounded Balance*,
  SIAM J. Comput. 2(1), 1973.
- Hirai, Y., Yamamoto, K. *Balancing weight-balanced trees*, JFP 2011.
  <https://hackage.haskell.org/package/containers/docs/Data.Map.Strict.html#g:15>
- Andersson, A. *Balanced Search Trees Made Simple*, WADS 1993.
  <https://user.it.uu.se/~arnea/abs/simp.html>
- Hinze, R. *Generalizing Generalized Tries*, 2001.
  <https://www.cs.ox.ac.uk/ralf.hinze/Publications.html>

### Hash functions

- Appleby, A. *MurmurHash3*, 2011 (public domain).
  <https://github.com/aappleby/smhasher>
- Collet, Y. *xxHash*.
  <https://github.com/Cyan4973/xxHash>
- Kaitchuck, T., Lemire, D. *The aHash Hash Function*.
  <https://github.com/tkaitchuck/aHash>
- Aumasson, J.-P., Bernstein, D. J. *SipHash: a fast short-input performance
  function*.
  <https://www.aumasson.jp/siphash/siphash.pdf>
- Lemire, D., Kaitchuck, T. *Fast keyed-hash functions for GPUs and CPUs*
  (predecessor publication).
  <https://arxiv.org/abs/1912.04865>

### Strings / ropes / ART

- Boehm, H.-J., Atkinson, R., Plass, M. *Ropes: an Alternative to Strings*,
  Software: Practice and Experience 25(12), 1995.
  <https://www.cs.tufts.edu/comp/150FP/archive/hans-boehm/ropes.pdf>
- Wikipedia: *Rope (data structure)*.
  <https://en.wikipedia.org/wiki/Rope_(data_structure)>
- Leis, V., Kemper, A., Neumann, T., Funke, F., Hildebrandt, T. *The Adaptive
  Radix Tree: ARTful Indexing for Main-Memory Databases*, ICDE 2013.
  <http://www-db.in.tum.de/~leis/papers/ART.pdf>
- Abseil `absl::Cord`.
  <https://github.com/abseil/abseil-cpp/blob/master/absl/strings/cord.h>
- `ropey` (Rust rope library).
  <https://docs.rs/ropey/>
- Wikipedia: *Piece table*.
  <https://en.wikipedia.org/wiki/Piece_table>
- `bstr` (Rust byte-string view).
  <https://docs.rs/bstr/>

### Clojure / Scala / Haskell / Rust persistent collections

- Clojure `PersistentHashMap`, `PersistentVector`, `PersistentTreeMap`,
  `PersistentArrayMap`, `PersistentHashSet`, `PersistentTreeSet`.
  <https://github.com/clojure/clojure/tree/master/src/jvm/clojure/lang>
- `scala.collection.concurrent.TrieMap`.
  <https://github.com/scala/scala/blob/2.13.x/src/library/scala/collection/concurrent/TrieMap.scala>
- `unordered-containers` (Haskell HAMT).
  <https://hackage.haskell.org/package/unordered-containers>
- `containers` (Haskell weight-balanced trees).
  <https://hackage.haskell.org/package/containers>
- `core_kernel` (OCaml).
  <https://github.com/janestreet/core/tree/master/core/src>
- `purescript-collections`.
  <https://github.com/purescript/purescript-collections>

### Erlang / ETS

- Erlang `ets` module.
  <https://www.erlang.org/doc/man/ets.html>
- Erlang `erl_db_hash.c`, `erl_db_tree.c` (BEAM source).
  <https://github.com/erlang/otp>

### Concurrent hash tables (for mutable baselines)

- Maier, T., Sanders, P., Dementiev, R. *Concurrent Hash Tables: Fast and
  General?(!)*, 2016.
  <https://arxiv.org/abs/1601.04017>

### Transient patterns

- Hickey, R. *Transients*, Clojure documentation.
  <https://clojure.org/reference/transients>
- Hickey, R. *On Transients*, 2010.
  <https://clojure.org/news/2010/06/05/transients-are-condemned>

### Other

- Judy Array, Baskins, D. <http://judy.sourceforge.net/>
- CORD (Boehm GC).
  <https://github.com/ivmai/bdwgc/>
- SGI C++ `std::rope`.
  <https://web.archive.org/web/20121225183151/http://www.sgi.com/tech/stl/Rope.html>