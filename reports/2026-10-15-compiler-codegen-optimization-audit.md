# Compiler optimization audit: improving generated release code

Reviewed 2026-10-15 at `9cb386afa59490b83e3f3a40071b03d9fd9fc964`,
with the working-tree changes recorded in the [capture metadata](compiler-codegen-audit/evidence/metadata.json).
Status: findings and proposed changes; **no compiler optimizations implemented**.

## Recommendation

The best immediate opportunities are to remove unnecessary GC-root slots and
frames, inline small direct calls before scalar replacement, and specialize
collection traversal. Next, remove immediate closure allocations and specialize
small structural comparisons. These changes eliminate work that is visible in
the current optimized output, rather than relying on another backend flag.

Release code generation already uses Cranelift's `speed` optimization level.
The missing layer is mostly language-aware MIR optimization: the compiler knows
concrete types, object layouts, ownership, and direct callees, but often turns
that knowledge into opaque runtime calls or separately compiled functions.
Cranelift cannot recover all of those facts afterward.

The priorities below concern the quality of generated programs. Frontend caches,
parallel compilation, and serialized-code caches can improve compiler latency,
but do not themselves make a running program's machine code better. The broader
[optimization plan](../OPTIMIZATION_PLAN.md) covers those areas; this report
expands its root, inlining, and runtime-crossing proposals with codegen evidence.

## Scope and evidence

Inspected MIR construction, scalar replacement, layouts, serialization, CLI/driver
release selection, Cranelift lowering, and the runtime operations called by the
generated code. The principal source locations are:

- [MIR lowering](../crates/lang_mir/src/lower.rs): `FnLower::finish`, `lower_for`,
  `lower_lambda`, `compare`, `alloc_enum`, and `tail_calls`.
- [Scalar replacement](../crates/lang_mir/src/sra.rs): `scalar_replace`.
- [MIR definitions](../crates/lang_mir/src/mir.rs) and
  [layouts](../crates/lang_mir/src/layout.rs): `Layouts::repr`, `record`, and `enum_variants`.
- [Code generation](../crates/lang_codegen/src/lib.rs): `Jit::new`,
  `FnCx::define`, `stmt`, `rvalue`, `rt_call`, `int_bin`, and `Body::set`.
- [Runtime ABI](../crates/lang_runtime/src/abi.rs),
  [list storage](../crates/lang_runtime/src/list.rs),
  [contiguous storage](../crates/lang_runtime/src/buffer.rs),
  [value comparisons](../crates/lang_runtime/src/value.rs), and
  [task/GC coordination](../crates/lang_runtime/src/task.rs).

The [probe program](compiler-codegen-audit/probes.lang) exercises scalar helpers,
nonescaping records, records passed across calls, unused aggregates, list
iteration, structural equality, and an immediately invoked capturing closure.
It prints `7`, `14`, `14`, `7`, `6`, `true`, and `8`, one per line.

Captured MIR, CLIF before optimization, CLIF after compilation, and the backend's
machine-instruction listing for both one and four workers:

- [One worker: MIR](compiler-codegen-audit/evidence/workers-1/program.mir),
  [before CLIF](compiler-codegen-audit/evidence/workers-1/before.clif),
  [after CLIF](compiler-codegen-audit/evidence/workers-1/after.clif),
  [machine listing](compiler-codegen-audit/evidence/workers-1/machine.txt).
- [Four workers: MIR](compiler-codegen-audit/evidence/workers-4/program.mir),
  [before CLIF](compiler-codegen-audit/evidence/workers-4/before.clif),
  [after CLIF](compiler-codegen-audit/evidence/workers-4/after.clif),
  [machine listing](compiler-codegen-audit/evidence/workers-4/machine.txt).

Capture used a temporary copy of the codegen module with output instrumentation
only. The normal driver first built the program, then the instrumented copy
compiled the same MIR with `opt_level=speed`. The capture helper copies the
workspace lockfile and uses the workspace's dependency declarations. It does
not alter production compiler sources or apply proposed optimizations. The
instrumented compilation is an inspection tool, not a timing benchmark.

The environment was Linux x86_64, AMD Ryzen 7 8745HS, Rust 1.98.1, Cranelift
0.136.3. The recorded source hashes were unchanged during the final capture.
Other work changed the shared checkout during the broader audit; conclusions
refer to the retained capture, not a claim that the entire working tree was
frozen throughout testing.

## What is already optimized

| Existing behavior | Consequence for this audit |
| --- | --- |
| `lang run --release` selects Cranelift `speed`; without it, `none` | Building the Rust toolchain with `cargo build --release` alone does not select optimized language code. Use both release switches for measurements. |
| Cargo release uses thin LTO and one codegen unit | These optimize the Rust compiler/runtime binary. They do not inline Rust runtime bodies into JIT-generated language functions. |
| Native ISA builder detects host CPU features | Adding “target native” is not a missing JIT optimization. A future portable native-code cache would need compatible target-feature keys. |
| Cranelift's enabled optimization path runs its e-graph pass; alias analysis defaults on | Do not add a duplicate generic arithmetic/CSE pass without showing a missed case in optimized output. |
| Demand-driven monomorphization with instance deduplication and concrete record offsets | Ordinary field access already avoids dynamic label lookup. Specialization opportunities concern calls, operations, and representation. |
| Local aggregate scalar replacement | Local record construction is already allocation-free when all uses qualify. Cross-call and tag-reading cases remain opportunities. |
| Self tail-call conversion and demand-generated word adapters | Neither self tail-call elimination nor emitting fewer unused adapters is a new proposal. |
| Inline budget decrement with cold poll path; cold defect blocks | Polls already have a cheap path. Their frequency and surrounding root work can still improve. |

The backend conclusions were checked against the installed 0.136.3 source.
The upstream [settings documentation](https://docs.wasmtime.dev/api/src/cranelift_codegen_meta/shared/settings.rs.html)
also describes `speed`, `speed_and_size`, and default-enabled alias analysis.
Cranelift provides [inlining machinery](https://docs.wasmtime.dev/api/cranelift_codegen/inline/index.html),
but the embedder must supply policy and callee bodies. This compiler currently
defines functions separately and invokes no such inlining stage.

## Confirmed costs in optimized output

These are structural observations, not estimates of runtime speedup. Byte counts
are emitted bytes for each individual function, including cold paths, excluding
its callees, adapters, and module allocation overhead. Shadow-frame bytes are
the explicit root slot, not the complete native stack frame.

| Probe function | Shadow frame | Code bytes, 1 worker | Code bytes, 4 workers | What remains after optimization |
| --- | ---: | ---: | ---: | --- |
| `identity` | 24 B | 167 | 200 | One environment root, frame link/unlink, stack check, and entry poll for a scalar identity body. |
| `scalarized` | 40 B | 256 | 320 | Record allocation removed, but the environment and two null aggregate locals still have root slots. |
| `through_call` | 40 B | 254 | 330 | A 16-byte record payload is allocated and passed to a separate `area` helper. |
| `unused_record` | 32 B | 185 | 218 | Allocation removed, but a null aggregate slot remains. |
| `sum_list` | 48 B | 426 | 594 | Runtime length/get calls; the get call is inside the loop; duplicate aliases of the list are rooted. |
| `records_equal` | 64 B | 284 | 347 | Descriptor-driven runtime equality for two known two-integer records; several pointer aliases are rooted. |
| `immediate_closure` | 48 B | 285 | 361 | A 24-byte closure payload allocation, code/adapter/capture stores, a code-pointer load, and an indirect call. |
| `area` | 40 B | 245 | 308 | A separate checked multiply helper with frame/poll overhead and duplicate record aliases. |

The closure's lambda is another 237 bytes with one worker, 300 with four.
The fact that `through_call` is slightly smaller than `scalarized` says nothing
about which executes faster: the former performs an allocation and calls a
separate function whose bytes are excluded from that row.

## Prioritized opportunities

P1: begin here. P2: follow after the relevant P1 foundations. P3: defer until
profiling justifies complexity. M means a subsystem change; L means substantial
analysis or compiler/runtime integration. Benefits are hypotheses unless they
describe the specific work visible in the retained output.

| ID | Priority / effort | Proposed optimization | Work it can remove |
| --- | --- | --- | --- |
| G1 | P1 / M initially, L for full liveness | Compact roots and omit provably empty shadow frames | Null/dead slots, redundant pointer stores, frame link/unlink, and accidental object retention. |
| G2 | P1 / M–L | Budgeted MIR inlining, followed by simplification and scalar replacement | Small helper calls and aggregates whose apparent escape is only passing to an inlineable helper. |
| G3 | P1 / M–L | Typed immutable collection reads and page-aware iteration | Per-element runtime transitions, representation dispatch, repeated page division, and proven redundant bounds checks. |
| G4 | P2 / M–L, after G2 | Devirtualize known closures and eliminate nonescaping environments | Closure allocation, code-pointer loads, indirect calls, and unnecessary adapter generation. |
| G5 | P2 / L | Range and predicate facts for checked arithmetic/access | Overflow/conversion/bounds checks when safety is actually proved. |
| G6 | P2 / M | Specialize small structural comparisons | Descriptor walks, generic equality calls, and associated lock transitions. |
| G7 | P2 / L, after G2/G4 | Tag-aware aggregate replacement and sequence fusion | Temporary `Option`/`Result` allocations, iterator wrappers, and intermediate materialization. |
| G8 | P2 / M–L, after G2 | Coalesce redundant polls while preserving bounded scheduling | Repeated budget loads/stores and cold-path branches exposed by inlining. |
| G9 | P2 / M–L | Preserve immutable-memory and runtime-behavior facts | Repeated loads and metadata operations obscured by generic calls; enables G1/G3/G5. |
| G10 | P3 / L | Selected aggregate ABI/layout improvements and vector kernels | Remaining cross-call boxing, enum boxes/padding, and suitable scalar bulk loops. |

### G1 — Remove dead/null roots before attempting full root liveness

`FnCx::define` gives every `Repr::Ptr` local a permanent slot, including `env`.
`Body::set` writes that slot on every assignment. `scalar_replace` replaces
eliminated aggregate pointers with null but retains their pointer-typed locals.
The optimized `scalarized` and `unused_record` listings confirm those slots and
stores survive. The `identity` listing confirms even a scalar helper has a frame.

Start with a pass that identifies unused pointer locals and locals that are
always null. Drop their root slots while retaining any effectful producer
evaluation. An unused closure environment needs no root if its contents are
never accessed. If the resulting root set is empty, omit shadow-frame storage,
linking, and unlinking. Keep stack overflow checking independent of this change.

Then add CFG liveness, reuse slots for disjoint live ranges, clear dead slots,
and consider writing roots only when required at safe points. This needs
interprocedural runtime-behavior summaries: calls, polls, suspension, callbacks,
and entry into the multi-worker runtime can expose a collection opportunity.
Caller roots must protect values across callees, and pending managed results
must stay protected through runtime transitions. Moving collectors are not
required for these hazards: a nonmoving collector can still reclaim an unrooted
object.

Acceptance: root-slot counts fall for the retained probes; dead temporaries
become collectible before function return; caller values and closure captures
survive forced collection and suspension on one/four workers. Keep stack
overflow, cancellation, and runtime callback cases. Do not remove allocations
merely because their destination local is dead unless removal is independently
valid.

### G2 — Inline at MIR level, then rerun the existing aggregate pass

The `through_call` probe allocates `{ w: n, h: 2 }`; the nearly equivalent
`scalarized` probe does not. `scalar_replace` treats passing an object to a call
as escape, and there is no MIR inliner before it. The `area` body is small and
already specialized to a concrete shape.

Introduce a release-only, size-budgeted direct-call inliner. Begin with small
nonrecursive helpers and fixed scalar/record operations. Remap locals/blocks,
parameters, environments, and return joins; preserve checked-operation source
locations and argument evaluation order. Follow with constant/copy propagation,
unreachable-block cleanup, effect-aware dead-local elimination, scalar
replacement, and root compaction. Repeat boundedly: one rewrite can expose the
next allocation for removal.

MIR is preferable for the first implementation because the existing allocation
and escape logic lives there. Cranelift's inliner is a viable later alternative,
but using it alone would require equivalent post-inline object/root cleanup.

Exclude or explicitly model global initializer getters, cleanup registration,
unknown callbacks, recursion, and live-reload boundaries. `--watch` and
`--inspect` call selected functions through replacement slots; inlining those
bodies would require invalidating/recompiling dependents. Initially disable
cross-reload inlining in those modes, including when `--release` is also set.

Acceptance: `through_call` loses its record allocation and `area` call, with
identical values and overflow locations. Measure runtime, JIT compilation time,
and total code bytes on a realistic call graph. Put a growth cap on each caller
and specialization, not just a callee-size threshold.

### G3 — Expose collection reads and specialize traversal

`lower_for` already hoists length out of list-style loops. It still calls
`lang_rt_list_get` for every element. That ABI helper tests the buffer/list
representation, checks bounds again, and dispatches into generic storage.
Paged `list::get` computes page quotient/remainder and dispatches on element
representation. With four workers, `rt_call` also emits enter/leave calls around
each runtime operation. This structure is present in both optimized captures.

Introduce typed MIR operations for immutable array/list/bytes length and access.
Start with frozen contiguous arrays: lower a known representation to a length
load, a necessary bounds branch, and a typed load. For compiler-generated
iteration, carry the relationship `0 <= index < length` so the second check
can be removed. General indexing must retain its checked failure path.

For paged lists, use a page cursor: load a page once, walk its valid elements,
then advance the directory. Constant element size can specialize addressing.
This avoids repeated division and per-element generic calls. The current
`chunks()` implementation constructs copied arrays, so it is not an existing
zero-copy substitute. Preserve partial-final-page lengths and stable element
references; builders can share pages with frozen list values.

Use an explicit compiler/runtime access contract. `ListData` and `BufData`
currently contain Rust-owned containers; generating loads at guessed Rust
`Vec` offsets would create a brittle ABI. Provide stable metadata or narrow
helpers first. Read shared page storage without racing mutable builder state.
Keep the owning object rooted and retain polls so another worker's collection
can synchronize safely. Removing a runtime call must not silently remove the
only publication or safe-point boundary a path relied on.

Acceptance: no generic get/enter/leave call in an eligible loop's hot body;
exact values across element widths, empty/partial/multiple pages, frozen
arrays, and pointer elements under forced GC. Measure one/four workers and
separate collection construction from traversal.

### G4 — Direct-call known closures and remove their environments

`immediate_closure` creates and immediately invokes a local lambda. Despite a
known lambda body and capture, optimized output still allocates its environment,
stores code and adapter addresses, reloads the code pointer, and calls indirectly.

Propagate closure identity through single-definition aliases, convert calls to
known targets, and inline where profitable. Once all environment uses are
visible, scalar-replace the environment and captures. A direct-call rewrite
alone removes dispatch but does not necessarily remove the allocation.
Recompute adapter demand after these transformations.

Begin with closures that never escape the current invocation. Keep the normal
representation for callbacks stored in tasks, channels, globals, or runtime
registries. Capture evaluation must stay at closure construction time; moving
it to invocation changes snapshot behavior. Preserve consuming-closure rules,
scoped lifetimes, and cleanup behavior.

Acceptance: the retained immediate-closure case becomes checked scalar addition
with no environment allocation or indirect call; snapshot, escaping-closure,
task, scoped-resource, and live-reload tests retain their behavior.

### G5 — Prove redundant checks; preserve checked release semantics

The MIR has checked arithmetic, conversions, overflow predicates, and branches,
but the inspected language-level pipeline has no general range/predicate pass.
`int_bin` expands ordinary checked operations into overflow flags or explicit
divisor/shift guards plus cold source-located defects. Cranelift already folds
some constants; the opportunity is facts implied by types, dominating branches,
and loop induction, rather than duplicating that constant folding.

Examples to investigate are addition after `0 <= x && x < 100`, widening an
unsigned byte before bounded arithmetic, repeated validated conversions, and
safe induction-variable access to an immutable collection. This item is a
source-supported opportunity, not a claim that every example currently retains
its checks: capture the optimized output for each before implementing a pass.

Track integer width, signedness, intervals, and dominating predicates, with
conservative joins and loop widening. Remove a check only when the operation
cannot fail on every incoming path. Never hoist a potentially failing operation
to a path that previously skipped it, or delete a failing expression because
its result is unused. Keep evaluation order and original defect locations.

Acceptance: boundary-value and differential tests over signed/unsigned widths,
MIN/-1 division, zero divisors, invalid shifts, and lossy conversion; reduced
checks only in positively proved cases. Release overflow and bounds failures
remain defects, as required by [the type contract](../spec/02-types-and-records.md).

### G6 — Generate small structural comparisons from concrete layouts

`compare` emits native scalar comparisons but delegates aggregate comparisons
to `lang_rt_eq`/`lang_rt_cmp`. `records_equal` confirms that two known fields of
type `Int` still take this path. The runtime walks descriptors and, for an
identity shortcut, may recursively determine whether a type contains floats.

Lower equality for small closed scalar records/tuples to typed field loads and
short-circuit comparisons. Cache or generate a per-concrete-type comparison
helper when expansion would be large. Keep dynamic values, unknown recursive
shapes, and complex collections on the generic fallback initially.

Do not substitute bytewise `memcmp`: padding, floating NaNs, signed zero, and
semantic equality make that incorrect in general. In particular, a record
containing NaN is not necessarily equal to itself. Ordering must follow the
language's field order and float comparison rules, not raw layout bytes.

Acceptance: the retained two-integer comparison loses the generic runtime call;
NaN, signed-zero, nested-value, enum-tag, map/set, and array equality cases
retain their semantics. Apply code-size limits to generated helpers.

### G7 — Extend aggregate elimination to enum tags and iterator pipelines

The current scalar replacement recognizes fixed-offset field loads/stores;
`Rvalue::Tag` and calls make candidates escape. Enums normally use a managed
pointer representation with an explicit tag and payload. Lazy sequence adapters
in [the sequence library](../crates/lang_hir/std/seq.lang) use closures and
`Option` values between stages. These are promising allocation-elimination
targets once G2/G4 expose their bodies.

First teach aggregate replacement about tags and known constructors followed
by matches. Represent local tags and live payload fields as scalars, preserving
control-flow validity. Then fuse known `map`/`filter`/`take` chains into their
consumer loop when effects, evaluation order, and close behavior permit.
Map/set iteration also currently materializes a list; a direct cursor can avoid
that materialization under a defined traversal contract.

Keep enum scalar replacement separate from a global niche representation change.
The latter also changes descriptors, tracing, runtime words, serialization,
closure adapters, and reload compatibility. Local removal can deliver value
with a much smaller representation surface.

Acceptance: count allocations per element for `Option`-returning iteration and
short pipelines; preserve early `break`, `return`, `?`, cancellation, and
exactly-once iterator closing. Preserve `Result` error-origin recording and
affine payload ownership. This opportunity was identified from source; the
retained probes do not benchmark complete sequence pipelines.

### G8 — Reduce poll duplication without unbounded execution

Function entries and loop heads emit `Stmt::Poll`; codegen turns each into
budget load/decrement/store and a cold runtime path. Inlining can leave several
polls on a short straight-line path. The current helper listings also show how
much of a tiny function consists of entry bookkeeping.

After inlining, coalesce polls on bounded paths with a documented work budget.
Keep coverage of every unbounded loop/call cycle. Bounded accounting may retain
more than one unit of budget charge even when combining checks. Consider a
local budget across a bounded hot region only when suspension/callback rules
allow synchronizing it with `TaskCtx` correctly.

Acceptance: fewer hot-path budget operations, bounded cancellation/GC latency,
and fairness between CPU-bound and waiting tasks on one/four workers. Removing
all polls from pure code is unsafe: compiled code runs outside the runtime lock,
and the collector waits for workers to reach runtime calls or polls.

### G9 — Keep runtime behavior and immutable-memory facts explicit

`Callee::Runtime` and `Callee::Intrinsic` mainly identify a name/signature;
`rt_call` brackets runtime operations uniformly in multi-worker programs.
Language effects such as `Io` are not enough to decide whether a call can
allocate, collect, suspend, enter the runtime lock, or invoke compiled callbacks.
Without those facts, root liveness and load-motion passes must be conservative.

Add reviewed runtime-behavior summaries and typed immutable reads. Distinguish
read-only managed payloads, mutable cells/builders, globals, root frames, and
task bookkeeping. Use backend memory flags or alias regions only when their
contracts really hold. Cranelift alias analysis is already enabled; supplying
sound facts and removing opaque calls is the opportunity.

Acceptance: repeated eligible field/metadata loads disappear across operations
proved not to modify them; mutation, callbacks, GC, live slot replacement, and
multi-worker publication still prevent invalid hoists. Begin with a few audited
operations, rather than declaring an entire intrinsic family pure or lock-free.

### G10 — Later representation and vectorization work

If profiles still show expensive cross-call boxing after inlining, consider a
specialized internal ABI passing small scalar aggregates in registers. Preserve
the existing managed ABI at runtime callback, public closure, FFI, and reload
boundaries using adapters. Pointer-bearing aggregates require a precise rooting
design; this is more invasive than G2.

Other later candidates are niche encodings for eligible `Option` values and
padding reduction for internal records. Current record layouts are canonical by
label, and structural/nominal coercions rely on compatible layouts. Any layout
change must update all producers/consumers together, retain `repr c` contracts,
and version serialized MIR/runtime ABI assumptions.

For vectorization, first expose contiguous scalar operations through G3 and
remove call/poll barriers in bounded regions. Byte copies/comparisons and
explicit wrapping arithmetic are better first kernels than arbitrary checked
integer reductions. Checked arithmetic must preserve the first visible defect;
floating reductions must preserve IEEE behavior and evaluation requirements.
No automatic-vectorization capability or achievable speedup was established by
this audit. Measure candidates before considering a second backend.

## Release policy and implementation sequence

Keep verification and semantics common across optimization levels. Add an
explicit optimization configuration for MIR/codegen rather than inferring it
from Rust's `cfg!(debug_assertions)`. The compiler can itself be a release binary
while generating unoptimized programs. Preserve neutral saved MIR or key/version
any optimized cache by optimization policy; source and binary-MIR execution
must both apply the intended policy, including runtime worker count and live mode.

| Slice | Depends on | Runnable acceptance proof |
| --- | --- | --- |
| 0. Stable measurements | None | Retain post-optimization CLIF/machine output; count roots, allocations, calls, code bytes, and compilation time for each probe. Add matched-call and escaping/nonescaping benchmark pairs. |
| 1. Root cleanup | 0 | G1 removes eliminated aggregate slots and unused environments. `identity`, `scalarized`, and `unused_record` show smaller root frames; GC/callback/overflow stress passes. |
| 2. Direct inlining | 0, 1 | G2 removes `through_call`'s allocation/call with bounded code growth; runtime checks keep original locations; live modes retain replacement behavior. |
| 3. Immutable traversal | 0, 1 | G3 removes generic per-element get calls in eligible loops; pointer-element and shared-page stress passes on one/four workers. |
| 4. Known closures and comparisons | 1, 2 | G4/G6 remove the immediate closure allocation and the small generic equality call, each measured separately. |
| 5. Facts and pipelines | 1–4 | Introduce G9 facts as needed, then G5/G7/G8 in separate changes. Prove reduced checks/allocations/polls and retained failure, cleanup, and scheduling behavior. |
| 6. Profile-driven extensions | Earlier slices measured | G10 or code-size tuning only where remaining profiles justify their maintenance and ABI costs. |

For each slice compare elapsed execution **and** startup/JIT compilation time,
code size, allocation count/bytes, GC pauses, and root/transition counts. Keep
fresh-process repetitions and a realistic application workload alongside small
probes. Do not sum projected speedups: inlining, root removal, and aggregate
elimination overlap.

`speed_and_size` is worth an explicit experiment if code size becomes limiting;
it is not a universally stronger `speed`. Do not propose simply enabling alias
analysis, native CPU flags, LTO, or self tail-call elimination: these are already
present. An AOT mode may improve deployment/startup and enable other backend
choices, but is not required for the improvements above.

## Validation and reproduction

From the repository root:

```sh
cargo build --release --offline
target/release/lang run --release --workers 1 reports/compiler-codegen-audit/probes.lang
target/release/lang run --release --workers 4 reports/compiler-codegen-audit/probes.lang
target/release/lang run --release --dump-mir reports/compiler-codegen-audit/probes.lang
python3 reports/compiler-codegen-audit/capture.py --out /tmp/lang-codegen-evidence-fresh
cargo test --workspace --offline
```

Use a fresh capture destination. The helper emits pre/post CLIF and machine
output from the same backend source; it adds a temporary capture binary to the
workspace's build artifacts, not a compiler feature. The ordinary
`LANG_DUMP_CLIF=1` dump occurs **before** `define_function`, so it is not
sufficient evidence of what survives optimization.

Validation completed:

- `cargo build --release --offline`: passed.
- `cargo test --workspace --offline`: passed, including adversarial optimized
  execution, source/MIR corpus execution, one/four-worker cases, live reload,
  service tests, and stress tests. This was the baseline test binary built near
  the start of the audit, before subsequent shared-tree MIR edits; it does not
  validate those later edits or any proposed optimization.
- The probe's expected output matched in all four combinations of generated
  optimization off/on and workers one/four. Commands and outputs are retained
  in [verification.json](compiler-codegen-audit/evidence/verification.json),
  alongside extracted code-byte and shadow-frame metrics.
- The final instrumented capture built successfully and recorded unchanged
  source hashes during its run. It included the then-present MIR verification
  work in the shared tree. The temporary helper build reported an `unused_mut`
  warning in that separately edited verifier; no compiler sources were changed
  by this audit.
- Probe formatting, capture-script Python syntax, report local links, and
  `git diff --check` passed.

Existing benchmark caveats still matter: the `record update (allocates)` label
does not account for scalar replacement; the rooted-call case contains an extra
scalar helper call; and `chunks()` includes copied-array construction. Do not
attribute their entire timing differences to allocation, root stores, or raw
traversal. No optimization A/B implementation or percentage speedup is claimed
in this report. Final code was inspected on one x86_64 host; other targets and
end-to-end performance remain to be measured when each change is implemented.
