# Library and framework changes — ergonomics backlog for the library team

**Date:** 2026-10-19
**Owner:** library team (standard library, extended library, application services framework) — may be executed by a single agent or split across two
**Status:** ready for pickup — every item is verified against the tree at `83c6568`
**Companion report:** [2026-10-19-language-and-compiler-changes.md](2026-10-19-language-and-compiler-changes.md) — the language, compiler, diagnostics and runtime-semantics work is in that file, and it is the compiler team's to schedule. This file is everything that can be landed without touching the compiler, plus the small number of items that are blocked on it.

## What this report is

This is the library/framework half of the merged ergonomics work. It
combines the **ergonomics investigation** (a systematic audit of the
language, the standard library, the web framework, the tooling, the
diagnostics and the docs) with the **experience report** (a
first-person account of writing `examples/13_rest_service.bdg`, a
673-line database-backed REST service, from a blank slate).

Everything here lands in one of three places:

- **the standard library** — `std.result`, `std.option`, `std.list`,
  `std.bytes`, `std.text`, `std.cli`, `std.validate` and friends
  (section 1);
- **the extended library** — the layers that sit on the standard
  library: configuration, codec/JSON, channels, and the developer
  tooling built on them (sections 2 and 4);
- **the application services framework** — the `route` / `http` /
  `system` / `pipeline` trio that a service is written against
  (section 3).

Section 3 is split the way the framework design is: **amendments**
(change what already exists), **improvements** (same capability, better
shape), **additions** (new capability). Section 4 covers the CLI and
tooling surface, and section 5 the documentation.

The split with the compiler team is deliberate: the two reports can be
worked in parallel, and every dependency that crosses the boundary is
listed explicitly in [§6](#6-compiler-dependencies) rather than
discovered halfway through.

## How to read an item

Each item carries a stable ID, the symptom, the evidence (file and
line), the fix, the effort (S ≈ 1 day, M ≈ 1 week), and who feels it.
Items marked **blocked on compiler** name the compiler item they wait
for.

## 1. Standard library

### S-01 — `bail!` / `must!` / `validate!` for early-return validation

**Symptom.** Every CRUD service has three to ten validation functions
and they all have the same shape:

```badger
if title == "" { return err(AppError.invalid_input("title must not be empty")) }
if title.len() > 200 { return err(AppError.invalid_input("title too long")) }
if author == "" { return err(AppError.invalid_input("author required")) }
```

**Evidence.** No such helpers and no `std.validate` module exist
anywhere in the tree; the only `validate` hits are unrelated
(`std/list.bdg:260`, `std/route.bdg:222`, `std/command.bdg:98`).

**Fix.** Add a small family to `std.result`, or a new `std.validate`
module:

```badger
pub fn bail<T, E>(error: E) -> T !{Never}            // for `?`-style use
pub fn must<T, E>(condition: Bool, error: E) -> Result<Unit, E>
pub fn must_eq<T: Eq, E>(a: T, b: T, error: E) -> Result<Unit, E>
pub fn check<T, E>(value: T, valid: fn(&T) -> Bool, error: fn() -> E) -> Result<T, E>
```

`bail` lowers to `return err(...)` and works under `?`. The `must*`
family returns a `Result` so it can be used in a sequence. A
`validate!` macro can collect multiple problems into one error.

**Effort.** S for the helpers, M for the macro. **Affected.** Every CRUD
service, every config parser, every CLI argument parser.

**Depends on.** Nothing for the helpers. The `?`-style use is much
better with compiler item **L-01** (`?` on `Option`) and **L-03**
(`let-else` ending in `err(...)`).

---

### S-02 — `result.bimap`

**Symptom.** `result.map` changes `T` and leaves `E` alone; the
idiomatic pipe `r |> result.map_err(AppError.http)` is awkward.

**Evidence.** [`crates/badger_hir/std/result.bdg:3-8`](../crates/badger_hir/std/result.bdg)
(`map` changes `T` only), `10-15` (`map_err` changes `E` only). There
is no `bimap` or `map_both` in the tree. The experience report hit
this repeatedly (F13) and attributed a series of "the wrong error type"
confusions to it.

**Fix.** Add `result.bimap(ok_fn, err_fn)`, plus a method form
`.bimap(ok_fn, err_fn)`. A one-liner, and the cheapest fix in this
report.

**Effort.** S. **Affected.** Every error-mapping function.

**Note.** The complementary *language* option — `?`-as-do-notation
with `From` conversion — is compiler item **L-05**. Doing S-02 first is
reasonable regardless.

---

### S-03 — `to_text_or` and the `unwrap_or` family

**Symptom.** The pattern `body.to_text().unwrap_or("?")` is common in
tests and handler logging. So is `params.get("id").unwrap_or("")` and
`result.unwrap_or(default)`. The API forces the two-step call.

**Fix.** Add `unwrap_or`-shaped methods on the relevant types
(`Bytes.to_text_or(default)`, and equivalents where a two-step call is
the only spelling).

**Effort.** S. **Affected.** Test code, handler logging.

---

### S-04 — `list.sorted_by_key` and friends

**Symptom.** `list.sort_by(key)` is a one-liner, and so are
`list.min_by(key)` and `list.max_by(key)`. There is no
`list.sorted_by_key` — the natural complement to Python's
`sorted(key=...)` — so the user writes
`list.sort_by(list.map(list, key))` or similar.

**Fix.** Add `sort_by_key`, `group_by_key` and `count_by_key` if they
are not already present.

**Effort.** S. **Affected.** Every list-shaped computation.

---

### S-05 — `Map` and `Set` literal ergonomics

**Symptom.** `map { "key" => value }` and `set { #a, #b }` are fine,
and `map { 1 => user1, 2 => user2 }` is good. The missing thing is
that type inference sometimes needs a hint when constructing a map or
set in an expression position with no other constraint.

**Fix.** Tighten inference for empty map/set literals and map
construction, so `{ .. }` style construction does not need a hint.

**Effort.** S. **Affected.** Map construction.

---

### S-06 — `Duration` as a first-class config field type

**Symptom.** `task.sleep(10ms)` and `worker.on_failure({ backoff:
10ms })` both accept duration literals, but `config.from_env<Settings>`
does not parse them: a `report_every: Duration` field fails with
"expected an integer". The workaround is a `report_every_ms: Int` field
and a `time.millis(...)` call at the use site.

**Evidence.** The codec decodes `Duration` only from a JSON integer
([`crates/badger_runtime/src/codec.rs:440-442`](../crates/badger_runtime/src/codec.rs)),
and the config codec has no `Duration` arm — a `Duration` field falls
through to the catch-all and arrives as a JSON string
([`crates/badger_runtime/src/intrinsics_codec.rs:395-421`](../crates/badger_runtime/src/intrinsics_codec.rs),
`:420`). Only a raw nanosecond integer decodes. `std/config.bdg` has no
`Duration` handling. The experience report hit this as F7.

**Fix.** Add `Duration` to the config codec: accept the duration
literal spelling (`BOOKS_REPORT_EVERY=10s`) and a millisecond integer.
It is a single primitive type, exactly like `Int` and `String`. The
duration literal syntax already exists in the language
([`crates/badger_syntax/src/ast.rs:323-349`](../crates/badger_syntax/src/ast.rs))
— only the config layer is missing.

**Effort.** S. **Affected.** Every configurable service that uses a
duration: timeout, backoff, retry, cache TTL, rate window.

---

### S-07 — A `Result` from a predicate

**Symptom.** `validate(input: NewBook) -> Result<NewBook, AppError>` is
six lines of guard clauses, repeated per validator.

**Fix.** A `validate` helper that takes a list of
`(condition, error_message)` pairs and returns the first violation:

```badger
pub fn validate<E>(value: T, rules: List<(Bool, E)>) -> Result<T, E>
```

Pairs naturally with S-01.

**Effort.** S. **Affected.** Every validator.

---

### S-08 — A `kv` helper for environment, headers and form data

**Symptom.** Every config block, every header read, every form parser
and every SQL parameter uses the same shape: a `List<(String, T)>` for
input, a `Map<String, T>` for output. The conversion is hand-written
each time.

**Fix.** A `kv` module with `from_list`, `to_list`, `merge`, `filter`,
`map_keys` and `map_values`. The `Map` type already has most of
these; the missing piece is the `List` form used for parsing.

**Effort.** S. **Affected.** Every config block.

---

### S-09 — Flattening a wrapper struct in JSON

**Symptom.** A newtype struct `BookId { value: 1 }` encodes as
`{"value": 1}`, so a `Book` with an `id: BookId` field encodes as
`{"id":{"value":1}, ...}`. Most web APIs flatten to `{"id":1, ...}`.

**Evidence / correction.** The experience report (F6) framed this as
"records serialize flat, structs serialize nested". That asymmetry does
not exist — structs and records are both `Kind::Record`
([`crates/badger_runtime/src/desc.rs:104-108`](../crates/badger_runtime/src/desc.rs),
[`crates/badger_mir/src/layout.rs:358-365`](../crates/badger_mir/src/layout.rs)),
and the JSON encoder, decoder and JSON Schema generator all ignore
`nominal`
([`crates/badger_runtime/src/codec.rs:209-234`](../crates/badger_runtime/src/codec.rs),
`450+`;
[`crates/badger_runtime/src/intrinsics_std.rs:673-690`](../crates/badger_runtime/src/intrinsics_std.rs)).
A struct field nests exactly like a record field. The real gap is that
there is no way to *ask* for flattening — and there is no `derive(Flat)`
or flatten knob in the derive set (`Eq`, `Hash`, `Display`, `Codec`,
`From`, `Error`; [`crates/badger_typeck/src/env.rs:876-943`](../crates/badger_typeck/src/env.rs)).

**Fix.** A flatten marker honoured by the JSON encoder, the JSON
decoder and the JSON Schema output — either a field attribute in the
type or a codec-level wrapper. If the marker lives in the type, the
compiler half is **L-09**.

**Effort.** M. **Affected.** Every DTO with a newtype field.

---

### S-10 — A `cli` argument parser module

**Symptom.** The CLI reference shows hand-rolled
`arg.parse_int().unwrap_or(default)` calls. There *is* a `std.cli`
module ([`crates/badger_hir/std/cli.bdg`](../crates/badger_hir/std/cli.bdg)),
but it is not the obvious thing to reach for, and the framework
example's argument handling
([`examples/framework/src/cli.bdg`](../examples/framework/src/cli.bdg))
is non-trivial.

**Fix.** Promote the existing `std.cli` helpers (or build a new
module) so a user can write:

```badger
let args = cli.parse({ host: cli.String("127.0.0.1"), port: cli.Int(8080) })
```

**Effort.** M. **Affected.** Every CLI program.

---

### S-11 — A channel that survives the system graph

**Symptom.** `channel.open` returns an `Endpoints<T>` record bound to
the `use` block's lifetime. The sender cannot be shared with one
provider and the receiver with another. The workaround is
`channel.create` — a plain allocation without a scope — which is not
documented well and is not bound to a system node. The experience
report planned an API-publishes-to-a-background-worker design on a
typed `Channel<Change>` and dropped the feature for a polling worker
(F9).

**Evidence / correction.** A channel is not categorically blocked from
crossing the graph: a resource provider may return arbitrary typed
values, including channels
([`spec/06-systems-and-pipelines.md:15-16`](../spec/06-systems-and-pipelines.md)),
and that is demonstrated in
[`docs/guide/08-systems-and-pipelines.md:197-214`](../docs/guide/08-systems-and-pipelines.md).
What is missing is a *declarative* channel resource the system owns
and closes with the node.

**Fix.** A `channel.resource` provider that registers an
`Endpoints<T>` in a system node: publishers pull the sender, subscribers
pull the receiver, and the channel closes when the system halts. This
is what makes "background notifier" patterns viable without falling
back to polling.

**Effort.** M. **Affected.** Any service that wants request-time events
observed by a background worker.

**Depends on.** Compiler item **L-16** (the `use`-block lifetime rule
must permit the channel to be stored in a provider's return value
without the scope-escape diagnostic firing).

---

### S-12 — One convention for module functions and methods

**Symptom.** `std.list` exports `list.map(list, f)` and `List<T>` also
exposes `.map(f)`. Both work, but the parser accepts one or the other
depending on position. In the experience report,
`sqlite.query_as<BookRow>(...) |> result.map(fn(rows) => list.map(rows, row_to_book))`
reported "no method map on …", while `rows.map(row_to_book)` worked;
earlier in the same session `text.trim(value)`, `text.contains(...)` and
`bytes.to_text(response.body)` had all worked without issue. The
dividing line is not consistent enough to predict (F3). The pipe
compounds it: `value |> f(a)` expands to `f(value, a)`, and when `f` is
a partially applied module function the partial application can shadow
the call.

**Fix.** Pick one convention. Method syntax everywhere would eliminate
this class of error outright; the alternative is to make the module
form and the method form strictly interchangeable in every position,
including in pipes.

**Effort.** M — this touches the method-resolution rule in the type
checker. **Affected.** Every call site, so the cost of getting it
wrong is high.

---

## 2. Extended library

### S-06 (above) — the configuration codec

`Duration` is the concrete gap today (S-06). While touching the
config codec, check the other primitives that the runtime uses
everywhere but the configuration layer does not: `Instant`,
`Timestamp`, byte sizes, and enum-from-string. The runtime's codec
already handles `Duration | Instant | Timestamp`
([`crates/badger_runtime/src/codec.rs:440-442`](../crates/badger_runtime/src/codec.rs));
the config codec has none of them
([`crates/badger_runtime/src/intrinsics_codec.rs:395-421`](../crates/badger_runtime/src/intrinsics_codec.rs)).

### S-09 (above) — the JSON codec

Flattening is the concrete gap today (S-09). The JSON Schema output
([`crates/badger_runtime/src/intrinsics_std.rs:673-690`](../crates/badger_runtime/src/intrinsics_std.rs))
must agree with whatever the encoder does, or the OpenAPI story breaks
in a new place (see **F-N4**).

### S-11 (above) — the channel library

The channel library needs a provider-shaped entry point, not just
`open` and `create`.

### S-13 — `std.validate` as a module

Split S-01 into two deliverables if it is easier to schedule: the
`bail` / `must` / `must_eq` / `check` functions can land in
`std.result` immediately, and a `std.validate` module can follow with
the `validate!` macro and the rule-list helper (S-07) in one place.

**Effort.** S.

### S-14 — A lazy `Seq` that a streaming query can produce

**Symptom.** The experience report wanted to demonstrate lazy
sequences — a `Seq<Book>` over a streaming SQL query — and fell back
to a `List<Book>`. The sequence surface exists and is being optimized
in a separate line of work
([`reports/2026-10-19-sequence-fusion-and-presized-builders.md`](2026-10-19-sequence-fusion-and-presized-builders.md)),
but a streaming query in `std.sqlite` cannot hand back a lazy
sequence, so the lazy story cannot be shown on a real workload.

**Fix.** Let a streaming query return a lazy `Seq<T>` rather than
materializing a list, and make sure the collection surface has the
operations a stream needs (`take`, `filter`, `map` without full
materialization).

**Effort.** M. **Affected.** Any service that returns a large result
set, and the ergonomics story of the sequence surface.

### S-15 — A `default!` macro and the `default` convention

**Symptom.** The ergonomics audit listed "a `default!` macro /
`Default` derive for records and structs" as one item, and the two
halves landed in different places: the derive is compiler work
(compiler item **L-06**), but the *value-level* helper is a library
concern. Today a default for a single value is written by hand at
every use site, and the convention for what `Int`'s default is lives
only in the audit.

**Fix.** Add a `default!` macro, or a `default` function per built-in
type, with the convention fixed in one place: `Int: 0`, `Float: 0.0`,
`Bool: false`, `String: ""`, `List<T>: empty()`, `Map<K, V>: empty()`,
`Option<T>: none()`. The compiler's `derive(Default)` then reads the
same convention rather than defining a second one.

**Effort.** S. **Affected.** Every settings / DTO / config record.

---

## 3. Application services framework

The framework is the `route` / `http` / `system` / `pipeline` group.
This section is organized the way the framework itself is designed:
**amendments** change what exists, **improvements** make what exists
better, **additions** are new capability.

### Amendments

#### F-A1 — A handler's `Err` must produce the status the application intended

**Symptom.** This is the single most-reported friction in the whole
ergonomics work, and the one the experience report called out as
"the single highest-leverage fix" (F1, ~25 minutes lost across two
passes). A handler returning `Err(AppError.invalid_input("title
required"))` becomes a `500 "internal server error"`. The application
intended a 400. The route system does this silently.

**Evidence.** Every route form maps `.err(e)` to a 500:

- [`crates/badger_hir/std/route.bdg:57-60`](../crates/badger_hir/std/route.bdg) (`route`)
- [`crates/badger_hir/std/route.bdg:100-103`](../crates/badger_hir/std/route.bdg) (`json`)
- [`crates/badger_hir/std/route.bdg:123-126`](../crates/badger_hir/std/route.bdg) (`json_get`)
- [`crates/badger_hir/std/route.bdg:157-160`](../crates/badger_hir/std/route.bdg) (`typed`)

and `std.http`'s handler forms do the same
([`crates/badger_hir/std/http.bdg:177-183`](../crates/badger_hir/std/http.bdg),
`250-257`, `266-273`). There is no `StatusError` or `HttpError` type
anywhere in `std`. A trace event *is* emitted, so it is not invisible
to an operator reading traces — but the client gets a generic 500 with
no status mapping, and there is no warning and no test failure that
points at it. The only signal the experience report had was
`expect 400 got 500`.

**Fix.** Two clean options:

- An `http.StatusError` type: `{ status: Int, error: E }`. The route
  system catches it and replies with `status` and a JSON body derived
  from `error.display()`. **No compiler change needed.**
- A `derive(HttpError)` on the application enum: the route system
  reads per-variant metadata attached by the derivation
  (`{ status: 404, body: { error: "not_found", ... } }`) and generates
  the reply. More declarative, and it gives OpenAPI the per-status
  shape it needs (F-N4) — but it needs new derive machinery from the
  compiler team.

**Recommendation.** Ship `StatusError` first: it is pure library work,
it fixes the dominant pain point on its own, and it does not gate on
the compiler. Add `derive(HttpError)` afterwards if the derive
machinery lands (see **L-20** in the compiler report and
[§6](#6-compiler-dependencies)).

**Effort.** S for `StatusError`, M for the derive.

**Affected.** Every HTTP service. **Pairs with** compiler items **L-01**
and **L-03** and library items **F-I1**, **F-I2** — the four together
turn a 10-line match cascade into a two-line `let x = expr?` flow and
make the error path right instead of silently wrong.

---

#### F-A2 — Routes must strip the query string before matching

**Symptom.** `route.get("/search", ...)` does not match
`GET /search?q=bad`. The path is treated as a literal including the
query, so the route 404s. The experience report had to redesign its
search endpoint to take the query as a path segment (`/search/:q`) to
work around it (F5). For a service-oriented language, not handling the
query string is a gap the user hits on day one.

**Evidence.** The runtime passes the full URI to the matcher
([`crates/badger_http/src/lib.rs:260`](../crates/badger_http/src/lib.rs),
`path_and_query()`, forwarded verbatim at `:222`), and `match_path`
splits the path on `"/"` only
([`crates/badger_hir/std/route.bdg:179-198`](../crates/badger_hir/std/route.bdg),
`169-171`), so `"/search?q=bad"` yields the segment
`"search?q=bad"` ≠ `"search"` and 404s at `route.bdg:292`. There is
no `route.params(request)` and no query parsing anywhere in `std`.

**Fix.** Split the request URI on `?` before matching the pattern, and
put the query into a `Params` record. Add `route.params(request) ->
Params` so a `route.get("/search", ...)` handler can also read
`params.get("q")`.

**Effort.** S. **Affected.** Every web service that uses query
parameters — that is, every web service.

---

#### F-A3 — `route.typed` must have the same error story as `route.get`

**Symptom.** The typed route form decodes path params into a record
(`{ id: Int }`), but the handler has to return `(Int, R)`, and the
result has to be wrapped in a user-side `with_status` to become an
`http.Response` — and any handler `Err` is still mapped to a 500. The
typed route therefore has a *worse* error story than the plain
`route.get`. The experience report worked around it by parsing the path
by hand and using `route.get` instead, losing the schema-validation
benefit the typed routes exist to provide.

**Fix.** Once F-A1 lands, make `route.typed` understand the same error
type and propagate the status the same way. If F-A1 is not done, the
typed route needs its own local mapping, which is strictly worse.

**Effort.** S (given F-A1), M (independent of it). **Affected.** Every
typed route with a 404 or a 400 case.

---

#### F-A4 — Pipeline `error` callbacks: make the typing rule obvious

**Symptom.** A stage's `error(ctx, error)` callback does not fire
without the explicit annotation `error(ctx, error: AppError)`. The
diagnostic is a silent failure, not an error. The experience report
lost ~10 minutes and two attempts to this (F8).

**Evidence / current state.** This has partly been fixed on the
compiler side: the error type now comes from the stage's result
annotation `-> Result<_, E>` or from the callback's own parameter
annotation, and a body that needs the error type without either now
gets a diagnostic ("cannot infer the enum of variant pattern",
[`crates/badger_typeck/src/infer.rs:2688`](../crates/badger_typeck/src/infer.rs))
instead of a callback that silently never fires. It is documented at
[`docs/guide/08-systems-and-pipelines.md:466-467`](../docs/guide/08-systems-and-pipelines.md).
An unannotated `error(ctx, error)` works when the body does not need
the type — see
[`examples/design/framework/src/orders/service.bdg:7-21`](../examples/design/framework/src/orders/service.bdg)
and [`examples/10_interceptors.bdg:32-35`](../examples/10_interceptors.bdg).

**Fix (library side).** Decide and document one rule — always infer,
or always require the annotation and emit a diagnostic when it is
missing — and put the rule in the guide, next to the pipeline chapter,
with one example of each. The current behaviour is the worst of both:
inference works sometimes, and a missing annotation fails silently.

**Effort.** S. **Affected.** Every error-handling pipeline stage.

---

#### F-A5 — Provider lifecycle: `task.scope()` inside a provider

**Symptom.** A provider that opens a `task.scope` to spawn a
long-lived background worker compiles cleanly and then hangs at
startup — no listening port, no log output, no worker pool, only two
threads instead of the worker pool. The failure mode is a silent hang
(F2, ~30 minutes lost). The framework's own example
([`examples/framework/src/worker.bdg:19`](../examples/framework/src/worker.bdg))
uses the other shape — the provider takes the lifecycle `scope` as its
first parameter and calls `scope.worker(...)`, which registers a
supervised task that lives as long as the system does.

**Fix.** The compiler team owns the decision (compiler item **L-15**):
reject `task.scope()` in a provider body at compile time, or rewrite
the runtime so the task lives as long as the system. The library side
is to make the correct shape obvious: the guide must say that a
provider that wants a long-lived task takes `scope` as a parameter and
uses `scope.worker`.

**Effort.** S for the documentation; the compiler item is S to reject,
L to support.

---

### Improvements

#### F-I1 — `with_status` / `reply_for_error` in `std.http`

**Symptom.** Every JSON API that maps an application error to a status
code writes the same four-arm match:

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

**Evidence.** `std.http` offers `json` / `text` / `stream`
([`crates/badger_hir/std/http.bdg:499-505`](../crates/badger_hir/std/http.bdg),
`279-281`) but no status-mapping helper, and no `StatusError` type
exists. The helper exists only as user code, duplicated per project
([`examples/13_rest_service.bdg:316-335`](../examples/13_rest_service.bdg)).

**Fix.** Put `reply_for_error` and `with_status` in `std.http` so
every project can use them, and document the pattern. Better still, if
F-A1 lands, make the route system do it automatically and keep
`with_status` only for handlers that want to build a body by hand.

**Effort.** S for the helper, M for the route-side change. **Affected.**
Every JSON API.

---

#### F-I2 — A `respond!`-style shorthand for `Result<http.Response, E>`

**Symptom.** A pipeline stage that maps every application error to a
reply has to be written by hand every time — see
[`examples/design/service/src/main.bdg:30`](../examples/design/service/src/main.bdg)
and the F1 workaround.

**Fix.** Provide a `respond_with(fn(error) -> http.Response)` stage
factory, plus a `derive(HttpReply)` on the application error enum that
generates the mapping. The route system and the pipeline stage
cooperate so the user writes only the error enum.

**Effort.** M. **Affected.** Every JSON API.

**Depends on.** The derive half is **blocked on compiler** — new derive
machinery that attaches per-variant reply metadata (compiler
**L-20**). The `respond_with` stage factory half is not.

---

#### F-I3 — A pipeline `around` / `try` stage

**Symptom.** "Time this handler, log on the way out, and on error log
and convert" requires `enter`, `leave` and `error` callbacks in three
separate stages. A single `around` callback that runs the handler and
sees the result expresses it in one place.

**Fix.** Add an `around(ctx, body)` stage, or let `enter` / `leave` /
`error` accept a single body. The current shape is fine for the common
case and verbose for the "wrap a handler with a try" case.

**Effort.** M. **Affected.** Every pipeline that combines timing, an
access log and an error mapping.

---

### Additions

#### F-N1 — A channel as a first-class system resource

The declarative `channel.resource` provider. Detail lives in **S-11**;
listed here because it is a framework-level capability, not just a
library helper.

#### F-N2 — `route.params(request) -> Params`

Read the query string from a plain `route.get` handler. Lands with
F-A2, which is what puts the query into a `Params` record in the first
place.

#### F-N3 — `derive(HttpError)`: per-variant status and body metadata

Attach `{ status: 404, body: { error: "not_found", ... } }` to each
variant of an application error enum; the route system reads it and
generates the reply.

**Blocked on compiler** — new derive machinery (compiler **L-20**).
Fallback: the explicit `http.StatusError` type from F-A1, which needs
nothing.

#### F-N4 — OpenAPI output with per-status response shapes

If F-A1 is implemented with a derive, the derivation already knows the
per-status body shape, and the OpenAPI document can state it. Without
the derive, the framework can at least emit the generic error schema
per status code.

#### F-N5 — An `error` → reply stage factory

`respond_with(fn(error) -> http.Response)` as a pipeline stage, so the
"map every error to a reply" stage is a one-liner rather than
hand-written per service. Pairs with F-I2.

## 4. Tooling, CLI and the developer loop

These are the `badger` command-line surface. They are in this report
rather than the compiler's because they are product surface, not
language or compiler work, and they should not be dropped between the
two reports.

**Evidence for the whole section.** The complete command set is the
`Command` enum in [`crates/badger_cli/src/main.rs:23`](../crates/badger_cli/src/main.rs):
`check`, `compile`, `release`, `start`, `run`, `replay`, `test`, `task`,
`format`, `repl`, `inspect`, `update`, `lsp`, `mcp`, `graph`, `query`,
`doc`, `new`, `generate`, `contracts`, `explain`. None of `install`,
`search`, `completions`, `watch`, `doctor`, `bench` or `init` exists.
`badger run --watch` and `badger run --bench` exist as flags
(`main.rs:113-114`, `127-128`) but not as commands. The MCP tool set
(`crates/badger_cli/src/mcp.rs:313-534`) is eval / generate / contracts
/ check / query / run, with no `install` or `search` counterpart.

| ID | Item | Effort | Notes |
|---|---|---|---|
| T-01 | `badger install NAME [VERSION]` — add the dependency to `badger.toml` and rewrite the lockfile | S–M | Today the user edits `badger.toml` by hand and runs `badger update` (`main.rs:226`). Also needs an MCP counterpart. |
| T-02 | `badger search QUERY` — search the installed stdlib and project modules for a definition | S–M | Today the closest is `badger query api std.list` and reading JSON. Also needs an MCP counterpart. |
| T-03 | `badger completions bash\|zsh\|fish\|powershell` | S | ~20 subcommands and many flags, typed by hand today. |
| T-04 | `badger watch [PATH] [-- COMMAND...]` — default `badger check` | S | Today the user loops `badger check` by hand. |
| T-05 | `badger doctor` (and `--json`) — versions, paths, config, runtime settings, recent diagnostics | S | Directly shortens bug reports. |
| T-06 | `badger bench [PATH]` and `badger bench --compare BASE_FILE` | S–M | `run --bench` exists but is undocumented and invisible; hook into the existing `bench/` infrastructure. |
| T-07 | `badger rename --line N --col N --name NEW PATH` | S | The query already does the work; this is a CLI wrapper. |
| T-08 | Format on save in the LSP | S–M | Check whether the LSP already does it; if not, make it and document it in `docs/editors.md`. |
| T-09 | `badger new NAME --template=cli\|service\|framework\|lib\|worker` | M | The framework example is the de-facto template today; it should be a `--template=framework` option. Templates live in a `templates/` directory. |
| T-10 | `badger repl --load FILE` | S | Today the REPL accepts only typed input, line by line. |
| T-11 | `badger init [--lib] [PATH]` | S | Turns a directory into a project or a library. |
| T-12 | `badger test --watch` and `badger test --changed` | S–M | `--watch` is a thin wrapper over T-04; `--changed` uses the file dependency graph. Depends on the compiler team's incremental check (**L-17**) to be fast. |
| T-13 | `badger generate service`, `generate cli`, `generate library` | M | `generate feature` and `generate worker` exist. |
| T-14 | `badger doc PATH --open` | S | Or emit a single-page HTML version of the generated docs. |
| T-15 | REPL history (up arrow) and completion (tab) | S–M | Wire the REPL to `rustyline` or `reedline`; save history to `~/.badger_history`. |

**Affected.** T-01 to T-03, T-05, T-09 and T-15: every new project
and every session. T-04, T-06, T-12: every iteration. T-07, T-08, T-10,
T-11, T-13, T-14: named but narrower.

## 5. Documentation

| ID | Item | Effort |
|---|---|---|
| D-01 | A "common errors" page grouping the ~20 most common diagnostic codes by what the user was probably trying to do, with a one-paragraph fix per group; cross-link from each `badger explain` output | S |
| D-02 | An "idiomatic Badger" guide: error types, validation, configuration, routes, pipeline stages, background workers, tests — cross-linked to the guide chapters | M |
| D-03 | A migration cookbook from Go, Python, Rust and TypeScript: one representative program per language, the Badger equivalent, and the list of differences to know about | M |
| D-04 | An "explain the type system" page: row inference, effect rows, `derive(From)` versus a manual `From`, and how `?` resolves `From` — one example per rule, in plain language | S |

**Context.** D-01 and D-04 matter more than they look: the two
diagnostic problems the experience report hit hardest (the wrong-line
error, and the silently-not-firing pipeline callback) are both
documentation problems as much as implementation problems. D-02 is the
document that would have made the framework example's conventions
discoverable, which is the difference between the framework being
learnable from the docs and being learned by reading source.

## 6. Compiler dependencies

The library team can start on everything in this report except the
items below, which wait on the compiler team. Full details are in
[§6 of the compiler report](2026-10-19-language-and-compiler-changes.md#6-compiler-side-work-required-by-the-library-and-framework-work).

| Library item | Waits for | Fallback if the compiler does not take it |
|---|---|---|
| S-11 (channel as a system resource) | L-16 (lifetime rule for a channel stored in a provider's return value) | Use a provider that returns the `Endpoints<T>` directly — the spec already permits it, just not declaratively |
| S-09 (JSON flatten) | L-09 (a marker in the type) | Put the marker in the codec call instead of the type |
| F-N3 (`derive(HttpError)`) | L-20 (per-variant metadata via derive) | Ship F-A1 with the explicit `http.StatusError` type — no compiler change, and it fixes the dominant pain point on its own |
| F-I2 (`derive(HttpReply)`) | L-20 | Ship the `respond_with` stage factory on its own |
| T-12 (`badger test --changed`) | L-17 (incremental check) | Fall back to `--watch` only |

**Recommended sequence for the two teams.** The library team should
start with F-A1 (`StatusError`), F-A2 (query string), F-I1
(`with_status`), S-01 (`bail!`/`must!`), S-02 (`result.bimap`) and S-06
(`Duration` in config). None of them need the compiler, and together
they remove the largest share of the reported friction. The compiler
team should start with L-01, L-10, L-13 and L-15. The two sets
compose: L-01 plus F-A1 plus F-I1 turns a handler from a ten-line
match cascade into `let x = expr?`.

## 7. Already shipped — do not re-implement

Verified fixed as of `83c6568`:

| Was | Now | Evidence |
|---|---|---|
| The logging family returns a `Result` that must be discarded with `let _ =` (audit 3.6) | `log.trace/debug/info/warn/error` return `Unit`; `log.with` still returns the body's result | [`crates/badger_hir/std/log.bdg:18-26`](../crates/badger_hir/std/log.bdg), `30-35`; the guide uses plain `log.info(...)` ([`docs/guide/10-services.md:489`](../docs/guide/10-services.md)) |
| `if let` / `while let` missing (audit 2.7) | Desugared to `match`; `if let .ok(x) = r` is documented | [`crates/badger_syntax/src/lower.rs:971-983`](../crates/badger_syntax/src/lower.rs); [`docs/guide/01-values-and-functions.md:844-885`](../docs/guide/01-values-and-functions.md) |
| Triple-quoted strings undocumented (audit 1.5, experience F10) | Documented in the tour and the guide | [`docs/tour.md:89-94`](../docs/tour.md); [`docs/guide/01-values-and-functions.md:210`](../docs/guide/01-values-and-functions.md) |
| `Option.ok_or` / `ok_or_else` missing | Present | [`crates/badger_hir/std/option.bdg:25-30`](../crates/badger_hir/std/option.bdg) |

## 8. What already works — protect this

The experience report is as much a list of what did *not* need fixing.
These are the features that carried the weight of a real service, and
the changes in this report should not regress them. They are also the
strongest argument for what the library team should build on rather
than around.

| Already works | Evidence | Note |
|---|---|---|
| `derive(Error, From)` on the application error enum — every `?` call site propagates without an explicit match; validation, decoding, database, HTTP, system and IO errors all collapse to one type | experience report item 1; `derive(From)` at [`crates/badger_typeck/src/env.rs:874-943`](../crates/badger_typeck/src/env.rs) | This is the foundation F-A1 and L-01 build on. The `From`-based `?` conversion it implies is exactly what L-01 needs for `Option`. |
| Effect inference — no `!{...}` annotation was written in the whole service; the compiler placed `Io`, `Wait` and `State` correctly and carried them through `?` into the handler | experience report item 2 | S-12 (module function vs method) and L-05 must not disturb effect inference. |
| The `system { ... }` graph with `ref(#name)` — five providers, fifteen lines, compile-time dependency checking | experience report item 3 | F-A1, S-11 and F-N1 all extend this graph; they should not require a new graph model. |
| Records with open rows and `{ ..r, field: value }` update syntax | experience report item 4 | The basis for S-01's `default!` and S-05. |
| Pipeline stages as data — timing, access log and a per-request counter composed in one `pipeline { ... }` literal | experience report item 5 | F-I3 (`around`) extends this model; it should stay a literal. |
| In-process tests with `http.call` — no socket, no port, no race; tests run as fast as function calls | experience report item 6 | F-A1 and F-A2 change the reply path; the in-process path must keep the same semantics as the socket path, and the new status mapping should be observable in both. |

**Two demonstrations the experience report could not deliver**, both
now tracked: parameterised contracts (compiler item **L-19**) and a
lazy `Seq` over a streaming query (library item **S-14**).

## 9. Suggested sequencing

**A few hours**

1. **F-A2** — strip the query string in the matcher. One-line change in
   `match_path`; removes the most-reported "why doesn't my route
   match?".
2. **S-02** — `result.bimap`. One-liner.
3. **S-06** — `Duration` in the config codec. One primitive; removes a
   known footgun.
4. **S-01** (helpers only) — `bail` / `must` / `must_eq` / `check`.
5. **D-01**, **D-04** — the common-errors page and the type-system
   page.

**A day**

6. **F-A1** — `http.StatusError` and the route system's status
   mapping. The single most-reported friction in the whole set.
7. **F-I1** — `with_status` / `reply_for_error` in `std.http`.
8. **F-A4** — document the pipeline `error` typing rule.
9. **T-03**, **T-05**, **T-11** — completions, doctor, init.

**A week**

10. **S-11** — the channel resource, once L-16 lands.
11. **S-09** — JSON flattening, once the marker has a home.
12. **F-A3** — `route.typed` error parity, given F-A1.
13. **F-I3** — the pipeline `around` stage.
14. **S-10** — the `cli` argument parser module.
15. **T-01**, **T-02**, **T-04**, **T-06** — the main CLI additions.

**A quarter**

16. **F-N3**, **F-N4**, **F-I2** — the derive-based error story, given
    the compiler's derive machinery.
17. **S-12** — one convention for module functions and methods.
18. **T-09**, **T-13** — the project templates and generators.
19. **D-02**, **D-03** — the idioms guide and the migration cookbook.

**The highest-leverage combination available without the compiler** is
**F-A1** + **F-I1** + **S-01**: a handler's error produces the status
the application intended, the mapping helper is in the standard
library instead of every project, and validation has a `bail!` /
`must!` family. Add compiler **L-01** (`?` on `Option`) on top and the
same handler becomes two lines.

## Appendix A — traceability

Where each item of the two source reports ended up. The companion
compiler report carries the same table for its half.

| Source item | Destination |
|---|---|
| Ergonomics 1.1 (`?` on `Option`) | Compiler L-01 |
| Ergonomics 1.2 (local `fn`, local generics) | Compiler L-02 |
| Ergonomics 1.3 (`?` on `Result` does not transform `E`) | Compiler L-05 + S-02 |
| Ergonomics 1.4 (`Duration` in config) | S-06 |
| Ergonomics 1.5 (triple-quoted strings) | Shipped |
| Ergonomics 1.6 (`let-else` must not return) | Compiler L-03 |
| Ergonomics 1.7 (single-variant enum ergonomics) | Compiler L-04 |
| Ergonomics 2.1 (`bail!` / `must!` / `validate!`) | S-01, S-07, S-13 |
| Ergonomics 2.2 (`derive(Default)`) | Compiler L-06 + S-15 |
| Ergonomics 2.3 (`derive(Builder)`) | Compiler L-07 |
| Ergonomics 2.4 (`with_status` / `reply_for_error`) | F-I1 |
| Ergonomics 2.5 (`to_text_or` and friends) | S-03 |
| Ergonomics 2.6 (channel across the system boundary) | S-11, F-N1 + compiler L-16 |
| Ergonomics 2.7 (`if let` for `Result`) | Shipped |
| Ergonomics 3.1 (handler `Err` → silent 500) | F-A1 |
| Ergonomics 3.2 (routes strip the query string) | F-A2, F-N2 |
| Ergonomics 3.3 (`route.typed` error story) | F-A3 |
| Ergonomics 3.4 (`task.scope()` in a provider) | F-A5 + compiler L-15 |
| Ergonomics 3.5 (pipeline `error` annotation) | F-A4 |
| Ergonomics 3.6 (`let _ = log.warn(...)`) | Shipped |
| Ergonomics 3.7 (`respond!` shorthand) | F-I2, F-N5 |
| Ergonomics 3.8 (pipeline `around` stage) | F-I3 |
| Ergonomics 4.1 – 4.9 (tooling) | T-01 – T-10 |
| Ergonomics 5.1 – 5.5 (diagnostics) | Compiler L-10 – L-14 |
| Ergonomics 6.1 (`derive(Copy)`) | Compiler L-08 |
| Ergonomics 6.2 (map / set literals) | S-05 |
| Ergonomics 6.3 (`sorted_by_key` and friends) | S-04 |
| Ergonomics 6.4 (`Result`/`Option` from predicate) | S-07 |
| Ergonomics 6.5 (`kv` helper) | S-08 |
| Ergonomics 6.6 (`cli` argument parser) | S-10 |
| Ergonomics 7.1 – 7.4 (documentation) | D-01 – D-04 |
| Ergonomics 8.1 – 8.2 (dev-loop performance) | Compiler L-17, L-18 |
| Ergonomics 9.1 – 9.5 (other CLI) | T-11 – T-15 |
| Experience F1 (silent 500) | F-A1 |
| Experience F2 (`task.scope()` hangs) | F-A5 + compiler L-15 |
| Experience F3 (module function vs method split) | S-12 |
| Experience F4 (no local `fn` / generics) | Compiler L-02 |
| Experience F5 (query strings) | F-A2, F-N2 |
| Experience F6 (structs serialize nested) | S-09 + compiler L-09 |
| Experience F7 (`Duration` not in config) | S-06 |
| Experience F8 (pipeline `error` annotation) | F-A4 |
| Experience F9 (channels across the boundary) | S-11, F-N1 + compiler L-16 |
| Experience F10 (multi-line strings) | Shipped |
| Experience F11 (errors at the wrong line) | Compiler L-10 |
| Experience F13 (`result.map` does not transform `E`) | S-02 + compiler L-05 |
| Experience — contracts could not be stated for a function with an edge case | Compiler L-19 |
| Experience — lazy `Seq` over a streaming query not demonstrable | S-14 |
