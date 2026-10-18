# Making the language a viable and desirable alternative to Go and Elixir

Date: 16 October 2026. Status: design and implementation recommendations; this report implements no language or runtime changes.

The implementation baseline is `d9907c4` on `main`, with existing uncommitted documentation left in place. Main is receiving concurrent work; further uncommitted runtime edits appeared during report preparation and are outside this assessment. Performance figures below belong to their explicitly named earlier revisions, rather than to this baseline.

## 1. Recommendation and intended application

Build a language for dependable services and integration workers whose data, concurrency, resources, and failures remain understandable as the application grows. The strongest initial application is a small service that accepts orders or events, validates and stores them, runs bounded background work, calls another system, and recovers safely after interruption.

The proposed advantage over Go is less application wiring and stronger checks around data transformation, task ownership, resource lifetimes, and external authority. The proposed advantage over Elixir is a statically checked native application model with efficient typed data and explicit backpressure. Both advantages must be demonstrated in complete applications. A new language also imposes substantial adoption costs: smaller ecosystem, unfamiliar diagnostics, operational uncertainty, and another toolchain to maintain.

Go is the primary practical comparison for native services. Its concurrency and garbage-collected runtime already cover much of our basic proposition; native execution and channels alone provide little differentiation. [Go design rationale](https://go.dev/doc/faq).

Elixir is the primary comparison for service behavior. Supervisors, child specifications, restart policies, and orderly shutdown already form a coherent application model. We need an equally coherent story for our own semantics. [Elixir Supervisor](https://hexdocs.pm/elixir/Supervisor.html).

The recommended investment order is:

1. Fix demonstrated correctness and soundness defects, and stabilize observable contracts.
2. Finish the foundational runtime and library boundary so first-party packages can grow independently.
3. Deliver one complete service and worker stack with strong cancellation, overload, supervision, and persistence behavior.
4. Make builds, dependencies, deployment, debugging, and upgrades predictable.
5. Improve measured runtime and compiler bottlenecks against equivalent Go and Elixir applications.
6. Add syntax only where application work demonstrates a repeated ergonomic problem.

“Viable” means a team can build, deploy, diagnose, upgrade, and recover the target application without bespoke runtime patches. “Desirable” means the language measurably reduces implementation or maintenance effort while meeting the application's performance and reliability requirements.

## 2. Existing direction and evidence

### 2.1 The library migration is an accepted direction

The user identified existing migration work. Its planning record (a local planning document) covers the foundational runtime, robust FFI, core/std and extended modules written in the language, occasional native libraries such as SQLite, retention of the paged list and useful intrinsics, and new integrations without runtime changes.

This report adopts that direction. The planning record is evidence of intended work, not evidence that extraction is implemented. The inspected baseline still embeds 42 standard modules and links HTTP, SQLite, parsing, codecs, and other services through `lang_runtime`. No separate, completed extended-library implementation was found in the inspected main tree.

The adjacent priorities record, `work-priorities-2026-10-16.md`, places bugs first, then runtime concurrency work, with small conformance improvements interleaved. It also requests stronger compile-time concurrency checks without a large language overhaul. The recommendations below preserve those priorities; they do not replace the current implementation queue.

### 2.2 What is already implemented

[SPEC.md](../SPEC.md), [ARCHITECTURE.md](../ARCHITECTURE.md), source modules, and integration harnesses establish a substantial starting point:

| Area | Existing foundation | Work still justified |
|---|---|---|
| Data and checking | Inferred structural records, row/effect inference, enums, protocols, checked arithmetic, ownership and region checks | Stabilize interactions, improve explanations, remove implementation limits that surface as runtime defects |
| Concurrent execution | Stackful native tasks, owned scopes, channels, cancellation, selection, managed worker restart | Application-level supervision APIs, overload contracts, fairness and latency evidence |
| Application composition | Typed systems and interceptor pipelines, dependency-aware cleanup | Coherent service conventions, partial restart/reload behavior, better diagnostics |
| Service facilities | HTTP/SQLite example, typed routing, schemas, outbound HTTP, typed SQL | Packaging as libraries, scalable networking, production driver coverage |
| Reliability facilities | SQLite-backed jobs, explicit uncertainty and reconciliation, reproduction bundles | Broader adapter contracts, more failure-window coverage, operational tooling |
| Tooling | Formatter, tests, LSP, MCP, inspection, profiling, debugging, live replacement | Package/deployment workflow, sustained editor responsiveness, managed-value debugging |

The existing durable jobs facility is a single-host SQLite-backed library, not a distributed workflow engine. Recording/replay has capture limits; SQLite and served HTTP are uncaptured boundaries. Current logical task failure containment shares a native runtime and heap and does not establish VM-level fault isolation.

### 2.3 Measurements that constrain the recommendations

The separate branch `bench/language-comparison`, at report commit `6ca402d`, retains a correctness-gated cross-language comparison using compiler/runtime snapshot `cd92e94`. This corrects the earlier assessment that no Go comparison existed: it exists in that branch, outside main. Its report is `reports/language-comparison/2026-10-16/README.md` and its methodology is `bench/compare/README.md` on that branch.

Selected warmed medians, milliseconds:

| Workload | Lang source, 1 worker | Go native | Interpretation |
|---|---:|---:|---|
| Bounded integer CPU work | 3.621 | 3.128 | Useful native-code foundation; a narrow arithmetic control |
| Materialized records and enriched copies | 6.408 | 0.636 | Investigate representation, allocation, and generated loops |
| Retained map snapshots | 5.897 | 2.422 | Persistent representation alone does not guarantee a win |
| Scalar producer/consumer handoff | 2.328 | 2.868 | Competitive on this particular bounded 1:1 contract |
| Fixed 64-item batches | 1.509 | 0.283 | Significant remaining collection/transfer overhead |

Several cells, including Go records, snapshots, and batches, have variation above 10%. Short warmed intervals can defer GC work, and ports use different ordinary runtime primitives. These are exploratory signals, not universal ratios, allocation-cost isolation, or production service measurements. No Elixir port is present in that comparison.

The [telemetry baseline](telemetry-indexer/2026-10-16-ec2b13e-baseline.md) at `ec2b13e` is more application-like, but deliberately excludes external I/O:

| Profile | 1 worker, events/s | 8 workers, events/s | Speedup |
|---|---:|---:|---:|
| Balanced | 561,257 | 482,062 | 0.86× |
| Backpressure | 533,677 | 590,583 | 1.11× |
| Fan-out | 145,582 | 147,966 | 1.02× |
| CPU-heavy | 208,091 | 617,993 | 2.97× |

One cache actor is intentionally serial. The figures establish limited scaling for those topologies; they do not assign all cost to the scheduler. Later scheduler and parallel-sweep work is outside those measurements. See the [measurement guide](../bench/telemetry.md) and [runtime update](2026-10-16-runtime-benchmark-update.md).

The comparison also exposed different MIR optimization behavior between source and saved MIR. That specific gap is now addressed by `d9907c4`; it belongs in completed work, followed by fresh verification and measurements. Its earlier timing difference must not be carried forward as an unchanged defect.

## 3. Boundaries between the five requested layers

The runtime should supply mechanisms needed to execute safe, efficient programs. Core std should supply a small coherent programming vocabulary. Extended libraries should supply maintained application capabilities and integrations. Some compiler support is necessary across these boundaries, but integration-specific names must not define the general language contract.

| Layer | Owns | Should avoid owning |
|---|---|---|
| Syntax | Functions, records, patterns, errors, borrows, scopes, typed description forms | Database, HTTP, actor, or framework-specific keywords |
| Language semantics | Type/effect/capture rules, transfer, region escape, task ownership, failure and cancellation contracts | Retry decisions for arbitrary remote operations; universal database semantics |
| Runtime implementation | Scheduling, GC, stack/root management, paged lists, measured intrinsic kernels, native resources, I/O completion, essential OS mechanisms | SQL execution policy, HTTP routing, job schemas, application restart policy |
| Core standard library | Common values and collections, iteration, bytes/text, errors, tasks/channels/time, foundational I/O, lifecycle and pipeline vocabulary | Every database driver, format, transport, or application framework |
| First-party extended library | HTTP service/client stack, SQLite/Postgres/Valkey, durable jobs, codecs/formats, observability exporters, service starter | Private access to heap/scheduler internals or exemptions based only on a `std` module name |

Core std need not be tiny at the expense of convenience. Systems, pipelines, and basic worker policies can remain in the supported standard surface while their implementation lives in ordinary language modules. Tooling modules such as `std.compiler` can be distributed with the toolchain without becoming dependencies of ordinary applications.

Package location, implementation language, and public import spelling are separate decisions. Preserve compatibility aliases while extracting code. A package written mainly in the language may have a narrow native companion. “Batteries included” should mean maintained, documented, and easy to select through the official distribution, with reproducible versions; it should not require linking every battery into every program.

## 4. Syntax-level changes

### S1. Stabilize the existing everyday syntax — P0

No mandatory new grammar feature is needed to make the first competitive application. Keep uppercase type names, lowercase values/fields/variants, immutable record literals and updates, `Result`/`?`, `use`, ordinary `fn`, first-argument pipelines, and options records.

The existing form is already suitable for concurrent service code:

```lang
use scope <- task.scope()
let first = scope.spawn(fn() { load_first() })
let second = scope.spawn(fn() { load_second() })
let (a, b) = scope.try_join(first, second)?
```

This is a syntax-shape illustration using the implemented scope API; `load_first` and `load_second` are placeholders. Ordinary application code should continue to read sequentially inside each task. A separate async syntax category would add API duplication without addressing the measured bottlenecks.

Acceptance: the service, background worker, stream processor, and shutdown examples share one canonical style; formatting is idempotent; diagnostics point to original source rather than lowered continuations.

### S2. Consider a local block form of scoped `use` — P2, conditional

Current `use` captures the following continuation. Nested acquisitions can make the extent of cleanup less obvious in a long function. If application studies confirm this problem, consider an explicitly delimited form:

```text
// Proposed syntax, not currently implemented:
use tx <- sqlite.transaction(database) {
    save(tx, order)?
}
```

The module spelling is illustrative and would remain compatible with the package migration. The form must lower to the same callback/scoping machinery as the existing construct, preserving return, `?`, cancellation, region checking, and cleanup behavior. Define the meaning of leaving its block before accepting the syntax; do not silently turn a local callback return into an enclosing-function return.

Acceptance: a before/after reading study on transaction and nested request code, parser/formatter coverage, and equivalent lifecycle behavior. If a normal helper function already solves the readability problem, defer this addition.

### S3. Keep service and actor APIs library-shaped — P1

Use existing records, enums, methods, and typed plans for service definitions and messages. Avoid adding `actor`, `receive`, database query quotation, annotation-based dependency injection, or a web route DSL during the first adoption phase. Evaluate those only against a working library implementation and concrete limitations.

The needed ergonomic work is chiefly inference and diagnostics: retain the field introduced at each pipeline stage; show which stage requires a missing field; explain the dependency edge responsible for a system cycle; show why a closure cannot cross a task or resource boundary. These belong to checking and tooling rather than extra punctuation.

### S4. Publish one visible API convention — P1

Choose consistent rules for options records, fallible constructors, scoped acquisition, stream consumption, and errors. Provide small examples of the same operation through a direct function, method, pipeline, and service handler. Avoid competing names for identical lifecycle behavior. Preserve explicit acknowledgement of discarded fallible results.

Acceptance: a developer can learn these patterns from the service starter and use a new first-party package without discovering a new control-flow convention.

## 5. Language-semantics changes

### M1. Stabilize effects, captures, regions, and package trust — P0

Separate “what calling this function can do” from “what this closure captures.” The documented sharing of effect and capture-row representation already causes a false region-escape rejection. Correct accepted programs must remain usable as core modules move into ordinary packages.

Audit `is_std` branches in type/effect/region checking before extraction. Some are bootstrap or implementation accommodations. Classify each as an ordinary language rule, a narrow compiler-known primitive contract, or an explicitly trusted implementation boundary. Do not grant broad checking exemptions to all first-party extended packages, or let a package acquire trust through its name.

Keep private representations for capabilities and opaque native resources. Make callback transfer and capture constraints survive separate compilation and imported public interfaces.

Acceptance: the same safe library API has the same caller guarantees as an embedded module and as an external package; negative tests cover closures, channels, collections, `dyn`, callbacks, and helpers hiding scoped values. Existing false-positive examples become accepted without accepting their escaping variants.

### M2. Complete the cancellation and resource contract — P0

The current owned-task model is valuable. Make its cross-library behavior precise and uniform:

- Expected `Result.err`, task defects, and cancellation remain distinct. Restart or retry requires an explicit policy decision.
- A scope cannot finish while work still uses its resources. State exactly when a child is considered settled.
- Cleanup runs in a defined order, with bounded cancellation shielding and explicit escalation for stuck cleanup. Keep the original error and cleanup errors observable.
- Nested deadlines use a consistent monotonic budget. An adapter cannot accidentally extend its caller's deadline.
- Cancellation of a blocking native operation does not authorize freeing its buffers or closing its resource while that operation still uses them.
- An expired request does not imply an external effect was undone. Adapters report an uncertain outcome where appropriate.

Go already offers deadlines/cancellation through `context` and grouped error handling through `errgroup`. Our opportunity is a coherent ownership contract across libraries, rather than treating cancellation itself as novel. [Go context](https://pkg.go.dev/context), [errgroup](https://pkg.go.dev/golang.org/x/sync/errgroup).

Acceptance: the same cancellation matrix passes for sockets, file operations, subprocesses, SQLite, and a second native adapter, including completion/cancel races and cleanup failure.

### M3. Add a coherent typed service/supervision model — P1

Build on existing managed workers. Define stable service identities, typed request/reply and cast interfaces, readiness, health, restart intensity, backoff with jitter, and shutdown modes. Keep policy implemented in language libraries where possible.

Begin with independent-child restart and clear escalation. Add ordered dependent-child restart when an application demonstrates the need. Define whether a service reference survives restart: a stable logical reference should reject or route requests according to policy, while a reference to one concrete generation must report staleness. Pending calls need specified outcomes when that generation exits. State reconstruction and migration are explicit operations.

Elixir's `GenServer` and supervisors are useful behavioral references, not APIs to copy mechanically. [GenServer](https://hexdocs.pm/elixir/GenServer.html), [Supervisor](https://hexdocs.pm/elixir/Supervisor.html).

Acceptance: crash one cache worker, reject or settle its pending callers, restart within budget, and demonstrate unaffected sibling request handling. Exceed the restart budget and show useful escalation through the system graph. No new actor syntax is required.

### M4. Make overload and boundedness part of public contracts — P1

Specify queue capacity, byte budgets, in-flight concurrency, and admission deadlines together. Define whether the count includes queued, staged, leased, or executing work. Zero capacity and close/drain behavior must remain explicit.

Introduce reusable admission permits tied to scopes, with exact-once release through success, failure, cancellation, and service restart. Quotas on a shared heap initially govern admission and attributable buffers; they cannot honestly promise exact per-service physical memory accounting when immutable values are shared.

Acceptance: a slow destination causes bounded retained work, predictable rejection, and stable recovery after pressure subsides. Measure task count, native buffers, queue occupancy, and RSS as well as managed live bytes.

### M5. Add conservative concurrency diagnostics — P1/P2

Honor the existing request for stronger compile-time concurrency checks. Useful initial targets include obvious self-wait, joining a task from itself, a local bounded send whose only receiver cannot run until that send completes, and a request/reply cycle proven from known local service calls. Extend the existing ownership/transfer analysis with explanatory task/resource graphs where they are reliable.

Reject violations only when the analysis proves them under the actual control-flow and aliasing rules. Use warnings or inspection output for suspected cycles, and retain runtime deadlock diagnostics for dynamic graphs. General deadlock/livelock freedom is not a realistic promise for arbitrary I/O, callbacks, and dynamic services.

Acceptance: every hard-error case has a minimal reproducer and a nearby valid control. Imported helpers and unknown aliases fall back conservatively. Diagnostics show the waiting cycle and source locations.

### M6. Define compatibility and specialization failure before deployment — P0/P1

Keep wire compatibility separate from structural source typing. A record that type-checks locally is not automatically a backward-compatible JSON event or persisted job state. Give serialization explicit field/default/version rules, unknown-field behavior, and migration hooks.

For library compilation, replace reachable specialization-depth stubs that fail only when called with either a supported dictionary fallback or an actionable compile-time diagnostic. A checked deployment should not acquire a hidden runtime defect merely by passing through a reusable generic library.

Acceptance: compatible event versions interoperate; incompatible changes fail a compatibility check or require a migration. Stress generic library composition beyond the existing specialization limit and verify controlled compiler behavior.

## 6. Runtime implementation changes

### R1. Build a supported native-resource and FFI boundary — P0

The current C boundary covers scalar/pointer/`repr c` calls and blocking/pinned execution contracts; it does not provide a complete managed-resource extension model. Make this the primary prerequisite for library extraction.

Provide an internal, versioned adapter interface for opaque handles, rooting/pinning, byte/string views, external memory accounting, task parking/completion, blocking work, and deterministic close. A generation-checked resource handle must reject use after close; it is not a raw address available to safe code. Distinguish task-confined, shareable, and thread-affine resources.

Document rooting and ownership at every transition: generated code to adapter, adapter to C, worker migration, external completion, cancellation, and return of a managed result. Foreign exceptions must not unwind through the ABI. Inline foreign calls must return promptly. Blocking calls use bounded workers and retain arguments until actual completion. Pinned resources need stable affinity for their entire lifecycle, including destruction.

Callbacks are currently rejected. SQLite's first extraction can avoid arbitrary C-to-language callbacks. A later callback API must specify which thread executes language code, rooting, reentry, cancellation, and lifetime; it is a separate supported feature, not a relaxed FFI flag.

Begin with statically linked, declared native companions and a versioned build contract. Dynamic loading, unloading, and binary ABI compatibility across arbitrary runtime versions can follow proven demand. Native extensions share the host's memory-safety boundary; capability checks do not sandbox arbitrary C.

Acceptance: SQLite and an unrelated native library build through this interface without new `lang_rt_sql_*` or package-specific runtime entry points. GC stress, multi-worker migration, errors, close races, and cancellation preserve resources and roots.

### R2. Provide scalable, cancellation-aware I/O mechanisms — P0/P1

The inspected HTTP implementation starts an OS thread per accepted connection. Move transport work toward readiness/completion-driven networking integrated with task parking. A bounded blocking pool remains useful for genuinely blocking APIs; moving every idle connection into that pool would merely change the bottleneck.

The substrate should cover sockets, connect/accept/read/write, timers, partial writes, half-close, EOF, cancellation races, and ownership of buffers until completion. File and process operations may initially use a blocking backend behind the same semantic contract. Keep HTTP parsing, routing, TLS policy, and database protocols in libraries above it.

Admission must start before allocating unbounded connection threads or reading full request bodies. Bound accepted connections, header/body bytes, idle time, parser work, and in-flight handlers. Preserve streaming backpressure to the actual socket.

Acceptance: many mostly idle connections use bounded OS threads; slow readers, oversized bodies, partial writes, malformed requests, connection churn, and shutdown do not leak tasks or buffers. State tested connection counts and latency/load conditions rather than claiming general scalability.

### R3. Continue scheduler work using measured ownership boundaries — P1

The current queue/GC/parking work is already active. Preserve its proven invariants and remeasure; this report does not recommend restarting it as a wholesale scheduler rewrite. The goal is useful work under load, fairness, and reliable wakeups, rather than the absence of locks as an end in itself.

Measure ready-queue delay, steals, migrations, wakeups, park/wake handshakes, synchronization wait, task churn, and timer delay. Apply topology specialization only when endpoint use is proven, retaining general MPMC behavior otherwise. Bulk receive and producer-owned batch credits should account for capacity, cancellation, and publication exactly as ordinary operations do.

Acceptance: fixed-topology balanced/fan-out/backpressure telemetry profiles, independent channels, allocation pressure, and sparse timer traffic all retain correctness. Repeat old/new experiments under comparable host conditions and preserve regressions rather than selecting only favorable microbenchmarks.

### R4. Reduce GC and memory costs with latency evidence — P1/P2

Parallel sweeping has landed at `bcf5c74`; it does not remove the shared heap's stop-the-world phases. Track pause distributions, root-scan cost, allocation rate, retained data, stack RSS, native buffers, and allocator pool high-water marks. Exercise one allocation-heavy service beside latency-sensitive services.

Prioritize dead-root elimination, lower temporary allocation, accurate external-memory pressure, buffer reuse, and returning unused memory where feasible. Explore incremental or generational collection only after profiling identifies the needed latency improvement and the required write barriers and foreign-root protocol are designed.

Erlang uses per-process heaps with generational collection. Our shared immutable-value heap has different tradeoffs; logical supervision does not imply equivalent GC or native-fault isolation. [Erlang garbage collector](https://www.erlang.org/doc/apps/erts/garbagecollection.html).

Acceptance: report p50/p95/p99/max pauses and request latency during allocation storms, with RSS after load recedes. OS address-space reservation for task stacks must be distinguished from resident memory. Do not promise millions of cheap tasks from spawn timing alone.

### R5. Make language-written libraries fast — P1

Retain paged lists and intrinsic kernels where their representation or measured cost justifies them, as the accepted migration direction requires. Extend optimization around language-written algorithms: copy propagation/DCE, root liveness, bounded inlining, closure devirtualization, unboxed/scalarized data, typed traversal, and known representation specialization.

Use the record, snapshot, and batch comparison gaps to select investigations. Inspect actual optimized code and allocation counts before assigning causes. In particular, a benchmark named “record update (allocates)” may have scalar replacement remove the allocation.

Consider inline records in collections or other representations as compiler/runtime work with explicit ABI and GC implications, not as a new user-visible record type. Preserve bounds, overflow, evaluation order, cleanup order, roots, and live-reload validity.

Acceptance: moving a pure algorithm from native support to a language package has a retained representative benchmark and an agreed overhead budget. `lang compile` and release source execution use the intended MIR passes; remeasure the fix in `d9907c4` rather than assuming parity in every execution mode.

### R6. Offer a reproducible deployable artifact — P1

Start with a self-contained release directory or bundle carrying validated code, runtime, native dependencies, lockfile identities, target/ABI information, and operational entry points. It must run without the source checkout, compiler development installation, or dependency network access. Measure startup including JIT and native loading.

A standalone AOT executable is a valuable later output if startup, footprint, customer deployment, or policy requirements justify it. It is not the only way to deliver a usable release: Go produces executables, while Elixir has an established release packaging model. [Go builds](https://go.dev/doc/tutorial/compile-install), [Mix releases](https://hexdocs.pm/mix/Mix.Tasks.Release.html).

Acceptance: build on a clean machine, launch in a clean environment, handle termination and readiness, retain useful symbols/backtraces, roll back, and reject an incompatible native adapter before execution. Publish an actual platform support matrix.

## 7. Core standard-library changes

### C1. Define a compact, stable common vocabulary — P0/P1

Keep primitives, records, errors, options/results, protocols, collections, iteration, bytes/text, numbers, clocks, tasks/channels, scoped resources, and essential OS capabilities in the supported core. Keep pure behavior in language modules whenever it remains efficient. Retain native implementations for representation-sensitive kernels and essential OS mechanisms.

Stabilize `Reader`/`Writer`-style interfaces, structured I/O errors, EOF behavior, incremental consumption, buffer ownership, flush/close behavior, and cancellation. Names are proposals; choose final API names through the existing style. No format or network package should need to invent a competing byte stream model.

Acceptance: a file copier, HTTP transport, compression wrapper, and in-memory test adapter compose over the same interfaces. Partial reads/writes and early exit are tested.

### C2. Expand collection and stream usefulness without hiding costs — P1

The current `Stream<T>` is a minimal suspending `next` abstraction with map/filter/collect. Add fallible streams, scoped closure, bounded parallel mapping, batching, draining, and early termination using the existing effects and ownership rules. Collecting an unbounded stream must be visibly an unbounded operation or require a limit.

Offer incremental algorithms and builders for additional data structures—queues, heaps, ordered maps, deques—when the service applications need them. They should be possible as ordinary packages, without adding one intrinsic per operation. Publish complexity, ordering, copying, and retention behavior.

Acceptance: stream a large result through transform and sink with bounded retained memory; break, cancel, or return an error midway and verify every upstream resource closes.

### C3. Standardize errors, time, resilience, and observability — P1

Keep error codes, causes, source locations, and typed details consistent across package boundaries. Avoid making application callers parse strings. Separate monotonic deadlines from wall timestamps. Treat retry as policy requiring an explicit error classification and idempotency contract; do not automatically retry arbitrary side effects.

Provide common spans/events/metrics interfaces with bounded buffers, dropped-data counters, and a context model that propagates through tasks and adapters. Scope cancellation context, tracing context, and authorization capabilities have different purposes and should not collapse into one untyped bag. Export protocols belong in extended packages.

Acceptance: follow a request through HTTP, a worker, SQL, and an outgoing call; inspect its error and deadline; overload telemetry without blocking critical cleanup or silently losing observability.

### C4. Make testing and library documentation first-class — P1

Preserve controlled clocks and scheduling, property tests, GC verification, and deterministic seeds. Add shared adapter conformance suites and versioned service test fixtures. Document precisely which deterministic tests run on one worker and which tests exercise actual multicore races.

Publish searchable API docs with examples, effect/transfer/lifetime summaries, cost notes, and error cases. Keep compiler/LSP/MCP answers consistent and expose package APIs without requiring agents to inspect runtime internals.

Acceptance: the same example works from docs, CLI tests, editor queries, and a downstream package. A captured failure can be shared without credentials, and replay stops clearly at unsupported external boundaries.

## 8. First-party extended libraries: batteries included

### 8.1 Distribution and support policy — P0/P1

Ship a documented default service set through the official toolchain or an offline-installable pack, while making application dependencies explicit and locked. Record language/compiler/runtime compatibility, native library versions, platform support, license information, and release/security maintenance ownership.

Begin with vendored/path packages and a compatibility manifest if that accelerates the extraction. Add reproducible artifact acquisition, hashes, caching, and transitive resolution before a public registry becomes necessary. A registry itself is not the first deliverable. Elixir's Hex demonstrates the value of a coherent package ecosystem, but the immediate requirement here is reliable first-party consumption. [Hex](https://hex.pm/).

Import compatibility shims can keep existing `std.sqlite` and `std.http` programs working while the implementation moves. Do not silently add dependencies or download native binaries during ordinary checking. Networked resolution, native compilation, and offline execution must have explicit modes.

Acceptance: create a new service offline from the official pack; update one compatible library deliberately; restore the previous lockfile; reproduce both builds.

### 8.2 Recommended first-party package families

The labels below describe responsibilities, not settled import names.

| Family | First deliverable | Later expansion | Essential runtime support |
|---|---|---|---|
| HTTP and routing | Robust HTTP/1.1 server/client, bounded streaming, TLS client, structured headers/URLs, typed routes and OpenAPI | WebSockets/SSE and HTTP/2 where application demand warrants | Socket I/O, timers, bytes, task scopes |
| SQLite | Extract existing capabilities; safe connections/transactions, prepared statements, typed rows, migrations, backup and contention policy | Query streaming, tuning and observability | Native handles, blocking/pinned execution, cleanup |
| PostgreSQL | Transactions, typed parameters/rows, connection pool, prepared statements, TLS, cancellation behavior | Notifications and richer type support | Generic I/O or native adapter boundary |
| Valkey/Redis protocol | Bounded client/pool, timeouts, explicit reconnect/retry semantics | Pub/sub, streams, clustering as separate capabilities | Generic transport and stream support |
| Durable jobs | Preserve existing state/outbox/lease/fence/uncertainty model as an ordinary package | Additional stores and operators after a real need | Transactions through database packages; no job-specific scheduler primitive |
| Data formats | JSON codec with limits and stable errors; CSV and TOML for the pilot | Compression, MessagePack/CBOR, protobuf and other formats by demand | Bytes/streams/schema metadata; optional native companions |
| Observability | Structured logs, metrics endpoint/exporter, tracing exporter, health/readiness | Integrations requested by deployed users | Core observation/context interfaces |
| Service toolkit | Config validation, secret-safe logging, system composition, graceful shutdown, test harness and release template | Authentication integration and application-specific starters | Existing system/pipeline/task facilities |

For server TLS, initially support a documented reverse-proxy deployment and select a maintained TLS implementation for direct termination when needed. Cryptography, certificate validation, and SQL engines should reuse maintained implementations through supported boundaries; they should not become new bespoke language projects.

Phoenix is relevant as an example of an integrated web experience. Matching its entire scope is not a prerequisite for a good first service toolkit. [Phoenix overview](https://hexdocs.pm/phoenix/overview.html).

### 8.3 SQLite extraction is the architectural proof

SQLite should be the first demanding extraction because it already exercises scoped handles, transactions, error conversion, blocking work, typed data, and compiler checks. Split the work explicitly:

1. Put public types, transaction orchestration, decoding, migrations, pooling policy, and provider integration in ordinary language modules.
2. Put SQLite engine calls in a narrow native companion using the supported resource/FFI interface. Preserve SQLite's threading rules and single-writer realities; “pool” must not imply unlimited useful write concurrency.
3. Preserve rollback after error, cancellation, and defect, with original and rollback failures retained.
4. Replace compiler dependence on `lang_runtime::sqlite::check` with a versioned validation interface or toolchain adapter component. Keep SQLite-specific schema interpretation out of the general type checker.
5. Keep validated query identity and result-shape facts tied to the package/schema/version used to check them. Dynamic SQL remains runtime-validated unless another explicit checking mechanism is used.

The first version can use an explicitly installed, trusted toolchain adapter with declared inputs. A general arbitrary compile-time plugin system is unnecessary. Validators must be bounded, deterministic for declared inputs, usable offline, and included in cache identity. Checking ordinary source must not run untrusted package code with ambient authority.

Acceptance: the existing orders service, typed SQL cases, and durable job tests work using extracted SQLite; the core runtime no longer depends on `rusqlite`; another driver can integrate through the same resource and validation boundaries. A PostgreSQL validator must account for its own dialect and schema and cannot reuse SQLite as a false compatibility check.

### 8.4 Extraction must preserve semantics and external boundaries

Moving code can otherwise break region/effect checks, restricted-project authority analysis, inspection, record/replay, and error origins. Define package adapter metadata for these facilities rather than silently losing them outside embedded std.

Provide a supported way to describe recordable external operations with stable identities, inputs, outcomes, redaction, and fail-closed replay. Keep uncaptured operations explicit. Database replay needs a separate design for transactions and nondeterministic state; generic external-operation hooks alone do not solve it.

Acceptance: an extracted outbound client records and replays the same safe fixture offline, refuses changed writes, and protects credentials. Extracted database and served-HTTP boundaries report unsupported replay honestly. Restricted packages cannot regain ambient access through a moved helper or forged handle.

## 9. Runnable delivery sequence

P0 denotes correctness or a prerequisite for safe extraction; P1 denotes the first viable service release; P2 denotes desirability improvements after that release. These are dependency priorities, not promised dates. Keep the last working service runnable after each slice and split large rows into smaller commits as needed.

| Slice | Deliverable and main dependencies | Named demo and acceptance proof |
|---|---|---|
| A. Freeze the baseline | P0; record source/artifact hashes, current bugs, benchmark contracts, API inventory | **Known working service**: orders + jobs + shutdown run; retained failures have owners and reproductions |
| B. Native resource foundation | P0; M1/M2 and R1 | **Two adapters**: scoped SQLite resource and unrelated native resource survive GC/migration/cancel/close tests |
| C. Extract SQLite | P0/P1; B, package compatibility manifest, validation interface | **Database outside runtime**: orders, typed SQL, and crash-recovery jobs pass without SQLite linked into core runtime |
| D. Prove extensibility | P1; B/C | **Second database**: a minimal PostgreSQL transaction and cancellation demo needs no new runtime intrinsic; extend toward production coverage separately |
| E. Shared I/O and HTTP | P1; M2/M4, R2, C1 | **Slow clients**: bounded connections, streamed upload/download, cancellation, overload, and shutdown with retained thread/RSS/latency results |
| F. Supervised service toolkit | P1; E, M3, C3 | **Worker failure**: typed request settles, worker restarts, restart budget escalates, siblings remain available |
| G. Real integration pilot | P1; C/E/F, jobs and capability contracts | **Order reconciliation**: duplicate/out-of-order events, destination ambiguity, restart, operator resolution, and inspectable history |
| H. Reproducible release | P1; package/native compatibility and G | **Clean deployment**: launch offline without checkout, terminate cleanly, upgrade persistent state, roll back safely |
| I. Competitive evidence | P1/P2; stable application contract | **Same service, three stacks**: Go and Elixir ports, performance plus failure/maintenance comparison |
| J. Targeted desirability | P2; measured pain from G/I | **Cheaper to change**: compiler/runtime improvements and any justified syntax reduce costs without semantic regressions |

Slices E and H can develop independently once their foundations are settled. Extraction can start before a full I/O reactor is finished by preserving the existing bounded blocking mechanism. A production-capable PostgreSQL library is more work than the minimal proof in D; do not label that demo as a finished driver.

## 10. Proof needed for competitive claims

### 10.1 Equivalent applications, not only equivalent loops

Keep existing primitive comparisons, add an Elixir port, and add three complete application contracts:

- HTTP JSON service with a database, bounded request admission, validation, tracing, and graceful shutdown.
- Integration worker with deduplication, retries, uncertain external outcomes, reconciliation, and real process interruption.
- Streaming processor with bounded fan-out, slow consumers, mixed payload sizes, cancellation, and sustained useful CPU work.

Use idiomatic implementations and the same application guarantees. Record when one implementation relies on a library that provides stronger or weaker semantics. Do not force Elixir into our queue representation or Go into our immutable collection representation. Include a simple implementation and an optimized one only when both remain reviewable and maintain the contract.

### 10.2 Measurements and correctness checks

Measure throughput; offered-rate and admitted-work latency separately; p50/p95/p99/p99.9 where sample counts support them; CPU; peak and settled RSS; native/managed allocation and GC; task/thread/connection counts; startup; build/check time; artifact/dependency size; and shutdown/recovery time. Use long enough runs to include collector behavior and state growth. Preserve warmup, all raw samples, variation, environment, affinity, versions, and source/binary hashes.

Use an independent result oracle, not agreement between implementations as the sole correctness check. Run latency-sensitive services alongside allocation/CPU storms. Sweep queue capacities, payload sizes, worker counts, fan-out, storage contention, and load below/near/above saturation. Include process crashes and network ambiguity at recorded transition points.

The existing tests are valuable starting evidence: normal/optimized/multicore/saved-MIR corpus runs, static rejection cases, HTTP/SQLite integration, adversarial regressions, chaos scheduling, GC verification, and durable job/replay tests. Benchmark compilation under `cargo test` does not itself execute or validate performance workloads. Controlled-scheduler replay is not exhaustive multicore exploration.

### 10.3 Adoption gates

Agree numerical budgets from the pilot's actual service requirements before collecting competitive results. Suitable decision gates include:

| Gate | Required evidence |
|---|---|
| Correctness | No unresolved demonstrated memory-safety/soundness issue in the exercised path; all defined failure transitions and conformance checks pass |
| Viable performance | The pilot meets agreed latency, throughput, memory, startup, and recovery budgets on documented hardware |
| Extensibility | Two integrations work outside the runtime, with no package-specific scheduler/heap entry points |
| Operations | Reproducible offline release, usable diagnostics, monitored overload, safe shutdown, backup/restore, and documented upgrade/rollback |
| Developer experience | A developer unfamiliar with the language builds the starter and makes a realistic change without editing compiler/runtime internals |
| Desirability | Repeated implementation/change exercises show a meaningful advantage in effort, correctness, or operational diagnosis that users value enough to adopt the toolchain |

Avoid declaring success because a chosen aggregate score averages away a serious weakness. An application that meets throughput but loses work on shutdown has not met the contract.

## 11. Deliberate deferrals

Defer a general macro system, an actor keyword family, algebraic effect handlers, automatic arbitrary error unions, universal ORM, broad web framework, transparent distributed shared memory, and a new collector architecture without measurements. Keep cross-node distribution, replicated state, and rolling distributed upgrades as a separate future product scope. A good single-host service language can be valuable before matching distributed Erlang.

Retain compatible live function replacement, but distinguish development reload from a production upgrade protocol. State/schema migration, external resource compatibility, dependency-group restart, and rollback need explicit policies. Do not promise arbitrary native crash containment, universal exactly-once external effects, general deadlock freedom, or complete replay of uncaptured I/O.

## 12. Evidence and review limits

Primary local sources inspected: the eight specification chapters; architecture; current standard modules and loader; runtime module/dependency inventory; FFI, HTTP, GC, codegen and typed-SQL boundaries; CLI integration/stress/adversarial harnesses; current optimization/concurrency reports; the telemetry harness/retained results; and the separate cross-language comparison branch.

The targeted tests run during the immediately preceding assessment passed: 44 program integration tests, five HTTP/SQLite service tests, the retained adversarial suite, the quick concurrency stress matrix, and nine telemetry harness tests. The optional extended soak was not enabled. Those runs preceded the latest parallel-sweep and saved-MIR changes and are not fresh validation of `d9907c4`. This report adds documentation only and does not rerun the performance suite or claim a new runtime audit.

The codebase audit contains candidate findings and areas requiring recheck. Treat them as an investigation queue; reproduce and classify each against its actual revision before presenting it as a confirmed defect. The accepted rule remains bugs before performance or feature work.

Report validation checked Markdown fence balance, every relative local link, coverage of all five requested layers, and whitespace with `git diff --check`. No implementation files were changed for this report.

All proposed APIs, package groupings, new syntax, priorities, and acceptance gates in this report are recommendations. Existing specification rules remain authoritative until deliberately revised. The central next proof is a service built from ordinary maintained libraries, with SQLite extracted through a reusable foundation and its existing correctness guarantees intact.
