# Services

A service is a [system](08-systems-and-pipelines.md) of providers: an HTTP server, database pools, a telemetry exporter, and your own components.
This chapter shows each piece with one small example and links to the full API.

## HTTP server

### Handlers and routes

```badger
import std.http
import std.io
import std.route
import std.text

type NewNote = { title: String }
type Note = { id: Int, title: String }

@derive(Error, From)
pub enum AppError {
    route(source: route.RouteError),
    io(source: io.Error),
}

fn routes() -> List<route.Route> {
    [
        route.get("/health", #health, fn(_request, _params) { http.text(200, "ok") }),
        route.json_get("/notes/:id", #notes/show, fn(_request, params) -> Result<(Int, Note), String> {
            ok((200, { id: 1, title: $"note {params["id"]}" }))
        }),
        route.json(#post, "/notes", #notes/create, fn(_request, _params, body: NewNote) -> Result<(Int, Note), String> {
            ok((201, { id: 2, title: body.title }))
        }),
    ]
}

fn show(r: http.Response) -> String {
    $"{r.status} {text.from_bytes(r.body).unwrap_or("?")}"
}

pub fn main() -> Result<Unit, AppError> {
    let handler = route.table(routes())?
    io.println(show(http.call(handler, http.request(#get, "/health", ""))))?
    io.println(show(http.call(handler, http.request(#get, "/notes/7", ""))))?
    io.println(show(http.call(handler, http.request(#post, "/notes", "{\"title\":\"milk\"}"))))?
    io.println(show(http.call(handler, http.request(#post, "/notes", "{}"))))?
    io.println(show(http.call(handler, http.request(#get, "/missing", ""))))?
    ok(())
}
```

```text
200 ok
200 {"id":1,"title":"note 7"}
201 {"id":2,"title":"milk"}
400 {"error":"route/invalid_body","message":"missing required field","path":"$.title"}
404 {"error":"route/not_found"}
```

`http.call(handler, request)` runs a handler in-process, with no socket.
Use it in tests.

| Route constructor | Handler receives | Notes |
|---|---|---|
| `route.get`, `post`, `put`, `delete` | `(request, params)` | Returns `Result<http.Response, E>` |
| `route.json_get(path, name, f)` | `(request, params)` | Returns `(status, reply)`, encoded as JSON |
| `route.json(method, path, name, f)` | `(request, params, body)` | The body is decoded and validated first; a failure is a 400 naming the field |
| `route.typed(method, path, name, f)` | `(request, path_params, body)` | Path parameters decode into a record too |

`route.table(routes)` validates the table (unique names, well-formed
paths, no two routes matching the same requests) and returns an
`http.Handler`. The same descriptions give `route.url(routes, name,
params)` and `route.openapi(routes, title, version)`.
`endpoint.bind` makes a typed route from an `app.Operation`, with an
explicit reply for each expected error ([Operations and
bindings](08-systems-and-pipelines.md#operations-and-bindings)).

| Handler constructor | Callback |
|---|---|
| `http.handler(f)` | `fn(http.Request) -> Result<http.Response, E>` |
| `http.scoped(f)` | Gets a `RequestScope<S>`: a scoped body reader, `respond`, `stream`, `events`, `websocket` |
| `http.streaming_handler(f)` | Returns `Result<http.Streamed, E>` (chunked reply) |
| `http.websocket_handler(f)` | `fn(http.Request, websocket.WebSocket) -> Result<Unit, E>` |

A handler error becomes the reply its status asks for — 500 unless the
error says otherwise. Reply helpers are `http.text(status, body)` and
`http.json(status, value)`. Values tied to one request (the body reader)
cannot outlive it (E0401).

### The status of an error

An error carries the status it should be answered with through the
`HttpStatus` protocol. The default is 500 ("internal server error"), so
an error type that says nothing about HTTP keeps the old behaviour; an
application implements it once, on its error type:

```badger
import std.http
import std.io
import std.route
import std.text

@derive(Error, From)
pub enum AppError {
    invalid_input(message: String),
    not_found(id: Int),
    route(source: route.RouteError),
    io(source: io.Error),
}

// The status each error is answered with.
impl HttpStatus for AppError {
    fn http_status(self: &Self) -> Int {
        match self {
            .invalid_input(_) => 400,
            .not_found(_) => 404,
            _ => 500,
        }
    }
}

fn show(r: http.Response) -> String {
    $"{r.status} {text.from_bytes(r.body).unwrap_or("?")}"
}

pub fn main() -> Result<Unit, AppError> {
    let handler = route.table([
        route.get("/notes/:id", #notes/show, fn(_request, _params) -> Result<http.Response, AppError> {
            err(AppError.not_found(1))
        }),
    ])?
    io.println(show(http.call(handler, http.request(#get, "/notes/1", ""))))?
    ok(())
}
```

```text
404 {"error":"not found","message":"not found: 1"}
```

A client error (a 4xx) is answered with the error's message; a server
error (a 5xx) keeps its message to itself. `http.StatusError` carries a
status for the one-off case: `http.status(404, error)`.

### Query parameters

A route matches its path; the query string is part of the request, not of
the path. `route.params(request)` reads it:

```badger
import std.http
import std.io
import std.route
import std.text

@derive(Error, From)
pub enum AppError {
    http(source: http.Error),
    route(source: route.RouteError),
    io(source: io.Error),
}

impl HttpStatus for AppError { }

fn show(r: http.Response) -> String {
    $"{r.status} {text.from_bytes(r.body).unwrap_or("?")}"
}

pub fn main() -> Result<Unit, AppError> {
    let handler = route.table([
        route.get("/search", #search, fn(request, _params) -> Result<http.Response, AppError> {
            let q = route.params(request).get("q").unwrap_or("")
            ok(http.json(200, { q: q })?)
        }),
    ])?
    io.println(show(http.call(handler, http.request(#get, "/search?q=dune&page=2", ""))))?
    ok(())
}
```

```text
200 {"q":"dune"}
```

A path parameter and a query parameter of the same name: the path
parameter wins. `route.path_of(uri)` and `route.query_of(uri)` split a
URI into its two parts.

### Serve it

```badger
import std.config
import std.http
import std.system

type Settings = { address: String, port: U16 }

@derive(Error, From)
pub enum AppError {
    config(source: config.Error),
    system(source: system.Error),
}

fn hello(request: http.Request) -> Result<http.Response, http.Error> {
    http.text(200, $"hello from {request.path}\n")
}

pub fn main() -> Result<Unit, AppError> {
    let settings = config.from_env<Settings>("APP_")?
    let plan = system {
        server: http.server({
            address: settings.address,
            port: settings.port,
            handler: http.handler(hello),
        }),
    }
    use running <- system.run(plan)
    running.wait_for_shutdown()?
    ok(())
}
```

```sh
APP_ADDRESS=127.0.0.1 APP_PORT=8080 badger run server.bdg
```

The server runs each request as its own task. One request failing does
not affect the others. On SIGINT or SIGTERM, `wait_for_shutdown` returns
and the system halts: the server stops admission and settles in-flight
requests.

| `[runtime]` setting | Default | Over the limit |
|---|---|---|
| `http_max_body_bytes` | 16777216 | 413 |
| `http_max_header_bytes` | 65536 | 431 |
| `http_max_in_flight` | 1024 | 503 |
| `http_max_stream_buffer_bytes` | 1048576 | The streaming task waits for the client |

`http.limited_server` takes explicit `limits` instead. A client that goes
away during a streamed reply cancels the request task, so its cleanups run.

### Server-sent events

```badger
import std.http
import std.io
import std.seq
import std.text

fn ticks(_request: http.Request) -> Result<http.Streamed, http.Error> {
    http.events(seq.range(1, 4).map(fn(n) { http.event($"tick {n}").with_id($"{n}") }))
}

pub fn main() -> Result<Unit, io.Error> {
    let handler = http.streaming_handler(ticks)
    let reply = http.call(handler, http.request(#get, "/ticks", ""))
    io.print(text.from_bytes(reply.body).unwrap_or(""))?
    ok(())
}
```

```text
id: 1
data: tick 1

id: 2
data: tick 2

id: 3
data: tick 3
```

`http.events(seq)` sends each `http.Event` as it is produced. Events take
`.named(name)`, `.with_id(id)`, and `.with_retry(millis)`; `http.comment`
makes a keep-alive line. `http.last_event_id(request)` reads a
reconnecting client's `Last-Event-ID`.

### WebSockets

```badger
import std.http
import std.system
import std.websocket

@derive(Error, From)
pub enum AppError {
    system(source: system.Error),
}

fn echo(_request: http.Request, ws: websocket.WebSocket) -> Result<Unit, websocket.Error> {
    loop {
        match ws.recv()? {
            .some(.text(t)) => ws.send_text($"echo: {t}")?,
            .some(.binary(b)) => ws.send_binary(b)?,
            .none => break,
        }
    }
    ok(())
}

pub fn main() -> Result<Unit, AppError> {
    let plan = system {
        server: http.server({
            address: "127.0.0.1",
            port: 8080,
            handler: http.websocket_handler(echo),
        }),
    }
    use running <- system.run(plan)
    running.wait_for_shutdown()?
    ok(())
}
```

Each upgrade request runs the session in its own task; a request that is
not an upgrade gets 426. `ws.recv()` returns `none` once the peer closed.
Receiving and sending wait independently, so one task can read while
another writes. Clients use `websocket.connect(url)`.

Full APIs: [std.http](../api/http.md), [std.route](../api/route.md),
[std.websocket](../api/websocket.md).

## Outbound HTTP

```badger
import std.env
import std.fetch
import std.io

pub fn main() -> Result<Unit, io.Error> {
    let base = env.get("API").unwrap_or("http://127.0.0.1:8080")
    let reply = fetch.get($"{base}/status")?
    io.println($"{reply.status}: {reply.body.trim()}")?
    let api = fetch.client(base)?
    let r = api.timeout(2s).get("/items")?
    io.println($"{r.status}")?
    ok(())
}
```

Every status is a reply, not an error; `io.Error` means the request
itself failed. Requests run on the blocking pool while the task waits.

| Function | Use |
|---|---|
| `fetch.get(url)`, `fetch.post(url, body, content_type)` | Text bodies, 30 s limit, no redirects |
| `fetch.get_json<T>(url)`, `fetch.post_json<T, R>(url, value)` | JSON in and out; a non-2xx status is an `#io/http_status` error |
| `fetch.request(method, url)` then `fetch.send(r)` | Any method, headers, byte body, time limit, redirects, body limit |
| `fetch.client(origin)` | A `Client` that may reach one origin only; pass it to code that should not reach others |

See [std.fetch](../api/fetch.md).

## SQLite

```badger
import std.io
import std.sqlite

type Item = { id: Int, name: String, qty: Int }

@derive(Error, From)
pub enum AppError {
    database(source: sqlite.Error),
    io(source: io.Error),
}

pub fn main() -> Result<Unit, AppError> {
    let db = sqlite.open(":memory:")?
    sqlite.execute(db, "CREATE TABLE items (id INTEGER PRIMARY KEY, name TEXT NOT NULL, qty INTEGER NOT NULL)", [])?
    sqlite.transaction(db, fn(tx) -> Result<Unit, AppError> {
        for name in ["milk", "eggs"] {
            sqlite.execute(tx, "INSERT INTO items (name, qty) VALUES (?, ?)", [Data.string(name), Data.int(2)])?
        }
        ok(())
    })?
    let items = sqlite.query_as<Item>(db, "SELECT id, name, qty FROM items ORDER BY id", [])?
    for item in items {
        io.println($"{item.id} {item.name} x{item.qty}")?
    }
    ok(())
}
```

```text
1 milk x2
2 eggs x2
```

| Function | Returns |
|---|---|
| `sqlite.execute(conn, sql, params)` | Changed row count |
| `sqlite.query(conn, sql, params)` | `List<Data>` |
| `sqlite.query_one(conn, sql, params)` | One row as `Data`; zero or several rows is an error |
| `sqlite.query_as<T>`, `query_one_as<T>` | Rows decoded as record `T` (an `Option` field for a nullable column) |
| `sqlite.transaction(pool, body)` | Commits after `ok`; rolls back after `err`, a defect, or cancellation |

`conn` is a pool or a transaction. Parameters are `?` placeholders bound
from a `List<Data>`. A transaction is confined to its task (E0402) and to
its callback (E0401). In a system, use the `sqlite.pool` provider with
`{ path }`.

Literal SQL is checked when the program is checked, against the project's
`[database] schema` file and the program's own `CREATE`/`ALTER`
statements:

| Code | Mistake |
|---|---|
| E0180 | The statement does not compile (unknown table or column, syntax), or does not suit the call |
| E0181 | The parameter list literal has the wrong length |
| E0182 | `query_as` row type does not match the result columns |

[Typed SQL](../typed-sql.md) generates typed functions from `.sql` files
of named queries, for SQLite and Postgres. See [std.sqlite](../api/sqlite.md).

### Migrations

A schema is a list of `sqlite.Migration` values (`{ version, name, sql }`,
versions increasing from 1). `sqlite.migrate(pool, migrations)` applies
the pending ones in order, each in its own transaction together with its
record in `badger_migrations` (version, name, and a checksum of the SQL),
and returns the versions it applied; a failing migration rolls back and
stops. `sqlite.schema(pool, migrations)` compares the database with the
program, and `sqlite.require_current(pool, migrations)` refuses to
continue when migrations are pending (`#sqlite/schema_behind`), the
database has a migration the program does not know (`#sqlite/schema_ahead`),
or an applied migration's SQL changed (`#sqlite/migration_changed`). A
store provider calls `require_current` so a server never starts on the
wrong schema, and a separate entry runs `migrate`
([`examples/framework`](../../examples/framework) has both).

## Postgres

```badger
import std.env
import std.io
import std.postgres

type User = { id: Int, name: String }

@derive(Error, From)
pub enum AppError {
    database(source: postgres.Error),
    io(source: io.Error),
}

pub fn main() -> Result<Unit, AppError> {
    let url = env.get("DATABASE_URL").unwrap_or("postgres://localhost/app")
    let db = postgres.connect(url)?
    let users = postgres.query_as<User>(db, "SELECT id, name FROM users WHERE name LIKE $1", [Data.string("a%")])?
    for u in users {
        io.println($"{u.id} {u.name}")?
    }
    ok(())
}
```

`std.postgres` has the shape of `std.sqlite` (`execute`, `query`,
`query_one`, `query_as`, `query_one_as`, `transaction`) over a connection
pool. Differences:

- Parameters are `$1`, `$2`, ..., parsed by the server as each parameter's
  type.
- `postgres.connect(url)` opens one connection immediately, so a bad
  address or credentials fail there. `connect_with(url, { max_connections })`
  sizes the pool (default 16).
- `postgres.describe(pool, sql)` reports parameter and column types without
  running the statement.
- In a system, use the `postgres.pool` provider with `{ url }`.
- `postgres.migrate`, `postgres.schema`, and `postgres.require_current`
  are the [SQLite migrations](#migrations) for Postgres: each migration
  runs in its own transaction (Postgres DDL is transactional), and
  concurrent migrators take turns on an advisory lock.

See [std.postgres](../api/postgres.md) and [Typed SQL](../typed-sql.md).

## Configuration

```toml
[package]
name = "notes"
entry = "src/main.bdg"

[config]
port = 8080
database = "notes.db"
greeting = "${GREETING:-hello}"

[profile.dev]
port = 3000

[profile.dev.runtime]
log_level = "debug"
```

```badger
import std.config
import std.io

type Settings = { port: Int, database: String, greeting: String }

@derive(Error, From)
pub enum AppError {
    config(source: config.Error),
    io(source: io.Error),
}

pub fn main() -> Result<Unit, AppError> {
    let s = config.load<Settings>("NOTES_")?
    io.println($"{s.greeting}: port {s.port}, database {s.database}")?
    ok(())
}
```

```text
$ badger run
hello: port 8080, database notes.db
$ badger run --profile dev
hello: port 3000, database notes.db
$ NOTES_PORT=9000 GREETING=hi badger run --profile dev
hi: port 9000, database notes.db
```

| Function | Reads |
|---|---|
| `config.from_env<T>(prefix)` | Environment variables only: `PREFIX` + upper-cased field name |
| `config.load<T>(prefix)` | `[config]`, merged with `[profile.NAME]` (`--profile NAME`), then prefixed environment variables |
| `config.resolve<T>(prefix, overrides)` | Like `load`, then `key=value` overrides; strict, and reports where each value came from |
| `config.resolve_or<T>(prefix, overrides, defaults)` | `resolve` with typed defaults as the lowest layer |

- A field without a value is an error (`config.Error` names the key). There
  are no implicit defaults; write them in `[config]` or with
  `${NAME:-default}`.
- `"${NAME}"` in a configured value reads the environment (it must be set);
  `$${` is a literal `${`.
- `[env] dotenv` files load before the program starts.
- `badger inspect config [--profile NAME]` prints the effective values and
  where each came from.

`config.resolve` is the framework form. Its layers, lowest first, are typed
defaults, `[config]`, the profile, `PREFIX_*` variables (nested fields
`_`-joined: `NOTES_DB_PATH` for `db.path`), and overrides such as the
program's arguments (`badger run -- port=9000 db.path=x.db`). A
`[config]` or profile key, a prefixed variable, or an override that names
no field is an error, not ignored. The result's `settings` give each
setting's source (`#default`, `#config`, `#profile`, `#env`, `#override`),
origin, value, and the sources it overrode:

```badger
import std.config
import std.io

type Settings = { port: Int, token: config.Secret }

@derive(Error, From)
pub enum AppError {
    config(source: config.Error),
    io(source: io.Error),
}

pub fn main() -> Result<Unit, AppError> {
    let defaults: Settings = { port: 8080, token: config.secret("dev") }
    let resolved = config.resolve_or<Settings>("NOTES_", ["port=9000"], defaults)?
    io.println($"port {resolved.value.port}, token {resolved.value.token}")?
    for s in resolved.settings {
        io.println($"{s.key} = {s.value} ({s.source} {s.origin})")?
    }
    ok(())
}
```

```text
port 9000, token [redacted]
port = 9000 (override port=9000)
token = [redacted] (default token)
```

A `config.Secret` is configured like a string but displays, encodes as
JSON, and appears in provenance as `[redacted]`; `secret.reveal()` gives
the value to the code that needs it.

Runtime settings (`[runtime]`, `[profile.NAME.runtime]`) are separate.
Each can be overridden by `BADGER_<SETTING>`, for example
`BADGER_WORKERS=4` or `BADGER_LOG_LEVEL=debug`. `badger inspect runtime`
lists every setting with its value and source. The
[configuration reference](../reference/config.md) has every key.

## Logging, tracing, and metrics

```badger
import std.log
import std.metrics
import std.trace

fn place_order(id: Int) -> Int {
    trace.span("place_order", { id }, fn() {
        log.info("order placed", { id, items: 3 })
        metrics.count("orders.placed", { region: "eu" })
        id * 10
    })
}

pub fn main() -> Unit {
    log.with({ request: "r-1" }, fn() {
        let total = place_order(7)
        log.debug("not shown at the default level", { total })
        log.warn("slow request", { millis: 1200 })
    })
}
```

Standard error, at the default level and format:

```text
2026-10-17T17:48:37.631Z INFO  order placed request=r-1 id=7 items=3
2026-10-17T17:48:37.631Z WARN  slow request request=r-1 millis=1200
```

With `BADGER_LOG_FORMAT=json BADGER_LOG_LEVEL=debug`:

```text
{"ts":"2026-10-17T17:48:37.683Z","level":"info","msg":"order placed","task":1,"trace_id":"1ee3b522db3efa8b5dc1f9d4f5059468","span_id":"b912d343e51cfeb8","request":"r-1","id":7,"items":3}
{"ts":"2026-10-17T17:48:37.683Z","level":"debug","msg":"not shown at the default level","task":1,"request":"r-1","total":70}
{"ts":"2026-10-17T17:48:37.683Z","level":"warn","msg":"slow request","task":1,"request":"r-1","millis":1200}
```

| Module | What it gives you |
|---|---|
| [std.log](../api/log.md) | `trace`, `debug`, `info`, `warn`, `error` with a record of fields. `log.with(fields, body)` adds fields to every record inside, including spawned tasks. Level and format come from `[runtime] log_level` / `log_format` or `BADGER_LOG_LEVEL` / `BADGER_LOG_FORMAT`. Logging never fails |
| [std.trace](../api/trace.md) | `trace.span(name, attrs, body)` and `span_result`: spans that spawned tasks continue; logs inside carry the trace and span ids. `trace.event(name, record)` records a structured event in a bounded sink (`BADGER_TRACE=1` also prints them). `traceparent()` and `continue_trace(header)` pass a trace between services |
| [std.metrics](../api/metrics.md) | `count`, `add`, `gauge`, `observe` (histogram), aggregated by name and attribute record. `snapshot()` reads them |
| [std.otel](../api/otel.md) | Exports logs, spans, and metrics to an OTLP/HTTP collector |

To export, add the exporter to the system:

```badger
import std.http
import std.otel
import std.system

@derive(Error, From)
pub enum AppError {
    system(source: system.Error),
}

fn hello(_request: http.Request) -> Result<http.Response, http.Error> {
    http.text(200, "hello\n")
}

pub fn main() -> Result<Unit, AppError> {
    let plan = system {
        telemetry: otel.exporter(otel.from_env()),
        server: http.server({ address: "127.0.0.1", port: 8080, handler: http.handler(hello) }),
    }
    use running <- system.run(plan)
    running.wait_for_shutdown()?
    ok(())
}
```

`otel.from_env()` reads `OTEL_EXPORTER_OTLP_ENDPOINT` (default
`http://localhost:4318`), `OTEL_SERVICE_NAME`, and
`OTEL_METRIC_EXPORT_INTERVAL` (milliseconds, default 5000). The exporter
sends from a background task and once more when the system halts. A failed
export is counted in the `otel.export_failures` metric and never fails the
program.

## Deploy with `badger release`

```sh
badger release . -o release/notes     # check, compile, and package the project
release/notes/run                     # start it; arguments go to the program
badger start release/notes -- ARGS    # the same, with an explicit executable
```

The release directory holds the `badger` executable, the program as
binary MIR, the native libraries it calls, and a manifest of digests. It
starts without the sources or a compiler installation, and `badger start`
refuses it if a digest, the target, or the native ABI does not match.

Configuration is saved as it was at release time (`--profile NAME` merges
a profile). Environment variables still apply when it starts: prefixed
`config.load` overrides, `${NAME}` references, and `BADGER_<SETTING>`.
Data files the program reads at run time are not copied. See
[Releases](../releases.md).

Back to the [guide index](README.md).
