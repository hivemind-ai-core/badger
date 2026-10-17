# Systems and pipelines

A `system { ... }` describes an application's components and their dependencies; the compiler checks it and `system.run` starts and halts it in order.
A `pipeline { ... }` composes typed interceptor stages around a handler.

## Describe a system

```badger
import std.io
import std.system

pub enum AppError {
    io(source: io.Error),
    system(source: system.Error),
} derive(Error, From)

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
- `system.provider({ name, init })` builds the same provider as a value,
  for providers made by functions.

| `InitScope` method | Effect |
|---|---|
| `scope.on_halt(f)` | Register cleanup; halts run in reverse registration order, once each |
| `scope.worker(factory, policy)` | Start a managed worker owned by the node |
| `scope.worker_with(name, factory, policy, shutdown)` | The same, with a name and a shutdown policy |
| `scope.acquire(provider, config)` | Start a nested resource whose cleanup belongs to this node |

Standard providers include `sqlite.pool`, `postgres.pool`, `http.server`,
`http.limited_server`, and `otel.exporter` ([chapter 10](10-services.md)).

## Startup failure

```badger
import std.io
import std.system

pub enum Failure {
    unreachable(host: String),
} derive(Error)

pub enum AppError {
    io(source: io.Error),
    system(source: system.Error),
} derive(Error, From)

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
import std.task
import std.worker

pub enum AppError {
    config(source: channel.ConfigError),
    io(source: io.Error),
    system(source: system.Error),
} derive(Error, From)

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

## Select and replace nodes

```badger
import std.io
import std.system

pub enum AppError {
    io(source: io.Error),
    system(source: system.Error),
} derive(Error, From)

pub enum Failure {
    invalid,
} derive(Error)

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

pub enum AppError {
    missing_name,
} derive(Error)

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

| Handler | Returns |
|---|---|
| `handle(f)` | `f(ctx) -> Result<Reply, E>`: a reply only |
| `handle_context(f)` | `f(ctx) -> Result<(ctx, Reply), E>`: a reply and updated field values |

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

pub enum AppError {
    failed,
} derive(Error)

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

Next: [Testing and contracts](09-testing-and-contracts.md).
