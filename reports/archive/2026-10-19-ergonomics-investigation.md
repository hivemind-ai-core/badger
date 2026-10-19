# Ergonomics investigation — what would make Badger more pleasant

This is an audit of rough edges in the language, the standard library,
and the tooling, gathered by reading the spec, the stdlib source, the
CLI reference, the experience report, and the example programs. It
is not a roadmap. The goal is to surface concrete papercuts and
group them so a maintainer can pick the cheapest high-impact items
first.

A papercut is a small, repeated cost on the way to a working program.
A pain point is something that costs a meaningful slice of a session.
A gap is something the user expected to be there and is not. This
report groups its findings by where the fix lives: in the language,
in the stdlib, in the standard web framework (route + system + HTTP),
in the tooling, in the diagnostics, and in the documentation. Each
item names the symptom, an example, the recommended fix, and an
impact estimate.

## Methodology

- Read the language spec chapters (syntax, types, functions, memory,
  tasks, systems, runtime).
- Read every standard library source file under
  `crates/badger_hir/std/`, focusing on `route.bdg`, `system.bdg`,
  `http.bdg`, `sqlite.bdg`, `result.bdg`, `option.bdg`, `text.bdg`,
  `bytes.bdg`, `list.bdg`, `map.bdg`, `set.bdg`, `config.bdg`,
  `channel.bdg`, `task.bdg` (via tasks), `app.bdg`, `pipeline.bdg`,
  `worker.bdg`, `log.bdg`, `trace.bdg`, `endpoint.bdg`, `codec.bdg`,
  `cli.bdg`, `command.bdg`, and `compiler.bdg`.
- Read `docs/reference/cli.md`, `docs/reference/repl.md`,
  `docs/reference/mcp.md`, `docs/reference/config.md`, and the
  guide chapters.
- Read the experience report from `reports/experience-report.md` and
  cross-referenced every friction item it names.
- Grepped the example programs and the stdlib itself for repeated
  patterns (`match .ok .err` chains, `|> result.map_err(...)`,
  `with_status`, `reply_for_error`, `build a Settings record and call
  config.from_env`, `route.get` with `match` cascades).

## Top ten — biggest wins, ordered

These are the items that would change the day-to-day feel of
writing Badger the most. The "effort" column is rough; the
"affected" column says who feels it.

| # | One-liner | Area | Effort | Affected |
|---|---|---|---|---|
| 1 | Routes must preserve handler error status, not silently 500 | stdlib (route) | M | every HTTP service |
| 2 | `?` should work on `Option<T>` when the enclosing error type is `Result<_, E>` | language | M | every request handler, every validator |
| 3 | `bail!` / `must!` / `validate!` macros for early-return validation | stdlib | S | every CRUD service |
| 4 | A `default!` macro / `Default` derive for records and structs | language | S | every Settings / DTO |
| 5 | Local `fn` declarations and local generic functions | parser/typecheck | L | every non-trivial function |
| 6 | Routes strip the query string before pattern matching | stdlib (route) | S | every web service |
| 7 | `Duration` is a first-class config field type | stdlib (config) | S | every configurable service |
| 8 | A `bail_with` / `?` chain helper for cascading `Result` errors in handlers | stdlib | S | every error-mapping handler |
| 9 | A `with_status`/`reply_for_error` helper in the route stdlib (so handlers don't all write one) | stdlib (route) | S | every JSON handler |
| 10 | `badger install` / `badger search` / a `badger completions` command | tooling | S | every new project |

The rest of the report covers these in detail, plus a long tail of
smaller items.

## Findings, by area

### 1. Language: syntax and type rules

#### 1.1 `?` on `Option` is the single highest-leverage syntactic change

**Symptom.** Every request handler that parses a string from the URL
or query string does this dance:

```badger
let id = match params.get("id") {
    .some(s) => match text.parse_int(s) {
        .some(n) => n,
        .none => return reply_for_error(AppError.invalid_input($"id is not a number: {s}")),
    },
    .none => return reply_for_error(AppError.invalid_input("id is required")),
}
```

The spec ([`spec/03-functions-and-errors.md:122`](../spec/03-functions-and-errors.md))
explicitly says: "`Option` must be matched or converted explicitly with
a missing-value error." That is a deliberate choice, and it is the
right choice when the missing-value error is application-specific.
But in the common case — "I just want to propagate this as
`AppError.invalid_input`" — the only escape is the cascading `match`
above, repeated once per parameter.

**Fix.** Allow `?` on `Option<T>` when the surrounding function
returns `Result<_, E>` and `E: From<OptionMissing>`, where
`OptionMissing` is a built-in empty struct with `derive(Error, From)`.
The compiler synthesizes the missing-value error at the `?` site, or
the user writes a one-liner mapping (`with AppError.invalid_input`
in a let-else block, or a `.ok_or` in the pipeline). The same
mechanism handles `let .some(x) = expr else { return err(...) }`
via the existing `let-else` form. The two together collapse the
cascading match to:

```badger
let id = params.get("id").ok_or(AppError.invalid_input("id is required"))?
let id = text.parse_int(id).ok_or(AppError.invalid_input($"id is not a number: {id}"))?
```

or even, with a `From<OptionMissing>` for the AppError, just:

```badger
let id = text.parse_int(params.get("id")?)?
```

**Affected.** Every HTTP request handler, every config parser, every
CLI argument parser. This is the difference between 4 lines of
cascading match and 1 line of `?` at every call site.

**Effort.** M. The type rule and lowering are the work; the rest is
documentation.

#### 1.2 Local `fn` declarations and local generic functions

**Symptom.** Helpers must be at module scope. A `lift<T>(...)` inside
a `provider` body parses as a syntax error. A nested `fn
inner_helper()` is not allowed. The same applies to local generic
parameters. The workarounds — inline lambdas, top-level helpers —
either add noise (`let lift = fn<X>(...)` everywhere) or
namespace pollution (every helper becomes a top-level name).

This was the cause of ~10 minutes and 2 attempts in the experience
report (item F4). It also makes "extract a helper" impossible
inside a function body, which is a real workflow loss.

**Fix.** Allow `fn name<T>(...)` inside any function body. The same
generic-inference and effect rules apply. The declaration is visible
until the end of the enclosing function (or block). The work is
parser + name resolution; type checking is unchanged.

**Affected.** Every non-trivial function with a helper.

**Effort.** L. The parser needs a new "function block" rule and the
resolver needs to track local defns. Worth doing.

#### 1.3 `?` on `Result` does not transform the error type

**Symptom.** `result.map` changes `T` but leaves `E` alone. The
idiomatic pipe `r |> result.map_err(AppError.http)` is awkward
because it requires piping the result first, then passing the
function. The reverse, `result.map_err(r, AppError.http)`, reads
more naturally to a Rust eye but breaks the pipeline.

**Fix.** Either:
- add `result.bimap(ok_fn, err_fn)` (Option already has `map`); or
- add `?`-as-do-notation: a block of `let x = expr?` lines that
  unwraps an `Err` of one type and lets the surrounding function
  return a different `Result`, with conversion via `From`.

**Affected.** Every error-mapping function. Many.

**Effort.** S. The bimap is a one-liner. The `?`-block is a
desugaring in the type checker.

#### 1.4 `Duration` literal `1s` works in code but not in config

**Symptom.** `task.sleep(10ms)` and `worker.on_failure({ backoff: 10ms })`
both accept duration literals. `config.from_env<Settings>` does not
parse them — the `report_every: Duration` field fails with
"expected an integer". The workaround is `report_every_ms: Int` and
`time.millis(...)` at the use site.

**Fix.** Add `Duration` to the config codec. It is a single primitive
type, just like `Int` and `String`. Either accept the literal
(`BOOKS_REPORT_EVERY=10s`) or a millisecond integer; both are common
in the industry.

**Affected.** Every configurable service that uses a duration
(timeout, backoff, retry, cache TTL, rate window).

**Effort.** S.

#### 1.5 Triple-quoted strings are in the spec but not visible enough

**Symptom.** SQL strings have to be flattened onto one line. The
spec says they exist ([`spec/01-syntax.md:101`](../spec/01-syntax.md))
but the first thing a new user tries is to write a multi-line SQL
`CREATE TABLE`, gets `unterminated string`, and is forced to look
for the workaround.

**Fix.** Document the `"""..."""` form in the tour and the SQL
guide. Mention it in the example SQLite migration.

**Affected.** Every database user. Every multi-line string user.

**Effort.** S (documentation).

#### 1.6 `let-else` is good but the `else` must not return

**Symptom.** The spec says the `else` block's type is `Never`: it
must end in `return`, `break`, `continue`, a panic, or another `Never`
return. That is correct, but it means `let .some(x) = expr else {
err(AppError.foo) }` is rejected — the user has to write
`return err(...)`. The `return` is the right choice for explicitness
but feels redundant when the entire pattern is on one line.

**Fix.** Allow the `else` block to end in `err(...)` as a shortcut for
`return err(...)` when the surrounding function returns a `Result`.
Same for `panic(...)` when it returns `Never`.

**Affected.** Every refutable pattern in a Result-returning function.

**Effort.** S.

#### 1.7 `Enum.variant` construction is verbose for one-variant enums

**Symptom.** A single-variant enum like `enum Status { open }` is
written as `Status.open`. Compare to a record: `{ kind: "open" }`.
The leading `Enum.` is six keystrokes and a screen of attention
spent for an enum with one variant.

**Fix.** Allow the bare variant in expression position when the type
is expected (the spec currently forbids this in
[`spec/01-syntax.md:247`](../spec/01-syntax.md#errors-and-continuation-binding)
on principle — but a small backward-compatible relaxation for
single-variant enums in a let-typed position would be useful, and
the framework already does this implicitly for `ok`/`err`/`some`/
`none`).

**Affected.** Domain enums, especially in records and lists.

**Effort.** S to M (parser change, but a small one).

### 2. Standard library: missing or awkward helpers

#### 2.1 `bail!` / `must!` / `validate!` macros for early-return validation

**Symptom.** Every CRUD service has 3-10 validation functions. They
all have the same shape:

```badger
if title == "" { return err(AppError.invalid_input("title must not be empty")) }
if title.len() > 200 { return err(AppError.invalid_input("title too long")) }
if author == "" { return err(AppError.invalid_input("author required")) }
```

**Fix.** Add a small set of helpers to `std.result` (or a new
`std.validate` module):

```badger
pub fn bail<T, E>(error: E) -> T !{Never}            // for `?`-style use
pub fn must<T, E>(condition: Bool, error: E) -> Result<Unit, E>
pub fn must_eq<T: Eq, E>(a: T, b: T, error: E) -> Result<Unit, E>
pub fn check<T, E>(value: T, valid: fn(&T) -> Bool, error: fn() -> E) -> Result<T, E>
```

`bail` lowers to `return err(...)` and works under `?`. The
`must*` family returns a `Result` to use in a sequence. The
`validate!` macro can collect multiple problems into one error.

**Affected.** Every CRUD service, every config parser, every CLI
argument parser.

**Effort.** S (the helpers themselves) to M (the macro).

#### 2.2 A `Default` derive for records and structs

**Symptom.** Every Settings type needs a `default_settings()`
function:

```badger
fn default_settings() -> Settings {
    { host: "127.0.0.1", port: 8080, debug: false, workers: 1, log_level: "info" }
}
```

This is repeated for every type with sane defaults. The runtime
already supports `derive(Eq, Hash, Display, Codec, From, Error)`.
A `derive(Default)` would generate a `default()` function that
returns each field's type's `default()` (with a `default!` macro
or per-type convention for the few built-ins).

**Fix.** Add `derive(Default)`. Define a `Default` protocol. The
derivation needs default values for every type. For built-ins,
`Int: 0`, `Float: 0.0`, `Bool: false`, `String: ""`, `List<T>: empty()`,
`Map<K, V>: empty()`, `Option<T>: none()`. For named types, the
user provides a `default_NAME` function, or the derivation marks
the type as "all fields have defaults" and synthesizes the literal.

**Affected.** Every Settings / DTO / config record.

**Effort.** M.

#### 2.3 A `Builder` derive for record construction

**Symptom.** A record with ten fields is constructed as a literal
in one place, with a long list of every field. Building the same
record incrementally — "default, then override `host` and `port`"
— requires a `{ ..default(), host: x, port: y }` expression,
which is fine, but you have to write the default every time. A
`SettingsBuilder` would be incremental.

**Fix.** Add `derive(Builder)`. Generates a `Builder` type with a
`set(field, value)` method, an `init(default)` method, and a
`build() -> Result<Self, ValidateError>` method. This is a very
common pattern in Go, Rust (`derive_builder`), and TypeScript
(`typebox`). A `derive(Default)` plus `{ ..default, field: value }`
is a workable substitute but ergonomically inferior.

**Affected.** Every record with 5+ fields.

**Effort.** M. Optional but very welcome.

#### 2.4 A `with_status` / `reply_for_error` helper in the route stdlib

**Symptom.** Every JSON handler that wants to map an `AppError` to
a status code writes the same four-arm match:

```badger
fn reply_for_error(error: AppError) -> Result<http.Response, AppError> {
    let body = match error {
        .decode(e) => { error: "bad request", message: e.message },
        .invalid_input(message) => { error: "bad request", message },
        .not_found(id) => { error: "not found", message: $"no book {id}" },
        _ => { error: "internal server error", message: error.display() },
    }
    let status = match error {
        .decode(_) | .invalid_input(_) => 400,
        .not_found(_) => 404,
        _ => 500,
    }
    http.json(status, body) |> result.map_err(AppError.http)
}
```

This is the same function in every JSON API. It should be in the
stdlib, and the route system should call it automatically when the
handler returns `Err`.

**Fix.** Two options:
- Put `reply_for_error` and `with_status` in `std.http` so every
  project can use them. Document the pattern.
- Better: make the route system understand an `AppError`-shaped
  error and reply with the matched status. See item 3.1 below.

**Affected.** Every JSON API.

**Effort.** S (the helper) to M (the route-side change).

#### 2.5 `Bytes.to_text_or(default)` and similar one-liners

**Symptom.** The pattern `body.to_text().unwrap_or("?")` is common
in tests and in handler logging. So is `params.get("id").unwrap_or("")`
and `result.unwrap_or(default)`. The existing API forces the two-
step call.

**Fix.** Add `unwrap_or`-shaped methods on the relevant types, or
make `unwrap_or` work on `Result<Bytes, _>` via a `to_text_or` method.

**Affected.** Test code, handler logging.

**Effort.** S.

#### 2.6 `Channel<T>` cannot outlive the `use` block in `channel.open`

**Symptom.** A `channel.open` returns an `Endpoints<T>` record
(`sender` and `receiver`) that is bound to the use block's
lifetime. You cannot share the `sender` with one provider and the
`receiver` with another. The workaround is `channel.create` (a
plain allocation without a scope), but the API in the stdlib does
not document this well, and `create` is not bound to a system node
either.

**Fix.** Make the system graph able to own a channel. Add a
`channel.resource` provider that registers a `Endpoints<T>` in a
system node, so subscribers can pull the receiver in their config
and publishers can pull the sender. The channel is then closed
when the system halts. This is what makes "background notifier"
patterns viable without falling back to polling.

**Affected.** Any service that wants request-time events observed
by a background worker.

**Effort.** M. Probably the right investment.

#### 2.7 `if let Some(ok)` for `Result` is missing

**Symptom.** A common pattern is "if this call succeeds, log it;
otherwise ignore":

```badger
let _ = match store.list() {
    .ok(books) => log.info("loaded", { count: books.len() }),
    .err(_) => (),
}
```

This needs a four-line match when one would do.

**Fix.** Add `if let .ok(value) = expr { ... }` (it is already
mentioned in the spec but is not in the syntax; check whether
it actually works). Alternatively, `result.tee(expr, fn)` returns
the same `Result` but runs the function on `.ok`.

**Affected.** Logging, tracing, optimistic writes.

**Effort.** S.

### 3. Standard web framework: the `route` / `http` / `system` trio

#### 3.1 Handler `Err` becomes a silent 500 — the single biggest papercut

**Symptom.** Detailed in `reports/experience-report.md` item F1.
A handler returning `Err(AppError.invalid_input("title required"))`
becomes a `500 "internal server error"`. The application intended
a 400. The route system does this silently.

**Fix.** Carry the status in the error. Two clean options:
- An `http.StatusError` type: `{ status: Int, error: AppError }`.
  The route system catches this and replies with `status` and a
  JSON body derived from `error.display()`.
- A `derive(HttpError)` on the application enum. The route system
  reads metadata attached to each variant by the derivation
  (`{ status: 404, body: { error: "not_found", ... } }`) and
  generates the reply.

The second is more declarative and gives OpenAPI the per-status
shape it needs.

**Affected.** Every HTTP service. This is the single most-reported
friction in the experience report.

**Effort.** M.

#### 3.2 Routes strip the query string before pattern matching

**Symptom.** `route.get("/search", ...)` does not match
`GET /search?q=bad`. The path is treated as a literal including
the query, so the route 404s.

**Fix.** Split the request URI on `?` before matching the pattern.
Put the query into a `Params` record (already done by the
`route.typed` form for `:name` parameters, but the query string
is not parsed). Add `route.params(request) -> Params` so a
`route.get("/search", ...)` handler can also call
`params.get("q")` to read the query string.

**Affected.** Every web service that uses query parameters. That
is, every web service.

**Effort.** S.

#### 3.3 `route.typed` path-param parsing — re-do `with_status` and path parsing

**Symptom.** The typed route form decodes path params into a
record (`{ id: Int }`), but to use it the user has to return
`(Int, R)`. The user then has to wrap that in `with_status` to
turn it into an `http.Response`, and the route system maps any
handler `Err` to a 500. So the typed route has a worse error story
than the plain `route.get`. The experience report worked around
this by parsing the path manually:

```badger
let id = match params.get("id") {
    .some(s) => match text.parse_int(s) {
        .some(n) => n,
        .none => return reply_for_error(AppError.invalid_input($"id is not a number: {s}")),
    },
    .none => return reply_for_error(AppError.invalid_input("id is required")),
}
```

**Fix.** Make `route.typed` understand the same `with_status`
wrapper as the user-side. Or, better, make the route system
understand the same error type as `route.get` and propagate the
status from the error.

**Affected.** Every typed route that has a 404 or 400 case.

**Effort.** S (if 3.1 is done) or M (independent of 3.1).

#### 3.4 `task.scope()` inside a provider hangs the system

**Symptom.** Detailed in `reports/experience-report.md` item F2.
`use scope <- task.scope()` in a provider body schedules a task
that disappears when the provider returns. The runtime hangs at
startup (no port, no log, no workers). The only signal is "no
listening port" — the user has to look at the fd table to see
"two threads, no eventpoll".

**Fix.** Either reject `task.scope()` inside a provider at compile
time (cleanest), or rewrite it to "register with the system" so
the task lives as long as the system does. The first is a small
parser change; the second is what `scope.worker` does.

**Affected.** Anyone who tries to add a background task in a
provider.

**Effort.** S to compile-time reject; L to "make it work".

#### 3.5 Pipeline `error` callbacks need an explicit type annotation

**Symptom.** Detailed in the experience report item F8. A stage's
`error(ctx, error)` callback does not fire without the explicit
type annotation `error(ctx, error: AppError)`. The diagnostic is
silent failure, not an error.

**Fix.** Either always infer (and document the rule), or always
require the annotation (and add a diagnostic when it is missing
in a pipeline error stage). The current behavior is the worst of
both: inference works sometimes, and a missing annotation fails
silently.

**Affected.** Every error-handling pipeline stage.

**Effort.** S.

#### 3.6 `let _ = log.warn(...)` to discard the result is noisy

**Symptom.** Logging never fails, but `log.warn` returns a `Result`
that has to be explicitly discarded: `let _ = log.warn(...)`.
The prelude's E0337 says "you cannot ignore a Result" and the
escape hatch is `let _ = expr`. The escape hatch is fine, but it
is repeated dozens of times in a service with structured logging.

**Fix.** Either make the prelude's logging family return `Unit`
explicitly (they are infallible), or add a `log_` family that
returns `Unit` for cases where the user does not care. The
`log.with(fields, body)` form already returns the body's result,
so it cannot change; the one-shot `log.info(...)` etc. could
return `Unit`.

**Affected.** Every file that uses structured logging.

**Effort.** S.

#### 3.7 A `respond!`-style shorthand for `Result<http.Response, AppError>` from an error

**Symptom.** A `pipeline { error_to_reply(), ... }` stage that
maps every `AppError` to a reply needs the `error_to_reply` stage
to be written by hand every time. The pattern is in
`examples/design/service/src/main.bdg:30` and in
`reports/experience-report.md` (the F1 workaround).

**Fix.** Provide a `respond_with(fn(error) -> http.Response)`
stage factory, plus a `derive(HttpReply)` on the application
error enum that generates the right mapping. The route system
and the pipeline stage cooperate so the user only writes the
error enum.

**Affected.** Every JSON API.

**Effort.** M.

#### 3.8 `pipeline` does not have a `around` or `try` stage

**Symptom.** A common pattern is "time this handler, log on the
way out, and on error log and convert". The user has to write
`enter`, `leave`, and `error` callbacks in three separate
stages. A single `around` callback that runs the handler and
sees the result would express this in one place.

**Fix.** Either add an `around(ctx, body) -> ...` stage, or make
`enter`/`leave`/`error` accept a single body. The current shape
is fine for the common case but verbose for the "wrap a handler
with a try" case.

**Affected.** Every pipeline that combines timing + access log
+ error mapping.

**Effort.** M.

### 4. Tooling

#### 4.1 `badger install NAME` and `badger search QUERY`

**Symptom.** Adding a dependency today means editing `badger.toml`
by hand and running `badger update`. Searching for a stdlib
function means `badger query api std.list` (or some module name)
and reading JSON. There is no `badger install` and no `badger
search`.

**Fix.** Add `badger install NAME [VERSION]` that adds the
dependency to `badger.toml` and rewrites the lockfile. Add
`badger search QUERY` that searches the installed stdlib (and
project modules) for the query and returns a list of
matching definitions. Both have MCP counterparts (`install`,
`search`).

**Affected.** Every new project; every "where is X in stdlib?"

**Effort.** S to M.

#### 4.2 `badger completions SHELL`

**Symptom.** The `badger` CLI has ~20 subcommands and many
flags. Typing them by hand is tedious and error-prone. Shell
completion is a one-day project that saves real time.

**Fix.** Add `badger completions bash` (and `zsh`, `fish`,
`powershell`) that prints a completion script to standard output.

**Affected.** Every CLI user, every day.

**Effort.** S.

#### 4.3 `badger watch` as a first-class command

**Symptom.** `badger run --watch` exists, but for "check on
save" the user has to either run `badger check` in a loop or
script it. A `badger watch` command that watches a path and
re-runs `badger check` (or `badger test`, or `badger run`) is
the obvious command.

**Fix.** Add `badger watch [PATH] [-- COMMAND...]`. The default
is `badger check`; `-- COMMAND` overrides.

**Affected.** Every iteration.

**Effort.** S.

#### 4.4 `badger doctor` / `badger info`

**Symptom.** A user with a confusing bug report has no command
that prints the relevant environment: the toolchain version,
the target triple, the install path, the running runtime
settings, the working directory, the badger.toml path. They
have to assemble it manually.

**Fix.** Add `badger doctor` that prints one section per thing:
versions, paths, config, runtime settings, recent diagnostics.
`badger doctor --json` for agents.

**Affected.** Every bug report, every fresh install.

**Effort.** S.

#### 4.5 `badger benchmark` as a first-class subcommand

**Symptom.** `badger run --bench` exists but is undocumented and
invisible. Benchmarking is a real workflow, and it deserves a
top-level command with a name and a `--compare` flag.

**Fix.** Add `badger bench [PATH]` and `badger bench --compare
BASE_FILE` (compare against a saved baseline). Hook into the
existing `bench/` infrastructure.

**Affected.** Every performance investigation.

**Effort.** S to M.

#### 4.6 `badger rename` as a CLI

**Symptom.** `badger query rename --line N --col N --name NEW`
works, but the CLI form is one indirection away from a normal
rename. A `badger rename --line N --col N --name NEW PATH` would
be more discoverable.

**Fix.** Add the CLI wrapper. The query already does the work.

**Affected.** Every rename.

**Effort.** S.

#### 4.7 Apply-on-save for the formatter

**Symptom.** `badger format --check` exists, but the typical
"format on save" experience requires the editor to be told.
The LSP could format on save; check whether it does. If not,
make sure it does and document it in `docs/editors.md`.

**Affected.** Every save.

**Effort.** S (if not already there) to M (if a feature is
missing in the LSP).

#### 4.8 A `badger new --template=...` family

**Symptom.** `badger new NAME` makes a project with one entry
and one test. That is a great default, but a `badger new NAME
--template=service` (with an HTTP service skeleton, a database
provider, and a test) would be a better starting point for the
most common case. The framework example is the de-facto
template today; it should be a `--template=framework` option
that scaffolds the same files.

**Fix.** Add `--template=cli|service|framework|lib|worker` to
`badger new`. The templates live in a `templates/` directory
in the repo.

**Affected.** Every new project.

**Effort.** M.

#### 4.9 `badger repl --load FILE`

**Symptom.** The REPL accepts only typed input, line by line.
Loading a file means copy-pasting each line.

**Fix.** Add `badger repl --load FILE` that reads and evaluates
each top-level item.

**Affected.** Anyone who wants to share a REPL session.

**Effort.** S.

### 5. Diagnostics

#### 5.1 The wrong-line problem

**Symptom.** Detailed in `reports/experience-report.md` item F11.
A type error that cascades — pipe expansion, generic
unification, effect rows — is reported at the place where
inference failed, not where the user wrote the bad code. The
worst case is the closing `}` of a block being flagged for an
error twenty lines up.

**Fix.** When inference fails partway through an expression,
keep walking up the expression and pick the smallest sub-
expression whose type is "the one that doesn't fit". This is
expensive to do well but cheap to do "good enough": emit a note
at the original inference site in addition to the primary label.

**Affected.** Every iteration.

**Effort.** M. Probably the highest-impact diagnostic change.

#### 5.2 "Did you mean...?" suggestions for unknown names

**Symptom.** The error `E0127: unknown name` is a single line.
A typo in an identifier (`reslt` for `result`) is a multi-
minute search.

**Fix.** When the unknown name is close to a name in scope
(Levenshtein distance ≤ 2, or a name from a similar module),
suggest the closest match. The check is a few hundred lines
in the resolver.

**Affected.** Every typo.

**Effort.** S.

#### 5.3 Auto-fix for common patterns

**Symptom.** A handful of patterns come up again and again:
- `result |> result.map(...)` should suggest `result.map(...)`.
- `result.map_err(AppError.x)` with no `?` should suggest adding
  `?`.
- An unused `Result` (E0337) should suggest `let _ = ...` or
  `?.ok_or(...)`.
- A pipe with `value |> module.fn(args)` where `module.fn`
  doesn't take `value` as the first arg should suggest the
  right order.

**Fix.** A small library of "common fix" patterns. The LSP can
expose them as `CodeAction`s; the CLI can apply with
`badger check --apply-fixes`.

**Affected.** Every common mistake.

**Effort.** M.

#### 5.4 A "did you mean to import...?" diagnostic

**Symptom.** A call to a function the user has not imported
shows up as `E0127: unknown name`. The fix is to add the
import. A `std` member used without an import is the most
common case.

**Fix.** For names that are in the stdlib but not imported,
suggest the import line. (The current `Note: ... is a standard
module: add import ...` does this for some cases; it should be
on every `E0127`.)

**Affected.** Every stdlib call from a new project.

**Effort.** S.

#### 5.5 Per-effect-row explanations

**Symptom.** The compiler can tell you that a function has
`!{Io, Wait, State}`, but it does not say *which call* brought
in the `State` effect. `badger explain effect` lists the
function but not the cause.

**Fix.** Make `badger explain effect` list, for each effect,
the call sites that require it. (This is the "constraint
explanations with origins" item in
[REAL_WORLD_USE.md](../REAL_WORLD_USE.md).)

**Affected.** Every effect-debugging session.

**Effort.** M.

### 6. Standard library: smaller ergonomic helpers

#### 6.1 A `Record` derive that produces `clone` (or makes records Copy)

**Symptom.** Records are "immutable, but not Copy". To pass a
record by value to a function, the record has to be moved
unless the receiver is `&Self`. This is correct, but it means
many `fn foo(record: &Record)` signatures where `fn foo(record:
Record)` would be more pleasant.

**Fix.** Records whose fields are all `Copy` could be `Copy`
themselves, with a `derive(Copy)` opt-in. The current "records
are immutable" model already implies cheap sharing; making
`Copy` explicit is the missing piece.

**Affected.** Every record passed by reference that could be by
value.

**Effort.** M.

#### 6.2 `Map` and `Set` literal syntax

**Symptom.** Map literals use `map { "key" => value }`, which
is fine, but `set { #a, #b }` is fine too. The combined
literal:

```badger
let users_by_id: Map<Int, User> = map { 1 => user1, 2 => user2 }
```

is good. The missing thing is `{}` to mean `map.empty()` and
`{ key => value, ... }` to mean "construct a map" — both work
today, but the type inference sometimes needs a hint.

**Affected.** Map construction.

**Effort.** S.

#### 6.3 `List.sorted_by_key` and friends

**Symptom.** `list.sort_by(key)` is a one-liner. So is
`list.min_by(key)` and `list.max_by(key)`. There is no
`list.sorted_by_key` (the natural complement to Python's
`sorted(key=...)`). The user has to write
`list.sort_by(list.map(list, key))` or similar.

**Fix.** Add `sort_by_key`, `group_by_key`, `count_by_key` if
not already present.

**Affected.** Every list-shaped computation.

**Effort.** S.

#### 6.4 A `Result`/`Option` from predicate

**Symptom.** `validate(input: NewBook) -> Result<NewBook, AppError>`
is six lines of guard clauses.

**Fix.** A `validate` helper that takes a list of
`(condition, error_message)` pairs and returns the first
violation:

```badger
pub fn validate<E>(value: T, rules: List<(Bool, E)>) -> Result<T, E>
```

**Affected.** Every validator.

**Effort.** S.

#### 6.5 A `kv` (key-value) helper for environment, headers, form data

**Symptom.** Every config block, every header read, every form
parser, every SQL parameter uses the same shape: a list of
`List<(String, T)>` for input, a `Map<String, T>` for output.
The conversion between them is hand-written.

**Fix.** A `kv` module with `from_list`, `to_list`, `merge`,
`filter`, `map_keys`, `map_values`, and the like. The Map type
already has most of these; the missing piece is the `List`
form used for parsing.

**Affected.** Every config block.

**Effort.** S.

#### 6.6 A `cli` argument parser module

**Symptom.** The example in the CLI reference shows hand-rolled
`arg.parse_int().unwrap_or(default)` calls. There is a
`std.cli` module (`crates/badger_hir/std/cli.bdg`), but it is
not the obvious thing to reach for.

**Fix.** Promote the existing `std.cli` helpers (or build a
new module) so a user can write:

```badger
let args = cli.parse({ host: cli.String("127.0.0.1"), port: cli.Int(8080) })
```

The framework example in `examples/framework/src/cli.bdg` is
non-trivial; the equivalent with a `std.cli` helper would be
half the lines.

**Affected.** Every CLI program.

**Effort.** M.

### 7. Documentation

#### 7.1 A "common errors" page

**Symptom.** E0001 through E0618 — 151 codes. New users
encounter the same five: "unknown name", "type mismatch",
"effect not permitted", "expected integer", "argument
count". The catalog exists, but is not connected to a
"common pattern" page.

**Fix.** A `docs/diagnostics.md` that groups the 20 most
common codes by what the user was probably trying to do, with
a one-paragraph fix per group. Cross-link from each `badger
explain` output to the relevant section.

**Affected.** Every new user.

**Effort.** S.

#### 7.2 An "idiomatic Badger" guide

**Symptom.** The framework example shows the right way to
organize a non-trivial service, but the pattern is implicit.
"Here is how to structure a system graph" is in chapter 8 of
the guide, but the related "here is how to structure your
errors", "here is how to validate", "here is how to test",
"here is how to log" are scattered.

**Fix.** A `docs/idioms.md` with short, opinionated patterns
for each of: error types, validation, configuration, routes,
pipeline stages, background workers, tests. Cross-link to the
relevant guide chapter.

**Affected.** Every user.

**Effort.** M.

#### 7.3 A migration cookbook

**Symptom.** "I have a Go service / a Python script / a Rust
crate / a TypeScript server. How do I write the equivalent in
Badger?" is an unanswered question.

**Fix.** A `docs/migration.md` for the most common source
languages. Each section is a 1-page example: a small
representative program in the source language, the Badger
equivalent, a list of the differences the user has to know
about.

**Affected.** Every new user coming from somewhere.

**Effort.** M.

#### 7.4 An "explain the type system" page

**Symptom.** Row inference, effect rows, the difference
between `derive(From)` and a manual `From` impl, the
`?` operator's `From` resolution — none of this is laid out
in one place. The spec has it, but the spec is dense.

**Fix.** A `docs/inference.md` that walks through each
inference rule with one example, in plain language.

**Affected.** Every user who hits a type error.

**Effort.** S.

### 8. Performance / dev loop

#### 8.1 Faster incremental `badger check`

**Symptom.** The first check of a project is fast (~50ms
based on the BACKLOG numbers), but a check after a one-line
edit re-parses and re-checks every entry, every task, every
imported module, and (for the parser and type checker) most
of the standard library. The BACKLOG already plans C1.2b,
C1.3, C1.4.

**Fix.** Continue the work in
[BACKLOG.md](../BACKLOG.md#compiler-and-runtime-performance-optimization_planmd).
A per-file parse cache keyed on content hash; a checked-
library reuse pass; a reusable compiler context.

**Affected.** Every iteration.

**Effort.** L (already in flight).

#### 8.2 A `badger run --incremental` for `badger test`

**Symptom.** `badger test` re-checks the whole project every
invocation, even if only one test changed. For a project with
50 tests across 20 modules, the edit/test cycle is dominated
by the check.

**Fix.** Reuse the parse/check cache. Optionally, run only
the test files that import changed files (the test dependency
graph is the right structure to drive this).

**Affected.** Every test cycle.

**Effort.** L.

### 9. Other

#### 9.1 A `badger init` for non-program uses

**Symptom.** The current `badger new NAME` creates a project
for a single program. There is no `badger init` for "turn
this directory into a badger project" or for "scaffold a
library".

**Fix.** Add `badger init [--lib] [PATH]`. `badger init .`
turns the current directory into a library project.

**Affected.** New projects.

**Effort.** S.

#### 9.2 A `badger test --watch` or `badger test --changed`

**Symptom.** `badger test` runs every test. There is no
`--watch` to re-run on save, and no `--filter FILE` to run
only tests in a changed file.

**Fix.** `badger test --watch` is a thin wrapper around
`badger watch -- badger test`. `badger test --changed` uses
the file dependency graph.

**Affected.** Test iteration.

**Effort.** S.

#### 9.3 More `badger generate` templates

**Symptom.** `badger generate feature` and
`badger generate worker` exist. `badger generate service`,
`badger generate cli`, `badger generate library` do not.

**Fix.** Add the missing generators. Each is a few hundred
lines of scaffolding code.

**Affected.** Project creation.

**Effort.** M.

#### 9.4 A `badger doc PATH --open` command

**Symptom.** `badger doc PATH --out DIR` writes Markdown
pages. There is no way to view them in a browser or open
the index file from the CLI.

**Fix.** Add `--open` that opens the generated `index.md`
in the default browser. Or write a single-page HTML version.

**Affected.** Documentation reading.

**Effort.** S.

#### 9.5 A `badger repl` history and completion

**Symptom.** The REPL has no history (up-arrow) and no
completion (tab). Both are basic interactive features.

**Fix.** Wire the REPL up to the line editor
(`rustyline` or `reedline`). Save history to a file
(`~/.badger_history`) by default.

**Affected.** Every REPL session.

**Effort.** S to M.

## What the experience report said, cross-referenced

The experience report from
[reports/experience-report.md](experience-report.md) lists 11
friction items (F1-F11, two numbers skipped). Here is where each
one fits in this audit's recommendations:

| Experience report item | This report's recommendation |
|---|---|
| F1 — Handler `Err` becomes silent 500 | 3.1, 3.3, 3.7 |
| F2 — `task.scope()` in a provider hangs the system | 3.4 |
| F3 — Module function vs method split | 2.5 (consistency), 1.6 (let-else) |
| F4 — No local `fn` or local generics | 1.2 |
| F5 — Routes don't strip query strings | 3.2 |
| F6 — Structs serialize as nested | already a known stdlib item; not in this audit |
| F7 — `Duration` not in config | 1.4 |
| F8 — Pipeline `error` needs type annotation | 3.5 |
| F9 — Channels can't cross system boundary | 2.6 |
| F10 — Strings can't span lines (already do) | 1.5 |
| F11 — Compile errors at the wrong line | 5.1, 5.3 |

Items the experience report did not cover but this audit did:

- 1.1 (`?` on Option), 1.3 (`?` on `Result` error type), 1.6
  (let-else err shortcut), 1.7 (single-variant enum ergonomics)
- 2.1 (bail! / must! / validate!), 2.2 (Default derive), 2.3
  (Builder derive), 2.4 (with_status / reply_for_error in
  stdlib), 2.5 (unwrap_or one-liners), 2.7 (tee)
- 3.6 (logging returns Unit), 3.8 (pipeline `around`)
- 4.1-4.9 (tooling improvements)
- 5.2 ("did you mean"), 5.3 (auto-fixes), 5.4 (import
  suggestions), 5.5 (per-effect explanations)
- 6.1-6.6 (stdlib helpers)
- 7.1-7.4 (documentation)
- 8.1-8.2 (performance)
- 9.1-9.5 (other)

## Reading the report

If you have a few hours:
1. **1.4** (`Duration` in config) — one-line change, removes a
   known footgun.
2. **3.2** (routes strip query) — one-line change in the
   matcher, removes the most-reported "why doesn't my route
   match?".
3. **2.4** (`with_status`/`reply_for_error` in stdlib) — small
   helpers, removes boilerplate from every JSON handler.
4. **1.5** (document `"""..."""`) — documentation only,
   removes a per-session surprise.
5. **5.4** (suggest imports in `E0127`) — one-line change in
   the diagnostic builder, removes a per-session lookup.

If you have a day:
6. **1.1** (`?` on Option) — type rule + lowering; touches the
   type checker and the prelude.
7. **3.1** (handler `Err` becomes a typed error) — design +
   route stdlib + a `derive(HttpError)`.
8. **2.1** (`bail!` / `must!` / `validate!`) — stdlib helpers
   that every CRUD service uses.
9. **4.1-4.6** (tooling) — a half-day of CLI work, a real
   workflow improvement.
10. **3.4** (reject `task.scope()` in providers at compile
    time) — one diagnostic, one parser rule.

If you have a week:
11. **1.2** (local `fn` and local generics) — parser + name
    resolution; the highest-impact language change after
    "errors must be first-class".
12. **2.2** (`derive(Default)`) and **2.3** (`derive(Builder)`)
    — type-checker work + macro / derivation.
13. **5.1** (correct diagnostic line numbers) — diagnostic
    infrastructure.

If you have a quarter:
14. **3.1 + 3.7 + 3.3** (the full handler-error story: typed
    errors, `respond!`, per-error status, OpenAPI shape).
15. **8.1 + 8.2** (faster incremental check) — already in
    BACKLOG.
16. **2.6** (channels as system resources) — system + channel
    integration.

If you have a year: the migration cookbook (7.3), the
toolchain (4.x), and the per-effect explanations (5.5) are
all worthwhile. The biggest single change for "feel" is the
`derive(HttpError)` + `?` on Option combination: together,
they turn every JSON handler from a 10-line match cascade
into a 2-line `let x = expr?; do_something(x)?` flow, and
they make the error path actually right instead of
silently wrong.

## Closing note

Most of the items in this report are *one day* or *one
week* changes, not architectural ones. The shape of the
language and stdlib is mostly fine. The missing pieces are
the small connective tissue: the helpers, the derive
machinery, the route-error story, the `?`-on-Option rule,
the `Duration`-in-config codec, the channel-as-resource
integration. Each one is a small step. Taken together,
they would close most of the gap between "the language I
saw" and "the language I would want to use every day".

The single highest-leverage recommendation in this entire
report is **3.1** (handler errors must be a first-class
outcome, not a silent 500), carried by **3.3** (route.typed
and the error story should match) and **2.1** (bail! /
must! / validate! helpers). Those three together fix the
dominant pain point of the experience report and turn the
crud-service template from "match every variant in every
handler" into "let the error flow do the work".