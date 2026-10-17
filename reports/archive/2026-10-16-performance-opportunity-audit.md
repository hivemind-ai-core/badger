# Performance opportunity audit

Audited revision: `ef8d846a3515bd7c8d5bbe46a5e3602bb051c013`.
Worktree: `/tmp/language-performance-audit-2026-10-16`, branch
`audit/performance-2026-10-16`. The main checkout's dirty changes were excluded.
The measurements below were captured before implementation changes. A subsequent
user-requested nullable-pointer optimization is recorded separately in
[the implementation note](performance-audit-2026-10-16/option-niche.md).
The audit evidence remains the pre-change baseline.

During the audit, another session advanced main to `d0232b3` with dotenv/config
changes. The audited MIR, codegen, GC, buffers, lists, text/sequence and native
boundary sources were unchanged by that commit. Some pre-existing dirty files
also changed externally, so their initial hashes no longer match. Work here
remained confined to the isolated worktree; no concurrent changes were reverted.

The best next steps are to account for buffer storage, remove full-string work
from tiny text slices, extend MIR scalar replacement to tagged values, and
finish root-slot compaction/lifetime management. Sequence fusion and direct
array reads follow those foundations. Retention policies deserve explicit
attention alongside throughput: the current collector can report little new
allocation while retaining considerable memory.

## Evidence and limits

The release toolchain was built offline in this worktree. Evidence includes
post-pass MIR, Cranelift's final machine listing, checksum-checked microprobes,
direct runtime retention probes, and the application-shaped telemetry workload.
`LANG_DUMP_CLIF` is before Cranelift optimization; it was not used to claim a
cost survives machine-code generation.

- [Microbenchmark measurements and hashes](performance-audit-2026-10-16/evidence/measurements.json)
- [MIR/root/code-size counts](performance-audit-2026-10-16/evidence/mir-metrics.txt)
- [Source release MIR and machine listing, one worker](performance-audit-2026-10-16/evidence/source-w1.txt)
- [Source release MIR and machine listing, four workers](performance-audit-2026-10-16/evidence/source-w4.txt)
- [Telemetry summary](performance-audit-2026-10-16/evidence/telemetry.md)
- [Telemetry raw measurements](performance-audit-2026-10-16/evidence/telemetry.json.gz)
- [Forced-GC/verification/chaos results](performance-audit-2026-10-16/evidence/stress-validation.json)

Microbenchmarks use five fresh processes after warmup, with shuffled
source/no-inlining/saved-MIR cases on one and four workers. Text cases use five
processes after warmup. Inherited `LANG_*` settings are removed. The final
microbenchmark capture ran separately from builds and telemetry. Telemetry uses
16,384 events, fixed eight parsers, one warmup and three measurements for every
profile on 1/2/4/8 workers, with an independent oracle. Its lock-statistics
diagnostics are separate from timed runs. The machine is a Ryzen 7 8745HS with
16 logical CPUs; CPU frequency and unrelated host activity were uncontrolled.

These are directional local measurements, not an optimization A/B: no proposed
production change was implemented. A plain loop establishes an opportunity's
scale, not the speedup an implementation will achieve. Runtime-only probes
isolate representation/retention and do not establish multi-worker safety or
whole-program memory usage. Raw ranges are retained; sub-millisecond cases
are particularly sensitive to noise.

## What is already optimized

The current source pipeline is monomorphization/lowering with local scalar
replacement, then release MIR inlining and another scalar replacement, then
channel receive splitting and verification, then the MIR dump, then Cranelift
with `opt_level=speed`. See [driver lines 952–969](../crates/lang_driver/src/lib.rs),
[inliner](../crates/lang_mir/src/inline.rs), and
[codegen](../crates/lang_codegen/src/lib.rs).

Small helper calls, immediately known closures, small scalar record equality,
list indexing, and page-aware list traversal already have optimizations.
Root liveness already selects which pointer locals need slots. The runtime
already has per-worker allocation/debt, small-object pools, generational
marking, in-place/parallel sweep, CHAMP maps, per-channel synchronization, and
unlocked runtime entries. Recommending these as new work would repeat landed
changes.

`lang compile` explicitly sets `optimize: true` in the CLI and saves the
optimized MIR. Source and saved-MIR controls both remove the helper aggregate.
Loading saved MIR does not rerun the source passes, but the standard compile
path already ran them. This investigation rejected an initial hypothesis that
ordinary saved artifacts missed inlining.

## Priorities

P1 means a strong next target; P2 means follow after measurement or prerequisite
work. S/M/L describe relative implementation effort, not elapsed estimates.

| ID | Priority / effort | Opportunity | Evidence |
|---|---|---|---|
| A1 | P1 / M | Account for external buffer storage and expose reserved/live/allocated bytes separately | 64 MB backing storage contributes only 24 bytes of GC pressure |
| A2 | P1 / S–M | Stop character slicing after the requested endpoint | One-character slices scale with the entire input |
| A3 | P1 / M–L | Tag-aware scalar replacement plus constant/CFG cleanup before the MIR dump | A local `Option.some` still allocates once per iteration |
| A4 | P1 / M–L | Clear/reuse root slots at proven deaths | Root selection is liveness-aware; root contents are not lifetime-scoped |
| A5 | P1–P2 / L | Specialize/fuse nonescaping sequence pipelines | Nested iterator closures, cells and boxed results remain |
| A6 | P2 / M–L | Inline typed array reads and pack narrow array elements | Generic `get` boxes results; `Byte` arrays use word elements |
| A7 | P2 / L | Bound hidden list-version retention | A 512-element prefix retains 2,048 strings after full collection |
| A8 | P2 / M | Add an old-generation reclamation policy for phase changes | Dead promoted objects survive five subsequent minor collections |
| A9 | P2 / L | Reclaim wholly free pool pages and bound retained metadata | Zero managed objects still leave the pool's RSS high |
| A10 | P2 / M | Prune functions and compact MIR after inlining | 31 candidate unreferenced function bodies still compile |
| A11 | P2 / M–L | Prove and remove selected redundant checks | Final helper code retains a multiplication-overflow branch after `% 100` |
| A12 | P2 / S–M | Stream collection/codec work instead of materializing it | Temporary vectors, tuples, copied chunks and JSON round trips |
| A13 | P2 / M–L | Separate GC rendezvous/mark/sweep timings and reuse sweep workers | Millisecond pauses; each parallel sweep creates OS threads |
| A14 | P2 / M–L | Reduce native wire overhead; cache compiler work for reloads | Owned wire trees and full rebuilds remain service/edit latency costs |

### A1 — Buffer memory accounting is the first memory fix

[buffer.rs lines 8–12 and 43–47](../crates/lang_runtime/src/buffer.rs) store
every element in `Vec<u64>` behind a boxed `BufData`, with only an eight-byte
managed pointer payload. Capacity reservation and later `buffer_push` growth
do not call `gc::add_pressure`; that function currently has no callers.

The direct `buffer` probe touches eight million `U8` elements. It creates one
managed object, records **24 bytes** of pressure, and increases RSS by about
61 MiB. The vector's element storage alone is 64,000,000 bytes; the box and
allocator overhead are additional. Consequently GC scheduling and managed-byte
statistics do not reflect this storage. Heap-budget accounting also needs to
include it; the direct probe does not exercise task-level budget enforcement.

Charge capacity deltas and releases, including adopted vectors, while avoiding
one shared atomic update per pushed element. Track external *live* bytes as
well as allocation pressure; merely increasing the latter cannot make full
collection/budget accounting correct. Audit channel queues, native result
buffers, task stacks, JIT images and descriptor storage separately.

Acceptance: equal-sized `Bytes`, `Array<Byte>` and `Array<U64>` workloads show
their actual managed/external capacities; growth, freeze, transfer and finalizer
paths neither lose nor double-count charges. Publish budget, allocation,
reserved-cell and RSS metrics with distinct definitions.

### A2 — Tiny Unicode slices should not index the whole string

[text_slice, lines 432–444](../crates/lang_runtime/src/intrinsics_std.rs)
collects all `char_indices()` into a `Vec<(usize, char)>` before selecting a
slice. On this target that is roughly 16 bytes of scratch per Unicode scalar,
even for `slice(s, 0, 1)`.

Measured time for **100 one-character slices** of a repeated `é` string:

| Input characters | 1 worker, ms | 4 workers enabled, ms |
|---:|---:|---:|
| 1,000 | 0.217 | 0.225 |
| 10,000 | 1.903 | 2.026 |
| 100,000 | 18.143 | 18.352 |
| 1,000,000 | 182.577 | 176.210 |

Walk only until the requested end boundary, without materializing the index.
An ASCII fast path can follow. Keep character indexing, negative/reversed/
out-of-range checks, empty slices and the exact end-of-string boundary.
Repeated length queries still need a scan unless a separate cached scalar
count is justified.

`text.lines` also advertises a lazy `Seq` but first materializes a
`Vec<String>` and managed list. `split`, `join`, `concat`, case conversion and
replacement have extra Rust intermediates/copies. Handle those as separate
streaming/direct-output changes; avoid introducing small slices that retain an
unexpectedly large input buffer.

### A3 — Generalize allocation elimination to tagged values

In [sra.rs](../crates/lang_mir/src/sra.rs), `Tag` appears among uses that make
an allocation escape. Therefore `let x = Option.some(v); match x { ... }`
retains allocation, tag stores, tag reads and branching, even though `x` never
escapes. The release machine listing confirms the allocator call inside the
`options` loop. Its MIR has one allocation site; this site executes 200,000
times.

On one worker the boxed case takes **2.820 ms** (range 2.744–3.015) versus
**0.196 ms** (0.184–0.285) for the same scalar accumulation without the
container. This is an allocation opportunity, not a promised 14× improvement
for arbitrary `Option` code. Channel receive results already have a dedicated
split pass; it does not cover generic locally constructed enums or array/map
lookup results.

Introduce known-tag facts and payload scalarization, fold tag switches, remove
unreachable blocks, propagate copies/constants, then rerun aggregate
replacement and effect-aware dead-assignment cleanup. Run this **before the
MIR dump and saved-artifact encoding**, so the optimization is inspectable and
applies consistently. Start with single-definition nonescaping values; handle
control-flow merges before general unboxed enum returns.

Acceptance: this probe has zero enum allocation sites; escaped/stored/returned
enums preserve representation; pointer payloads survive allocation, suspension,
callbacks and multi-worker GC. Preserve checked failures and evaluation order.

### A4 — Root selection and root lifetime are separate problems

[roots.rs lines 120–169](../crates/lang_mir/src/roots.rs) computes a union of
locals needing protection at any safe point. `ToWord` conservatively requires
a pointer's slot for the call. [Body::set, lines 1510–1518](../crates/lang_codegen/src/lib.rs)
updates such slots on assignment; it does not clear them after their final
use. A pointer can therefore retain an object until the containing function
returns or that local is overwritten. Aliases can require several slots.

The combined probe's main function has **70 root slots**, despite significant
scalar replacement. The retained MIR also contains many always-null pointer
locals introduced by scalar replacement/inlining, but zero directly always-null
locals among the 70 selected roots in main. Root selection already excludes
those trivial cases here. Remove null roots where they remain, then add
death-point clears, alias-aware slot reuse and safe-point root
plans. Keep the stack-overflow check independent of shadow-frame omission.

Acceptance: publish root counts, root stores and retained objects after the
last source use, not only call timings. Do not clear a root while an untraced
word, derived page reference, closure capture, pending return or suspended
callee still relies on it. Exercise debug/live modes and runtime callbacks as
well as ordinary direct calls.

### A5 — Lazy sequence abstraction still has a large execution cost

[std.seq](../crates/lang_hir/std/seq.lang) represents adapters through nested
`start`/`next` closures and `Cell` state. Known-closure analysis in
[inline.rs lines 61–119](../crates/lang_mir/src/inline.rs) recognizes local
closure allocations/copies, rather than general closure values loaded through
adapter records and returned iterator factories.

`range → map → filter → fold` takes **7.289 ms** (7.190–7.880) on one worker
versus **0.223 ms** (0.201–0.321) for its checksum-equivalent direct loop.
Inlining the outer sequence helper into main still leaves **three closure
call sites in main**. Its separately retained body has nine allocation sites
and three closure calls; those are static sites, not allocation counts.

Use shape/closure facts across adapter fields and returned factories, with
tag-aware scalarization from A3. Consider an explicit MIR iterator operation
or a constrained loop-fusion pass if generic inlining cannot reliably recover
the loop. Do not simply raise the current 40/80-statement limits and two-round
budget: main already has 586 locals and 318 blocks in this small probe.

Acceptance includes scoped iterators: effects remain in iteration order and
resources close exactly once on exhaustion, `take`, `break`, return, failure
and cancellation. Keep escaped/reusable sequences working.

### A6 — Arrays need both a read fast path and a representation decision

`Array.get` reaches `buffer_get`, which reads a vector slot and allocates
`Option.some`; array building has specialized compiler paths, but this generic
read still crosses the runtime. The optimized `array_reads` body retains the
intrinsic. The probe takes **3.034 ms** for 200,000 indexed reads on one
worker. This timing includes the one-time array construction; it is not an
isolated access benchmark.

Add a typed checked read/split-result operation, then use range facts to remove
only proven redundant checks. Immutable array length/storage can be hoisted
where the ownership and GC contract permits. Preserve optional out-of-range
results and frozen-buffer defects.

Separately, `Vec<u64>` expands `Byte` by 8× and `U16` by 4× before metadata.
Packed storage can improve footprint and cache traffic, but requires changes
to native/codec conversion, tracing, indexing and representation dispatch.
List pages are already element-width-aware; use that existing distinction as
a guide. Measure narrow and pointer arrays independently.

### A7–A9 — Three independent reasons memory can stay high

**Shared list reachability.** [list tracing, lines 73–101 and 155–160](../crates/lang_runtime/src/list.rs)
follows a list's whole shared directory, and directories trace all claimed
pages. Retaining a 512-element prefix while extending it to 2,048 retains all
2,048 strings after a non-generational full collection. Dropping the prefix
then collects everything. This is real graph reachability, not a missed sweep.
Later directory copies and small-page growth affect the exact retention bound;
the probe does not imply every prefix retains an arbitrary entire descendant.
Investigate length-bounded views, persistent directory nodes, or copy policies.
Preserve persistent append complexity, concurrent claims and stable references.

**Old-generation policy.** [collect_inner, lines 542–551 and 700–715](../crates/lang_runtime/src/gc.rs)
requests major collection based on promoted size versus a growth threshold
(minimum 64 MiB), budget pressure or stress. After promoting 100,000 strings
and dropping all roots, five more ordinary collections leave all 100,000
objects in the default mode. With generational collection disabled, zero
remain. Consider an idle/elapsed-time/phase-change trigger or explicit full
collection facility. Balance memory reclamation against major-pause frequency;
minor collections deliberately do not reclaim old garbage.

**Pool reservation.** [Pool/alloc_cell/free_cell, lines 153–226](../crates/lang_runtime/src/gc.rs)
retain cells in size-class free lists. Chunk bases and per-chunk occupancy are
not retained for reclamation. In the 500,000-object probe, a full collection
reduces object count to zero while RSS remains roughly 35 MiB, versus roughly
2.4 MiB initially. This includes other retained allocator/vector capacities,
so do not attribute every byte to pool cells. Introduce reclaimable slabs or
whole-chunk accounting before attempting trimming; never advise away pages
containing free-list links still in use. Measure reuse performance and
worker-to-worker imbalance as well as RSS after quiet periods.

### A10–A11 — Finish MIR cleanup and exploit proven facts

[codegen::compile, lines 158–215](../crates/lang_codegen/src/lib.rs) declares
and compiles every function still in `Program.funcs`. After inlining, the probe
has 56 bodies; a conservative graph seeded by entries, tests, display hooks
and every static closure reaches 25. The other 31 account for **9,693 of
26,437 emitted function bytes**. Adapters, JIT allocation overhead and data
are excluded. These are pruning candidates, not a claim that 36.7% of the
complete executable can be removed. The analyzer follows direct/address
references; production pruning must verify every metadata/callback root and
remap all function IDs, static closure origins, hooks and serialized references.

Compact locals and unreachable CFG blocks after transforms, then compute roots.
Reduce repeated whole-function scans/clones in inlining and root analysis only
after recording pass costs on larger generated inputs. Prefer a worklist and
cached size facts to repeatedly scanning a growing caller.

The final `helpers` listing also retains a multiplication-overflow branch after
`(i % 100) * 2`. Its result is bounded even for signed remainders. Preserve the
running-total overflow check, whose safety is not proved for arbitrary `n`.
Add bounded integer/range facts for this sort of local proof, optional
read checks and loop induction. Cranelift already strength-reduces constant
remainder; replacing it wholesale is not justified. Signed minima, division
by zero, negative indexes and diagnostic locations need explicit coverage.

### A12 — Use existing visitors to avoid materialization

[map::entries](../crates/lang_runtime/src/map.rs) allocates a full entry vector
although `hamt::for_each` and managed cursors already exist. It is used not
only by public `entries`, but by map/set hashing, native encoding and other
runtime consumers. Public map entries additionally allocate tuple objects
and list pages. Stream internal visitors; retain the public materialized
result where the API promises one.

`list_chunks` copies each page into an external word vector and calls generic
`list::get` per element. `list_to_array` builds a temporary vector and then a
second destination vector; `buffer_to_list` clones its vector. Transfer owned
storage where valid and copy page spans directly where transfer is impossible.
Do not reinterpret the existing chunk benchmark as traversal-only timing.

[codec::from_data](../crates/lang_runtime/src/codec.rs) renders `Data` as JSON,
parses a `serde_json::Value`, then decodes the destination. `codec_to_data`
encodes and reparses in the opposite direction; record/config helpers do
related work. Introduce direct descriptor-driven conversion or decode from an
existing JSON tree. Preserve numeric precision, wire names, optional fields,
error paths and managed rooting. These are source-confirmed extra copies;
this audit did not measure an achieved codec speedup.

### A13 — Optimize latency with phase attribution

The collector has serial marking and root scanning. Parallel sweeping above
32,768 candidates creates scoped OS threads on each collection, partitions by
worker object list, joins them and runs finalizers on the collector thread.
See [gc.rs lines 648–652 and 800–846](../crates/lang_runtime/src/gc.rs).
Persistent helpers and balanced sweep chunks are candidates; creation/join
overhead versus useful work needs a matched A/B.

`task::stop_the_world` waits for cooperative runtime entry/budget exhaustion.
The inline poll decrements a 10,000-poll budget; it does not check the stop flag
on every loop iteration. Long unlocked runtime loops also execute between GC
safe points. Add rendezvous, roots, mark, sweep, finalizer and helper-thread
phase timings, young/old survivor counts and external bytes before choosing a
new collector. Consider periodic safepoints for long bulk operations with
their arguments/results correctly rooted. Tune for measured tail latency;
more frequent checks have a throughput cost.

Fresh telemetry throughput, thousands of events/s:

| Profile | 1 worker | 2 | 4 | 8 |
|---|---:|---:|---:|---:|
| balanced | 721 | 866 | 1,248 | 1,673 |
| backpressure | 636 | 768 | 1,096 | 1,434 |
| fanout | 258 | 331 | 390 | 340 |
| cpu | 310 | 459 | 762 | 828 |

Fanout loses about 13% from four to eight workers and reaches only 1.32×
one-worker throughput. Its median maximum GC pause is about 2.4 ms at eight
workers; balanced is about 1.8 ms. This is a reason to investigate, not proof
that either GC or a scheduler lock explains the whole scaling gap. The cache
actor, service work, allocation and wake topology all contribute. Histogram
p99.9 values have too few samples here for robust tail claims. Process CPU/RSS
include frontend/JIT work, whereas event throughput uses the pipeline timer.

### A14 — Native boundary and interactive compiler costs

The runtime no longer implements the old HTTP/fetch/SQLite transport paths;
those services use native modules. [native::call](../crates/lang_runtime/src/native.rs)
encodes declared values into owned wire bytes; the module
[dispatcher](../crates/lang_native/src/lib.rs) decodes an owned `Value` tree,
then returns another encoded outcome for managed reconstruction. Even inline
calls create call state and synchronize result delivery. Investigate borrowed
wire readers, capacity hints, batch APIs and bulk byte operations. Preserve
the useful boundary: managed pointers do not escape into asynchronous native
work, and cancellation/replay/handle lifetime remain correct. Benchmark scalar
inline calls separately from large bodies and database result sets.

For compiler latency, [Session::parse](../crates/lang_driver/src/lib.rs) still
clones source text; reachable standard modules are reparsed in new sessions.
[effect_fixpoint](../crates/lang_typeck/src/program.rs) repeatedly scans call
edges and clones label sets. [live::reload](../crates/lang_driver/src/live.rs)
builds a fresh session on the scheduler thread and intentionally retains JIT
images with `mem::forget`. Add reusable parsed inputs/worklist propagation and
off-thread rebuild publication before attempting a parser replacement. Retire
old code only with proof no active frame, closure, hook or global references
it. Cache invalidation must include sources, impl/effect environments, spans,
interning, ABI and optimization/live settings. No parser rewrite or code-cache
speedup was measured in this audit.

## Suggested implementation sequence and acceptance gates

1. **Memory visibility and text slice:** land external-storage accounting and
   explicit byte metrics; implement bounded Unicode slicing as a separate
   small change. Keep both runnable probes.
2. **MIR simplification:** known tags/copies, CFG cleanup, enum scalarization,
   another scalar replacement, and conservative reachability pruning. Compare
   MIR before dump, optimized code bytes, allocations and compile time.
3. **Root plan:** omit null slots, clear dead roots and then reuse slots.
   Demonstrate collection of dead large temporaries during a long-running
   function, with callback/word/pointer/suspension controls.
4. **Collections and sequences:** direct checked array reads and fusion of
   constrained sequence shapes; eliminate internal materialization. Keep
   escaped/side-effecting/scoped alternatives as correctness controls.
5. **GC policies:** measure age/phase changes and slab reuse, then add bounded
   reclamation and persistent sweep workers if the measurements justify them.
6. **Service/compiler work:** profile native marshalling and reload sessions
   on real service/edit workloads before changing their representations.

For each slice, compare immutable before/after release binaries with source
and executable hashes, raw ranges, cleared inherited settings and interleaved
runs. Report allocation counts, managed live/reserved/external bytes, RSS,
pause phases and 1/2/4/8-worker throughput. Include telemetry and long-lived
phase-change workloads alongside microbenchmarks. Compiler changes need debug,
release, saved-MIR and live-mode coverage; GC/root changes additionally need
forced collection, verification, cancellation, callbacks and chaos scheduling.

## Validation and reproduction

The release CLI and both audit-only Rust examples build offline. Both language
probes check and run with asserted checksums. Reduced microprobes pass with
`LANG_GC_STRESS=1`, `LANG_GC_VERIFY=1` and `LANG_CHAOS=20261016`, in debug and
optimized generated code on one/four workers. All 48 measured telemetry runs
pass the independent oracle; the runner's nine Python unit tests pass. Rust
and language formatting, evidence consistency and concurrent main-file changes
are checked separately. These are the audit baseline's focused checks; the
subsequent optimization's full workspace validation is recorded in its linked
implementation note.

```sh
cargo build --release --offline
cargo build --release --offline -p lang_runtime --example performance_audit
cargo build --release --offline -p lang_codegen --example mir_audit
python3 reports/performance-audit-2026-10-16/run.py
python3 bench/telemetry.py --lang "$PWD/target/release/lang" \
  --events 16384 --sample-every 16 --profiles balanced backpressure fanout cpu \
  --workers 1 2 4 8 --reps 3 --warmups 1 --runtime-stats \
  --out reports/performance-audit-2026-10-16/evidence/telemetry
```

The microbenchmark runner records commands, settings, raw outputs, input hashes
and binary hashes. MIR counts are conservative graph/IR observations; machine
listings are Cranelift's post-compilation instruction listings, rather than a
hardware-counter profile. Existing historical reports were consulted for
context and rechecked against this revision; their older performance figures
were not reused as current measurements.
