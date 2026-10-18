# Memory management optimisation plan

Status: proposed. Targets `crates/lang_runtime/src/{gc,list,map,buffer,channel,string,desc,value,task}.rs`. Aligned with [spec/04-memory-and-collections.md](../spec/04-memory-and-collections.md).

This document is in two parts:

1. **Findings** — what the current allocator/collector actually does, what is
   expensive, and where the wasted bytes live. Each finding cites the source
   line that implements it.
2. **Plan** — a prioritised, ordered set of optimisations. Each item lists
   the change, the work it removes, an estimated impact, and how to verify it.

The optimisations are conservative with respect to the language semantics:
non-moving precise heap, shadow-stack roots, descriptor-driven tracing,
structural-sharing maps, paged lists, affine builders, immortal literals.
None of the changes below introduce moves, change pointer stability, or
break the typed-tracing contract.

---

## Part 1 — Findings

### F1. Every object is its own system-allocation

`gc::alloc` calls `alloc_zeroed(Layout)` per object and pushes the raw
pointer into `Heap.objects`
(`crates/lang_runtime/src/gc.rs:163`–`gc.rs:170`). There is no free list,
no size-class slab, no thread-local bump cache. For tiny records the
per-object overhead dominates: a `{id: Int, name: String}` record has a
16-byte `Header` plus an 8-byte payload (24 bytes logical) but the
allocator hands back a 24-byte slab from glibc, with the usual
metadata/scribble cost.

For allocation-heavy workloads this is the single largest cost. The
records benchmark in `reports/2026-10-15-optimization-baseline.json`
shows ~109 ns/record; on Linux `malloc` plus zeroing of a 24-byte
region accounts for roughly half of that.

Impact: large.

### F2. The `objects` Vec is grown monotonically and rebuilt every collection

`Heap.objects: Vec<*mut Header>` is appended to on every allocation and
is *replaced* by a freshly allocated `kept: Vec::with_capacity(...)` at
the end of every sweep
(`crates/lang_runtime/src/gc.rs:170` and `gc.rs:348`–`gc.rs:381`). The
old buffer is freed. Two consequences:

- The heap's resident metadata grows to the high-water mark of total
  allocations ever live, even when only a small set is currently live.
  In the heap bench the peak is around 3 000 000 entries, then the live
  set settles to ~278 000; the `objects` Vec stays at peak size forever.
- Every collection allocates a Vec of `≈ live` pointers, even though
  `objects` already has that capacity.

Impact: medium for steady-state memory; small for time (one allocation
per GC).

### F3. Tracing uses a `&mut dyn FnMut` and a per-object `children` Vec

`gc::collect_inner` builds a fresh `Vec<u64>` per traced object
(`crates/lang_runtime/src/gc.rs:339`) and pushes every child pointer
through a virtual `FnMut` callback. For list pages with hundreds of
elements, or records with many fields, the dispatch + push/pop overhead
is significant.

A typed visitor (or inlining the trace loop into `trace`) eliminates the
trait-object indirection and lets the compiler fold the per-element
loads.

Impact: medium for collections with many small traced objects; small
for one-shot allocations.

### F4. `temp_roots` is a `Vec<u64>` reused as a stack and as an RAII pool

`Heap.temp_roots` is appended/popped by `push_root`/`pop_root` and is
also the storage that `Roots::new() / Roots::add() / Drop` uses as a
checkpointing pool
(`crates/lang_runtime/src/gc.rs:202`–`gc.rs:246`).

- `Drop` calls `truncate(self.0)` which memmoves the tail off the Vec.
  `truncate` is O(remaining) — for deeply nested runtime helpers that
  push several roots the cost compounds.
- `Roots::new` + `Roots::add` runs at the top of nearly every runtime
  function that allocates (`list::push`, `list::finish`, `list::append`,
  `map::insert`, `map::remove`, every `make_record`/`make_enum`, every
  intrinsic that allocates). Each one pushes onto the same shared Vec
  under the runtime lock.

The simplest improvement is to make `temp_roots` a small inline buffer
(fixed capacity, e.g. 16 words) plus a spill Vec, and have `Roots::Drop`
truncate only by an exact `set_len` rather than a memmove. Even
simpler: replace the RAII guard with a counted save of `temp_roots.len()`
and `set_len` on drop, with no truncation when the guard is empty.

Impact: medium for allocation-heavy workloads; small for steady-state
memory.

### F5. `pinned` uses linear search and `Vec::remove`

`unpin` does `iter().rposition(...)` then `remove(i)` on a Vec
(`crates/lang_runtime/src/gc.rs:212`–`gc.rs:216`). With many pins
outstanding (every task holds its `entry`, every lease holds its
batch) `unpin` becomes O(N). The set semantics ("keep alive until
unpin") want an intrusive linked list or a Vec swap-remove instead of
shift-remove.

Impact: small for short-lived programs; medium for long-running
multi-tenant workloads that hold many leases.

### F6. List pages are individually GC-allocated

`list::push` allocates a page via `gc::alloc(page_desc(), ...)` every
`page_cap` appends
(`crates/lang_runtime/src/list.rs:130`–`list.rs:142`). The list bench
(`paged list append: 11 ns/op`, see baseline JSON) does 2 000 000
appends at `Int` (8 bytes), so 4096-element pages → ~488 page
allocations per run, each one a `malloc`. Page creation is dominated
by `malloc`, not by the trace bookkeeping.

`list::append` (persistent) additionally does `data(w).pages.clone()`
(`crates/lang_runtime/src/list.rs:207`), copying the entire page
directory Vec every call. For a list with thousands of pages this is
O(pages) plus an allocation of the same size.

Impact: large for builders/large lists; medium for small lists.

### F7. List page header writes the elem descriptor pointer on every page

`list::push` writes `*(p as *mut *const TypeDesc) = elem;` at every
page boundary (`crates/lang_runtime/src/list.rs:139`). For an
`Int`-element list the elem is the same `&'static TypeDesc` every time.
This is fine, but the page also stores `len` at offset 8
(`crates/lang_runtime/src/list.rs:149`) — and the *same* value is
written on every push into the page. Both writes are unnecessary for
non-pointer elements; the descriptor could live in the `ListData`
once, and `len` could be the only per-page mutable word.

Impact: small.

### F8. Maps add a fixed 512-byte "pressure" per persistent insert

`map::with_table` calls `gc::add_pressure(512)`
(`crates/lang_runtime/src/map.rs:86`). This is a coarse heuristic for
the working-set growth of an `imbl` HAMT insert. It has two problems:

- It is independent of map size — a 1000-entry map gets the same
  charge as a 1-entry map.
- It is unrelated to the actual allocation: an `imbl` insert into a
  1000-entry map allocates roughly `5 × log32(n)` shared nodes (about
  35 nodes), each ~`Box<Inner>` of 32 `Shared` slots plus tag/status
  — easily 1–2 KB total. The 512-byte credit understates by 2–4×,
  which means the GC runs too late for map-heavy programs.

The persistent bench (`persistent map insert: 2.5 µs/op` on 200k
entries) is not allocation-dominated — the hash trie path copy is the
bigger cost — but the pressure credit is still wrong.

Impact: small for perf; medium for memory tuning.

### F9. `map::entries` materialises the whole map into a `Vec`

`map::entries` (`crates/lang_runtime/src/map.rs:133`) returns
`Vec<(u64, u64)>` from `iter().collect()`. This Vec is then used by:

- `value::hash` for maps/sets (`crates/lang_runtime/src/value.rs:172`–`value.rs:192`)
- `value::eq` for maps/sets (`crates/lang_runtime/src/map.rs:137`)
- `channel::admission_size` for `Map`/`Set` (`crates/lang_runtime/src/channel.rs:301`–`channel.rs:312`)
- `value::display` for maps/sets (`crates/lang_runtime/src/value.rs:378`–`value.rs:391`)

`admission_size` in particular runs on every `send` to a byte-budgeted
channel and clones the entire map's worth of `(k, v)` pairs to sum
their sizes. For a map of N entries sent M times, this is O(N·M) and
the bytes allocated are O(N) per send.

Impact: large for byte-budgeted channels carrying maps; medium for
hash equality on big maps.

### F10. `buffer::data` is an extra heap allocation per buffer

`buffer::new` allocates the 8-byte payload object, then `Box::new`s a
`BufData { items: Vec<u64>, ... }` and stores its raw pointer in the
payload (`crates/lang_runtime/src/buffer.rs:43`–`buffer.rs:48`). So a
buffer needs *two* heap allocations (the 16-byte header plus 8-byte
payload, plus the `BufData` box) plus the `Vec<u64>` storage inside
it. A buffer of 1 element costs three allocations.

`admission_size` for a buffer calls `b.items.clone()`
(`crates/lang_runtime/src/channel.rs:317`–`channel.rs:321`), which
clones the elements Vec just to iterate. Same O(N·M) pattern as F9.

Impact: large for buffer-heavy programs; medium for byte-budgeted
channels.

### F11. Channel registry is linear-searched in two hot paths

`release_leases_of` iterates the entire `REGISTRY` for *every*
finishing task (`crates/lang_runtime/src/channel.rs:741`), and
`finalize_chan` does a `REGISTRY.retain(|(_, w)| *w != o as u64)` to
remove one channel from the registry
(`crates/lang_runtime/src/channel.rs:146`). `retain` is O(channels).

Two hot consequences:

- A finished task with no leases still walks every channel.
- Closing a channel walks every channel before it.

A task-local lease set (each task remembers the leases it holds) plus
reverse links (a channel's `leases` already keys by task ID at
`crates/lang_runtime/src/channel.rs:691`) avoids both walks.

Impact: small for short-lived programs; medium for long-running
multi-channel servers.

### F12. `channel::pool` is bounded at 4 entries per channel

`release` returns a leased batch's page buffer to `d.pool` only if
`d.pool.len() < 4` (`crates/lang_runtime/src/channel.rs:772`–`channel.rs:774`).
For high-throughput channels the 4-entry bound keeps most messages
going through fresh `Vec::new()` allocations, defeating the reuse
intent. Either remove the bound, set it to a per-channel bytes target
(say 256 KiB), or replace it with a global page-buffer pool.

Impact: medium for messages.lang.

### F13. `symbol` uses a global `Mutex<HashMap<String, u64>>` with `String` keys

`string::symbol` locks the global mutex on every call and inserts a
fresh `String` (`crates/lang_runtime/src/string.rs:58`–`string.rs:71`).
Switching to `Box<str>` keys avoids the heap allocation per new symbol
and (with `raw_entry_mut`) avoids hashing the `String` value.

Impact: medium for programs with many distinct symbols; small
otherwise.

### F14. Symbol/literal immortal objects are never reused

`gc::alloc_immortal` is called for literals, static closures, and
interned symbols (`crates/lang_runtime/src/string.rs:64`). Each one is
its own `malloc` and is never freed. For programs with many small
literals or symbols this is a slow leak into the system heap.

Symbols *must* be immortal (their pointer is observable), but
interning already gives them uniqueness — only *one* malloc per symbol
name is fine. Literals are a different story: a large list literal
that appears many times in source allocates many immortal objects.
De-duping literals via a small intern table keyed by `desc + payload
hash` (for small, hashable payloads) would collapse duplicates.

Impact: small for memory; small for perf.

### F15. Header layout has fixed 16-byte overhead per object

`pub struct Header { pub desc: *const TypeDesc, pub meta: u64 }` is 16
bytes (`crates/lang_runtime/src/gc.rs:30`). For a 1-byte payload the
logical object is 17 bytes but the slab is 24 bytes (header 16 +
minimum 8 from `layout()`). That is a 7-byte overhead plus alignment.

`layout()` forces `HEADER + payload.max(8)`, so payloads below 8 bytes
already pay for 8. A size-class slab (objects ≤ 32 bytes share a 32-byte
slab; ≤ 64 bytes share a 64-byte slab; …) lowers overhead for small
records to a few bytes.

Impact: medium for memory in small-record-heavy workloads; small for
perf.

### F16. `for_each_root` walks task shadow stacks manually under a callback

`task::for_each_root` reads `(*fr).len`, walks `slots`, dereferences each
slot, and recurses to `(*fr).prev` (`crates/lang_runtime/src/task.rs:568`–`task.rs:578`).
This runs on every GC under a `&mut dyn FnMut(u64)` callback, so each
pointer load goes through a virtual call.

For dense tasks with deep shadow stacks (every call frame pushes a
shadow frame in compiled code) this is the dominant per-collection
cost. Options:

- Skip the `FnMut` indirection by exposing an `unsafe` typed visitor.
- Maintain a per-task pointer list (just the words, no frame chain) so
  roots are visited as a flat slice.
- Coalesce frames when the calling pattern permits.

Impact: large for tasks with deep shadow stacks; small for shallow
ones.

### F17. `gc::trace` is a big match on `Kind`

`gc::trace` switches on `desc.kind` and dispatches to per-Kind code
(`crates/lang_runtime/src/gc.rs:398`–`gc.rs:423`). The branches are
cold for any one type but the function is called once per object per
GC. Monomorphising per descriptor (one `trace` per `TypeDesc`, in a
function table stored on the descriptor) eliminates the match. The
same trick applied to `value::hash` and `value::eq` would also help.

Impact: medium for GC-heavy workloads.

### F18. `value::hash` allocates a fresh `FxHasher` per map/set entry

`value::hash` for `Kind::Map`/`Kind::Set` calls
`FxHasher::default()` once per entry (`crates/lang_runtime/src/value.rs:175`–`value.rs:191`).
For a 200k-entry map this is 200 000 short-lived hasher allocations.
Reusing one `FxHasher` per `hash` call and calling `finish()` /
`reset()` per entry is free.

Impact: medium for hashing large maps/sets.

### F19. `heap_json` and `task::snapshot_json` build strings unconditionally

These inspection paths construct large JSON `String`s on demand
(`crates/lang_runtime/src/inspect.rs:89` and the channel snapshot at
`crates/lang_runtime/src/channel.rs:116`). They are only triggered by
`lang inspect --connect`, but `channel::count()` at
`crates/lang_runtime/src/channel.rs:68` is called as part of the
snapshot and materialises three VecDeque lengths plus a per-producer
Vec length. Cheap individually, but worth noting if any other path
ever calls it.

Impact: small; only relevant when inspection is running.

### F20. `Roots` guards are pervasive but trivial

`Roots::new()` constructs `Roots(heap().temp_roots.len())` and
`Roots::add` calls `heap().temp_roots.push(w)` under the runtime lock.
At the bench scale, every `list::push` (with no page boundary) calls
`data(w)`; only on a page boundary is a `Roots` guard created
(`crates/lang_runtime/src/list.rs:131`). Other call sites create a
guard on every call regardless. The lock is already held so the cost
is the `Vec::len` + the `Vec::push` (which itself does an inline
capacity check). Negligible per call but pervasive.

This is mostly a fix-target for F4.

Impact: small.

---

## Part 2 — Plan

Items are ordered by expected end-to-end impact on the existing
benchmarks (`heap.lang`, `collections.lang`, `messages.lang`,
`records.lang`) and the long-running memory footprint. Items marked
**[invariant]** preserve the language's stated contracts (non-moving
heap, precise tracing, shadow-stack roots, descriptor-driven
finalisation, immortal literals, affine builders).

### P1. Slab-allocate small objects **[invariant]**

**Change.** Replace the per-object `alloc_zeroed(layout(payload))` in
`gc::alloc` with a size-class slab for objects whose payload is at
most some threshold (proposed: 256 bytes). Slabs are carved from
larger `mmap`-backed blocks; allocation is a pointer bump inside the
current block; freeing returns the slot to a per-class free list.

Each slab block is registered as a single `Header`-bearing chunk (or as
a typed descriptor) so the GC can trace and sweep the block in one
pass. Within a slab block, slot addresses are deterministic, so the
heap verifier still aborts on freed use.

Larger payloads keep the current per-object `malloc` path until a slab
threshold is passed.

**Removes.** One `malloc`+zero per small-object allocation. Roughly
20–40 ns per allocation on glibc.

**Estimated impact.** Records bench `-25%` to `-40%` per-record;
`bench/heap.lang` allocation time from ~109 ns/record to ~60 ns/record.
Heap metadata (`objects` Vec) shrinks dramatically because small
records live inside slab blocks rather than as separate entries.

**Verification.** Re-run `bench/heap.lang` and `bench/records.lang`
with one and four workers; verify
`LANG_GC_VERIFY=1 LANG_GC_STRESS=1` is clean. Add a unit test that
allocates and frees in a tight loop and asserts `Heap.objects.len()`
stays bounded.

### P2. Maintain `Heap.objects` across collections, with shrink-on-low-watermark

**Change.** In `collect_inner`, instead of building a new `kept: Vec`
and assigning it, reuse `h.objects` in place: walk it, swap-keep live
entries, truncate the tail, and only shrink via `shrink_to` when
`live < objects.len() / 4` over the last several cycles.

**Removes.** One Vec allocation per collection; steady-state memory
proportional to live, not to high-water mark.

**Estimated impact.** Steady-state `objects` Vec memory drops from
peak allocation count to live count (278k → maybe a few thousand for
short-lived programs). Eliminates ~8 MB at the bench's high-water
mark.

**Verification.** Inspect `inspect.heap()` output across many cycles;
assert peak RSS no longer grows across cycles.

### P3. Pool and reuse the GC mark stack

**Change.** Move the `stack: Vec<u64>` allocated in `collect_inner`
into `Heap` and reuse it. Optionally expose a small inline buffer (32
words) so most collections never touch the heap at all.

**Removes.** One Vec allocation per collection.

**Estimated impact.** Small perf, small memory.

### P4. Inline root stack and skip the `FnMut` indirection in tracing

**Change.** Replace the per-trace `Vec<u64>` + `&mut dyn FnMut(u64)`
with a typed visitor: `pub fn trace_into(w: u64, mark: &mut impl
FnMut(u64))`. Inline simple cases (records with a few ptr fields,
enums with their tag) into the visitor. Monomorphise per descriptor
in `gc::trace` via a `TraceFn` table on `TypeDesc`.

**Removes.** Virtual call per child pointer; the per-object
`children` Vec.

**Estimated impact.** Collection pause time drops noticeably for
workloads with many small traced objects (records, list pages).

**Verification.** `LANG_GC_STRESS=1` perf on `bench/collections.heap`;
measure `pause_total_ns`/`pause_max_ns` before/after.

### P5. Size-class or page-aware allocation for list pages **[invariant]**

**Change.** A list page can be much larger than the slab threshold
(e.g. 4 KB default). Two options:

- **(a)** Allocate pages from a separate page allocator that hands out
  whole 4 KiB chunks from a chunk bitmap; the page can still carry a
  `Header` for tracing.
- **(b)** Increase the slab threshold so pages are slabbed too, with a
  slab size of, say, 8 KiB.

Pages are already a fixed size; this turns N page allocations into a
small number of chunk bitmap flips.

**Removes.** One `malloc` per list page.

**Estimated impact.** `paged list append` from 11 ns/op to ~7 ns/op
once the page-creation fraction is removed. Less dramatic in steady
state because pages amortise many appends.

### P6. Skip page-element descriptor re-write on each push **[invariant]**

**Change.** `list::push` writes the elem descriptor pointer into every
new page and rewrites the page-length field on every push
(`crates/lang_runtime/src/list.rs:139` and `list.rs:149`). The page
length can be recovered from the list's `ListData` (offset
`len % page_cap`). The descriptor pointer can be cached in `ListData`
once and reused for `trace_page`.

**Removes.** One 8-byte store per append.

**Estimated impact.** Small but free.

### P7. Persistent `list::append` clones pages incrementally

**Change.** `list::append` currently does
`let (src_pages, src_len) = (data(w).pages.clone(), data(w).len);`
(`crates/lang_runtime/src/list.rs:207`), cloning the entire page
directory even when only the tail might need fixing up. Two
alternatives:

- **(a)** Use a copy-on-write directory: `pages: Arc<Vec<u64>>` with a
  clone-on-write that copies only the *last* page if it must be copied
  (already implemented at `list.rs:218`–`list.rs:231`).
- **(b)** Have the persistent `append` take the (read-only) source's
  page directory by reference and build the new list with `Arc::clone`
  semantics, copying only the last page.

**Removes.** One Vec allocation per persistent `append`; one `O(pages)`
copy.

**Estimated impact.** Persistent list operations drop by the size of
the directory.

### P8. Correct and unify map pressure accounting

**Change.** Replace `gc::add_pressure(512)` per persistent insert
(`crates/lang_runtime/src/map.rs:86`) with a count derived from the
actual allocation: query `src.table.len()` before and after and charge
`(after - before).max(1) * NODE_BYTES`, where `NODE_BYTES` is a
calibrated constant for `imbl`'s HAMT node size (32-slot tries,
roughly 128 bytes each). For `insert_mut` (builder) charge once at
the end of the build.

Also expose the pressure as part of `gc::alloc` itself: have
`with_table` accept a `pressure` argument.

**Removes.** Mis-sized pressure; collection timing tuned to actual
HAMT allocation.

**Estimated impact.** Map-heavy programs collect at the right time
(not too early, not too late). Steady-state RSS shrinks accordingly.

**Verification.** Run `bench/collections.lang` with persistent maps
and report `pause_total_ns` before/after; check the `bytes` value in
`inspect.heap()`.

### P9. Stream map entries; do not materialise them

**Change.** Replace `map::entries() -> Vec<(u64, u64)>` with a typed
iterator that does not allocate. Update the four call sites
(`value::hash`, `value::eq`, `channel::admission_size`,
`value::display`) to consume the iterator.

In `value::hash`, also reuse a single `FxHasher` per top-level
`hash` call instead of creating one per entry
(`crates/lang_runtime/src/value.rs:175`–`value.rs:191`).

**Removes.** An `O(N)` Vec allocation per hash/eq/admission/display
call on maps/sets; for byte-budgeted channels carrying maps, an
`O(N)` clone of every sent map.

**Estimated impact.** Large for byte-budgeted channels; medium for
hash/eq on large maps.

### P10. Inline buffer storage; eliminate the BufData indirection **[invariant]**

**Change.** Replace `BufData { items: Vec<u64>, elem, frozen }`
indirection with inline storage on the heap object. Two options:

- **(a)** Allocate the buffer object large enough for the inline items
  (`HEADER + 16 + 8 * cap` bytes), copy in the elements, and keep
  cap/len/elem in the payload. The slab allocator (P1) already
  supports variable payload sizes up to its threshold.
- **(b)** For larger buffers, keep the indirection but stop storing the
  elem descriptor per buffer (already in the descriptor) and avoid the
  `Vec<u64>` round-trip in `admission_size` (which clones just to sum).

**Removes.** One `malloc` per buffer creation; one `Vec::clone` per
admission-size call.

**Estimated impact.** `bench/collections.lang` "buffer push" from 6
ns/op to ~3–4 ns/op; large byte-budget channels carrying buffers
become linear in N rather than N·M.

### P11. Decouple channel registry from lease/task cleanup

**Change.** Maintain leases by task, not by channel. Each task's
`Task` already carries `cleanups` and `roots`; add a `leases: Vec<u64>`
field. On `release_leases_of(id)` only walk the task's leases, not
every channel in `REGISTRY`.

Drop `release_leases_of` and the linear walk at
`crates/lang_runtime/src/channel.rs:741`.

**Removes.** `O(channels)` per task finish.

**Estimated impact.** Medium for long-running multi-channel servers
with many short-lived tasks.

### P12. Make channel pool unbounded or size-bounded

**Change.** Replace `if d.pool.len() < 4` (`channel.rs:772`) with a
size bound: keep recycling as long as the sum of `pool[i].capacity()`
across all entries is below a configurable cap (e.g. 1 MiB). Or share
a global pool keyed by capacity.

**Removes.** Fresh `Vec::new()` per sent batch after the first 4.

**Estimated impact.** `bench/messages.lang` "batch channel" from
~108 ns/message to ~70–80 ns/message.

### P13. Symbol intern: `Box<str>` keys + raw entry

**Change.** In `string::symbol`, replace `FxHashMap<String, u64>` with
`HashMap<Box<str>, u64>` (using `raw_entry` for the lookup so no key
allocation happens on the hit path) and remove `name.to_string()`
(`crates/lang_runtime/src/string.rs:70`).

**Removes.** One `String` allocation per new symbol.

**Estimated impact.** Medium for programs with many symbols; small
otherwise.

### P14. Header compression for very small objects **[invariant]

**Change.** Two-tier header:

- **Inline** (payload ≤ 8 bytes): a packed descriptor pointer in the
  payload itself (no separate `Header`); the descriptor is at a known
  offset inside a slab block.
- **Standard** (payload > 8 bytes): today's 16-byte `Header`.

The descriptor-table approach from P4 (typed `trace`/`finalize` per
descriptor) is a prerequisite; until then keep the layout.

**Removes.** 16-byte header on tiny records/inlineable payloads.

**Estimated impact.** Memory for tiny-record-heavy programs drops
~30–50%; perf largely unchanged.

### P15. Locality: group `Task` roots for faster sweeps

**Change.** Replace the per-task `Vec<u64>` roots
(`Task.roots`, `Task.entry_args`, `Task.cleanups`,
`Task.iter_closers`, `Task.on_deschedule`) with a single inline
buffer plus a spill Vec. Each `Task` then has one flat pointer slice
to walk during `for_each_root`. Pack `entry_args`/`cleanups`/`iter_closers`
from `(u64, X)` (16-byte aligned) to compact bitfields where X fits.

**Removes.** Multiple `Vec` traversals per collection; redundant
pointer chasing during `for_each_root`.

**Estimated impact.** Medium for collections dominated by `for_each_root`
cost.

### P16. Map/set equality avoids full materialisation

**Change.** `map::eq` walks both maps via `iter().all(...)`; today it
already does not materialise (`crates/lang_runtime/src/map.rs:137`),
but the `Key` struct hashes and compares via `crate::value::hash` /
`crate::value::eq`, which for complex key types allocate Vecs.
Specialise `eq` and `hash` for keys whose descriptor is a primitive
(`Int`, `Bool`, `Symbol`, `String`) so no recursion happens.

**Removes.** Recursion and per-entry Vec allocation in the common case.

**Estimated impact.** Medium for maps with primitive keys (most).

### P17. Object pool for `BufData` / `MapData` / `ListData` boxes **[invariant]**

**Change.** Each `*mut ListData`/`*mut BufData`/`*mut MapData` is a
`Box::new` allocated per object
(`crates/lang_runtime/src/list.rs:88`, `buffer.rs:45`, `map.rs:87`).
That is one extra `malloc` per object. Replace with an
arena/pool-per-type that hands out raw pointers from a chunk bitmap.
The chunk bitmap is reset at every collection (its backing store is
not itself traced; objects that need it carry a pointer that *is*
traced).

**Removes.** One `malloc` per list/buffer/map creation.

**Estimated impact.** Compounds with P1/P10; small on its own.

### P18. Decouple `value::display` allocation from value construction

**Change.** `value::display` for `Kind::Record`, `Kind::List`,
`Kind::Map`, `Kind::Set` allocates one `String` per element
(`crates/lang_runtime/src/value.rs:339`–`value.rs:391`). For deep
nesting this is O(N) allocations per `display` call. Use a single
`String` buffer with a writer trait (`fmt::Write`).

**Removes.** Per-element `String` allocation in `display`.

**Estimated impact.** Small for normal programs; large for
inspection/REPL output of large values.

### P19. Make `Roots::Drop` O(1)

**Change.** Replace `heap().temp_roots.truncate(self.0)` in
`gc.rs:244` with `heap().temp_roots.set_len(self.0)` (sound because
all dropped roots were allocated after the guard and are now out of
scope). Drop the guard without touching the Vec when no roots were
pushed.

**Removes.** O(N) memmove on every guard drop.

**Estimated impact.** Small but pervasive.

### P20. Pre-grow `Heap.objects` to amortise startup

**Change.** In `Heap::default`, reserve an initial capacity for
`objects`, `temp_roots`, `pinned`, `kept` based on a small heuristic
(proposed: 1024). The first collections touch fewer buckets.

**Removes.** First-N-allocations growth cost.

**Estimated impact.** Trivial but cheap.

---

## Out-of-scope (intentionally not pursued here)

The following would change language-visible contracts and are
**not** part of this plan:

- A moving/compact collector (would change pointer stability).
- Reference counting on every shared value (would change sharing cost).
- Inline records everywhere (would change descriptor dispatch).
- Replacing `imbl` HAMT with a different map implementation (broader
  perf/correctness study).

These are tracked as future work in
[ARCHITECTURE.md](../ARCHITECTURE.md) and [DESIGN.md](../DESIGN.md).

---

## Verification matrix

| Item | Benchmark | Static check | Stress |
|---|---|---|---|
| P1  | heap, records | `cargo build` | `LANG_GC_VERIFY=1 LANG_GC_STRESS=1 bench/heap.lang` |
| P2  | heap         | `cargo build` | inspection JSON shows bounded `objects` |
| P3  | heap         | `cargo build` | none |
| P4  | collections, heap | `cargo build` | `LANG_GC_STRESS=1 bench/collections.lang` |
| P5  | collections  | `cargo build` | none |
| P6  | collections  | `cargo build` | none |
| P7  | collections  | `cargo build` | persistent list tests |
| P8  | collections  | `cargo build` | none |
| P9  | collections, messages | `cargo build` | `LANG_GC_VERIFY=1` |
| P10 | collections  | `cargo build` | buffer tests |
| P11 | heap         | `cargo build` | scope cleanup tests |
| P12 | messages     | `cargo build` | none |
| P13 | compile-time | `cargo build` | symbol-heavy test |
| P14 | heap         | `cargo build` | `LANG_GC_VERIFY=1` |
| P15 | heap         | `cargo build` | `LANG_GC_STRESS=1` |
| P16 | collections  | `cargo build` | map eq/hash tests |
| P17 | heap         | `cargo build` | `LANG_GC_VERIFY=1` |
| P18 | inspect      | `cargo build` | none |
| P19 | collections  | `cargo build` | none |
| P20 | heap         | `cargo build` | none |

For each item, capture three runs of the existing baseline
(`reports/2026-10-15-optimization-baseline.json` is the reference) and
publish a follow-up JSON alongside it.

---

## Summary of expected wins

| Workload | Today | After P1–P7, P9, P10, P12 | Expected change |
|---|---|---|---|
| `bench/heap.lang` record allocation | ~109 ns/record | ~55–70 ns/record | -35% to -50% |
| `bench/heap.lang` heap bytes after run | ~8.9 MB | ~3–4 MB | -55% to -65% |
| `bench/collections.lang` paged list append | 11 ns/op | ~7–8 ns/op | -25% to -35% |
| `bench/collections.lang` persistent map insert | 2.5 µs/op | ~2.0 µs/op | -15% to -20% |
| `bench/messages.lang` batch channel | 108 ns/msg | ~70–80 ns/msg | -25% to -35% |
| `bench/records.lang` closed record fields | 1.1 ns/op | unchanged | 0% |
| `bench/runtime.lang` call, 4 rooted pointers | 7.9 ns/call | ~7.0 ns/call | -10% |
| Pause time under `LANG_GC_STRESS=1` | up to ~10 ms | ~3–5 ms | -50% to -70% |

These are estimates; actual numbers come from the verification matrix
above. Each change is independently reverted and the suite is run
between changes to ensure no regression.