# Language design direction

Status: discussion draft. These examples propose syntax and semantics; the
compiler scaffold does not implement them.

The coherent current proposal is now [SPEC.md](SPEC.md) and its linked chapters.
This document preserves the earlier reasoning and alternatives. Where the two
differ, SPEC.md governs the proposed draft-0 design; additional choices there
are design recommendations, not retroactive claims of user approval.

The latest interceptor refinement uses prefix-limited **live** views: stage N
sees the current values of fields introduced by itself or earlier successful
enters, while later-only fields remain hidden. See
[the full rule](spec/06-systems-and-pipelines.md). This supersedes the separate
saved-state suggestion below.

## Agreed direction

- Purpose: an application language for services and automation. All subsequent
  design decisions should serve this target.
- Native compilation through Cranelift. No JavaScript target is planned.
- Static typing with inference, informed by F#, Gleam, and Rust, while allowing
  more flexible data composition than Gleam.
- Every named type starts with an uppercase character, including primitive
  types. Initial uppercase is a syntactic role identifying a type.
- Learn from Clojure library ergonomics and architecture without requiring
  Lisp syntax or Clojure semantics.
- Records, Integrant-style lifecycle-managed components, and interceptors are
  central programming concepts. Their exact division between language support
  and blessed standard-library facilities remains under discussion.
- Use Integrant as the lifecycle and dependency-management reference instead
  of Component, and Pedestal/Sieppari as the interceptor references.
- A substantial runtime is acceptable when it delivers useful built-in behavior.
- Execution model: sequential execution within each task, concurrent tasks
  throughout the application, and optional multicore execution underneath.
- Apply useful Erlang/OTP ideas with a simple modern API: cheap tasks, private
  state, observable failures, supervision where appropriate, and scheduling
  fairness. Long-running message processors are not required for all services.
- Preserve the appeal of Elixir and Clojure: accessible data, composable
  sequence operations, and a pleasant development experience.

## Type identifier rule

Examples: `Int`, `Bool`, `String`, `I32`, `U64`, `Person`, `Map<K, V>`.
These are illustrative primitive names, not a finalized primitive inventory.
Apply the rule to written type parameters and row variables too: `T`, `Rest`.
For a qualified type such as `http.Request`, the type identifier is `Request`.
Anonymous type expressions need no artificial name or uppercase punctuation.
Do not use initial uppercase for an ordinary value identifier. Constructor and
variant syntax must be designed consistently with this distinction; it is not
yet settled.

## Settled purpose: services and automation

A native application language for services and automation that coordinate data,
resources, and concurrent work. "Devices" in the discussion was a typo for
"services", not a hardware target. The recommended first reference application
is a service with background jobs, persistent storage, external I/O, graceful
shutdown, and useful runtime inspection.

This purpose puts the proposed core features on the same critical path:

- Records model requests, configuration, events, and results.
- Lifecycle descriptions assemble resources and services with explicit wiring.
- Interceptors describe processing, validation, tracing, and failure handling.
- Scopes and channels govern concurrent work, cancellation, and backpressure.
- The runtime exposes the running system and connects diagnostics to source.

The purpose is accepted. Its adoption argument should still be tested with real
small applications and developer feedback; choosing it does not establish
unmet market demand.

| Direction | Proposed priority and rationale |
|---|---|
| Services and background workers | Primary: directly exercises all central concepts |
| CLI tools and local automation | Early: useful small entry point with the same APIs |
| Desktop application logic | Later: reuses the application model but requires a UI ecosystem |
| Games | Later: tools and prototyping first; hard frame-time constraints would change runtime priorities |
| Low-level systems and embedded software | Requires different primary constraints around runtime size, allocation, and platform control |
| Domain-specific language | Support domain APIs within the language; keep the core broadly useful |

Native execution is a performance and interoperability choice, not a promise
of hard real-time behavior, a tiny runtime, or automatic standalone executable
packaging. Initial deployment can use a versioned compile-and-run runtime.
Startup cost, caching, and eventual artifact packaging need explicit designs.
Keep the first application runtime local-process focused; durable distributed
workflows and transparent distributed recovery are separate problems.

## Human understanding and machine verification

Recommended design stance: readable application intent and inspectable behavior,
with tooling that supplies precise feedback to both people and coding agents.
An AI-specific syntax or superior code-generation accuracy is not established.

Candidate facilities:

- Stable structured diagnostics with inferred shapes, missing-field locations,
  and clear explanations of invalid plan composition.
- Small conventional syntax, a formatter, and precise public contracts while
  retaining local inference.
- Inspectable dependency and processing plans, including source locations.
- Typed substitution of providers for tests, controlled clocks and randomness,
  and scripted external I/O.
- Correlated traces for lifecycle changes, task activity, and interceptor stages.
- Controlled replay when nondeterminism is recorded or replaced; no promise to
  replay arbitrary native I/O and concurrency deterministically.
- A standard programmatic interface to checks, symbols, schemas, plans, and
  diagnostics; any editor or coding tool can consume it.

These features support human review of generated changes as well as manual
development. Typechecking verifies modeled static properties, not arbitrary
business correctness. A new language also begins with a small library and
example corpus. Evaluate claimed benefits using repeatable implementation and
repair tasks, behavioral tests, and human assessment of the resulting code.

## Proposed organizing ideas

1. Records describe data, with fields inferred and checked statically.
2. Scopes govern external resource lifetimes and child tasks.
3. Plans describe application wiring and processing before execution.

These are an organizing proposal, not a claim that the underlying features
are novel. Row polymorphism, typed pipelines, and scoped resources have existing
precedents. The opportunity is to make them fit together coherently.

## Inferred structural records

An unannotated record literal establishes an exact set of field labels and
field types:

```text
let person = { name: "Ada", age: 36, needs_glasses: false }
// inferred: { name: String, age: Int, needs_glasses: Bool }

fn greet(person) {
    "Hello, " + person.name
}
// inferred: forall Rest. { name: String, ..Rest } -> String

fn birthday(person) {
    { ..person, age: person.age + 1 }
}
// inferred: forall Rest. { age: Int, ..Rest } -> { age: Int, ..Rest }
```

Here `Rest` represents the other statically known fields. It is a type variable,
not a runtime map. Field requirements come from function use; complete shapes
come from construction and constraints. A spelling mistake still fails when
the function is called with a record that lacks the required field.

Extension, replacement, removal, and merging should have explicit typing rules.
The draft update syntax replaces an existing label or adds a new one. The result
can have a different type from the input. Labels are unique within a shape; row
constraints must track absence and overlap. General record merge should specify
which side wins, including what happens when overlapping field types differ.

There is no automatic fallback to `Any` when inference fails. Branches and
homogeneous collections need compatible element shapes, explicit optional
fields, or tagged alternatives. A field absent on one branch is not silently
treated as a nullable field.

Field labels should have namespaced identities available for library extension
points. Whether spelling uses symbols, qualified identifiers, or another form
is open. Record field access does not require runtime symbol hashing.

## Named shapes and nominal structs

Most named data can be a structural alias:

```text
type Person = {
    name: String,
    age: Int,
    needs_glasses: Bool,
}

let person: Person = { name: "Ada", age: 36, needs_glasses: false }
```

An alias introduces no new identity or representation. There is no repeated
constructor name. If a distinction from ordinary structural data is needed,
an opt-in nominal declaration could look like:

```text
struct Person {
    name: String,
    age: Int,
    needs_glasses: Bool,
}
```

The alias and struct examples are alternative declarations of `Person`.
Nominal structs would support abstraction, invariants, and representation
contracts. Matching fields do not authorize bypassing private fields or a
validating constructor. A public transparent struct may allow contextual
construction from a record literal.

Record and nominal-struct distinctions, including whether both deserve surface
syntax, remain open. Prefer a single record model for ordinary data.

## Layout and conversion

Proposed native representation: compiler-known shapes, inline numeric fields,
and ordinary typed references for strings or collections. Records do not need
per-value field-name tables or boxed dynamic fields.

The compiler should intern shapes and choose a deterministic internal layout
based on labels, field representations, alignment, and its layout policy.
Literal source order should not affect type identity. Internal layout need not
be a stable external ABI; explicit ABI representations require separate rules.

| Operation | Intended implementation |
|---|---|
| Access a field of a concrete shape | Load at a known offset |
| Call a row-generic function with a known shape | Specialize for that shape; retain unused fields |
| Rename a structural type using an alias | No representation change |
| Convert to a transparent nominal type with compatible layout | Eligible for no field reconstruction, subject to ownership and invariants |
| Produce an owned subset with another layout | Project the fields into a new value; copying/moving may be necessary |
| Read selected fields of a larger shape | Specialized borrowed access; no owned subset needs to be created |
| Turn an existing record into a runtime map | Materialize runtime keys and values as necessary |

Same field names alone are insufficient for a safe representation conversion.
Field types, layout, validity, ownership, and destruction rules matter. Ordinary
by-value passing may still move bytes even when conversion needs no rebuild.
Superset-to-subset coercion should not silently discard fields; prefer explicit
projection when a closed owned value is required.

Specialization trades runtime overhead for compile time and code size. When a
shape cannot be specialized, a generic implementation can receive typed field
offset information rather than hash every field name. Separate compilation,
first-class polymorphic functions, and heterogeneous runtime storage need an
explicit representation strategy; specialization does not solve these alone.

## Records, maps, and dynamic data

The middle ground is three distinct capabilities with clear boundaries:

- Records: a fixed set of fields, possibly different types, known to the compiler.
- `Map<K, V>`: runtime-variable keys with one statically known value type.
- Explicit dynamic data, such as `Data`: heterogeneous values inspected and
  decoded at runtime, useful for JSON, configuration, and plugin boundaries.

Brace syntax can be contextual: identifier-label literals default to records,
and an expected map type can select map construction. Computed-key syntax needs
to be unambiguous. A heterogeneous literal should not silently become
`Map<Symbol, Any>`; an expected `Map<Symbol, Data>` would explicitly require
conversion of each supported field into `Data`.

Literal construction can target the expected representation directly. Conversion
of an existing runtime value is a different operation and may have a cost.
Decoding external dynamic data to a record must validate it at runtime.

Reflection for encoding, inspection, and traversal can be derived from static
shapes. Returning heterogeneous fields in a homogeneous sequence requires an
explicit union/dynamic representation or a typed visitor; reflection is not a
reason to box every stored record field.

## Integrant-inspired lifecycle management

The reference model is configuration descriptions containing references to
other entries. Initializers produce arbitrary values: functions, pools,
channels, or records. Dependencies are between those values, rather than between
mandatory component objects.

Proposed blessed module: `std.system`, built on general resource scopes.
A provider describes its resolved configuration type, produced value type,
initializer, and optional halt/suspend/resume behavior.

Illustrative configuration:

```text
let plan = system.plan({
    database: resource(db.pool, { url: config.database_url }),
    handler: value(api.routes, { database: ref(:database) }),
    server: resource(http.server, {
        port: 8080,
        handler: ref(:handler),
    }),
})

use running <- system.run(plan)
running.server.wait()
```

This syntax is aspirational. Typed references and heterogeneous provider entries
must be supported deliberately; putting them in a record is not enough to make
all graph relationships typecheck automatically. Static key witnesses and
compiler-derived shape information are candidate mechanisms.

For statically described graphs, check reference types, missing dependencies,
and detectable cycles before startup. For graphs assembled from external data,
decode and validate before acquiring resources. Dynamic provider registries
must package type information and report runtime failures at their boundary.

Start in dependency order and halt in reverse order. Proposed improvements:
clean up successfully started resources after a later startup failure, attempt
remaining cleanup after a halt error, and report all cleanup failures. Partial
acquisition inside a failing initializer remains that initializer's obligation,
assisted by nested resource scopes.

Scopes also govern access: resource-bearing running values must not remain
usable after their owning scope closes. How the type system enforces this is an
open memory/lifetime decision. Halt and suspend are different: suspend must not
destroy a value that resume is expected to reuse.

Keep graph traversal and provider policy in the standard library. Reserve core
language support for general-purpose scoped cleanup, task cancellation,
continuation binding, and any necessary typed shape reflection.

## Interceptors with typed contexts

Keep Ring's request/response adapter contract, but make interceptor chains the
primary processing description. Interceptors have optional enter, leave, and
error behavior; execution tracks the entered stack and unwinds it in reverse.
Specify short-circuit, cancellation, failure, and recovery behavior explicitly.

Static composition can change context shape:

```text
{ request, ..Rest }
  -> { request, user: User, ..Rest }
  -> { request, user: User, response: Response, ..Rest }
```

A handler requiring `user` must follow a step that guarantees it on the success
path. Authentication failure takes an explicit stop/error branch. Reverse leave
and error callbacks cannot assume later fields exist after early termination;
the type contract must express what each callback can actually receive.
Cancellation must unwind entered scopes and finish required cleanup.

Such a chain is not a homogeneous `List<fn(Context) -> Context>` when each stage
changes its type. It needs a typed composition builder or heterogeneous plan
representation. Inspectable descriptions can then compile into specialized
control flow while retaining diagnostic names and source locations.

Dynamic insertion should initially be limited to preserving an established
context contract, or use an explicit checked/dynamic extension boundary.
Arbitrary runtime rewriting and unrestricted static shape changes cannot both
be assumed free.

### Interceptors around lifecycle-managed components

A component provider acquires a running value. Its exported operations can use
interceptor chains for validation, authorization, tracing, and deadline policy.
Lifecycle operations can also have tracing or validation hooks, but acquisition
and cleanup must remain governed by resource scopes. An operation interceptor
does not own the dependency graph or make restart and retry safe automatically.

Distinguish halt/resume from supervision of ongoing task failures. Integrant-style
startup ordering alone does not give running processes supervision semantics.
Retries require an operation-specific safety contract; a generic wrapper cannot
guarantee that repeating an external side effect is safe.

## Task execution

Accepted: sequential execution within each task, concurrent tasks throughout
the application, and optional multicore execution underneath. Make inexpensive
runtime-managed tasks a core language/runtime facility.
Concurrency describes independent ongoing work; parallelism describes running
that work simultaneously on multiple CPU cores. Many tasks can run on one
scheduler thread. The language should support that execution mode without
making single-thread execution its permanent concurrency contract.

The detailed lifetime, communication, and scheduling rules below are proposals:

- A task executes sequential code and produces a typed result or failure.
- Tasks belong to a scope; parent shutdown cancels children and waits for
  required cleanup. Cancellation is observed at defined safe points, not by
  abruptly destroying an arbitrary running native stack.
- Standard I/O, channel operations, and timers suspend without blocking a
  scheduler worker. Blocking foreign calls use a separate bounded worker pool.
- Communication uses typed bounded channels, including rendezvous channels and
  selection across operations. Queue limits and overload policy are explicit.
- Mutable state is confined to its owning task or accessed through an explicit
  synchronized abstraction. A channel payload must be safely transferable:
  immutable reachable data can be shared, or exclusive ownership can be moved.
  An immutable record containing a mutable reference is not automatically safe.
- Aim for fair scheduling through execution budgets checked at compiler safe
  points, including long loops and recursive calls. This requires a resumable
  representation and cannot be obtained simply by calling Cranelift.
- Permit one or several scheduler workers without changing these semantics.
  First implement one worker, then add multicore scheduling with the same
  confinement and transfer rules. Single-worker execution is still interleaved
  and is not inherently deterministic.

Illustrative surface API:

```text
use scope <- task.scope()
let jobs = channel<Job>(capacity: 32)

scope.spawn(fn() { accept_jobs(jobs.sender) })
scope.spawn(fn() { process_jobs(jobs.receiver) })
scope.wait()
```

Syntax and suspension representation remain open. Stackful fibers and
compiler-lowered resumable frames have different memory, debugging, and FFI
tradeoffs. In either case, account for captured values, queues, and stacks when
measuring task cost. "Lightweight" is a measured goal, not a promise of a fixed
tiny allocation or a million useful tasks on arbitrary hardware.

The desired surface is ordinary sequential functions with inferred suspension
behavior. Validate whether this can preserve simple call sites and precise
effects without imposing avoidable async/sync variants on library APIs.

The first scheduler may yield only at explicit suspension operations while
resumable execution is established. That is a prototype limitation: a task
performing a long native computation can monopolize a worker until additional
safe-point scheduling is implemented. Compiler-generated scheduling checks
must preserve cleanup, stack unwinding, and task-local state invariants.
Opaque foreign code cannot be given the same fairness guarantee. Cancelling a
caller does not necessarily stop a foreign operation already running; its
resources must remain valid until that operation finishes or is isolated.

### Tasks, components, interceptors, and supervision

Tasks are the execution primitive; actor-like services can be a blessed pattern
that owns a task, private state, and a typed inbox. Ordinary functions and
resource-only components do not need a task or a message queue of their own.

- Components acquire resources and establish application dependencies.
- Interceptors shape operation execution and policy within a task.
- Tasks provide concurrent activity and scoped lifetimes.
- Supervisors observe ongoing failures and apply bounded restart policies.

A supervisor should be owned by the application's lifecycle scope. Expected
operation errors need not crash a service. Restart policies must specify which
state is lost, how queues are treated, and whether dependents also restart.
Dependents must not silently retain invalid handles after a resource restart;
use stable typed handles with explicit availability/generation semantics, or
restart the dependent closure in lifecycle order.

Task failure containment covers modeled language failures, not arbitrary native
memory corruption or process-wide resource exhaustion. Supervision does not
promise durable job recovery or exactly-once external effects. Those require
separate storage and protocol contracts.

### Short operations and long-running activities

Use the same task primitive at different lifetimes rather than introduce a
second low-level execution primitive for tasklets versus actors.

For a database-backed request, propose one request scope and ordinary direct
function calls through its interceptors and domain logic. It may suspend while
waiting for I/O. Acquire a transaction within that scope; the transaction helper
commits after successful completion and rolls back on failure or cancellation
where the database protocol permits it. Do not move transaction work to other
tasks by default or allow its scoped handle to escape. Any deliberate parallel
work on one transaction must respect the driver's access constraints.

```text
fn create_order(request, services) {
    let input = decode<OrderInput>(request.body)?
    use tx <- db.transaction(services.database)
    let order = orders.insert(tx, input)?
    ok({ status: 201, body: order })
}
```

The transaction call owns commit/rollback around the callback introduced by
`use`, so a successful callback result is not itself proof of a successful
commit. The lowercase `ok` helper/constructor is illustrative; result syntax
remains open. Avoid automatically restarting or replaying failed requests.
Commit uncertainty and
external side effects require application-level idempotency and recovery rules.

Use long-running tasks where a stable activity or a single owner of state helps:
queue consumers, connection managers, schedulers, subscription streams, and
in-memory state coordinators. A consumer may admit many concurrent child jobs;
it need not serialize all job execution behind a single inbox loop. Bound both
queued work and active concurrency.

Request scopes should be siblings under a server/service scope. Failure of one
request does not automatically cancel unrelated requests. Inside a request,
unhandled child failures can cancel sibling work according to its task-group
policy. Expected errors remain ordinary typed results.

### Local communication without deep payload copying

Proposed goal: avoid deep-copying safely transferable payloads within one
runtime process. Distinguish physical payload storage from who is allowed to
read, mutate, or destroy it.

| Payload | Proposed behavior |
|---|---|
| Small inline record | Move/copy its inline representation into a channel slot |
| Transitively immutable heap-backed data | Share a managed reference; no payload duplication |
| Exclusively owned mutable buffer or graph | Transfer ownership; sender loses usable aliases |
| Ordinary mutable state with live aliases | Reject transfer or require an explicit synchronized interface |
| Borrow into task-local scratch storage | Reject escape unless a checked scoped lifetime permits it |

This does not mean literally zero bytes move: envelopes, inline fields,
pointers, queue bookkeeping, synchronization, and wakeups still have costs.
Large inline records may benefit from an explicit owned allocation rather than
forcing indirection on every small record. Remote-process and network transport
have separate serialization and lifetime contracts.

For heap-backed data intended to cross tasks, propose stable allocations whose
lifetime is managed independently of the creating task. A process-wide managed
heap with thread-local allocation caches is one candidate. Shared allocation
space does not grant shared mutable access. Message roots held by a channel or
receiver must survive sender failure and release exactly once when no longer
needed. Atomic reference counting is a candidate for immutable shared values;
collection of cycles and allocator details remain unresolved.

For unique ownership, a builder can create a transferable buffer directly in
eligible storage and then hand it off without relocating its payload. Freezing
such a builder may instead make it immutable and shareable if the compiler can
prove there are no surviving mutable aliases. No general zero-copy promotion
from an arbitrary task-local arena is promised: backing storage lifetime and
all reachable references must be compatible with transfer.

Propose a channel capacity reservation API for large producers: reserve capacity,
construct the message, then commit ownership to the channel. Dropping an unused
reservation releases its slot; a failed or cancelled pre-commit send leaves the
value with the caller or returns it. After commit, the channel is responsible
for the message even if the sender stops. Successful enqueue does not prove
that a receiver processed the message. Queue disposal, receiver failure, and
durable delivery remain explicit policies. Slot count alone does not bound
payload bytes; large-message paths also need a byte budget or size limit.

Normal immutable records should get safe sharing with minimal annotation.
Exclusive mutable transfers need a visible ownership contract when inference
cannot express it clearly. Pony's immutable/isolated capabilities and Rust's
shared ownership are relevant precedents; importing their complete type-system
surface is not an accepted decision.

### Proposed batching at scheduling boundaries

Explore a typed channel whose producer appends messages into an exclusively
owned page chain. At a scheduling boundary, seal the current chain, publish its
descriptor to the receiver, and acquire a fresh chain from a pool. Filled pages
can also be published before the task yields. Published element storage does
not need to be copied into another queue.

The append fast path can use ordinary local writes when it already owns a page
and reserved capacity. Multicore publication still requires synchronization,
receiver notification, and a correct wakeup protocol. Allocation, credit
reservation, and reclamation may also synchronize. Batch publication amortizes
these costs; it does not remove them.

Logical producer ownership must be distinct from physical thread locality.
Thread-local page caches are an implementation option. A task can migrate
between workers, so a thread-local pointer cannot silently become its persistent
message state. Flush/detach old-worker state before migration or carry a uniquely
owned producer state explicitly. A worker-local producer buffer is also possible
if every task switch seals it and attribution/order rules are retained.

Yielding is a convenient ownership boundary, not proof that all aliases into
the old pages have disappeared. Public APIs must prevent retained mutable
aliases to published data. Pages remain alive while a receiver or an escaped
read-only view retains them, and return to the pool only after those references
are gone. Reuse must destroy each initialized owned element exactly once; raw
pointer stability alone does not provide lifetime safety.

Proposed semantic distinction, still open:

- `Channel<T>` has prompt publication on a successful send, with batching only
  where its contract permits it.
- `BatchChannel<T>` accepts messages into a pending batch, with receiver
  visibility established by publication. It flushes on scheduling out, on page
  or batch limits, before parking on dependent work or capacity, on explicit
  flush, and during normal exit or modeled task failure. A configured latency
  policy can request flushing at additional scheduler safe points.

For the proposed batch contract, an accepted append transfers ownership to
runtime-managed pending storage even before publication. Accepted messages
must not disappear merely because the sender stops; channel closure/disposal
has a defined policy. A failed append retains or returns ownership to the caller.
Capacity reservations not yet accepted can still be cancelled normally.
This ownership contract differs from an explicit uncommitted batch builder.

Specify whether `flush` only publishes or also waits for downstream processing;
prefer publication only, with processing acknowledgments as a separate operation.
Do not equate publication with a database transaction or durable delivery.

Capacity includes staged and published messages, not only visible batches.
Flush staged data before waiting for more credits. Enforce message/byte budgets
and fair draining across producers. Two buffers are sufficient only if the
consumer releases one before the producer needs another; sustained imbalance
requires a bounded pool and backpressure. Preserve FIFO within a logical
producer; do not imply a total cross-producer order without specifying and
implementing its additional synchronization.

Prototype publication, receive registration, and parking together to rule out
lost notifications and send-then-wait deadlocks. A producer that never suspends
requires size limits and scheduling safe points for bounded staging latency.
Buffered visibility also interacts with nonblocking receive and external effects;
it cannot silently replace a prompt-publication channel everywhere.

### Minimal supervision surface

Start with resource/task scopes and opt-in restart policy on managed worker
definitions. Do not require a manually assembled supervisor tree for a small
service. The runtime can derive a supervision hierarchy from owning scopes.

Policy needs a factory that can create a fresh worker, a restart budget,
backoff, shutdown behavior, and an escalation destination. It must identify
worker-local state and resources that are rebuilt versus dependencies supplied
by the enclosing system. Begin with restarting one independent worker or
escalating failure; add group restart behavior only for demonstrated needs.

Keep the component dependency graph distinct from the failure/lifetime scope
hierarchy. Shared dependencies, restart ordering, and resource generations are
not solved by making those two graphs identical. Persistent input can make
worker recovery useful; restarting does not recover an in-memory message that
was already removed from its queue.

## Sequences and memory

Use a typed sequence/iterator abstraction across collections for familiar
`map`, `filter`, `fold`, `take`, and grouping operations. Lazy views should have
defined evaluation and resource behavior. Make collection into a concrete
container explicit. Try to specialize/fuse pipelines without promising that
every composition is allocation-free. Suspended streams add cancellation and
backpressure to the model rather than disguising them as ordinary pure lists.

Memory policy is unresolved. Native records and static types do not require a
particular choice between ownership, reference counting, tracing collection,
or regions. Functional update semantics can coexist with reuse of uniquely
owned storage. Any reuse must preserve observable aliases; it is not permission
to mutate a shared old value. Cycles, task sharing, and allocation predictability
need to be addressed before committing to a collector or ownership model.

### Proposed paged list and contiguous buffers

Explore an append-oriented paged representation for `List<T>` and a unique
builder for construction. The representation is also called a segmented or
chunked vector. Fixed-capacity chunks are allocated independently, possibly
from a pool keyed by capacity and alignment. Growing the list adds a chunk
without relocating existing elements. A directory of chunk pointers may itself
grow or copy metadata without moving the element storage.

| Candidate type | Intended use |
|---|---|
| `List<T>` | A paged sequence optimized for building and iterating |
| `ListBuilder<T>` | Unique append access; freezing transfers storage into a list |
| `Buffer<T>` | Contiguous growable storage, permitted to relocate on growth |
| `Array<T>` | Contiguous storage with fixed length; static extent syntax remains open |

These names and literal defaults are proposals, not final declarations. Start
by benchmarking paged lists as the ordinary collection/`collect` result rather
than assuming they dominate small or known-size contiguous workloads.

For equal-capacity pages holding `page_size` elements and a page directory:

```text
page = index / page_size
slot = index % page_size
element = pages[page][slot]
```

This is constant-time access with an extra indirection. Linked page chains
instead require traversal; irregular chunk sizes need a different index
strategy. Restrict partial pages to a tail for the simple indexing rule.
A page-and-slot handle is also possible but must obey collection lifetime and
reuse rules. It is not automatically a permanently valid raw pointer.

Iteration should expose an ordinary element iterator and an optional chunk
view. Both can use a nested page/item loop internally without making callers
write it. `map`, `filter`, `fold`, and codecs can specialize for chunk iteration;
FFI requiring one contiguous span needs `Buffer<T>`, scatter/gather support, or
an explicit flattening copy.

The precise guarantee is no relocation of existing elements during append
growth. It is not no copying in every operation. Sorting, middle insertion,
flattening, snapshot updates, and conversion may move/copy elements. Initially
avoid middle insertion/removal in the stable append interface; use collection
views or builders for transformations. Bounds and element lifetimes still
apply when pages are reclaimed.

Immutable snapshots require their own rules. A unique builder can freeze its
pages without element copies. Appending to an aliased immutable list may share
complete pages and copy a partial tail, or use another representation; do not
advertise mutable append semantics as persistent value semantics for free.
Likewise, appending an existing value into a page can move its inline fields.
True construction without that initial move requires an in-place slot/builder
API that safely tracks initialization and cancellation.

Benchmark tiny lists, large inline records, large heap-backed values, sparse
tails, page allocation/reuse, chunk iteration, indexed access, and message
latency. Small-page policy and pool retention can matter more than eliminating
a growth copy. Page reuse should be bounded and release unused memory rather
than permanently retaining peak workload allocations.

## Development and implementation boundaries

Dropping shadow-cljs as a target does not preclude a native REPL, incremental
checks, source inspection, or controlled hot replacement. Cranelift emits code;
definition tracking and resource-safe reload are separate compiler/runtime work.

Keep Gleam-style `use` as general continuation-binding syntax: the remainder of
the block becomes a callback supplied to a function. Resource helpers can use
that syntax, but cleanup and non-escape guarantees must come from their scoped
contracts, not from the keyword alone. Do not conflate arbitrary repeated
callbacks with one-shot resource scope continuations.

Before implementing lifecycle syntax, prototype these semantic tests:

1. Infer a field-reading function and a row-preserving update function.
2. Specialize each for two layouts with different offsets and field types.
3. Distinguish a same-layout construction from an owned subset projection.
4. Typecheck a two-stage shape-changing interceptor chain, including early exit.
5. Resolve a three-node lifecycle graph; demonstrate startup failure cleanup.

## References

- Integrant: https://github.com/weavejester/integrant
- Pedestal interceptor semantics: https://pedestal.io/pedestal/0.7/reference/interceptors.html
- Sieppari: https://github.com/metosin/sieppari
- Flix extensible records: https://doc.flix.dev/records.html
- PureScript row operations: https://pursuit.purescript.org/builtins/docs/Prim.Row
- Gleam use: https://tour.gleam.run/advanced-features/use/
- Perceus reference counting and reuse: https://www.microsoft.com/en-us/research/publication/perceus-garbage-free-reference-counting-with-reuse/
- Erlang/OTP supervision design: https://www.erlang.org/docs/27/system/design_principles.html
- Evaluation of coding agents: https://www.anthropic.com/engineering/demystifying-evals-for-ai-agents
- Erlang process costs and scheduler threads: https://www.erlang.org/doc/system/eff_guide_processes.html
- Runtime tasks and cooperative scheduling: https://docs.rs/tokio/latest/tokio/task/
- Tasks on a single thread: https://docs.rs/tokio/latest/tokio/task/struct.LocalSet.html
- Blocking operation cancellation limits: https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html
- Safe sharing and isolated data: https://tutorial.ponylang.io/reference-capabilities/reference-capabilities.html
- Shared allocation lifetime and reference counting: https://doc.rust-lang.org/std/sync/struct.Arc.html
- Bounded channels and capacity permits: https://docs.rs/tokio/latest/tokio/sync/mpsc/
- Supervisor restart policies and budgets: https://www.erlang.org/doc/system/sup_princ.html
- Segmented vectors and stable append storage: https://docs.rs/segvec/latest/segvec/
- Split vectors and constant-time indexed growth strategies: https://docs.rs/orx-split-vec/latest/orx_split_vec/
- Concurrent append-only vector precedent: https://docs.rs/boxcar/latest/boxcar/
- Buffer swapping queue precedent: https://docs.rs/swap-buffer-queue/latest/swap_buffer_queue/
- Publication memory ordering: https://doc.rust-lang.org/std/sync/atomic/enum.Ordering.html
