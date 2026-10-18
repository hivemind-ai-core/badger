# Hands-on experience report — writing `examples/13_rest_service.bdg`

This is a first-person account of writing a single-file database-backed
REST service in Badger from a blank slate. The goal was to exercise
as much of the language as possible in one program (records, structs,
enums, errors, generics, lambdas, pipes, pattern matching, SQLite,
typed routes, a pipeline of interceptor stages, an `Atom`, a `task`
scope, supervised workers, the `system` graph, structured logging,
tracing, and tests) and to land a runnable service with passing
`badger test`.

The summary: the bones are good, the experience was harder than it
should have been. A "simple" CRUD service took about a dozen
check/test cycles to land; roughly half of those were language
friction rather than application design.

## Background

The session produced `examples/13_rest_service.bdg` — 673 lines, one
file, eight `test_*` functions. The final program passes `badger
check`, all eight tests pass under `badger test`, and `badger run`
opens a TCP port and serves the documented endpoints with the expected
status codes (201/200/400/404). The exercises dropped during the
attempt (channel-based notifications, in-band stats via lazy
sequences, contracts on `trim`, the `route.typed` path-param
decoding) are listed in the next section.

## Revision and verification

- Tooling: `target/release/badger` from `beb0139` (the head of
  `main` at the time).
- The source is `examples/13_rest_service.bdg` and the
  `examples/README.md` line added for it.
- All claims below are reproducible by deleting the file and
  re-running the iterations described, in order, against the same
  `badger` binary.

## What worked well

1. `derive(Error, From)` on the application error enum. Once the enum
   is written, every `?` call site propagates correctly without an
   explicit match. Validation, decoding, database, HTTP, system, and
   IO errors all collapse to the same `AppError` and the route
   handler deals with one error type.
2. Effect inference. I did not write a single `!{...}` annotation.
   The compiler placed `Io`, `Wait`, and `State` on the right
   functions, and the typed query helpers (e.g. `sqlite.query_as`,
   `sqlite.transaction`) carried the effects through the `?`
   operator and into the handler without ceremony.
3. The `system { ... }` graph with `ref(#name)` typed references.
   Five providers (database pool, store, API, stats reporter, HTTP
   server) wired in fifteen lines, with dependency order checked at
   compile time.
4. Records with open rows and the `{ ..r, field: value }` update
   syntax. Building the `StatsRow` / `BookRow` decoding shape and the
   `Book` domain type out of plain records was immediate.
5. Pipeline stages as data: `timing`, `access_log`, and the per-
   request counter stage composed in one `pipeline { ... }` literal.
6. In-process tests with `http.call`. No socket, no port, no race.
   Tests run as fast as ordinary function calls.

## Friction ranked by impact

The "time spent" estimates below are wall-clock during the session;
they include diagnosing the problem, attempting a fix, and
re-running `badger check`/`badger test` to confirm.

### F1 — Route handlers' `Err` becomes a silent 500. (Medium, ~25 min)

The route handlers in `std.route` map any non-`Ok` return to a
`500 "internal server error"` reply inside the route's own `run`
closure (`crates/badger_hir/std/route.bdg:59`). The application
`AppError.invalid_input(...)` and `AppError.not_found(...)` are
`Display`, so the route system sees a normal error and silently
replies 500 — the *opposite* of what the application intended.

There is no warning, no diagnostic, and no test failure that points
at this. The only signal is a 500 in the response body with the
default JSON shape `{ "error": "internal server error" }`. I lost
~25 minutes across two passes (validation tests failed at
`expect 400 got 500`; the not-found test failed at `expect 404 got
500`) before reading `std/route.bdg` and discovering the
auto-conversion.

The framework's example (`examples/design/service/src/main.bdg:30`)
works around this with a `respond_to_errors` pipeline stage that
runs *inside* the route's pipeline. That pattern is fine, but it is
not the obvious choice, and there is no documentation that says
"validation must happen in a stage, not in a handler".

**Recommendation:** Either (a) treat the handler's `Err` as opaque
and let the caller map it (the current behavior, but with a
diagnostic when an unmapped error reaches the route), or (b) carry
the status code in the error variant and surface it here. The
silent-500 is the worst of both worlds.

### F2 — `use scope <- task.scope()` inside a provider doesn't keep the task alive. (Medium, ~30 min)

I started with a `task.scope`-based background task inside the
`stats_reporter` provider:

```badger
provider stats_reporter(config: ...) -> Result<Unit, AppError> {
    use scope <- task.scope()
    scope.spawn(fn() { loop { task.sleep(...) ... } })
    ok(())
}
```

The runtime accepted this, but `badger run` then hung: no listening
port, no log output, no scheduled workers (only two threads instead
of the runtime's expected worker pool). The framework's own example
(`examples/framework/src/worker.bdg:19`) uses a different shape:

```badger
provider reminders(scope, config: ...) -> Result<Unit, AppError> {
    scope.worker(fn() { ... }, worker.on_failure(...))
    ok(())
}
```

The provider takes the lifecycle `scope` as its first parameter,
and `scope.worker` registers a supervised task that lives as long
as the system does. `task.scope()` is an inline block — its tasks
end when the block ends. The two look superficially similar in
syntax but mean very different things, and the failure mode of the
inline form is "the program silently hangs at startup".

**Recommendation:** Either error in the compiler if `task.scope()`
appears inside a provider body, or document the constraint. The
silent hang is a debugging cliff.

### F3 — Module function vs method syntax split is inconsistent. (Low/Medium, ~15 min, 4 attempts)

`std.list` exports `list.map(list, f)`. `List<T>` also exposes
`.map(f)` as a method. Both work, but the parser accepts one or
the other depending on position. When I wrote

```badger
sqlite.query_as<BookRow>(...)
    |> result.map(fn(rows) => list.map(rows, row_to_book))
```

the type checker reported `no method map on ...`. Switching to
method syntax `rows.map(row_to_book)` worked. But earlier in the
session I had used `text.trim(value)` (module function), `text.contains(...)`
(module function), and `bytes.to_text(response.body)` (method on
`Bytes`) without issue — the dividing line is not consistent enough
to predict.

The pipe operator compounds this: `value |> f(a)` expands to
`f(value, a)`. When `f` is itself a partially-applied module
function, the partial application can shadow the call.

**Recommendation:** Pick one convention and stick to it. Method
syntax everywhere would be the simplest and would eliminate this
class of error entirely.

### F4 — Generics and `fn` declarations are not allowed inside function bodies. (Medium, ~10 min, 2 attempts)

I wrote a local helper to lift a `Result<X, sqlite.Error>` into a
`Result<X, AppError>`:

```badger
fn lift<X>(r: Result<X, sqlite.Error>) -> Result<X, AppError> {
    r |> result.map_err(AppError.database)
}
```

The parser reported `expected '(', found '<'` and refused to
parse. Generic functions must be at module scope. The same
restriction applies to plain `fn` declarations inside a function
body — only `let`-bound lambdas are allowed there. The
workaround is to either (a) move every helper to the module
top level (pollutes the namespace), or (b) inline the
lambda at every call site (`let lift = fn(r) { ... }` inside the
provider is fine, but you can't reuse it).

**Recommendation:** This is a real ergonomic tax. Allowing local
`fn` declarations and generic functions inside function bodies
would remove a class of "where does this helper go?" decisions
that currently have no good answer.

### F5 — Route patterns don't strip query strings. (Low, ~5 min)

`route.get("/search", ...)` does not match `GET /search?q=bad`.
The path is treated as the literal string including the query, so
the route 404s. I had to redesign the search endpoint to take the
query as a path segment (`/search/:q`). For a service-oriented
language, not handling the query string is a gap that the user
will hit on day one.

### F6 — Structs serialize as their nested record shape. (Low, cosmetic)

`BookId { value: 1 }` encodes as `{"value": 1}`, so a `Book` with
an `id: BookId` field becomes `{"id":{"value":1}, ...}`. Most web
APIs flatten this to `{"id":1, ...}`. The standard library's
record types serialize flat; structs serialize nested. I worked
around it by either using a record type or exposing the value
field as `pub`. A `derive(Flat)` or a clear "flatten in JSON" knob
would close this gap.

### F7 — `Duration` isn't parseable from `config.from_env`. (Low, ~5 min)

`Settings.report_every: Duration` failed at runtime with
"expected an integer" because `Duration` does not have a
string/environment codec. I had to switch the field to
`report_every_ms: Int` and call `time.millis(...)` at use site.
The runtime uses durations everywhere (`task.sleep(10ms)`,
`worker.on_failure({backoff: 10ms})`); the configuration layer not
accepting them is an inconsistency between two parts of the
standard library.

### F8 — Pipeline `error` callbacks need an explicit type annotation. (Low, ~10 min, 2 attempts)

```badger
stage error_to_reply() {
    error(ctx, error) { ... }   // <-- callback does not fire
}
```

The callback ran in `enter` mode but did not fire on handler
errors. Adding `error(ctx, error: AppError)` made it fire. Why
inference works for `enter` and not for `error` is not obvious from
the docs. The framework's example happens to have a short
`AppError`-typed payload so the annotation is short.

### F9 — Channels don't survive across the system graph. (Medium, ~15 min, feature dropped)

I planned a typed `Channel<Change>` for the API to publish
notifications, with a background worker subscribed to it. The
channel's `open` is a `use`-block callback:

```badger
let bus: channel.Channel<Change> = channel.open({ ..channel.defaults, capacity: 64 })?
```

`bus` cannot escape the `use` block. To share the receiver across
providers, the channel has to live somewhere — but there is no
system-level primitive for "channel resource owned by a node". I
ended up dropping the feature and using the polling-based
`stats_reporter` worker instead. The `Channel` is great for
request-scoped coordination; for long-lived publishers, the API
encourages an awkward shape.

### F10 — Strings can't span lines, multi-line SQL has to be flattened. (Cosmetic)

The first attempt at a `CREATE TABLE` migration failed with
"unterminated string". I had to flatten it onto one line. For a
language where SQL is a first-class feature (typed SQL,
`sqlite.query_*` checks SQL against the schema), the inability to
write multi-line SQL strings is a daily papercut.

### F11 — Compile errors can point at the wrong line. (Medium cumulative, ~20 min across many attempts)

Several times the error was reported at the closing `}` of a
block when the actual problem was twenty lines up. The parser's
recovery is decent for lexical errors, but for type errors that
cascade — especially around generics, effect rows, and the pipe
operator — the diagnostic points at the place where inference
failed, not where the user wrote the bad code.

### F13 — `result.map` does not transform the error type. (Low, ~5 min)

`result.map(f)` changes the `T` but leaves `E` alone. Several
"the wrong error type" issues came from assuming otherwise. A
combined `result.map_both(ok, err)` or a `do`-notation-style
syntax would make these flows less error-prone.

## Things I would have liked to demonstrate but couldn't

- **Channel-based notifications** (F9): dropped because of the
  channel lifetime problem. The plan was: API publishes on every
  write; a background listener subscribes and aggregates; an `/events`
  endpoint streams the running totals back. The current `stats_reporter`
  polls the database instead. Polling is fine for a demo; an event
  bus would be better for production.
- **Contracts on `trim`.** I tried `requires value != ""` but
  validation calls `trim("")` legitimately and the contract
  fired as a precondition violation. I removed the contract.
  `badger contracts` would have been useful to confirm that
  every caller satisfies it — but I did not have time to design
  the right contract (something like "value is a UTF-8 string of
  length ≤ M", parameterised).
- **`route.typed` path-param decoding.** The `route.typed` form
  has a typed `params: { id: Int }` argument that decodes the
  `:id` path segment. I worked around this by using `route.get`
  and parsing the path myself. That works, but it loses the
  schema-validation benefit the typed routes are meant to provide.
- **Lazy `Seq` of search results.** The book search returns a
  `List<Book>` via SQL. A `Seq<Book>` over a streaming query would
  show off the lazy-sequences story better. I left it as a list.

## Net assessment

The single-file service took roughly twelve `badger check` cycles
to land clean and another six or so to land all eight tests
green. About half of those cycles were diagnosing friction items
above; the other half were ordinary application design.

If this were a "what does this language feel like for an everyday
service?" evaluation, I would say:

- A Go developer with `chi` and `database/sql` ships a CRUD
  service in an afternoon and does not have to read
  `crates/badger_hir/std/route.bdg` once.
- A Kotlin/Spring developer ships one in a day with similar
  margin.
- Badger's strengths — effect discipline, derived error types,
  typed records, the system graph, supervised workers, in-process
  tests — all show up in this program. They are real. But they
  arrive attached to enough rough edges (F1, F2, F4, F9, F11) that
  a fluent Badger developer probably wins on the longer-tail
  cases, while a one-shot user is more likely to give up first.

The single highest-leverage fix is **F1** (handler errors must be
a first-class outcome, not a silent 500). Everything else on the
friction list is small in comparison; fixing F1 alone would have
made this program take about six fewer iterations to land.