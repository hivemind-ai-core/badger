# Language and compiler changes — ergonomics backlog for the compiler team

**Date:** 2026-10-19
**Owner:** compiler team (frontend: parser, type checker, lowering; backend: diagnostics, runtime semantics, build pipeline)
**Status:** ready for handoff — every item is verified against the tree at `83c6568`
**Companion report:** [2026-10-19-library-and-framework-changes.md](2026-10-19-library-and-framework-changes.md) — every item in this report that has a standard-library or framework counterpart is cross-referenced there, and the split is designed so the two teams can work in parallel.

## What this report is

This is the merged language/compiler half of the ergonomics work. It
combines two sources:

- the **ergonomics investigation** — a systematic audit of the language,
  the standard library, the web framework, the tooling, the diagnostics
  and the docs, read off the spec, the stdlib sources, the CLI reference
  and the example programs;
- the **experience report** — a first-person account of writing a
  673-line database-backed REST service (`examples/13_rest_service.bdg`)
  from a blank slate, which cost roughly eighteen `badger check`/`test`
  cycles, about half of them spent on language friction rather than
  application design.

Both are now consolidated into two reports, one per owner. This file is
the compiler team's half. Every claim below was re-verified against the
current tree; items that have since been fixed are in [§7](#7-already-shipped--do-not-re-implement)
rather than in the backlog, so nobody picks up work that no longer
exists.

## How to read an item

Each item carries a stable ID (`L-nn`), the symptom, the evidence
(file and line, so you can check it yourself), the fix, the effort
(S ≈ 1 day, M ≈ 1 week, L ≈ 1 quarter), who feels it, and its
dependencies. Items marked **needs library** have a counterpart in the
library report; the compiler-side work is listed here because it cannot
be finished without the library change landing.

## 1. Syntax changes

### L-01 — `?` should work on `Option<T>` when the function returns `Result<_, E>`

**Symptom.** Every handler that reads a string out of the URL or the
query string writes a nested match:

```badger
let id = match params.get("id") {
    .some(s) => match text.parse_int(s) {
        .some(n) => n,
        .none => return reply_for_error(AppError.invalid_input($"id is not a number: {s}")),
    },
    .none => return reply_for_error(AppError.invalid_input("id is required")),
}
```

**Evidence.** The spec makes this deliberate — "`Option` must be
matched or converted explicitly with a missing-value error"
([`spec/03-functions-and-errors.md:122`](../spec/03-functions-and-errors.md)) —
and the type checker rejects it with E0313, "`?` requires a `Result`",
with an Option-specific note ([`crates/badger_typeck/src/infer.rs:1917-1928`](../crates/badger_typeck/src/infer.rs)).
A partial mitigation already exists: `Option.ok_or` / `ok_or_else`
([`crates/badger_hir/std/option.bdg:25-30`](../crates/badger_hir/std/option.bdg),
`139`), documented in [`docs/guide/03-errors.md:110`](../docs/guide/03-errors.md).
The gap is that the ergonomic spelling is `expr?`, and the ergonomic
error is application-specific, so it cannot be resolved without the
application's error type.

**Fix.** Allow `?` on `Option<T>` when the enclosing function returns
`Result<_, E>` and `E: From<OptionMissing>`, where `OptionMissing` is a
built-in empty struct with `derive(Error, From)`. The compiler
synthesizes the missing-value error at the `?` site. Combined with
`let-else` (L-03) this collapses the cascade to:

```badger
let id = text.parse_int(params.get("id")?)?
```

**Effort.** M — the type rule and the lowering; the rest is
documentation.

**Affected.** Every request handler, every config parser, every CLI
argument parser. **Highest-leverage single syntax change in this
report.**

**Depends on.** Nothing. **Pairs with** library item S-01
(`bail!`/`must!`/`validate!`) and framework item F-A1 (status-carrying
handler errors) — the three together are the single biggest feel
change available.

---

### L-02 — Local `fn` declarations and local generic functions

**Symptom.** Helpers must live at module scope. A `fn` declaration
inside a function body is a syntax error; so is a local generic
function. The workarounds are inline lambdas (which cannot be reused)
or top-level helpers (which pollute the namespace).

**Evidence.** Block items are only `let` / `use` / assignment /
expression ([`crates/badger_syntax/src/grammar.rs:1010`](../crates/badger_syntax/src/grammar.rs),
`1023-1035`), and `cst::Stmt` has no `Fn` variant
([`crates/badger_syntax/src/cst.rs:331-355`](../crates/badger_syntax/src/cst.rs)).
Lambdas cannot carry generic parameters
([`crates/badger_syntax/src/grammar.rs:440-452`](../crates/badger_syntax/src/grammar.rs),
`cst.rs:367-374`). The experience report hit this twice and lost ~10
minutes to it (F4): the intended helper was

```badger
fn lift<X>(r: Result<X, sqlite.Error>) -> Result<X, AppError> {
    r |> result.map_err(AppError.database)
}
```

and the parser answered `expected '(', found '<'`.

**Fix.** Allow `fn name<T>(...)` inside any function body. The
declaration is visible until the end of the enclosing block. Generic
inference and the effect rules are unchanged; the work is a new
"function block" rule in the parser plus local-definition tracking in
the resolver.

**Effort.** L — parser and name resolution. **Affected.** Every
non-trivial function that wants a helper.

---

### L-03 — `let-else`: allow the `else` block to end in `err(...)`

**Symptom.** The `else` block's type is `Never`, so it must end in
`return`, `break`, `continue` or a panic. `let .some(x) = expr else
{ err(AppError.foo) }` is rejected — the user must write `return
err(...)`, which feels redundant on a one-line pattern.

**Fix.** Accept `err(...)` as a shortcut for `return err(...)` when the
enclosing function returns a `Result`, and `panic(...)` when it returns
`Never`.

**Effort.** S — a parser and typing rule. **Affected.** Every refutable
pattern in a `Result`-returning function.

---

### L-04 — Bare enum variant in expression position for single-variant enums

**Symptom.** `enum Status { open }` is constructed as `Status.open` —
six keystrokes and a screen of attention for a one-variant enum, where
a record would be `{ kind: "open" }`.

**Evidence.** The spec forbids bare variants in expression position
([`spec/01-syntax.md:247`](../spec/01-syntax.md)) on principle, but
the framework already does it implicitly for `ok` / `err` / `some` /
`none`.

**Fix.** Allow the bare variant when the expected type is known and
the enum has a single variant.

**Effort.** S to M — a small, backward-compatible parser relaxation.

---

### L-05 — `?`-as-do-notation (error-type transformation for `Result`)

**Symptom.** `result.map` changes `T` and leaves `E` alone; the
idiomatic pipe `r |> result.map_err(AppError.http)` is awkward because
it requires piping the result and then passing the function, while the
reversed `result.map_err(r, AppError.http)` breaks the pipeline
convention. Every error-mapping function pays this.

**Evidence.** [`crates/badger_hir/std/result.bdg:3-8`](../crates/badger_hir/std/result.bdg)
(`map` changes `T` only), `10-15` (`map_err` changes `E` only). There
is no `bimap` / `map_both` in the tree.

**Fix.** Two options, pick one:

- add `result.bimap(ok_fn, err_fn)` — a one-liner in the library, see
  library item **S-02**; or
- add `?`-as-do-notation: a block of `let x = expr?` lines that
  unwraps an `Err` of one type and returns a different `Result`,
  converting through `From`.

**Effort.** S for the library option, M for the language option.

**Note.** The experience report recorded this as F13 and attributed
"the wrong error type" confusion to it repeatedly.

---

## 2. Type system and semantics

### L-06 — `derive(Default)` for records and structs

**Symptom.** Every settings type and every DTO needs a hand-written
`default_settings()` returning a full record literal, repeated for
every type with sane defaults.

**Evidence.** The supported derives are `Eq`, `Hash`, `Display`,
`Codec`, `From` (enums only) and `Error`
([`crates/badger_typeck/src/env.rs:876-943`](../crates/badger_typeck/src/env.rs));
anything else is E0204, "`{other}` cannot be derived", with the list in
the message (`env.rs:938-942`). Derives attach to structs and enums
only, not to records
([`crates/badger_syntax/src/grammar.rs:1169`](../crates/badger_syntax/src/grammar.rs),
`1200`).

**Fix.** Add `derive(Default)` and a `Default` protocol. The derivation
needs a default for every field type: `Int: 0`, `Float: 0.0`,
`Bool: false`, `String: ""`, `List<T>: empty()`, `Map<K, V>: empty()`,
`Option<T>: none()`. For named types the user supplies a
`default_NAME` function, or the derivation synthesizes the literal when
every field has a default.

**Effort.** M. **Affected.** Every settings / DTO / config record.

**Depends on.** Library item S-15 (the `default!` macro is the
companion for single values, and it fixes the default convention in
one place for the derivation to read).

---

### L-07 — `derive(Builder)` for record construction

**Symptom.** A ten-field record is built as one literal with every
field listed. Building it incrementally — "default, then override
`host` and `port`" — requires `{ ..default(), host: x, port: y }`,
which repeats the default every time.

**Fix.** Add `derive(Builder)`: generates a `Builder` type with
`set(field, value)`, `init(default)` and
`build() -> Result<Self, ValidateError>`. The pattern is common in Go,
Rust (`derive_builder`) and TypeScript.

**Effort.** M. **Affected.** Every record with 5+ fields.

**Note.** A `derive(Default)` plus `{ ..default, field: value }` is a
workable substitute, but ergonomically inferior. Build L-06 first.

---

### L-08 — `derive(Copy)` for records

**Symptom.** Records are immutable but not `Copy`, so passing one by
value moves it. Many functions take `&Record` where `Record` by value
would be more pleasant.

**Fix.** Allow `derive(Copy)` for records whose fields are all `Copy`.

**Effort.** M. **Affected.** Every record passed by reference that
could be passed by value.

---

### L-09 — A flatten knob for wrapper structs in JSON

**Symptom.** A newtype struct `BookId { value: 1 }` encodes as
`{"value": 1}`, so a `Book` with an `id: BookId` field encodes as
`{"id":{"value":1}, ...}`. Most web APIs flatten to `{"id":1, ...}`.

**Evidence / correction.** The experience report (F6) described this as
records-flat-versus-structs-nested. That asymmetry does not exist:
structs and records are both `Kind::Record`
([`crates/badger_runtime/src/desc.rs:104-108`](../crates/badger_runtime/src/desc.rs),
[`crates/badger_mir/src/layout.rs:358-365`](../crates/badger_mir/src/layout.rs)),
and the JSON encoder, decoder and JSON Schema generator all ignore
`nominal`
([`crates/badger_runtime/src/codec.rs:209-234`](../crates/badger_runtime/src/codec.rs),
`450+`;
[`crates/badger_runtime/src/intrinsics_std.rs:673-690`](../crates/badger_runtime/src/intrinsics_std.rs)).
A struct field nests exactly like a record field. The real gap is that
there is no way to ask for flattening.

**Fix.** Two halves:

- **needs library** — a codec-level flatten marker or a
  `#[flatten]`-equivalent in the record/struct shape, plus the JSON
  Schema output; see library item **S-09**;
- **compiler** — a `derive(Flat)` (or a field attribute) that marks
  the wrapper, if the marker is expressed in the type rather than in
  the codec call.

**Effort.** M. **Affected.** Every DTO with a newtype field.

---

## 3. Compiler diagnostics

### L-10 — Type errors point at the wrong line

**Symptom.** A type error that cascades — pipe expansion, generic
unification, effect rows — is reported where inference failed, not
where the bad code was written. The worst case is the closing `}` of a
block being flagged for an error twenty lines up. The experience report
measured ~20 minutes lost across many attempts (F11).

**Evidence.** The parser's recovery is decent for lexical errors, but
the type checker's error spans are anchored at the inference site.

**Fix.** When inference fails partway through an expression, keep
walking up and pick the smallest sub-expression whose type is the one
that does not fit. A cheap "good enough" version: emit a note at the
original inference site in addition to the primary label.

**Effort.** M. **Affected.** Every iteration. **This is the single
highest-impact diagnostic change in this report.**

---

### L-11 — "Did you mean…?" for unknown names

**Symptom.** E0127, "unknown name", is a single line. A typo
(`reslt` for `result`) is a multi-minute search.

**Fix.** When an unknown name is close to a name in scope
(Levenshtein distance ≤ 2, or a name from a similar module), suggest
the closest match. A few hundred lines in the resolver.

**Effort.** S. **Affected.** Every typo.

---

### L-12 — Auto-fix for common patterns

**Symptom.** A handful of patterns recur: `result |> result.map(...)`
should be `result.map(...)`; `result.map_err(AppError.x)` with no `?`
should suggest the `?`; an unused `Result` (E0337) should suggest
`let _ = ...`; a pipe into a module function that does not take the
piped value first should suggest the right argument order.

**Fix.** A small library of "common fix" patterns, exposed as LSP
`CodeAction`s and as `badger check --apply-fixes`.

**Effort.** M. **Affected.** Every common mistake.

---

### L-13 — "Did you mean to import…?" for unimported stdlib members

**Symptom.** Calling a function the user has not imported shows up as
E0127, "unknown name". The current note — "`…` is a standard module:
add import …" — fires for some cases only.

**Fix.** Put the import suggestion on every E0127 where the name
resolves in the stdlib but is not in scope.

**Effort.** S. **Affected.** Every stdlib call from a new project. The
experience report's suggested five-minute wins included this one.

---

### L-14 — Per-effect-row explanations

**Symptom.** The compiler can tell you a function has
`!{Io, Wait, State}` but not *which call* brought in the `State`
effect. `badger explain effect` lists the function, not the cause.

**Fix.** Make `badger explain effect` list, for each effect, the call
sites that require it. This is the "constraint explanations with
origins" item in [`REAL_WORLD_USE.md`](../REAL_WORLD_USE.md).

**Effort.** M. **Affected.** Every effect-debugging session.

---

## 4. Runtime and system semantics

### L-15 — `task.scope()` inside a provider body hangs the system

**Symptom.** A provider that opens a `task.scope` to spawn a
long-lived background worker compiles cleanly and then hangs at
startup: no listening port, no log output, no worker pool. The only
signal is "no listening port".

**Evidence.** There is no compile-time rejection. The only
provider-specific check in the type checker is scope *escape*
([`crates/badger_typeck/src/regions.rs:22-49`](../crates/badger_typeck/src/regions.rs));
nothing mentions `task.scope` in a provider.
`use scope <- task.scope()` desugars to `task.scope(fn(scope){…})`
([`crates/badger_hir/src/lower.rs:1974-2023`](../crates/badger_hir/src/lower.rs)),
and `task.scope` closes the scope by waiting for its children
([`crates/badger_hir/std/task.bdg:70-78`](../crates/badger_hir/std/task.bdg)),
so a long-lived worker blocks the provider's initialization. Note the
API has changed since the experience report was written: the zero-arg
`task.scope()` no longer exists, `task.scope` takes a body.

**Fix.** Two options:

- **compile-time reject** (cleanest, and the compiler team's half):
  reject `task.scope` in a provider body with a diagnostic that
  points at the provider's `scope` parameter and at
  `scope.worker(...)` as the intended form;
- **make it work**: rewrite it to "register with the system" so the
  task lives as long as the system does — that is what `scope.worker`
  does, and it is a runtime change.

**Effort.** S to reject at compile time; L to make it work.

**Depends on.** If the compile-time rejection is chosen, the runtime
must not also change. If the runtime option is chosen, this becomes a
framework item instead. **Decide once, then implement in one place.**

---

### L-16 — Channels that cross the system graph boundary

**Symptom.** A channel opened with `channel.open` is bound to the `use`
block's lifetime, so its sender cannot be shared with one provider and
its receiver with another. The experience report planned an
API-publishes-to-a-background-worker design on a typed
`Channel<Change>` and dropped the feature for a polling worker (F9).

**Evidence / correction.** A channel is not categorically blocked from
crossing the graph: a resource provider may return arbitrary typed
values, including channels
([`spec/06-systems-and-pipelines.md:15-16`](../spec/06-systems-and-pipelines.md)),
and that is demonstrated in
[`docs/guide/08-systems-and-pipelines.md:197-214`](../docs/guide/08-systems-and-pipelines.md).
What is missing is a *declarative* channel resource that the system
owns and closes with the node.

**Fix.** The library half — a `channel.resource` provider that
registers an `Endpoints<T>` in a system node, so publishers pull the
sender and subscribers pull the receiver, and the channel closes when
the system halts — is library item **S-11**. The compiler half is
that the `use`-block lifetime rule must permit the channel to be
stored in a provider's return value without the scope-escape
diagnostic firing.

**Effort.** M on the library side, S on the compiler side. **Affected.**
Every request-time-events design.

---

## 5. Compiler and build performance

### L-17 — Faster incremental `badger check`

**Symptom.** A first check of a project is fast, but a check after a
one-line edit re-parses and re-checks every entry, every task, every
imported module, and (for the parser and the type checker) most of the
standard library.

**Fix.** A per-file parse cache keyed on content hash; a
checked-library reuse pass; a reusable compiler context. This is
already planned as C1.2b / C1.3 / C1.4 in
[`BACKLOG.md`](../BACKLOG.md#compiler-and-runtime-performance-optimization_planmd)
and there is in-flight work in
[`reports/2026-10-19-frontend-session-reuse.md`](2026-10-19-frontend-session-reuse.md)
and
[`reports/2026-10-19-std-check-reuse.md`](2026-10-19-std-check-reuse.md).
**This report does not re-plan it; it is listed so the compiler team's
scope is complete.**

**Effort.** L (already in flight). **Affected.** Every iteration.

---

### L-18 — Incremental `badger test`

**Symptom.** `badger test` re-checks the whole project every
invocation even if one test changed. For 50 tests across 20 modules
the edit/test cycle is dominated by the check.

**Fix.** Reuse the parse/check cache from L-17. Optionally run only
the test files that import changed files, using the file dependency
graph.

**Effort.** L. **Affected.** Every test cycle.

---

### L-19 — Parameterised contracts

**Symptom.** The experience report tried `requires value != ""` on
`text.trim` and the contract fired on a legitimate call to `trim("")`,
so the contract was removed. The problem is that the contract language
cannot express a bound: "a UTF-8 string of length ≤ M" is not
expressible with the current form, and the only available constraint
is one that is wrong for some legitimate callers.

**Evidence.** A `badger contracts` command exists
([`crates/badger_cli/src/main.rs:338`](../crates/badger_cli/src/main.rs)),
and contracts are part of the type-checking story, but the precondition
vocabulary is not rich enough to be useful on a function with a
legitimate edge case.

**Fix.** Allow a parameterised precondition — a bound, a predicate over
the argument, or a call to a named user function — so a contract can be
stated as "a UTF-8 string of length ≤ M" rather than as a fixed
inequality. Whatever the form, `badger contracts` must be able to
confirm that every caller satisfies it.

**Effort.** M. **Affected.** Any code that wants to state a
precondition on a function with edge cases.

---

### L-20 — Derive machinery that can carry per-variant metadata

**Symptom.** The library side wants `derive(HttpError)` — an error enum
whose variants carry a status code and a body shape — and
`derive(HttpReply)`, which generates a reply mapping from the same
enum. The current derive set (`Eq`, `Hash`, `Display`, `Codec`,
`From`, `Error`;
[`crates/badger_typeck/src/env.rs:876-943`](../crates/badger_typeck/src/env.rs))
has no way to attach metadata to a variant, and every derive is
closed over a fixed protocol. The framework's own F-A1 fix would be
noticeably better with it: the derive already knows the per-status
body shape, so the OpenAPI document can state it (library **F-N4**).

**Fix.** Generalize the derive machinery so a derivation can attach
per-variant metadata to the enum it derives from, and let the library
read that metadata. This is compiler work only if the metadata is
expressed in the type; if the framework instead reads an explicit
side-table, no compiler change is needed (see the fallback in the
library report).

**Effort.** M. **Affected.** Every service that wants a declarative
error-to-status mapping.

---

## 6. Compiler-side work required by the library and framework work

These are owned by the library team in day-to-day terms, but they are
**blocked on the compiler**. They are listed here so the compiler team
can schedule the enabling work, and cross-referenced in the library
report so nothing is assumed to be free.

| Compiler work needed | Blocks (library report) |
|---|---|
| A `derive` that can attach per-variant metadata (status code, body shape) to an error enum | **F-A1** status-carrying handler errors, **F-N3** `derive(HttpError)` — compiler **L-20** |
| A `derive` that can generate a reply mapping from an error enum | **F-I2** `respond!` / `derive(HttpReply)` — compiler **L-20** |
| `derive(Default)` / `derive(Builder)` / `derive(Copy)` / flatten marker (L-06 – L-09) | **S-15**, **S-09** |
| A `Default` protocol in the type system | **S-15** `default!` macro and the default convention |
| A `?` rule that converts through `From` (L-01) | **S-01**, **F-A1** |
| A compile-time error for `task.scope()` in a provider (L-15) | **F-A5** |

If the compiler team would rather not own the `HttpError` /
`HttpReply` derivations, the fallback is an explicit type
(`http.StatusError { status: Int, error: E }`) with the mapping done
by hand in the library — less ergonomic, but it removes the dependency
on new derive machinery. That choice is the library team's to make.

## 7. Already shipped — do not re-implement

Verified fixed as of `83c6568`:

| Was | Now | Evidence |
|---|---|---|
| `if let` / `while let` missing (audit 2.7) | Implemented as a desugaring to `match`; `if let .ok(x) = r` is documented | [`crates/badger_syntax/src/lower.rs:971-983`](../crates/badger_syntax/src/lower.rs), `1034+`; [`spec/01-syntax.md:209-221`](../spec/01-syntax.md); [`docs/guide/01-values-and-functions.md:844-885`](../docs/guide/01-values-and-functions.md); [`docs/tour.md:392`](../docs/tour.md) |
| Triple-quoted strings undocumented (audit 1.5, experience F10) | Documented in the tour and the guide | [`docs/tour.md:89-94`](../docs/tour.md); [`docs/guide/01-values-and-functions.md:210`](../docs/guide/01-values-and-functions.md), `231-232`; [`spec/01-syntax.md:101-102`](../spec/01-syntax.md) |
| `Option.ok_or` / `ok_or_else` missing | Present; a partial mitigation for L-01 | [`crates/badger_hir/std/option.bdg:25-30`](../crates/badger_hir/std/option.bdg), `41-46`, `139` |
| Logging returns a `Result` that must be discarded (audit 3.6) | `log.trace/debug/info/warn/error` return `Unit` | [`crates/badger_hir/std/log.bdg:18-26`](../crates/badger_hir/std/log.bdg) |

Partially addressed, with the remainder tracked here:

| Item | Done | Remaining |
|---|---|---|
| Pipeline `error` callback typing (audit 3.5, experience F8) | The error type is taken from the stage's `-> Result<_, E>` annotation or the callback's own parameter annotation; a body that needs the type without either now gets a diagnostic instead of a callback that silently never fires | The diagnostic path and the "works only sometimes" inference rule are still not obvious from the docs — see library item **F-A4** |
| `derive(From)` | Supported for enums, with a warning for ambiguous variants | The library side of `From`-based `?` conversion is unexercised — see L-01 |

## 8. Suggested sequencing

**A few hours**

1. **L-03** — `let-else` `err(...)` shortcut. Parser rule, one day.
2. **L-13** — import suggestions on every E0127. One line in the
   diagnostic builder; removes a per-session lookup.
3. **L-11** — "did you mean" for unknown names.

**A day**

4. **L-15** — reject `task.scope()` in a provider at compile time. One
   diagnostic, one parser rule. Removes a silent startup hang.
5. **L-10** (first pass) — emit a note at the original inference site
   in addition to the primary label. Cheap, and it is the change the
   experience report measured at ~20 minutes lost.

**A week or two**

6. **L-01** — `?` on `Option`. Type rule plus lowering; touches the
   type checker and the prelude.
7. **L-06** — `derive(Default)`, with the `Default` protocol.
8. **L-02** — local `fn` and local generics. Parser plus name
   resolution.

**A quarter**

9. **L-07**, **L-08** — `derive(Builder)`, `derive(Copy)`.
10. **L-14** — per-effect explanations.
11. **L-04**, **L-05**, **L-09** — bare enum variants, do-notation,
    flatten marker.
12. **L-17**, **L-18** — incremental check and test (already in flight
    via the BACKLOG).
13. **L-19** — parameterised contracts.
14. **L-20** — derive machinery with per-variant metadata, unblocking
    the framework's `derive(HttpError)` / `derive(HttpReply)`.

**The combination that changes the feel of the language most** is
**L-01** (`?` on `Option`) + **L-06** (`derive(Default)`) + **L-02**
(local functions): they turn a handler from a match cascade with
top-level helper pollution into `let x = expr?` and a helper where it
belongs. The error-path half of that story —
"the handler's `Err` should produce the status the application
intended" — belongs to the framework team and is library item **F-A1**.

## Appendix A — traceability

Where each item of the two source reports ended up.

| Source item | Destination |
|---|---|
| Ergonomics 1.1 (`?` on `Option`) | L-01 |
| Ergonomics 1.2 (local `fn`, local generics) | L-02 |
| Ergonomics 1.3 (`?` on `Result` does not transform `E`) | L-05 |
| Ergonomics 1.4 (`Duration` in config) | Library S-06 |
| Ergonomics 1.5 (triple-quoted strings) | Shipped |
| Ergonomics 1.6 (`let-else` must not return) | L-03 |
| Ergonomics 1.7 (single-variant enum ergonomics) | L-04 |
| Ergonomics 2.1 (`bail!` / `must!` / `validate!`) | Library S-01 |
| Ergonomics 2.2 (`derive(Default)`) | L-06 |
| Ergonomics 2.3 (`derive(Builder)`) | L-07 |
| Ergonomics 2.4 (`with_status` / `reply_for_error`) | Library F-I1 |
| Ergonomics 2.5 (`to_text_or` and friends) | Library S-03 |
| Ergonomics 2.6 (channel across the system boundary) | L-16 + library S-11 |
| Ergonomics 2.7 (`if let` for `Result`) | Shipped |
| Ergonomics 3.1 (handler `Err` → silent 500) | Library F-A1 |
| Ergonomics 3.2 (routes strip the query string) | Library F-A2 |
| Ergonomics 3.3 (`route.typed` error story) | Library F-A3 |
| Ergonomics 3.4 (`task.scope()` in a provider) | L-15 |
| Ergonomics 3.5 (pipeline `error` annotation) | Partly shipped; library F-A4 |
| Ergonomics 3.6 (`let _ = log.warn(...)`) | Shipped |
| Ergonomics 3.7 (`respond!` shorthand) | Library F-I2 |
| Ergonomics 3.8 (pipeline `around` stage) | Library F-I3 |
| Ergonomics 4.1 – 4.9 (tooling) | Library T-01 – T-10 |
| Ergonomics 5.1 (wrong-line errors) | L-10 |
| Ergonomics 5.2 ("did you mean") | L-11 |
| Ergonomics 5.3 (auto-fixes) | L-12 |
| Ergonomics 5.4 (import suggestions) | L-13 |
| Ergonomics 5.5 (per-effect explanations) | L-14 |
| Ergonomics 6.1 (`derive(Copy)`) | L-08 |
| Ergonomics 6.2 (map / set literals) | Library S-05 |
| Ergonomics 6.3 (`sorted_by_key` and friends) | Library S-04 |
| Ergonomics 6.4 (`Result`/`Option` from predicate) | Library S-07 |
| Ergonomics 6.5 (`kv` helper) | Library S-08 |
| Ergonomics 6.6 (`cli` argument parser) | Library S-10 |
| Ergonomics 7.1 – 7.4 (documentation) | Library D-01 – D-04 |
| Ergonomics 8.1 – 8.2 (dev-loop performance) | L-17, L-18 |
| Ergonomics 9.1 – 9.5 (other CLI) | Library T-11 – T-15 |
| Experience F1 (silent 500) | Library F-A1 |
| Experience F2 (`task.scope()` hangs) | L-15 |
| Experience F3 (module function vs method split) | Library S-12 |
| Experience F4 (no local `fn` / generics) | L-02 |
| Experience F5 (query strings) | Library F-A2 |
| Experience F6 (structs serialize nested) | L-09 + library S-09 |
| Experience F7 (`Duration` not in config) | Library S-06 |
| Experience F8 (pipeline `error` annotation) | Library F-A4 |
| Experience F9 (channels across the boundary) | L-16 + library S-11 |
| Experience F10 (multi-line strings) | Shipped |
| Experience F11 (errors at the wrong line) | L-10 |
| Experience F13 (`result.map` does not transform `E`) | L-05 + library S-02 |
| Experience — contracts could not be stated for a function with an edge case | L-19 |
