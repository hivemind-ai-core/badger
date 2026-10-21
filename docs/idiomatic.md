# Idiomatic Badger

The [guide](guide/README.md) teaches the language: what a value is, what `?`
does, what a provider is. This page is about the conventions. It says what
an application should look like for the seven things every application has:
one error type, validation, configuration, routes, pipeline stages,
background workers, and tests.

Each section gives the form to use first, the alternative second, and one
line on when to use which. The chapter links carry the detail, and the
[API reference](api/README.md) has every signature.

## One error type

Give the application one error enum. `derive(Error, From)` gives it
`Display`, a stable code, and a `From` for every variant with a single
payload, so a `?` anywhere in the code propagates without a `match`. The
conversion rules are in
[Errors](guide/03-errors.md#error-conversion-with-from); the `Error`
protocol itself is in
[the same chapter](guide/03-errors.md#the-error-protocol).

```badger
import std.http
import std.io
import std.text

@derive(Error, HttpError, From)
pub enum AppError {
    @http.status(400) @display("{message}")
    invalid_input(message: String),
    @http.status(404) @display("no note {id}")
    not_found(id: Int),
    io(source: io.Error),
}

fn load(id: Int) -> Result<String, AppError> {
    if id == 0 {
        return err(AppError.not_found(id))
    }
    ok($"note {id}")
}

pub fn main() -> Result<Unit, AppError> {
    io.println(load(7))?
    let reply = http.error_reply(AppError.not_found(0))
    io.println($"{reply.status} {text.from_bytes(reply.body).unwrap_or("")}")?
    ok(())
}
```

```text
ok(note 7)
404 {"error":"not_found","message":"no note 0"}
```

- Every function returns `Result<_, AppError>`. The name is `AppError` in a
  service; in a small module `Error` is enough.
- `derive(Error, From)` is what makes `?` work. Without the `From` part,
  every `?` needs a `match` or a `map_err`.
- `derive(HttpError)` answers each variant with its `@http.status(code)`, a
  variant wrapping another error (`store(source: store.Error)`) with that
  error's status and code (so a layer's error enum states nothing twice),
  and 500 for any other variant, so an error type that says nothing about
  HTTP keeps the old behaviour. `@display("no note {id}")` is the
  variant's text (its `Display`), interpolating its fields; without one a
  variant reads `not found: 0`. (Implementing `HttpStatus` or `Display` by
  hand decides them some other way.)
- A 4xx is the application's own code and message, and the client gets
  both: `{"error": "not_found", "message": "..."}`. The code is the variant
  name (`http_code` overrides it), the same convention as the route
  system's own `route/invalid_body`. A 5xx is an internal detail, and the
  client gets only the status text.

The alternative is `http.status(code, error)`, which wraps one error with a
status. Use it when the status belongs to that one call site rather than to
the error type; `http.error_reply(error)` builds the reply, and
`http.reply_for_error(error)` the same as a `Result`, for the places that
build a reply outside the route system — a pipeline stage, a worker.
`http.status_text(code)` gives the reason phrase. The route system's own
handling is in
[The status of an error](guide/10-services.md#the-status-of-an-error).

## Validation

State the rules of a value once, as a list, and check the value against
them. Rules that only look at one field can be written on the field
itself: `pub type NewBook = { @validate.not_blank @validate.max_len(200) title: String }`,
checked by `validate.check(book)`, which reports every broken one. [std.validate](api/validate.md) holds the rules, and
[std.result](api/result.md) holds the guard clauses for the checks that
belong to one function.

```badger
import std.io
import std.result
import std.validate

@derive(Error, From)
pub enum AppError {
    invalid_input(message: String),
    io(source: io.Error),
}

fn title_rules(title: String) -> List<validate.Rule<AppError>> {
    [
        validate.rule(validate.not_blank(title), AppError.invalid_input("title is required")),
        validate.rule(validate.at_most(title, 20), AppError.invalid_input("title is too long")),
    ]
}

fn check_title(title: String) -> Result<String, AppError> {
    let value = validate.validate(title, title_rules(title))?
    ok(value)
}

fn check_port(port: Int) -> Result<Int, AppError> {
    result.must(validate.in_range(port, 1, 65535), AppError.invalid_input("port out of range"))?
    ok(port)
}

pub fn main() -> Result<Unit, AppError> {
    io.println(check_title("milk"))?
    io.println(check_title("  "))?
    io.println(check_port(8080))?
    io.println(check_port(0))?
    ok(())
}
```

```text
ok(milk)
err(invalid input: title is required)
ok(8080)
err(invalid input: port out of range)
```

| Call | Gives |
|---|---|
| `validate.rule(condition, error)` | one rule: a condition and the error it reports |
| `validate.validate(value, rules)` | the value when every rule holds; the first failing rule's error otherwise |
| `validate.validate_all(value, rules)` | every failing rule's error, in rule order, as a list |
| `validate.not_blank(text)`, `at_most(text, n)`, `at_least(text, n)` | the text conditions |
| `validate.in_range(n, low, high)`, `greater_than(n, low)`, `less_than(n, high)` | the number conditions |
| `validate.check(value)` | the value when its fields keep their rules (`@validate.not_blank`, `min_len(n)`, `max_len(n)`, `min(n)`, `max(n)`, `one_of([...])` on the fields of its type); every broken rule otherwise, as `Violation`s (`validate.summary` joins their messages) |
| `result.must(condition, error)` | `ok(())` when the condition holds, the error otherwise |
| `result.must_eq(a, b, error)` | the same, for equality |
| `result.check(value, valid, error)` | the value when the predicate accepts it; the error is built only when it is rejected |
| `result.bail(error)` | leave the function with an error |
| `result.bimap(ok_fn, err_fn)` | change both sides of a result at once |

The alternative is a chain of guards in the function itself:

```badger fragment
if title == "" { return err(AppError.invalid_input("title is required")) }
if title.len() > 200 { return err(AppError.invalid_input("title is too long")) }
```

Use the guards when the function has one or two checks and nothing else to
say; use the rule list when the same rules apply in more than one place,
or when a caller should see every problem at once (`validate_all`). A guard
that is a single condition is `result.must(condition, error)?`, and a guard
that is a single equality is `result.must_eq(a, b, error)?`.

## Configuration

Give the application a settings record, read it from every source at once,
and let the record's field types do the conversions.
[std.config](api/config.md) has the four functions, and they differ in what
they read:

| Call | Reads |
|---|---|
| `config.from_env<T>(prefix)` | environment variables only: `PREFIX` + the upper-cased field name (or the variable a field's `@config.env("DATABASE_URL")` names) |
| `config.load<T>(prefix)` | the project's `[config]` table, then the selected profile, then the prefixed variables |
| `config.resolve<T>(prefix, overrides)` | the same, plus `key=value` overrides; strict, and it reports where each value came from |
| `config.resolve_or<T>(prefix, overrides, defaults)` | `resolve` with typed defaults as the lowest layer |

`resolve` is the framework form: it is the only one that tells you where a
value came from, and it is strict — a key that names no field is an error
rather than a shrug.

```badger
import std.cli
import std.config
import std.io

type Settings = {
    host: String,
    port: Int,
    timeout: Duration,
    token: config.Secret,
}

@derive(Error, From)
pub enum AppError {
    cli(source: cli.Error),
    config(source: config.Error),
    io(source: io.Error),
}

pub fn main() -> Result<Unit, AppError> {
    let opts = cli.parse_env({ host: cli.text("127.0.0.1"), port: cli.integer(8080), verbose: cli.flag(false) })?
    let defaults: Settings = {
        host: opts.host,
        port: opts.port,
        timeout: 30s,
        token: config.secret("dev-token"),
    }
    let resolved = config.resolve_or<Settings>("APP_", [], defaults)?
    io.println($"{resolved.value.host}:{resolved.value.port}")?
    io.println($"timeout {resolved.value.timeout}")?
    io.println($"token {resolved.value.token}")?
    io.println($"verbose {opts.verbose}")?
    for s in resolved.settings {
        io.println($"{s.key} = {s.value} ({s.source} {s.origin})")?
    }
    ok(())
}
```

```text
127.0.0.1:8080
timeout 30s
token [redacted]
verbose false
host = 127.0.0.1 (default host)
port = 8080 (default port)
timeout = 30000000000 (default timeout)
token = [redacted] (default token)
```

- The record's field types are the conversions. A field of type `Int` is
  read as an integer, a `Bool` as `true` or `false`, a `Float` as a number,
  and a `Duration` as a duration:

  | Field type | Configured as |
  |---|---|
  | `Int` | `8080` |
  | `Bool` | `true` |
  | `Float` | `0.5` |
  | `String` | `"text"` |
  | `Duration` | `"10s"`, `"500ms"`, or a plain number of milliseconds (`500`) |

  A duration that is not in the language's spelling is an error, not a
  guess. The provenance line lists a duration in nanoseconds, because that
  is what the runtime keeps.
- A field of type `config.Secret` is configured like a string, and shows
  as `[redacted]` in `Display`, in JSON, in logs, and in the provenance
  list. `secret.reveal()` gives the value to the code that needs it. Use a
  secret for anything that must not reach a log.
- A configured value may read the environment: `"${NAME}"` (which must be
  set) or `"${NAME:-default}"`. `$${` is a literal `${`.
- A setting whose variable the platform names reads it directly:
  `pub type Settings = { @config.env("DATABASE_URL") database: String, port: Int }`
  reads `DATABASE_URL` instead of `APP_DATABASE` (and strict resolution
  then refuses `APP_DATABASE`).
- The command line is a value too: `cli.parse_env` takes a record of
  defaults (`cli.text`, `cli.integer`, `cli.flag`, and friends) and returns
  the parsed options. The usual shape is to parse the command line, use it
  for the defaults of the settings record, and let the environment and the
  configuration override it. The module is [std.cli](api/cli.md); the
  configuration is in [Services](guide/10-services.md#configuration) and in
  [the configuration reference](reference/config.md).

A block of key/value pairs — the settings of a service, a header map, a
parsed form — is a `Map<String, String>`, and [std.kv](api/kv.md) has the
operations for that shape:

```badger
import std.io
import std.kv

pub fn main() -> Result<Unit, io.Error> {
    let settings = kv.from_list([("host", "127.0.0.1"), ("port", "8080")])
    io.println(kv.get_or(settings, "host", "?"))?
    io.println(kv.get_or(settings, "scheme", "http"))?
    let merged = kv.merge(settings, kv.from_list([("port", "9000")]))
    io.println(kv.get_or(merged, "port", "?"))?
    io.println(kv.len(merged))?
    ok(())
}
```

```text
127.0.0.1
http
9000
2
```

`kv.get_or` for a lookup with a fallback, `kv.merge` for the layers of a
configuration, `kv.filter` and `kv.map_values` for the rest.

The default of each built-in type is in one place, so a settings record
does not repeat the zero values: [std.default](api/default.md) has
`default.int()`, `default.float()`, `default.bool()`, `default.string()`,
`default.list()`, `default.map()` and `default.option()`.

```badger
import std.default
import std.io

pub fn main() -> Result<Unit, io.Error> {
    let settings = {
        port: default.int(),
        name: default.string(),
        tags: default.list(),
    }
    io.println($"{settings.port} {settings.name} {settings.tags.len()}")?
    ok(())
}
```

```text
0  0
```

## Routes

Describe the routes in one list, give each one a name, and let the
handler take its typed parameters and return the reply or an error.

```badger
import std.http
import std.io
import std.route

type Note = { id: Int, title: String }
type NewNote = { title: String }

@derive(Error, HttpError, From)
pub enum AppError {
    @http.status(404) @display("no note {id}")
    not_found(id: Int),
    http(source: http.Error),
    route(source: route.RouteError),
    io(source: io.Error),
}

fn routes() -> List<route.Route> {
    [
        route.get_json("/notes/:id", #notes/show, fn(p: { id: Int }) -> Result<Note, AppError> {
            if p.id > 100 {
                return err(AppError.not_found(p.id))
            }
            ok({ id: p.id, title: $"note {p.id}" })
        }),
        route.post_json("/notes", #notes/create, fn(_: {}, body: NewNote) -> Result<Note, AppError> {
            ok({ id: 2, title: body.title })
        }),
        // a field the path does not bind comes from the query string
        route.get_json("/search", #search, fn(p: { q: String, limit: Option<Int> }) -> Result<{ q: String, limit: Int }, AppError> {
            ok({ q: p.q, limit: p.limit.unwrap_or(10) })
        }),
        route.get("/health", #health, fn(_request, _params) -> Result<http.Response, AppError> {
            ok(http.text(200, "ok")?)
        }),
    ]
}

pub fn main() -> Result<Unit, AppError> {
    let handler = route.table(routes())?
    for (method, path, body) in [
        (#get, "/notes/7", ""),
        (#get, "/notes/x", ""),
        (#get, "/notes/999", ""),
        (#post, "/notes", "{\"title\":\"milk\"}"),
        (#post, "/notes", "{}"),
        (#get, "/search?q=dune", ""),
        (#get, "/health", ""),
        (#get, "/missing", ""),
    ] {
        io.println($"{http.call_text(handler, method, path, body)}")?
    }
    ok(())
}
```

```text
(200, "{\"id\":7,\"title\":\"note 7\"}")
(400, "{\"error\":\"route/invalid_path\",\"message\":\"expected an integer\",\"path\":\"$.id\"}")
(404, "{\"error\":\"not_found\",\"message\":\"no note 999\"}")
(201, "{\"id\":2,\"title\":\"milk\"}")
(400, "{\"error\":\"route/invalid_body\",\"message\":\"missing required field\",\"path\":\"$.title\"}")
(200, "{\"limit\":10,\"q\":\"dune\"}")
(200, "ok")
(404, "{\"error\":\"route/not_found\"}")
```

| Constructor | Handler receives | Returns |
|---|---|---|
| `route.get_json`, `delete_json(path, name, f)` | `(params)` | the reply, sent with 200 |
| `route.post_json`, `put_json(path, name, f)` | `(params, body)` | the reply, sent with 201 (POST) or 200 (PUT) |
| `route.typed(method, path, name, f)` | `(request, params, body)` | `(status, reply)`, for another status or the request itself |
| `route.json_get`, `json(method, ...)` | `(request, params map[, body])` | `(status, reply)`; the parameters as a map |
| `route.get`, `post`, `put`, `delete` | `(request, params map)` | `Result<http.Response, E>`, for a reply that is not JSON |

- The idiomatic form is `get_json` and its siblings: the path's `:name`s
  and the query's parameters decode into the handler's parameter record
  (`{}` for none; numbers, booleans, and `Option`s are parsed), the body
  into its body record, and an invalid one is a 400 naming the field before
  the handler runs. The reply is encoded from a record, so the shapes are
  in the description and in the OpenAPI document (query parameters
  included).
- `route.table(routes)` checks the table — unique names, well-formed
  paths, no two routes matching the same requests — and gives an
  `http.Handler`. The same descriptions give `route.url(routes, name,
  params)` and `route.openapi(routes, title, version)`.
- The error-to-status rule: a handler that returns `err(e)` is answered
  with `e`'s status (`derive(HttpError)`), code, and message. A 4xx
  carries the code and message, a 5xx neither. A route that does not match
  is answered 404 (`route/not_found`).
- `http.call_text(handler, method, path, body)` runs the table in-process
  and gives the reply's status and body text: the shape tests check.

The routes are in [Services](guide/10-services.md#handlers-and-routes), the
query string in
[Query parameters](guide/10-services.md#query-parameters), and the
signatures in [std.route](api/route.md) and [std.http](api/http.md).

## Pipeline stages

A stage is a small piece of work around a handler: it enters, the handler
runs, and it leaves. Write it with the `stage` form, so the stage names
its own error type once, and let the pipeline's error type be the default.

```badger
import std.io
import std.pipeline
import std.pipeline.{handle}

type Reply = { status: Int, body: String }

@derive(Error, From)
pub enum AppError {
    missing_name,
    io(source: io.Error),
}

stage timed() {
    enter(ctx) {
        let _ = io.println($"enter [{ctx.name}]")
        pipeline.next(ctx)
    }
    leave(ctx, reply: Reply) {
        let _ = io.println($"leave [{ctx.name}] -> {reply.status}")
        pipeline.leave(ctx, reply)
    }
}

stage name_required() -> Result<_, AppError> {
    enter(ctx) {
        if ctx.name == "" {
            return err(AppError.missing_name)
        }
        pipeline.next({ ..ctx, name: ctx.name.to_upper() })
    }
}

pub fn main() -> Result<Unit, AppError> {
    let run = pipeline.compile(pipeline {
        timed(),
        pipeline.recover_with(fn(e: AppError) -> Reply {
            match e {
                .missing_name => { status: 400, body: "name required" },
                _ => { status: 500, body: "internal error" },
            }
        }),
        name_required(),
        handle(fn(ctx) -> Result<Reply, AppError> { ok({ status: 200, body: $"hello {ctx.name}" }) }),
    })
    io.println(run({ name: "ada" })?.body)?
    io.println(run({ name: "" })?.body)?
    ok(())
}
```

```text
enter [ada]
leave [ADA] -> 200
hello ADA
enter []
leave [] -> 400
name required
```

| Callback | Gives |
|---|---|
| `enter(ctx)` | `pipeline.next(ctx)` to continue, `pipeline.stop(ctx, reply)` to answer early |
| `leave(ctx, reply)` | `pipeline.leave(ctx, reply)` |
| `error(ctx, error)` | `pipeline.propagate(ctx, error)` to keep the error, `pipeline.recover(ctx, reply)` to answer with a reply |

- The error type of a stage is the pipeline's error type — the `E` of the
  handler's `Result<_, E>` — unless the stage names one itself, either in
  the callback (`error(ctx, error: AppError)`) or in the factory
  (`stage NAME() -> Result<_, AppError>`, as `name_required` above). A
  stage that names a different error type does not fit the pipeline it is
  used in, and the compiler says so.
- `pipeline.recover_with(fn(error) -> reply)` turns the error into a
  reply, so the pipeline ends with that reply instead of an error. Put it
  outside the stages that may fail: it catches the error of every stage
  below it, and the stages outside it then unwind with `leave`, as the
  second run above shows.
- A stage's `enter` may add fields to the context, so the handler sees
  `ctx.name` in its own spelling and the input did not need it. A stage
  cannot read a field that only a later stage adds.

The stages are in
[Systems and pipelines](guide/08-systems-and-pipelines.md#pipelines), the
error type of a stage in
[the same chapter](guide/08-systems-and-pipelines.md#the-error-type-of-a-stage),
and the signatures in [std.pipeline](api/pipeline.md).

## Background workers

A provider that wants work of its own — a heartbeat, a poller, a
notifier — starts it on the scope the provider is given. The scope belongs
to the node: the worker is supervised, and it stops when the system halts.

```badger
import std.io
import std.system
import std.task
import std.worker

@derive(Error, From)
pub enum AppError {
    system(source: system.Error),
    io(source: io.Error),
}

provider heartbeat(scope, _config: { every: Int }) -> Result<Unit, AppError> {
    scope.worker(fn() -> Result<Unit, AppError> {
        loop {
            task.sleep(5s)
        }
        ok(())
    }, worker.on_failure(worker.defaults))
    ok(())
}

pub fn main() -> Result<Unit, AppError> {
    let plan = system {
        heartbeat: heartbeat({ every: 5 }),
    }
    system.run(plan, fn(_running) -> Result<Unit, AppError> { ok(()) })?
    io.println("stopped")?
    ok(())
}
```

```text
stopped
```

- The provider's first parameter is the node's scope. `scope.worker(factory,
  policy)` starts the work, `scope.worker_with(name, factory, policy,
  shutdown)` names it and says how it stops, and `scope.on_halt(f)` registers
  the node's cleanup.
- The factory returns `Result<Unit, E>`. `ok(())` is normal completion; an
  error or a defect goes to the restart policy
  (`worker.on_failure({ max_restarts, window, backoff })`,
  `worker.never()`).
- Do **not** open a `task.scope()` inside a provider to get a task of its
  own. A scope belongs to the function that opened it, so the provider's
  scope ends when the provider returns, the task it waits for never
  finishes, and the system hangs at startup — with no port, no log line,
  and no worker pool:

```badger fragment
provider heartbeat(scope, _config: { every: Int }) -> Result<Unit, AppError> {
    task.scope(fn(s) -> Result<Unit, AppError> {
        s.spawn(fn() { loop { task.sleep(5s) } })
        ok(())
    })?
    ok(())
}
```

- A service that should run until it is interrupted waits for the
  shutdown inside the system: `running.wait_for_shutdown()?`.

The providers are in
[Systems and pipelines](guide/08-systems-and-pipelines.md#providers), the
background work in
[the same chapter](guide/08-systems-and-pipelines.md#a-providers-background-work),
and the restart policies in [std.worker](api/worker.md).

## Tests

A test is a function whose name starts with `test_`. It may return a
`Result`, and returning `err` fails it. For an HTTP handler, call the
handler in process: `http.call_text` runs it with no socket and gives the
reply's status and body.

```badger
import std.http
import std.route
import std.test

type Note = { id: Int, title: String }

@derive(Error, HttpError, From)
pub enum AppError {
    @http.status(404) @display("no note {id}")
    not_found(id: Int),
    route(source: route.RouteError),
}

fn routes() -> List<route.Route> {
    [
        route.get_json("/notes/:id", #notes/show, fn(p: { id: Int }) -> Result<Note, AppError> {
            if p.id > 100 { return err(AppError.not_found(p.id)) }
            ok({ id: p.id, title: $"note {p.id}" })
        }),
    ]
}

requires amount > 0, "withdraw a positive amount"
fn withdraw(balance: Int, amount: Int) -> Int { balance - amount }

fn test_a_note_is_found() -> Result<Unit, AppError> {
    let handler = route.table(routes())?
    test.assert_eq(http.call_text(handler, #get, "/notes/7", ""), (200, "{\"id\":7,\"title\":\"note 7\"}"))
    ok(())
}

fn test_missing_note_is_404() -> Result<Unit, AppError> {
    let handler = route.table(routes())?
    test.assert_eq(http.call_text(handler, #get, "/notes/999", "").0, 404)
    ok(())
}

fn test_withdraw_keeps_its_contract() {
    test.assert_eq(withdraw(100, 30), 70)
}
```

- `badger test FILE` runs the functions whose name starts with `test_`. A
  test file needs no `main`: the tests are its entry points. A test may
  return a `Result`, and returning `err` fails it.
- `http.call_text(handler, #get, "/notes/7", "")` is the way to test a
  route: no socket, no port, and the reply's `(status, body)` is a value
  you can assert on; the last argument is the request body.
  `http.call(handler, http.request(...))` gives the whole `http.Response`
  (headers too).
- `test.assert_eq(actual, expected)`, `test.assert_true(condition,
  message)`, `test.expect_err(result)` and `test.fail(message)` are the
  assertions. A failure reports the caller's location.
- A contract (`requires`, `ensures`) is checked in every build, so a test
  does not have to repeat it. Contracts on providers and stages are
  checked against each node's configuration when the system starts.
- For time and scheduling, `std.test` has a controlled clock
  (`test.manual_clock`, `test.advance`), seeded scheduling
  (`test.schedule_seed`, `test.explore`), and property checks
  (`test.property`).

The tests are in
[Testing and contracts](guide/09-testing-and-contracts.md#write-and-run-tests),
the controlled runtime in
[the same chapter](guide/09-testing-and-contracts.md#controlled-time-scheduling-and-properties),
and the signatures in [std.test](api/test.md).
