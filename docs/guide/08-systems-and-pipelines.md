# Systems and pipelines

A `system { ... }` describes an application's components and their dependencies; the compiler checks it and `system.run` starts and halts it in order.
A `pipeline { ... }` composes typed interceptor stages around a handler.

## Describe a system

```badger
import std.io
import std.system

@derive(Error, From)
pub enum AppError {
    io(source: io.Error),
    system(source: system.Error),
}

fn say(line: String) -> Unit {
    let _ = io.println(line)
}

provider store(scope, config: { prefix: String }) -> Result<_, AppError> {
    say("store: start")
    scope.on_halt(fn() -> Result<Unit, AppError> {
        say("store: halt")
        ok(())
    })
    ok({ key: fn(id: Int) { $"{config.prefix}-{id}" } })
}

provider api(scope, config) -> Result<_, AppError> {
    say("api: start")
    scope.on_halt(fn() -> Result<Unit, AppError> {
        say("api: halt")
        ok(())
    })
    ok({ show: fn(id: Int) { $"item {config.store.key(id)}" } })
}

pub fn main() -> Result<Unit, AppError> {
    let plan = system {
        api: api({ store: ref(#store) }),
        store: store({ prefix: "order" }),
    }
    say($"start order: {plan.labels()}")
    use running <- system.run(plan)
    io.println(running.api.show(7))?
    ok(())
}
```

```text
start order: [#store, #api]
store: start
api: start
item order-7
api: halt
store: halt
```

- Each entry applies a provider to its config: `key: provider(config)`.
  Nothing runs yet: the entry describes a node. (`component(provider,
  config)` is the same, written out.)
- `ref(#key)` inside a config is another node's output, typed. Here
  `config.store` in `api` is the record `store` returned.
- Startup follows dependencies; source order breaks ties. Halting runs in
  reverse.
- `system.run(plan, body)` gives `body` the running system: each node's
  output by key (`running.api`), plus `running.started` and
  `running.wait_for_shutdown()`.
- A component's interface is whatever its provider returns. A record of
  functions is common; there is no component base type.

The `system { }` form is checked before anything runs:

| Code | Mistake |
|---|---|
| E0601 | `ref(#key)` names no key of the plan |
| E0602 | `ref` used outside a system configuration |
| E0603 | An entry is not a provider application `key: provider(config)` |
| E0604 | Configurations reference each other in a cycle (the path is reported) |
| E0605 | `system.select` given keys that are not a literal list |
| E0606 | A generic provider's output is not concrete at a node |

## Providers

`provider NAME(scope, config) -> Result<Output, E> { ... }` declares a
provider. Its body is the initializer: it receives the node's
initialization scope and its resolved config, and returns the node's
output. `E` must implement `Error` and must not hold scoped values (E0411);
report failures as plain data.

- Write `(config)` when the initializer registers nothing on its scope.
- The name in diagnostics and inspection is `#MODULE/NAME`; `provider NAME
  as #app/name(...)` sets it.
- A `pub provider` declares its config type and `-> Result<Output, E>`.
- `requires` / `ensures` lines before a provider (or a stage) are its
  contract; each node's config is checked against it
  ([chapter 9](09-testing-and-contracts.md#contracts-on-providers-and-stages)).
- `system.provider({ name, init })` builds the same provider as a value,
  for providers made by functions.

| `InitScope` method | Effect |
|---|---|
| `scope.on_halt(f)` | Register cleanup; halts run in reverse registration order, once each |
| `scope.on_stop(f)` | Run `f` as the system starts halting, before any worker stops (stop taking work) |
| `scope.worker(factory, policy)` | Start a managed worker owned by the node |
| `scope.worker_with(name, factory, policy, shutdown)` | The same, with a name and a shutdown policy |
| `scope.acquire(provider, config)` | Start a nested resource whose cleanup belongs to this node |

Standard providers include `sqlite.pool`, `postgres.pool`, `http.server`,
`http.limited_server`, and `otel.exporter` ([chapter 10](10-services.md)).

### A provider's background work

A provider that wants work of its own — a heartbeat, a poller, a
notifier — starts it on the scope the provider is given (its first
parameter). The scope belongs to the node: the worker is supervised, and
it stops when the system halts.

```badger
import std.system
import std.task
import std.worker

@derive(Error, From)
pub enum AppError {
    system(source: system.Error),
}

provider heartbeat(scope, _config: { every: Int }) -> Result<Unit, AppError> {
    scope.worker(fn() -> Result<Unit, AppError> {
        loop {
            task.sleep(1s)
        }
        ok(())
    }, worker.on_failure({ max_restarts: 3, window: 1s, backoff: 10ms }))
    ok(())
}
```

Do **not** open a `task.scope()` inside a provider to get a task of its
own. A scope belongs to the function that opened it: the provider's
scope ends when the provider returns, so the task it waits for never
finishes, and the system hangs at startup — with no port, no log line,
and no worker pool. The scope the provider is given is the one to use.

## Startup failure

```badger
import std.io
import std.system

@derive(Error)
pub enum Failure {
    unreachable(host: String),
}

@derive(Error, From)
pub enum AppError {
    io(source: io.Error),
    system(source: system.Error),
}

provider cache(scope, _config: {}) -> Result<Int, Failure> {
    scope.on_halt(fn() -> Result<Unit, Failure> {
        let _ = io.println("cache: halt")
        ok(())
    })
    ok(1)
}

provider remote(_scope, config: { host: String, cache: Int }) -> Result<Int, Failure> {
    err(Failure.unreachable(config.host))
}

pub fn main() -> Result<Unit, AppError> {
    let plan = system {
        cache: cache(),
        remote: remote({ host: "db.internal", cache: ref(#cache) }),
    }
    let r = system.run(plan, fn(_running) -> Result<Unit, AppError> { ok(()) })
    match r {
        .err(e) => io.println($"{e}")?,
        .ok(_) => io.println("started")?,
    }
    ok(())
}
```

Saved as `failure.bdg`, it prints:

```text
cache: halt
system: system node remote: unreachable: db.internal (provider failure/remote, node at failure.bdg:28:9)
```

When a node fails to start, the nodes already started halt in reverse
order and `system.run` returns a `system.Error`. It names the node, the
provider, and where the node is declared; `cause` holds the provider's
error as data.

## Lifecycle and halting

| Phase | What happens |
|---|---|
| Start | Nodes initialize one at a time in dependency order |
| Run | `body` runs with the running system |
| Halt | For each node in reverse order: its workers settle, then its halt callbacks run |
| Failure | A failing halt does not stop the others; failures are collected as notes on the `system.Error` |

- If `body` returned an error and a halt also fails, the result keeps the
  body's error in `original`; it is never silently replaced.
- A defect or cancellation inside `run` also halts everything.
- `running.wait_for_shutdown()` waits for SIGINT or SIGTERM. A service's
  `main` usually ends with it.
- The running system and its outputs cannot escape the `system.run`
  callback (E0401).

## Workers

```badger
import std.channel
import std.io
import std.system
import std.worker

@derive(Error, From)
pub enum AppError {
    config(source: channel.ConfigError),
    io(source: io.Error),
    system(source: system.Error),
}

provider tally(scope, config: { capacity: Int }) -> Result<_, AppError> {
    let inbox = channel.create<Int>({ ..channel.defaults, capacity: config.capacity })?
    let total = Atom.new(0)
    scope.worker_with(#tally/sum, fn() -> Result<Unit, AppError> {
        loop {
            match inbox.receiver.recv() {
                .some(n) => { let _ = total.update(fn(t) { t + n }) },
                .none => break,
            }
        }
        ok(())
    }, worker.on_failure(worker.defaults), worker.drain(100ms))
    scope.on_halt(fn() -> Result<Unit, AppError> {
        let _ = io.println($"halt: total {total.get()}")
        ok(())
    })
    ok({ add: fn(n: Int) { inbox.sender.try_send(n).is_ok() }, stop: fn() { inbox.sender.close() } })
}

pub fn main() -> Result<Unit, AppError> {
    let plan = system {
        tally: tally({ capacity: 16 }),
    }
    use running <- system.run(plan)
    for n in 1..=4 {
        let _ = running.tally.add(n)
    }
    running.tally.stop()
    io.println("body done")?
    ok(())
}
```

```text
body done
halt: total 10
```

A worker factory returns `Result<Unit, E>`. `ok(())` is normal completion.
An error or a defect goes to the restart policy. Each attempt runs in a
fresh task; a restart rebuilds the worker's state through its factory and
does not replay messages it already consumed.

| Restart policy | Behavior |
|---|---|
| `worker.never()` | A failure escalates to the owning scope |
| `worker.on_failure(options)` | Restart after a failure, at most `max_restarts` times per `window`, waiting `backoff` between attempts; past the budget, escalate |

`worker.defaults` is `{ max_restarts: 3, window: 1s, backoff: 10ms }`.
Each restart records a `#worker/restart` trace event.

| Shutdown policy | When the node halts |
|---|---|
| `worker.cancel()` (default) | Cancel the worker at once |
| `worker.drain(timeout)` | Let it finish on its own for up to `timeout`, then cancel |

A worker that still has not settled after the cleanup grace period
(`[runtime] cleanup_grace_ms`, default 5000) keeps its node's resources
alive: the remaining halts are skipped and reported. `worker.supervise(name,
factory, policy)` runs the same restart loop in any task, outside a system.

### Durable jobs

A worker's restart does not replay lost work: anything that must survive a
crash goes through [std.jobs](../api/jobs.md), a SQLite job store with
leases, fencing, retries, and an outbox. `jobs.store({ path })` provides
a store in a system, and `jobs.processor(store, processing, handle)` makes
the factory of a worker that runs one kind of job: it claims a due job,
decodes its state, and calls `handle(claim, state)`, whose `jobs.Step`
(the step reached, the new state, `next`, and operations to dispatch)
commits only while the claim still holds.

```badger fragment
provider mailer(scope, config: { store: jobs.Store }) -> Result<Unit, Error> {
    let p: jobs.Processing = { kind: "mail", owner: "mailer", version: 1, lease: 30s, backoff: 5s, idle: 100ms }
    scope.worker(jobs.processor(config.store, p, send_mail), worker.on_failure({ max_restarts: 3, window: 1min, backoff: 1s }))
    ok(())
}
```

Two kinds of failure stay distinct. An expected error from `handle` is a
failed attempt of the job: it runs again after `backoff` until its
`max_attempts` are used. A defect or a store error fails the worker, which
restarts by its policy; the job it held is claimed again once its lease
expires (and the old claim can no longer commit). A job whose process died
is continued the same way by another process sharing the store.

To make a domain change and its follow-up atomic, keep the job store in
the application's database (`jobs.attach(pool)`) and submit in the same
transaction (`jobs.submit_in(tx, kind, event, version, state,
max_attempts)`): the job exists exactly when the change commits. A step's
`sends` are outbox operations, each with a stable key that destinations
should treat as an idempotency key; `jobs.dispatcher(store, dispatching,
send)` is a worker that sends them. An operation is marked `sending`
before it is sent, so a dispatcher that crashes mid-send leaves it
`sending` until its timeout and then `uncertain`; uncertain operations are
never sent again automatically. `store.reconcile(kind, check)` asks the
destination about them by key, and `store.resolve(key, outcome)` records
an operator's decision.

### Decisions and events

[std.decide](../api/decide.md) keeps a domain's rules pure. A
`decide.Decider` is an `initial` state, a `decide(state, command,
coeffects)` that returns the events to record or a refusal, and an
`evolve(state, event)`; both functions are `!{}`, so what a decision
needs from the world (the time, a fresh id) arrives as the `coeffects`
value and the rules are tested directly with `decide.decide(decider,
history, command, coeffects)`.

`decide.execute(decider, stream, command, coeffects, attempts)` runs a
command against an event stream with optimistic concurrency: it loads the
events and their version, decides, and appends only if the version is
unchanged; after a conflict it loads and decides again, calling
`coeffects` afresh. Its result holds the committed events: act on them
(send email, `jobs.submit_in`) only after it returns. `decide.memory()`
is an in-memory stream; a durable one implements `load` and `append`
(`decide.Stream`).

### A channel between nodes

`channel.resource<T>()` is a provider of a channel the system owns: it is
created when its node starts and closed as the system starts halting,
before any worker stops, so a consumer whose worker drains
(`worker.drain(limit)`) receives every queued event and then the end. One
node publishes through `ref(#events).sender`, another consumes
`ref(#events).receiver`, so a request handler hands work to a background
worker without polling:

```badger fragment
let plan = system {
    events: component(channel.resource<Change>(), { ..channel.defaults, capacity: 64 }),
    notifier: component(notifier, { changes: ref(#events).receiver }),
    api: component(api, { publish: ref(#events).sender }),
}
```

## Select and replace nodes

```badger
import std.io
import std.system

@derive(Error, From)
pub enum AppError {
    io(source: io.Error),
    system(source: system.Error),
}

@derive(Error)
pub enum Failure {
    invalid,
}

provider clock(config: { start: Int }) -> Result<Int, Failure> {
 ok(config.start)
}

provider fixed_clock(_config: { start: Int }) -> Result<Int, Failure> {
 ok(1000)
}

provider report(config: { now: Int }) -> Result<String, Failure> {
 ok($"report at {config.now}")
}

pub fn main() -> Result<Unit, AppError> {
    let plan = system {
        clock: clock({ start: 5 }),
        report: report({ now: ref(#clock) }),
    }
    let only_clock = system.select(plan, [#clock])
    let n = system.run(only_clock, fn(running) -> Result<Int, AppError> { ok(running.clock) })?
    io.println($"selected: {only_clock.labels()} -> {n}")?
    let test_plan = system.replace(plan, #clock, fixed_clock)?
    let text = system.run(test_plan, fn(running) -> Result<String, AppError> { ok(running.report) })?
    io.println(text)?
    ok(())
}
```

```text
selected: [#clock] -> 5
report at 1000
```

- `system.select(plan, [#keys])` keeps the selected nodes and their
  dependencies. The output record has only those keys.
- `system.replace(plan, #key, provider)` swaps one node's provider, for
  tests. The replacement must take the same config type and produce the
  same output type; the compiler checks both.

## Inspect a plan

For the first example in this chapter, saved as `app.bdg`:

```text
$ badger inspect plans app.bdg
system plan at line 32
  #api = api <- #store
  #store = store
  startup order: #store -> #api
```

`badger graph systems FILE` draws the same graph (DOT by default,
`--format mermaid` or `json`). `plan.describe()` returns each node's key,
provider, location, and dependencies as data.

## Pipelines

```badger
import std.io
import std.pipeline
import std.pipeline.{handle}

type Reply = { status: Int, body: String }

@derive(Error)
pub enum AppError {
    missing_name,
}

stage log_stage() {
    enter(ctx) {
        let _ = io.println($"enter {ctx.path}")
        pipeline.next(ctx)
    }
    leave(ctx, reply: Reply) {
        let _ = io.println($"leave {ctx.path} -> {reply.status}")
        pipeline.leave(ctx, reply)
    }
}

stage errors_stage() {
    error(ctx, error: AppError) {
        match error {
            .missing_name => pipeline.recover(ctx, { status: 400, body: "name required" }),
        }
    }
}

stage user_stage() -> Result<_, AppError> {
    enter(ctx) {
        if ctx.name == "" {
            return err(AppError.missing_name)
        }
        pipeline.next({ ..ctx, user: ctx.name.to_upper() })
    }
}

pub fn main() -> Result<Unit, AppError> {
    let run = pipeline.compile(pipeline {
        log_stage(),
        errors_stage(),
        user_stage(),
        handle(fn(ctx) -> Result<Reply, AppError> { ok({ status: 200, body: $"hello {ctx.user}" }) }),
    })
    let ok_reply = run({ path: "/hi", name: "ada" })?
    let _ = io.println(ok_reply.body)
    let bad_reply = run({ path: "/hi", name: "" })?
    let _ = io.println(bad_reply.body)
    ok(())
}
```

```text
enter /hi
leave /hi -> 200
hello ADA
enter /hi
leave /hi -> 400
name required
```

A pipeline is a list of stages ending in a handler. `pipeline.compile`
turns it into a function from the input context to `Result<Reply, E>`.
Each stage's `enter` may add fields to the context, so the handler sees
`ctx.user` and the input did not need it.

| Stage callback | Signature (schematic) | Returns |
|---|---|---|
| `enter` | `fn(ctx) -> Result<Enter, E>` | `pipeline.next(ctx)` to continue, `pipeline.stop(ctx, reply)` to answer early |
| `leave` | `fn(ctx, reply) -> Result<(ctx, reply), E>` | `pipeline.leave(ctx, reply)` |
| `error` | `fn(ctx, error) -> ErrorStep` | `pipeline.propagate(ctx, error)` or `pipeline.recover(ctx, reply)` |
| `around` | `fn(ctx, next) -> Result<Reply, E>` | `next(ctx)` (the rest of the pipeline), or another result |

`stage NAME(params) { ... }` declares a stage factory: a pipeline calls it
(`log_stage()`), and parameterized factories capture their arguments.

- Each callback is optional, at most once, with a block or `=> expression`
  body. A stage without `enter` passes the context through.
- `stage NAME() -> Result<_, E>` names the error type once, for every
  callback (`errors_stage` above could write `error(ctx, error)`).
- The name in traces and diagnostics is `#MODULE/NAME`; `stage NAME as
  #app/name()` sets it.
- `pipeline.stage({ name, enter, leave, error })` builds the same stage as a
  value.
- `around(ctx, next)` wraps the rest in one place (time it, log on the
  way out, map or recover an error), instead of an `enter`/`leave`/`error`
  trio. It is a stage of its own: no other callback goes with it. It sees
  the result, not context fields the later stages set.
  `pipeline.around(name, f)` builds one as a value.

```badger fragment
stage timed() -> Result<_, AppError> {
    around(ctx, next) {
        let t0 = time.now()
        let r = next(ctx)
        log.info("handled", { path: ctx.path, ok: r.is_ok(), ms: time.as_millis(time.since(t0)) })
        r
    }
}
```

| Handler | Returns |
|---|---|
| `handle(f)` | `f(ctx) -> Result<Reply, E>`: a reply only |
| `handle_context(f)` | `f(ctx) -> Result<(ctx, Reply), E>`: a reply and updated field values |

### The error type of a stage

One rule, in one place: **a stage's callbacks use the pipeline's error
type** — the `E` of the handler's `Result<_, E>` — unless the stage names
one itself. A stage names one in either of two ways:

- the callback's own parameter: `error(ctx, error: AppError)`;
- the factory's result annotation: `stage NAME() -> Result<_, AppError>`,
  which names it for every callback of that stage.

So the same stage may be written three ways, and they mean the same
thing. Inferred (the pipeline's error type is `AppError`):

```badger
import std.pipeline
import std.pipeline.{handle}

pub enum AppError {
    failed(message: String),
}

stage logging() {
    enter(ctx) { pipeline.next(ctx) }
    error(ctx, error) {
        // `error` is an `AppError` here: the pipeline's handler says so
        match error {
            .failed(message) => { pipeline.propagate(ctx, AppError.failed(message)) },
        }
    }
}

pub fn main() -> Result<Unit, AppError> {
    let plan = pipeline {
        logging(),
        handle(fn(_ctx) -> Result<Int, AppError> { ok(1) }),
    }
    let _ = plan
    ok(())
}
```

Named by the callback:

```badger
import std.pipeline
import std.pipeline.{handle}

pub enum AppError {
    failed(message: String),
}

stage logging() {
    enter(ctx) { pipeline.next(ctx) }
    error(ctx, error: AppError) {
        match error {
            .failed(message) => { pipeline.propagate(ctx, AppError.failed(message)) },
        }
    }
}

pub fn main() -> Result<Unit, AppError> {
    let plan = pipeline {
        logging(),
        handle(fn(_ctx) -> Result<Int, AppError> { ok(1) }),
    }
    let _ = plan
    ok(())
}
```

Named by the factory, for a stage used in pipelines of several error
types:

```badger
import std.pipeline
import std.pipeline.{handle}

pub enum AppError {
    failed(message: String),
}

stage logging() -> Result<_, AppError> {
    enter(ctx) { pipeline.next(ctx) }
    error(ctx, error) {
        match error {
            .failed(message) => { pipeline.propagate(ctx, AppError.failed(message)) },
        }
    }
}

pub fn main() -> Result<Unit, AppError> {
    let plan = pipeline {
        logging(),
        handle(fn(_ctx) -> Result<Int, AppError> { ok(1) }),
    }
    let _ = plan
    ok(())
}
```

A stage that names a *different* error type does not fit the pipeline it
is used in, and the compiler says so — a stage's `error` callback never
fails silently.

### Order of callbacks

- On success, entered stages unwind in reverse, each calling `leave`.
- On an error, entered stages unwind in reverse, each calling `error`.
  `propagate` keeps unwinding with the error; `recover` supplies a reply
  and continues with `leave` at the next earlier stage.
- A stage joins the stack only when its `enter` returns `next`. If its
  `enter` fails, its own `error` callback does not run; earlier stages
  handle it. In the second run above, `user_stage` failed and `errors_stage`
  recovered.
- Each stage runs at most one unwind callback.
- Defects and cancellation skip `error` callbacks, still run scope
  cleanup, and record a `#pipeline/aborted` trace event per entered stage.

### What a stage can see

```badger
import std.io
import std.pipeline
import std.pipeline.{handle_context}

@derive(Error)
pub enum AppError {
    failed,
}

stage counting() {
    enter(ctx) {
 pipeline.next({ ..ctx, steps: 0 })
    }
    leave(ctx, reply: String) {
        let _ = io.println($"steps seen on leave: {ctx.steps}")
        pipeline.leave(ctx, reply)
    }
}

pub fn main() -> Result<Unit, AppError> {
    let run = pipeline.compile(pipeline {
        counting(),
        handle_context(fn(ctx) -> Result<_, AppError> {
            ok(({ ..ctx, steps: ctx.steps + 2 }, $"done {ctx.id}"))
        }),
    })
    let reply = run({ id: 7 })?
    let _ = io.println(reply)
    ok(())
}
```

```text
steps seen on leave: 2
done 7
```

A stage's `leave` and `error` callbacks see the fields that existed when
it entered, plus its own, with their latest values. Later stages may
update those values (at the same type); the update above is visible to
`counting`'s `leave`.

A stage cannot read a field that only a later stage adds. Reading
`ctx.user` in `log_stage`'s `leave` makes that field a requirement of the
pipeline's input, so the call `run({ path: "/hi", name: "ada" })` is E0301
(missing field `user`).

Inside a pipeline, enter steps may add fields and replace values, but not
remove a field or change its type. Put a decoded value under a new label.

### Adapters

| Function | Purpose |
|---|---|
| `pipeline.map_handler(h, reply, error)` | Convert a handler's reply and error types (`map_reply`, `map_error` convert one) |
| `pipeline.adapt(plan, reply, error)` | Convert a whole plan's reply and error types |
| `pipeline.nest(plan)` | Run one pipeline as another's handler |
| `pipeline.with_stages(stages, inner)` | Wrap a plan with stages chosen at run time; they must keep the context type |

`examples/design/service/src/main.bdg` uses a pipeline as an HTTP handler
inside a system. [std.pipeline](../api/pipeline.md) and
[std.system](../api/system.md) have the full APIs.

## Operations and bindings

A feature's significant entry points can be *operations*: typed values
with a stable identity and documentation. `app.from_fn(info, f)` wraps a
function of the call and the input; `app.from_pipeline(info, plan)` wraps a
pipeline whose context is `{ call, input }`. An operation keeps its input,
output, error, and effect types, so callers use it directly:
`op.call(caller, input)` runs it as a nested call of `caller`.

An `app.Call` is the invocation metadata: an id, a correlation shared by
the whole chain, the parent call, the operation entered, an optional
deadline (inherited; `within` shortens it but never extends it), and an
optional verified actor. It carries no dependencies; those are the
operation's captured providers.

`std.endpoint` binds an operation to HTTP and `std.command` binds it to a
command line. A binding says how the input is decoded and how the output
is rendered; the operation, and how its errors are reported, are the same
for both:

```badger
import std.app
import std.command
import std.endpoint
import std.http
import std.io
import std.route
import std.text

type Greeting = { text: String }

// how each transport reports the error is said once, on its type
@derive(Error, HttpError, CommandError)
pub enum GreetError {
    @message("a name is required") @http.status(400) @http.code(#hello/unnamed) @command.exit(1)
    unnamed,
}

@derive(Error, From)
pub enum AppError {
    io(source: io.Error),
    route(source: route.RouteError),
}

pub fn main() -> Result<Unit, AppError> {
    let greet = app.from_fn({ id: #hello/greet, version: 1, doc: "Greet someone." }, fn(_call, name: String) -> Result<Greeting, GreetError> {
        if text.trim(name) == "" { err(GreetError.unnamed) } else { ok({ text: $"Hello, {name}" }) }
    })
    let api = route.table([
        endpoint.bind({
            method: #get,
            path: "/hello/:name",
            operation: greet,
            input: fn(params: { name: String }, _body: {}) { params.name },
            status: 200,
        }),
    ])?
    let reply = http.call(api, http.request(#get, "/hello/Ada", ""))
    io.println(reply.body.to_text().unwrap_or("?"))?

    let commands = [
        command.bind({
            name: "greet",
            operation: greet,
            input: fn(options: { name: String }) { options.name },
            output: fn(g) { g.text },
        }),
    ]
    let outcome = command.run("hello", commands, app.call(), ["greet", "--name", "Ada"])
    io.println($"{outcome.exit}: {outcome.out}")?
    ok(())
}
```

```text
{"text":"Hello, Ada"}
0: Hello, Ada
```

An endpoint's path parameters decode into the input function's first
parameter and the body into its second; malformed input is a 400 reply
naming the field, before the operation runs. A command's options are a
`std.cli` record. The route name is the operation's id and its OpenAPI
summary the documentation; `command.help` lists the commands with their
documentation. An operation's expected error is answered as its type
says: an endpoint replies with its `HttpStatus` (`derive(HttpError)` reads
`@http.status`, `@http.code`, and the enum's `@http.codes(#prefix)`; a
server error's reply names the status only), and a command exits with its
`ExitStatus` (`derive(CommandError)` reads `@command.exit`), printing its
display. `command.run` returns an outcome (exit status, standard
output, standard error) without printing, for tests; `command.main` prints
it and returns the exit status, for `main -> Int`. Each request is a new
call, joined to the caller's `x-correlation-id` when the request has one.

### Health and bootstrap

`app.health(checks)` tracks an application's phase and readiness checks
(`app.check(name, probe)` is a required check with a one-second limit; set
`required: false` for one that only degrades). `app.run(plan, health,
body)` is `system.run` with the phases: ready once every node started,
stopping (so unready) before the nodes halt, stopped after, and never
ready when startup fails. It keeps `system.run`'s contract: the running
system does not leave `body`. `endpoint.health(health)` adds `GET
/health/live` (200 while the process serves) and `GET /health/ready` (the
report: 200 when ready or degraded, 503 when starting, stopping, or a
required check fails).

```badger fragment
let health = app.health([app.check(#db, fn() { pool.ping() })])
let plan = system {
    api: web.api({ health }),
    server: http.server({ address, port, handler: ref(#api) }),
}
use running <- app.run(plan, health)
running.wait_for_shutdown()?
```

### Identity, policies, and admission

`app.guard(op, policy)` runs `policy(call, input)` before the operation;
its error is the operation's, so a refused call never reaches the
operation. `app.require_role(role, refuse)` is a policy over the call's
verified actor.

`admission.gate(name, { capacity, queue })` admits at most `capacity`
calls at once and lets at most `queue` more wait; `gate.admit(call, body)`
runs `body` once admitted or returns `Rejected.full` or
`Rejected.deadline`. A waiter waits until its call's deadline, a released
permit goes to the longest waiter, and a permit is returned once the body
settles (also when its task is cancelled). `gate.stats()` gives the
counters.

`endpoint.bind_in(context, binding)` applies transport policies to an
endpoint: `identify` (for example `endpoint.bearer(verify)`, which reads
`Authorization: Bearer TOKEN`), a `timeout` counted from the request's
arrival (so time in the queue uses the same budget), a `gate`, and
`max_body`. It checks the body size (413), identity (401), and input
(400), then admits (503) and calls the operation with the actor and
deadline in its call. `endpoint.bind` is `bind_in(endpoint.public(), ...)`.

```badger fragment
let context: endpoint.Context = {
    ..endpoint.public(),
    identify: endpoint.bearer(sessions.verify),
    timeout: Option.some(2s),
    gate: Option.some(admission.gate(#orders/writes, { capacity: 16, queue: 64 })?),
}
endpoint.bind_in(context, { method: #post, path: "/orders", operation: app.guard(create, writers), ... })
```

`app.index(infos)` validates a set of operations' metadata (unique ids,
versions from 1, documentation) without erasing their types.
[`examples/framework`](../../examples/framework) is a small application
built this way: [std.app](../api/app.md), [std.endpoint](../api/endpoint.md),
and [std.command](../api/command.md) have the APIs.

Next: [Testing and contracts](09-testing-and-contracts.md).
