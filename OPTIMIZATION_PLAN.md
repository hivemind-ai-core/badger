# Compiler and runtime optimization plan

Reviewed 2026-10-15 against `c5c7b484c37bbc6dab46b23471bce80626ff01b1`
and the working tree present during review. This is a plan, not an implementation
or a claim of achieved speedups. Existing uncommitted changes to `README.md`,
`gc.rs`, and `task.rs`, plus the untracked use document and stress cases, were
left intact. Runtime observations include those working-tree changes.

## Recommended direction

Start with compiler phase measurements, reusable frontend work, GC scratch
allocation, compact list pages, and constant-time channel admission. Then reduce
runtime crossings and redundant GC roots in generated code. Revisit allocator
and scheduler architecture only after the smaller changes have been measured.

The implementation already has substantial optimizations: concrete record
layouts, demand-driven monomorphization with instance deduplication, scalar
replacement of local aggregates, self tail-call elimination, demand-generated
closure adapters, inline scheduling-budget checks, shared persistent map tries,
transfer of list-builder storage on finish, reusable task stacks, and binary
MIR deployment. Extend these mechanisms instead of replacing them wholesale.

## Evidence and starting measurements

Built the current toolchain with `cargo build --release --offline`. Ran each
case in a fresh process, with one warmup and three measured repetitions,
sequentially, removing inherited `LANG_*` variables. The machine was Linux
x86_64, AMD Ryzen 7 8745HS, Rust 1.98.1. Filesystem caches were warm; CPU affinity
and frequency were uncontrolled. These are directional measurements, not
publication-quality comparisons or phase profiles.

All cases completed successfully. Full commands, individual outputs, timings,
environment metadata, and the starting dirty-tree listing are retained in
[the baseline JSON](reports/2026-10-15-optimization-baseline.json).

| Case | Workers | Compilation/startup median, ms¹ | Execution median, ms¹ |
| --- | ---: | ---: | ---: |
| Minimal source program | 1 | 138.451 | 0.079 |
| Same program from binary MIR | 1 | 0.393 | 0.073 |
| `bench/records.lang` | 1 | 145.796 | 129.111 |
| `bench/collections.lang` | 1 | 143.193 | 606.379 |
| `bench/collections.lang` | 4 | 144.030 | 703.043 |
| `bench/messages.lang` | 1 | 154.145 | 44.005 |
| `bench/messages.lang` | 4 | 153.553 | 156.145 |
| `bench/parallel.lang` | 1 | 143.611 | 709.773 |
| `bench/parallel.lang` | 4 | 144.705 | 183.087 |
| `bench/runtime.lang` | 1 | 169.459 | 60.295 |
| `bench/heap.lang` | 1 | 153.176 | 574.185 |

¹ Existing `--bench` boundaries in `lang_driver::run_entry`: compilation includes
startup work before entry execution; binary MIR still needs loading and JIT
compilation. Whole-process time is recorded separately in the JSON. The minimal
source is `pub fn main() -> Int { 0 }`; its check-only process median was
138.475 ms. The source/MIR difference points to work bypassed by saved MIR,
but does not identify parsing versus checking versus MIR lowering costs.

Four workers deliver about 3.88× speedup on the CPU workload, while messaging
takes about 3.55× as long. The collections workload has no explicit parallel
tasks, so its four-worker result measures the cost of enabling that mode,
not collection parallel speedup. Lock contention and runtime transitions are
plausible explanations to measure, not a proven attribution from these totals.

Benchmark interpretation needs two repairs before performance claims:

- The `records.lang` label `record update (allocates)` is misleading: a separate
  `--dump-mir` run confirmed scalar replacement removes the update allocation
  from that loop. Add separate escaping and nonescaping cases with allocation
  counters; retain the useful scalar-replacement case.
- `items.chunks()` currently constructs and copies arrays for all pages before
  traversal. Its timing includes that preparation. The rooted-call benchmark
  also makes an extra scalar call; it is not an isolated root-store measurement.
  Split construction/traversal and matched-call-shape cases.

## Priorities

P1 means begin after instrumentation; P2 follows the relevant P1 work; P3 requires
profile evidence. Effort is relative: S is a local change, M spans a subsystem,
and L crosses compiler/runtime boundaries. Benefits below are hypotheses unless
the evidence column describes a directly visible allocation or algorithm.

| ID | Priority / effort | Opportunity | Evidence and intended benefit |
| --- | --- | --- | --- |
| C1 | P1 / M | Reuse frontend work and verified build inputs | Every load reparses the entire embedded standard library; repeated entries repeat the pipeline. Reduce source startup and check/reload latency. |
| C2 | P2 / M | Dependency-driven analysis and cheaper type operations | Effect propagation and constraint solving rescan work; types are structurally hashed. Reduce analysis cost on larger programs. |
| C3 | P2 / L | Reuse concrete checking and unchanged code | Every new build rechecks reachable instances and compiles its MIR functions. Improve edit/reload time and long-session memory growth. |
| C4 | P1 / M–L | Reduce root frames and stores | Every function links a frame and every pointer assignment writes a root slot. Improve call cost and shorten accidental retention. |
| C5 | P2 / L | Bounded inlining and aggregate/iterator elimination | Existing scalar replacement cannot see through ordinary calls. Remove wrappers and temporary aggregates where proven safe. |
| R1 | P1 / M | Reduce collector scratch work; account for external storage | Tracing buffers children per object; sweep allocates a survivor vector; external pressure is incomplete. Reduce pauses and improve memory visibility. |
| R2 | P1 / S–M | Compact pages and avoid eager collection copies | Narrow list pages reserve excess bytes; chunks copy; persistent append clones the directory. Reduce memory and traversal preparation. |
| R3 | P1 / M | Constant-time channel occupancy and fewer admission copies | Batch occupancy scans batches/producers; byte admission materializes collections. Improve messaging under load. |
| R4 | P2 / L | Inline proven immutable reads | Multi-worker code locks around every generated runtime call. Reduce transitions without weakening GC synchronization. |
| R5 | P2 / M | Direct codec conversion and cached descriptor facts | `Data` decoding serializes and reparses JSON; equality repeatedly walks type descriptors. Reduce service allocations and repeated metadata work. |
| R6 | P2 / M | Bound transport work and shorten blocking runtime sections | HTTP spawns a thread before request admission; output writes/flushes run synchronously. Improve overload behavior and runtime responsiveness. |
| R7 | P3 / L | Pool small managed allocations; reconsider scheduling only if needed | Each managed allocation uses the system zeroing allocator. Reduce allocation cost after object-count reductions land. |

## Compiler work

### C1 — Share frontend work with explicit invalidation

Evidence: [driver](crates/lang_driver/src/lib.rs), `Session::load`, `parse`,
`check`, `check_one`, and `lower_to_mir`; [stdlib](crates/lang_hir/src/stdlib.rs);
[dependency verification](crates/lang_driver/src/deps.rs), `content_hash`.

`load` adds and parses every `STD_MODULES` entry for every compilation. Project
checking reloads and rechecks the main entry and each named entry/task. The
parser also clones the full source string before the layout pass makes another
buffer. Dependency verification reads and retains file contents before hashing.

1. Record loading, dependency verification, layout, parsing, HIR lowering,
   checking, MIR lowering, code generation, and finalization separately.
2. Remove the avoidable source clone through disjoint source/interner borrows.
   Reuse loaded sources and standard ASTs across entries within a session.
3. Introduce a reusable compiler context for watch/REPL builds. Cache parsed
   modules by content plus parser/toolchain identity. Rebind spans, `FileId`s,
   and interned symbols correctly: cached ASTs are not portable across today's
   independent source maps and interners without remapping.
4. Only then cache reusable checked library results. User protocol implementations
   affect standard-library checking and effects, so a checked stdlib cannot simply
   be frozen independently of the program's implementation environment.
5. Stream dependency hashes in sorted path order instead of retaining all file
   bodies. Preserve exact lockfile verification; modification times alone are
   not a sufficient cache key.

Prove: multi-entry projects retain identical diagnostics and dependency-change
rejections; repeated builds hit the intended cache; edited stdlib/imports/impls
invalidate it. Measure both a minimal CLI and large multi-module projects.
Keep binary MIR as the existing low-startup deployment path.

### C2 — Make analysis work proportional to changed facts

Evidence: [type checking](crates/lang_typeck/src/program.rs), `effect_fixpoint`;
[solver](crates/lang_typeck/src/solve.rs), `solve` and implementation search;
[types](crates/lang_typeck/src/ty.rs), `Ty = Rc<TyKind>`;
[region summaries](crates/lang_typeck/src/regions.rs), `store_summaries`.

Effect propagation repeatedly scans all call edges and clones label vectors.
Use indexed effect sets and a reverse-call worklist, or SCC summaries. Preserve
protocol/dynamic-call edges and callback effect propagation. The solver likewise
revisits pending constraints; instrument retries before introducing variable
dependency queues. Associated projections, defaults, and unification rollback
must still wake every affected obligation.

Implementation search already indexes by type head. Refine candidates by protocol
and head before cloning candidate definitions. Cache only ground, stable queries
with the implementation/givens environment included. A result involving mutable
inference variables must not survive a snapshot rollback or later refinement.

`Rc` cloning itself is cheap; structural hashing/equality, substitutions, and
reconstruction of type trees are the more relevant candidates. Profile these
before introducing canonical IDs for ground types or memoized transformations.
Do not replace all type representation as a prerequisite.

Prove: equivalent results and diagnostics for effect cycles, overlapping impls,
rows, associated types, generics, ownership, and region escapes. Add scaling
fixtures for long call chains, wide records, and many protocol implementations;
report constraints revisited and edges processed as well as elapsed time.

### C3 — Reuse instance results and compile changed functions

Evidence: [monomorphization](crates/lang_mir/src/lower.rs), `Mono::instance`,
`lower_instance`, and `run`; [concrete checking](crates/lang_typeck/src/program.rs),
`check_instance`; [JIT](crates/lang_codegen/src/lib.rs), `compile`;
[reload](crates/lang_driver/src/live.rs), `reload`.

The current monomorphizer already deduplicates `(DefId, type arguments)` within a
build. The opportunity is reuse across builds, and substitution of previously
checked bodies where equivalent to today's concrete recheck. Instrument counts
and time per definition/instance first; preserve `check_instance` as the fallback
for resolution, coercion, or capability obligations requiring concrete types.

Cache keys need stable definition identity, body content, concrete types,
referenced schemes/impls, compiler/runtime ABI, target, optimization mode, and
live-dispatch/worker assumptions. Raw arena IDs and runtime addresses cannot be
used across sessions. Reuse the relocation discipline in
[binary MIR serialization](crates/lang_mir/src/serial.rs) for persisted artifacts;
strengthen identity beyond package version and format number if adding automatic
caches across different builds of the same version.

Reload currently creates a fresh session and JIT image on the scheduler thread,
then retains accepted images with `mem::forget` so old frames and closures stay
valid. First reuse unchanged function code and metadata. Moving compilation off
the scheduler requires separating compiler-side descriptor/constants construction
from the global runtime heap and installing a completed version at a safe point.
Code reclamation requires explicit frame, closure, global, and descriptor-hook
lifetime tracking; it is a separate later change.

Prove: changed function bodies replace correctly, unchanged bodies reuse code,
incompatible types/effects remain rejected, old closures/frames remain usable,
and global initializers keep their established state. Measure reload pause and
RSS/code bytes over hundreds of edits, alongside source startup.

### C4 — Emit roots for live pointers, with a simple first slice

Evidence: [code generation](crates/lang_codegen/src/lib.rs), `FnCx::define`,
`Body::set`, and return lowering. Every function allocates and links a shadow
frame, even if no actual pointer needs rooting; every pointer local gets a slot
for its entire invocation and every assignment stores to it. The closure
environment is also pointer-typed, even for direct calls passing null.

Begin by omitting provably empty frames and roots that are always null. Next
compute pointer liveness across every possible collection/suspension point,
reuse slots for disjoint lifetimes, and clear dead slots so they stop retaining
objects. Only then consider sinking stores to safe points instead of emitting
them on every assignment. A caller's pointers must remain visible when a callee
allocates, parks, invokes a callback, or another worker collects.

Prove: a large temporary becomes collectible before its containing function
returns; allocation-heavy callees preserve caller values; closure environments,
returned pointers awaiting the lock, cancellation, and parked frames remain
rooted. Compare matched scalar/pointer call cases and CLIF root-store counts.
Retain stack-overflow checks independently of shadow-frame elimination.

### C5 — Inline small known callees, then repeat existing simplification

Evidence: [MIR passes](crates/lang_mir/src/lower.rs), `FnLower::finish` and
`tail_calls`; [scalar replacement](crates/lang_mir/src/sra.rs);
[sequences](crates/lang_hir/std/seq.lang); [loop lowering](crates/lang_mir/src/lower.rs),
`lower_for`.

Add a release-mode, size-budgeted MIR inliner for small direct callees. Start with
nonrecursive scalar helpers and wrappers; follow with copy propagation, dead
temporary removal, and another scalar-replacement pass. This can expose local
record/tuple/enum construction across calls. Extend enum elimination conservatively
after records, keeping tags, control flow, and payload initialization correct.

Measure lazy `map/filter/take/fold` chains: their closures and `Option` values can
create per-element overhead. Fuse only recognized chains with known callees,
preserving left-to-right evaluation, callback effects, affine consumption, early
termination, and exactly-once iterator closing. Unknown callbacks retain the
ordinary path. Disable or constrain inlining through live dispatch slots so
edits still affect subsequent calls.

Do not remove checked arithmetic, bounds checks, or polls just because a loop is
hot. Any elimination needs a range/control-flow proof; polling still bounds
cancellation and stop-the-world rendezvous latency. Track compilation time and
native code size so a runtime win does not create excessive specialization.

## Runtime work

### R1 — Remove collector scratch allocations and expose real memory pressure

Evidence: [collector](crates/lang_runtime/src/gc.rs), `alloc`, `collect_inner`,
`add_pressure`, and `stats`; [maps](crates/lang_runtime/src/map.rs), `trace` and
`with_table`; [buffers](crates/lang_runtime/src/buffer.rs), `new`;
[heap inspection](crates/lang_runtime/src/inspect.rs), `heap_json`.

`collect_inner` creates a child vector for each traced object, then visits it to
mark children. Objects with children therefore allocate scratch storage during
the pause. Mark/enqueue through the visitor directly, or reuse scratch storage,
while preserving verification attribution. Reuse the mark-stack capacity and
compact/swap the object registry without allocating a fresh full survivor vector
on every collection. Scalar-key/scalar-value maps can return from tracing before
iterating the entire trie.

The collector counts managed payloads, and map creation adds an approximate
512-byte pressure charge. Buffer capacities, list directories, and channel
queues can grow outside those payloads without corresponding accounting. A
collection resets `allocated` to managed live bytes, losing those pressure
estimates. Separate managed live bytes, external owned capacity, allocation
pressure since collection, and process RSS; avoid double-counting shared tries.
Use measured estimates explicitly where exact shared-node ownership is costly.

Prove: identical reachability/finalization with verification enabled, lower
scratch allocation counts, bounded external-storage growth, and improved pause
distributions at the same retained working set. Report rendezvous wait separately
from tracing and sweeping; do not hide a pause increase by changing thresholds.

### R2 — Fix compact storage before changing collection structures

Evidence: [lists](crates/lang_runtime/src/list.rs), `new`, `push`, `append`, and
`finish`; [intrinsics](crates/lang_runtime/src/intrinsics_std.rs), `list_chunks`,
`list_concat`, and `list_slice`; [buffers](crates/lang_runtime/src/buffer.rs).

List capacity is derived from `4096 / element_size`, but page allocation uses
`capacity * element_size.max(8)`. Stores use the actual representation width.
Thus I8, I16, and I32 pages reserve respectively 8×, 4×, and 2× the needed element
storage. Allocate `PAGE_HDR + capacity * representation_size` with appropriate
alignment; cover both new pages and persistent-tail copies. Buffer/array elements
use `Vec<u64>`, so narrow arrays have a separate space tradeoff: measure before
changing that ABI.

`chunks` eagerly copies page contents to frozen buffers. Preserve its current
return contract, but add an internal page cursor for generated loops; consider
a separate public borrowed page view only with explicit lifetime semantics.
Bulk-copy eligible scalar ranges for concat/slice and avoid repeated page lookup.
For pointer pages preserve tracing, source roots, and logical lengths of shared
tails.

Persistent append clones the whole page directory: repeated immutable append is
O(number of pages) per append, even though element storage is shared. Favor
existing affine builders in generated/library bulk construction. Compare a
persistent directory or chunked directory only on large append/branch workloads;
never mutate an older version without a valid uniqueness proof.

Prove: exact values for every scalar width, stable element references, old
versions unchanged after branching append, builder consumption, and GC of shared
pointer pages. Measure bytes per element and peak RSS as well as ns/element.

### R3 — Make channel bookkeeping constant-time

Evidence: [channels](crates/lang_runtime/src/channel.rs), `ChanData::count`,
`append`, `publish`, `recv_batch`, `release`, `admission_size`, and `uncharge`.

`count` sums every queued batch and producer staging vector on admission. Maintain
explicit queued/staged/leased counts, updating them at each transition. Validate
cached counters against the old recount in debug builds before removing scans
from the hot path.

Byte admission builds temporary map-entry vectors and clones buffer elements;
release recomputes message sizes. Traverse without materializing collections
where safe, and carry accepted size metadata with queued/staged/leased messages.
Cached sizes must follow the channel's existing admission semantics, especially
for transferable payloads whose interior state may change. Preserve recycling
of batch buffers; it already exists.

Benchmark many producers, many small sealed batches, long-held leases, byte
limits, and both prompt/rendezvous modes. Prove conservation and exactly-once
selection under close, cancellation, timeout, and task exit. Capacity must include
staging, queues, and leases throughout; throughput alone is insufficient.

### R4 — Reduce runtime crossings before splitting the global lock

Evidence: [generated runtime calls](crates/lang_codegen/src/lib.rs), `rt_call`;
[runtime synchronization](crates/lang_runtime/src/task.rs), `lang_rt_enter`,
`lang_rt_leave`, and `stop_the_world`; [core collection ABI](crates/lang_runtime/src/abi.rs).

Every generated runtime call acquires the global runtime lock in multi-worker
mode. Even small list length/element reads incur the transition. Introduce an
explicit intrinsic contract describing allocation, suspension, callbacks, and
shared mutation; language effects alone do not establish these properties.

Start with generated reads of immutable, rooted data, such as frozen-array
length/element access and eligible list traversal. Keep the worker classified as
executing compiled code while it reads, so GC still waits for its next safe
point. Nonmoving memory still needs roots. List pages can share an extended
tail, so use the immutable list's logical length and avoid racing on mutable
tail metadata. Mutable buffers/cells and operations touching runtime registries
stay on synchronized paths until separately proven safe.

Measure transitions per element/message, lock wait/hold distributions, and
one/two/four/eight-worker throughput and latency. Only consider per-worker
allocation or separate scheduler/channel locks when remaining contention is
demonstrated. That redesign must first remove dependence on unsynchronized global
`UnsafeCell` state and process-wide temporary-root stacks.

### R5 — Avoid codec round trips and repeated descriptor traversal

Evidence: [codecs](crates/lang_runtime/src/codec.rs), `from_data`, `decode`, and
`dec`; [standard intrinsics](crates/lang_runtime/src/intrinsics_std.rs),
`error_details`; [value operations](crates/lang_runtime/src/value.rs),
`eq` and `may_hold_float`.

`from_data` encodes `Data` to JSON text, parses it into `serde_json::Value`, then
decodes it. Implement direct descriptor-guided `Data` conversion and error
details construction. Preserve wire aliases/names, numeric range checks, nesting
behavior, and precise error paths. Streaming JSON decoding is a later option
if temporary JSON trees still dominate realistic service profiles.

Cache descriptor facts such as pointer offsets and possible floating payloads.
Same-object record/enum equality currently recursively computes `may_hold_float`
with a new visited vector. Compute stable facts after recursive descriptors are
complete, with conservative handling of cycles and dynamic values. Preserve NaN
partial equality and order-independent map/set hashing from the retained
adversarial regressions. Avoid materialized list/map vectors where serialization
can traverse directly; keep sorting where deterministic output requires it.

Prove: byte-for-byte encoding where required, identical decode successes/error
paths, arrays included, equal-key hash coherence, and NaN behavior. Benchmark
small and large nested service payloads with allocation counts and peak memory.

### R6 — Bound transport work and prevent blocking output from stalling tasks

Evidence: [HTTP](crates/lang_runtime/src/http.rs), accept loop, `handle_conn`,
`read_request`, and response/stream construction;
[output](crates/lang_runtime/src/intrinsics_io.rs), `write_out`;
[blocking pool](crates/lang_runtime/src/task.rs), `run_blocking`.

The accept loop creates an OS thread per connection; `max_in_flight` is checked
inside `handle_conn` after request parsing. It bounds admitted request handlers,
not the thread/read work before admission. Add an explicit bounded connection
budget before spawning/read work and a bounded executor, preserving 413/431/503
behavior, streaming backpressure, and graceful shutdown. A reactor is a larger
alternative only if the bounded executor is insufficient.

Output writes and stdout flushing happen synchronously inside the intrinsic;
multi-worker generated calls hold the runtime lock there. Route potentially
blocking writes through the existing blocking mechanism using owned or explicitly
rooted buffers, and batch writes while preserving visible flush/order semantics.
Likewise reduce response/chunk copies only where ownership remains valid across
the external thread boundary.

Prove: slow readers, slow stdout consumers, connection floods, cancellation during
streaming, bounded threads/RSS, and existing HTTP/SQLite service behavior. Report
tail latency and rejection rate alongside accepted request throughput.

### R7 — Change allocation/scheduling architecture only after profiling

Evidence: [allocation](crates/lang_runtime/src/gc.rs), `alloc`/sweep;
[task stacks and queues](crates/lang_runtime/src/task.rs), `STACK_POOL`,
`make_coroutine`, and `worker_loop`.

Each managed object uses `alloc_zeroed` and is independently freed. If this still
dominates after C4/C5/R1/R2, prototype size-class slabs/free lists while keeping
the nonmoving object contract. Zeroing cannot simply be removed: partially
initialized objects may be traced, and verification relies on poisoned/quarantined
freed objects. Compare throughput, fragmentation, retained capacity, and RSS.

Task stacks already have a bounded reuse pool. Measure stack virtual/resident
bytes, first-touch faults, high-water usage, and completed task retention before
changing stack size or pooling task metadata. Consider per-worker ready queues
only if central-queue contention remains significant after R3/R4; preserve
controlled-test scheduling, migration, wakeups, timers, and cancellation fairness.
Generational/moving GC and broad scheduler rewrites are outside the initial plan.

## Delivery sequence and acceptance proof

Each slice should leave the existing compiler and examples runnable. Split local
changes into separate reviewable commits; do not combine allocator, GC protocol,
and scheduler changes in one step.

| Slice | Depends on | Deliverable and demonstration |
| --- | --- | --- |
| 0. Measurement | None | Phase timings, allocation/root/transition counters, repeatable benchmark runner, matched-call and escaping-record cases, and small-width/many-producer fixtures. Publish raw samples and noise estimates. |
| 1. Local allocation fixes | 0 | C1 source-copy removal, R1 scratch reuse/scalar-map tracing, R2 narrow pages, each measured independently. Demonstrate reduced counted allocations/bytes without changed output. |
| 2. Frontend reuse | 0, C1 copy cleanup | Shared parsed stdlib/modules across project entries and watch/REPL sessions; content-based invalidation demonstrated on imports and impl changes. |
| 3. Analysis efficiency | 0, phase profile | C2 effect worklist and targeted solver/type improvements. Demonstrate lower work counts on scaling fixtures and identical acceptance/rejection outcomes. |
| 4. Channel admission | 0 | R3 counters, then admission-copy/size-metadata changes. Demonstrate conservation and bounded occupancy under adversarial schedules. |
| 5. Roots and immutable reads | 0, 1 | C4 empty-frame/root cleanup, then liveness; R4 eligible read lowering separately. Demonstrate fewer root stores/transitions and successful GC/scheduling stress. |
| 6. Cross-call optimization | 5, code-size baseline | C5 bounded inlining and repeat simplification; iterator specialization only after measured benefit. Demonstrate fewer allocations while keeping evaluation/close order. |
| 7. Service paths | 0, 1 | R5 direct conversions and descriptor facts; R6 bounded transport/output as independent changes. Demonstrate lower payload allocations and bounded overload resources. |
| 8. Incremental code | 2, 3 | C3 stable instance/MIR/code reuse; demonstrate short reload pauses and controlled code/RSS growth while old closures run. |
| 9. Architecture decision | Profiles after relevant slices | Accept or reject R7 slabs and further lock/queue changes based on remaining costs; record the decision and measured tradeoffs. |

For every optimization, compare fresh source compilation, binary-MIR startup,
steady execution, peak RSS, allocation counts, and relevant latency distributions.
Use one warmup and at least ten recorded repetitions for short benchmarks,
interleave baseline/candidate runs, and retain raw samples. Keep cold filesystem
tests separate. Run compiler latency tests on small, large, generic-heavy, and
multi-entry projects; test release and development JIT modes separately.

An initial acceptance policy is a repeatable improvement beyond measured noise
on the targeted workload, with no unexplained regression above 5% on the control
suite. This is a proposed review threshold, not an achieved result or speedup
promise. A memory reduction may justify a small explicit time tradeoff. A change
that improves only a synthetic case should remain experimental until exercised
by a representative application.

Correctness checks should follow the subsystem changed: `cargo test --workspace
--offline` before integration; retained adversarial tests; service tests for
codec/HTTP/I/O changes; and focused GC/root/channel tests with
`LANG_GC_STRESS=1`, `LANG_GC_VERIFY=1`, and several `LANG_CHAOS` seeds on one and
four workers. Use bounded fixtures/timeouts for stress, separately from timings.
The current untracked `tests/stress/` inputs are useful candidates but need an
explicit runner and assertions before being treated as established CI coverage.

## Reproduction and review limits

Representative commands from the baseline (all exact invocations are in JSON):

```sh
cargo build --release --offline
target/release/lang run --release --bench --workers 1 bench/collections.lang
target/release/lang run --release --bench --workers 4 bench/collections.lang
target/release/lang run --release --bench --workers 1 bench/messages.lang
target/release/lang run --release --bench --workers 4 bench/messages.lang
target/release/lang run --release --bench --workers 1 bench/parallel.lang
target/release/lang run --release --bench --workers 4 bench/parallel.lang
target/release/lang run --release --dump-mir bench/records.lang
```

The minimal source and cache-build command are recorded in JSON; regenerate its
temporary paths before rerunning that case. Repeat the six existing benchmarks
(`records`, `collections`, `messages`, `heap`, `runtime`, `parallel`) under the
same worker counts and environment to compare with the retained baseline.

This review inspected the implementation and ran the release build, benchmark
cases, minimal check/source/cache cases, and the separate MIR inspection. It did
not run the full workspace suite, GC stress, service load tests, heap profiling,
hardware-counter sampling, or latency-distribution measurements. No compiler or
runtime code was changed. Priorities beyond the visible redundant work should
be revised when slice 0 supplies phase and contention profiles.
