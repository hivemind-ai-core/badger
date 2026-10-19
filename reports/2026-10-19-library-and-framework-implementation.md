# Library and framework changes — implementation notes

**Date:** 2026-10-19
**Branch:** `feature/library-changes-2026-10-19` (worktree `/tmp/language-lib-changes-2026-10-19`)
**Implements:** [2026-10-19-library-and-framework-changes.md](2026-10-19-library-and-framework-changes.md)

What follows is what was actually landed, item by item, and what was
left out (with the reason). The companion compiler report
([2026-10-19-language-and-compiler-changes.md](2026-10-19-language-and-compiler-changes.md))
is unchanged: nothing here waits on it, and nothing here needs it.

## The headline: a handler's error now produces the status it intended

The report's most-reported friction (F1, "the single highest-leverage
fix") was that a handler's `Err` became a 500 whatever the application
intended. The fix landed in three parts.

**`HttpStatus`, a protocol in the prelude** (`crates/badger_hir/std/prelude.bdg`):

```badger
pub protocol HttpStatus {
    fn http_status(self: &Self) -> Int { 500 }
}
```

An error type says once how it is reported; the default is 500, so an
error type that says nothing about HTTP keeps the old behaviour:

```badger
impl HttpStatus for AppError {
    fn http_status(self: &Self) -> Int {
        match self {
            .invalid_input(_) => 400,
            .not_found(_) => 404,
            _ => 500,
        }
    }
}
```

**The route system asks for it.** Every route form (`route.get`,
`route.post`, `route.put`, `route.delete`, `route.json`,
`route.json_get`, `route.typed`) and every handler form in `std.http`
(`http.handler`, `http.scoped`, `http.streaming_handler`) now bounds the
handler's error type with `E: Display, E: HttpStatus` and answers an
error with `http.error_reply(e)`: the status the error carries, and —
for a client error — the error's message. A server error keeps its
message to itself, as before.

**`http.StatusError`** (`crates/badger_hir/std/http.bdg`) carries a
status for the one-off case (`http.status(404, error)`), and
`http.reply_for_error` builds the same reply by hand (F-I1) for a
pipeline stage that maps an error to a reply.

A route's error type is now bound by `HttpStatus`; an error type that
does not implement it is a compile error with a clear message
(`E0341: `AppError` does not implement `HttpStatus``), not a silent
500. The standard library's own error types implement it (25 modules),
so a handler may return `io.Error`, `config.Error`, `sqlite.Error` and
the rest unchanged.

This is a **breaking change** for a route handler whose error type is
neither `String` nor an `HttpStatus` implementor: it now needs
`impl HttpStatus for MyError { }` (the default 500) or a body that
names the statuses. The repository's own programs are updated.

## Item by item

| Item | What landed | Where |
|---|---|---|
| **F-A1** | The `HttpStatus` protocol, the route system's status mapping, `http.StatusError`, `http.error_reply`, `http.status_text` | `std/prelude.bdg`, `std/http.bdg`, `std/route.bdg` |
| **F-A2** | The query string no longer stops a route from matching: the matcher sees the path only. `route.path_of`, `route.query_of`, `route.query_params` split a URI; the handler's `params` map holds the path parameters *and* the query parameters (the path wins) | `std/route.bdg` |
| **F-N2** | `route.params(request) -> Params` reads the query string of a request | `std/route.bdg` |
| **F-A3** | `route.typed` answers an error with the same status mapping as `route.get` (it shares the machinery now) | `std/route.bdg` |
| **F-A4** | The rule for a stage's error type is written down, with an example of each of the three ways to write it: inferred from the pipeline, named by the callback, named by the factory | `docs/guide/08-systems-and-pipelines.md` |
| **F-A5** | The rule for a provider's background work: the scope the provider is given, not a `task.scope()` opened inside it (which hangs the startup) | `docs/guide/08-systems-and-pipelines.md` |
| **F-I1** | `http.with_status` (attaches a status to a reply built by hand), `http.reply_for_error`, `http.error_reply`, `http.status_text` in `std.http` | `std/http.bdg` |
| **F-I2 / F-N5** | `pipeline.recover_with(fn(error) -> reply)`: a stage that turns an error into a reply | `std/pipeline.bdg` |
| **S-01** | `result.bail`, `result.must`, `result.must_eq`, `result.check` (the early-return and guard family) | `std/result.bdg` |
| **S-02** | `result.bimap` (and the `.bimap` method) | `std/result.bdg` |
| **S-03** | `bytes.to_text_or` (and the method) | `std/bytes.bdg` |
| **S-04** | `list.sort_by_key`, `list.group_by_key`, `list.count_by` (and the methods) | `std/list.bdg` |
| **S-06** | `Duration` (and `Instant`, `Timestamp`) in the configuration codec: the duration spelling (`10s`, `500ms`, `2us`, `1ns`, `5min`, `1h`) and a plain number of milliseconds | `crates/badger_runtime/src/intrinsics_codec.rs` |
| **S-08** | The `std.kv` module: `from_list`, `to_list`, `get`, `get_or`, `len`, `merge`, `filter`, `map_keys`, `map_values` | `std/kv.bdg` |
| **S-10** | The command line as a value: `cli.parse_env({ host: cli.text("127.0.0.1"), port: cli.integer(8080) })`, with `cli.text`, `cli.integer`, `cli.number`, `cli.flag`, `cli.text_list` | `std/cli.bdg` |
| **S-13 / S-07** | The `std.validate` module: `rule`, `validate` (the first violation), `validate_all` (every violation), and the common conditions (`not_blank`, `at_most`, `at_least`, `in_range`, `greater_than`, `less_than`) | `std/validate.bdg` |
| **S-15** | The `std.default` module: the default value of each built-in type, in one place (`int`, `float`, `bool`, `string`, `list`, `map`, `option`) | `std/default.bdg` |
| **T-02** | `badger search QUERY` finds a definition in the standard library and in the project's modules (by name or by documentation) | `crates/badger_cli/src/search.rs` |
| **T-03, T-04, T-05, T-11** | `badger completions`, `badger watch`, `badger doctor`, `badger init` | `crates/badger_cli` |
| **D-01** | A "common errors" page: the most common diagnostics, grouped by what the user was probably trying to do — and every `badger explain` output points at it | `docs/common-errors.md`, `crates/badger_cli/src/main.rs` |
| **D-02** | An "Idiomatic Badger" guide: error types, validation, configuration, routes, pipeline stages, background workers, tests — cross-linked to the guide chapters | `docs/idiomatic.md` |
| **D-04** | An "explain the type system" page: row inference, effect rows, `From`, and how `?` resolves it | `docs/type-system.md` |

### New modules

`std.validate`, `std.kv` and `std.default` are new modules: a file in
`crates/badger_hir/std/` and an entry in `crates/badger_hir/src/stdlib.rs`.

## Not landed, and why

| Item | Why |
|---|---|
| **S-05** (map/set literal inference) | A type-checker change, not a library one: the compiler team's area, and the report does not list it as blocked on them either. The ergonomics gap is small (an empty literal in an expression position). |
| **S-09** (JSON flattening) | The report's own recommendation is to wait for the marker to have a home (compiler **L-09**). The fallback — a codec-level wrapper — needs a runtime design for "flatten this field" that the report leaves open. |
| **S-11 / F-N1** (a channel as a system resource) | Blocked on compiler **L-16** (the scope-escape rule). The fallback — a provider that returns the `Endpoints<T>` — is already possible, as the report notes. |
| **S-12** (one convention for module functions and methods) | A type-checker change (the method-resolution rule), with a high cost of getting it wrong. The compiler team's call. |
| **S-14** (a lazy `Seq` from a streaming query) | A quarter-sized piece of work on its own; the sequence surface is being optimized in a separate line of work. |
| **F-I3** (a pipeline `around` stage) | The `around` callback needs the type checker to infer a function-typed parameter (`fn(ctx) -> Unwind<..>`) for a stage whose type parameters are inferred at the use site. That is compiler work, in the compiler's area, and it was left for the compiler team rather than landed half-way. |
| **F-N3 / F-N4 / F-I2's derive half** | Blocked on compiler **L-20** (per-variant metadata through a derive). The fallback — the explicit `http.StatusError` and the `HttpStatus` protocol — landed instead, and covers the same ground without the compiler. |
| **T-01, T-06, T-07, T-08, T-10, T-13, T-14, T-15** | The rest of the tooling surface: not started (the report's own ranking puts them below the five that landed). |
| **D-02, D-03** | The idioms guide landed (`docs/idiomatic.md`); the migration cookbook (D-03) was not started (a large writing job). |

## Notes on the design

**Why a protocol and not `derive(HttpError)`.** The report recommends
shipping `StatusError` first and the derive afterwards (compiler
**L-20**). A protocol does the same job with less ceremony and without
the compiler: one `impl HttpStatus for AppError` in the application
replaces the per-site wrapping `StatusError` would need (and the
`?` operator does not convert error types today — that is compiler
**L-05** — so a wrapper would have to be built at every `?`).
`StatusError` shipped as well, for the one-off case.

**Why the status is not part of the `Error` protocol.** `Error` lives in
the prelude and is about "an expected failure"; a status is about HTTP.
`HttpStatus` is a separate protocol in the prelude (so that every
standard module can implement it for its own error type without
importing `std.http`, which would be an import cycle).

**Why a 5xx keeps its message.** The old behaviour (a 500 without the
error's text) is right: the message of an internal failure is not the
client's business. A 4xx is the application's own message, and the
client needs it.

**The query string.** `Request.path` still holds the whole URI (the
runtime passes it verbatim), so a handler can read the query with
`route.params(request)`. The *matcher* splits the URI first: a route
matches its path, and the query parameters are added to the handler's
`params` map, with the path parameters winning.

**Durations in the configuration.** A duration in the configuration is
written the way the language writes one (`BOOKS_REPORT_EVERY=10s`), or
as a plain number of milliseconds (`BOOKS_REPORT_EVERY=500`). The
JSON codec keeps its own convention (a duration is a number of
nanoseconds), unchanged.

**One unrelated fix.** `bench/gc/coverage.bdg` was not in the canonical
format at the base commit, which made `cargo test -p badger_syntax
--test format_repo` fail before any of this work. It is formatted now,
so the test is green.

## Tests

- `tests/run/routes_status.bdg`: the status of an error, the query
  string, and a path parameter against a query parameter of the same
  name.
- `tests/run/stdlib_validation.bdg`: `bimap`, `must`, `check`, the
  `std.validate` rules, the `std.kv` helpers, `std.default`, `count_by`,
  `to_text_or`.
- `tests/run/cli_defaults.bdg`: the command line as a value.
- `crates/badger_cli/tests/tools.rs` and `crates/badger_cli/tests/search.rs`:
  the new commands.
- `crates/badger_runtime/src/intrinsics_codec.rs`: unit tests for the
  duration spelling.
- The existing corpus (`cargo test -p badger_cli --test programs`)
  passes in all four modes, and the whole workspace's tests pass, with
  one exception: `cargo test -p badger_cli --test stress` fails in
  `stress_programs` with a crash in `badger_runtime::gc::collect`. That
  failure is **pre-existing**: it reproduces on the base commit
  (`83c6568`) with none of these changes, and it is in the runtime's
  garbage collector, not in the code here.

## Audit of the change (2026-10-19)

A correctness audit of the merged change (against `main` at `f4b4802`)
found five defects in the standard library and two stale comments. All of
them are fixed here.

| Defect | Fix |
|---|---|
| Seven standard-library error types (`NumericError`, `codec.DecodeError`, `system.ProviderError`, `channel.ConfigError`, `channel.SendError<T>`, `task.PanicInfo`, `task.TimedOut`) did not implement `HttpStatus`, so they could not be a route's error type — and the orphan rule (E0211) stopped an application from adding the implementation. | Every standard-library error type now implements `HttpStatus` (the 500 default). |
| `route.path_of`, `route.query_of` and `route.query_params` did not strip a URI's `#fragment`, so `/x?a=1#frag` gave the query the value `1#frag`. | The fragment is stripped first; the path and the query are what RFC 3986 says they are. |
| `http.StatusError` and `http.reply_for_error` promised a reply that carries the error's message; a 5xx does not (it keeps the message to itself). | The two comments state the rule `http.error_reply` already documents. |
| `http.Handler`'s comment still said that a handler's error becomes a 500. | It says that the error is answered with the status it carries. |
| `http.with_status` was `http.json` under a second name and could not attach a status to a reply built by hand. | `http.with_status(response, status)` returns the response with that status. |

The audit also confirmed as sound: the query-string edge cases (an empty
name, a repeated parameter, a parameter without a value, percent
encoding, a `?` inside a segment), the path parameter winning over a
query parameter of the same name, the matcher, the status and reply
bodies, the new `std.result` family, `std.validate`, `std.kv`,
`std.default`, `std.cli`, `pipeline.recover_with`, and the free-function
and method forms of every new operation.
