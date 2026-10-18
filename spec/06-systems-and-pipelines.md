# Systems, resources, and typed interceptor pipelines

Status: draft 0, implemented by this repository's toolchain except where marked
**Proposed:**. See [the specification index](../SPEC.md) for how to read it.

## Core support and standard policy

The language supports typed records, scope brands, `use`, and the compiler-checked
description forms `system { ... }` and `pipeline { ... }`. The standard library
owns provider execution, lifecycle policy, HTTP adapters, structured events
(`std.trace`), and worker restart behavior. These are first-class inspectable descriptions, not general
macros or opaque framework annotations. Their compiler hooks are explicit parts
of the specification.

Resource providers return arbitrary typed values: pools, handles, functions,
records of functions, channels, or worker controllers. No component base class
or mandatory component object is required.

## Typed system descriptions

```badger fragment
let plan = system {
    database: sqlite.pool({ path: config.database_path }),
    api: api.provider({ database: ref(#database) }),
    server: http.server({
        address: "127.0.0.1",
        port: 8080,
        handler: ref(#api),
    }),
}

use running <- system.run(plan)
running.wait_for_shutdown()
```

Plan keys are unique static labels, including qualified symbol labels. `ref`
is valid only inside a system configuration and identifies another plan key.
It may occur inside nested records/lists. It is a typed dependency expression,
not a runtime symbol lookup that can produce arbitrary `Any`.

An entry `key: provider(config)` applies a provider to a deferred typed
configuration (`provider()` has the empty configuration `{}`); it is the
same as `key: component(provider, config)`, the explicit marker. Inside
`system { ... }` an application does not run anything: it describes a
node. Compiler elaboration first collects nodes and their output
schemes, resolves reference holes, checks the resolved config against each
provider, and rejects missing references, invalid fields, and dependency cycles.
It then produces a normal inspectable `SystemPlan` value. Generic providers must
yield a concrete output at each node. Reference resolution never executes an
initializer to discover its type.

A provider's output may be indexed by a fresh lifetime region introduced when
the system runs. Dependent configurations see the same region-consistent
handle. The output row is a type-level function of that system region. A
provider cannot return an unbranded dangling raw handle and call it a resource.

`system.run` initializes the dependency closure in order inside child scopes.
Draft 0 uses deterministic sequential startup, with source label order breaking
ties. **Proposed:** parallel startup as an explicit policy. If startup fails,
already initialized nodes halt in reverse order. A failing initializer's own
scope releases partial acquisitions. Cleanup attempts continue after errors,
retaining the initial failure and attaching cleanup failures.

The runtime closes admission, settles/cancels owned tasks according to shutdown
policy, then halts resources in reverse dependency order. Task/resource cleanup
are coordinated; shutdown cannot free a database still used by child tasks.
Callback-scoped running values cannot escape `system.run`.

## Provider contracts and configuration

A provider has a typed configuration schema and an acquisition operation run
with an initialization scope. It may register halt behavior there. Pure value
providers register no cleanup. Retaining a resource across reload must be
explicit. **Proposed:** suspend/resume hooks, distinct from halt/init.
Provider IDs and source locations are retained for inspection and diagnostics.

The initializer's `InitScope<S>` has these checked operations:

- `scope.acquire(provider, config)` acquires/adopts a nested resource whose
  handle is valid until this scope closes, registering its cleanup.
- `scope.on_halt(callback)` registers a one-shot, possibly fallible cleanup
  callback. It may capture this scope's resources; it cannot outlive the scope.
- `scope.worker(factory, policy)` starts a managed activity owned by the node,
  with its declared restart and shutdown policy.

Cleanup callbacks run in reverse registration order after owned activities have
settled. Fallible provider errors must implement `Error` and must not carry a
resource from the closing region; system diagnostics retain an explicit erased
error interface/schema rather than erasing arbitrary application values.
Adapters for raw foreign resources use an explicit owned-resource adoption
contract. General safe code cannot manufacture an untracked scoped handle.

### Provider declarations

```badger fragment
provider greeting(scope, config: { prefix: String }) -> Result<Greeter, AppError> {
    scope.on_halt(fn() -> Result<Unit, AppError> { ok(()) })
    ok({ greet: fn(name: String) { $"{config.prefix}, {name}" } })
}
```

`provider NAME(scope, config) -> R { body }` declares the module value
`NAME = system.provider({ name: #MODULE/NAME, init: fn(scope, config) -> R { body } })`:
the body is the initializer, run with the node's initialization scope and
its configuration. A provider that registers nothing on its scope may
write `(config)`. `provider NAME as #label(...)` names it explicitly; the
default name is the module path and the declaration name
(`#orders.store/database`), shown by inspection and in diagnostics.
`return` and `?` in the body leave the initializer. The result annotation
and parameter types are inferred when omitted; a `pub provider` declares
its configuration type and `-> Result<Output, Error>`, and its type is
`system.Provider<Config, Output, Error>`. `requires` / `ensures` lines
before a provider are its initializer's contract (over `scope`, `config`,
and `result`): they are checked when a node starts, and each node's
configuration in a `system { }` is checked against the preconditions when
the program is checked. `provider` is a keyword only where
a declaration starts. The descriptor form `system.provider({...})` remains
for providers built by functions (generic providers, providers chosen from
data).

Configurations are ordinary typed values after validation. Profiles and
environment overlays merge explicit configuration data, then decode/validate
before acquisition. There are no hidden environment reads during imports.
**Proposed:** selecting among explicitly registered providers from external
data. Dynamic provider choice would use a typed output interface and runtime
graph/schema validation; it could not introduce a new statically unknown
output shape.

`system.select(plan, keys)` takes the dependency closure of selected roots.
Replacing a provider for tests preserves its declared configuration/output
contract. A changed output contract requires rechecking dependents. This is
the canonical dependency-injection mechanism; no ambient service locator.
Implementation: a plan's type is `SystemPlan<Os, Cs>`, its nodes' outputs
and providers' configurations by key (`Cs` may be elided in annotations),
so `system.replace(plan, #key, provider)` checks both the replacement's
output and configuration types when the program is checked.

## The user's prefix-visible context rule

Stage N can see fields established by its own successful enter and by earlier
successful stages. Its leave/error callback sees the **latest values** of those
fields, including updates made later. It cannot inspect fields introduced only
by later stages. This is a live typed view, not an earlier record snapshot.

Example forward prefixes:

```text
C0 = {request: Request, database: Pool<S>}
C1 = {request: Request, database: Pool<S>, started: Instant}
C2 = {request: Request, database: Pool<S>, started: Instant, user: User}
C3 = {request: Request, database: Pool<S>, started: Instant, user: User, order: Order}
```

The C2 stage's unwind callbacks can read `user` after a C3 operation updated its
value, but cannot read `order`. The C1 stage cannot read `user`. Earlier fields
are inherited by identity and type, even though physical layouts differ.

To make this sound, enter steps may add fields and replace existing values,
but may not remove or change the type of a field visible to an earlier stage.
General record expressions outside pipelines retain type-changing updates.
Inside a chain, put a new decoded representation under a new label instead of
changing the established raw field's type. Namespaced labels avoid collisions.
Nested record field types also remain stable; widening a nested record is a
type change unless its declared type already permits the operation.

## Stage API and reply channel

A stage description has a name and optional `enter`, `leave`, and `error`
callbacks. Normalize an omitted enter to identity and omitted unwind callbacks
to forwarding. A terminal handler is required. Replies travel separately from
context fields, so earlier stages can always observe/transform a reply without
pretending that a later-added `response` field existed on their input.

```badger fragment
let plan = pipeline {
    tracing(),
    authentication(database),
    handle(create_order),
}
let handler = pipeline.compile(plan)
```

`handle` marks a terminal function of `FinalContext -> Result<Reply, E>`.
The compiled pipeline has `InputContext -> Result<Reply, E>`. All stages agree
on the reply and error types through inference or direct adapters. A chain is
not a homogeneous list: each enter can establish a new context shape.
Implementation: `pipeline.map_handler`, `map_reply`, and `map_error` adapt a
handler's reply and error types; `pipeline.adapt` adapts a whole plan and
`pipeline.nest` runs one pipeline as another's handler.

The helper signatures, expressed schematically, are:

```text
enter: In -> Result<Enter<In, Out, Reply>, E>
leave: (LiveView<Out>, Reply) -> Result<(LiveView<Out>, Reply), E>
error: (LiveView<Out>, E) -> ErrorStep<LiveView<Out>, Reply, E>

Enter = next(Out) | stop(In, Reply)
ErrorStep = propagate(Context, E) | recover(Context, Reply)
```

### Stage declarations

```badger fragment
stage decode_order() -> Result<_, AppError> {
    enter(ctx) {
        let input = codec.decode<OrderInput>(ctx.request.body)?
        pipeline.next({ ..ctx, input })
    }
    error(ctx, error) => pipeline.propagate(ctx, error)
}
```

`stage NAME(params) { callbacks }` declares the stage factory
`fn NAME(params) { pipeline.stage({ name: #MODULE/NAME, enter: ..., leave: ..., error: ... }) }`;
a pipeline uses it as `pipeline { NAME(args), ... }`. Each callback
(`enter(ctx)`, `leave(ctx, reply)`, `error(ctx, error)`) is written at most
once, with a block or `=> expression` body, and at least one is present;
callbacks capture the factory's parameters. A callback may annotate its own
result (`enter(ctx) -> Result<_, E> { ... }`), which takes precedence over
the stage's. The optional `-> Result<_, E>`
names the stage's error type once: it annotates the results of `enter` and
`leave` and the type of `error`'s error parameter. `as #label` names the
stage explicitly, as for providers. `requires` / `ensures` lines before a
stage are its factory's contract. A `pub stage` declares its parameter
types; its `pipeline.Stage` type follows from its callbacks. `stage` is a
keyword only where a declaration starts, and `enter`, `leave`, and `error`
only inside one.

`pipeline.next(ctx)` creates successful continuation;
`pipeline.stop(ctx, reply)` creates successful early completion;
`pipeline.leave(ctx, reply)` creates a successful unwind pair.
`pipeline.propagate(ctx, error)` and `pipeline.recover(ctx, reply)` construct
error decisions. `error` is a decision callback rather than a `Result` function;
fallible work inside it is explicitly matched and propagated/recovered.

## Entry, unwind, and early failure

An enter step commits its context and joins the entered stack **only after**
returning `next(Out)`. If its enter fails, its own newly added fields do not
exist, so its own error callback is not eligible. Earlier entered stages handle
the error using their known prefixes. Its temporary resource scope still runs
cleanup. An early `stop(In, Reply)` similarly unwinds earlier stages without
pretending that the stopping stage successfully established `Out`.

The terminal handler receives the final entered context by a checked view. It
may mutate established field *values* through an explicit context-update
result; a plain `handle` returns only a reply and makes no context changes.
`handle_context` instead returns `Result<(FinalContext, Reply), E>` and checks
the same field/type preservation rules. Context updates before an enter or
handler returns are committed only with a successful returned context; external
side effects are not transactionally rolled back by this rule.

On success, pop entered stages in reverse order and call `leave` with the latest
context view and reply. A leave can update its visible fields at the same types
and return an updated reply. Those writes are visible to earlier callbacks;
unknown later fields are preserved. If leave returns an error, the already
popped stage is not called again; error processing continues with earlier stages.

On an expected error, pop entered stages in reverse order and call `error`.
`propagate` keeps unwinding in error mode with its updated visible fields.
`recover` supplies a reply and resumes normal leave processing at the next
earlier stage. Each popped stage runs at most one unwind callback. No leave is
run twice. Task defects and cancellation skip ordinary error recovery but run
all scope-registered resource cleanup and report a trace of the aborted chain.

Resources acquired during a callback can use the execution scope, explicitly
passed in the input context when needed, to outlive enter. Callback-local `use`
resources cannot escape into later stages. Transaction wrappers normally use
a surrounding scoped helper, rather than smuggling a closed transaction out
of an enter callback.

## How a live prefix view typechecks and lowers

`LiveView<Prefix>` is explanatory notation, not a dynamic field bag. Each
unwind callback is checked as universally preserving an opaque later row:

```text
forall Later: Row.
({..Prefix, ..Later}, Reply) -> Result<({..Prefix, ..Later}, Reply), E>
```

The checker makes the tail rigid: field accesses must come from `Prefix`.
Callbacks cannot refine the hidden tail by demanding a later label. Updates
may change only visible field values, preserve their types, and preserve the
unseen tail. A generic spread cannot be used to smuggle a new label into that
tail. This universal-row contract is inferred/checked by the pipeline form;
users need not spell the schematic two-spread type, which is not ordinary
record-type syntax.

Lower a known chain to specialized control flow for its possible success and
early-exit shapes, or use a typed offset/patch dictionary. A live prefix is not
a pointer cast to a physically contiguous prefix: record layouts may interleave
fields. Passing a projected copy and discarding its writes would violate the
specified live semantics. Context field liveness can eliminate unused values;
the abstraction does not require one universal dynamic context map.

Runtime-selected stages must preserve an established context contract. They
may carry private existential state through a checked adapter, but cannot
introduce new statically visible fields into an unknown chain. Conditional
static chains require compatible branch contracts or explicit sum types.

Precedents: [Integrant's data-driven lifecycle](https://github.com/weavejester/integrant)
and [Pedestal's enter/leave/error traversal](https://pedestal.io/pedestal/0.7/reference/interceptors.html).
Prefix-visible live views are this language's own design.
