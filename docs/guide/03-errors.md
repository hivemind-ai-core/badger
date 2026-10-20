# 3. Errors

How Badger represents absence and failure: `Option`, `Result`, `?` with
error conversion, the `Error` protocol, and defects for bugs.

## Three kinds of "it didn't work"

| Situation | Use | Example |
|---|---|---|
| A value may be absent | `Option<T>` | `list.get(i)`, `map.get(k)`, `text.parse_int(s)` |
| An operation can fail in an expected way | `Result<T, E>` | reading a file, decoding input, validation |
| The program is wrong | a defect | overflow, index out of bounds, `assert(false)`, `panic(...)` |

There is no `null`, no exceptions, and no `throw`/`catch`. Expected
failures are ordinary values. Defects end the task.

## Option

```badger fragment
enum Option<T> {
    none,
    some(value: T),
}
```

```badger
import std.io

fn find_user(id: Int) -> Option<String> {
    if id == 1 { some("ada") } else { none }
}

pub fn main() -> Result<Unit, io.Error> {
    io.println(find_user(1))?
    io.println(find_user(2))?
    match find_user(1) {
        .some(name) => io.println($"found {name}")?,
        .none => io.println("no such user")?,
    }
    io.println(find_user(2).unwrap_or("guest"))?
    ok(())
}
```

```text
some(ada)
none
found ada
guest
```

- `some(x)` and `none` are prelude helpers; `Option.some(x)` and
  `Option.none` are the same values.
- Get the value out with `match`, `if let`, or a combinator such as
  `unwrap_or` (below).

## Result

```badger fragment
enum Result<T, E> {
    ok(value: T),
    err(error: E),
}
```

`ok(x)` and `err(e)` are the prelude helpers. `E` can be any type: a
`String` for a script, an enum for an application.

A `Result` must be used. Leaving one unhandled is E0337, including a
`Result` inside a list, struct, or enum, and one discarded at the end of
a loop body. Handle it with `?` or `match`, or discard it on purpose:

```badger fragment
io.println("hi")          // E0337: unused `Result`
let _ = io.println("hi")  // explicitly discarded
```

## `?`

`expr?` takes the value out of `ok`, or returns the `err` from the
enclosing function or lambda.

```badger
import std.io

fn half(n: Int) -> Result<Int, String> {
    if n % 2 == 0 { ok(n / 2) } else { err($"{n} is odd") }
}

fn quarter(n: Int) -> Result<Int, String> {
    let h = half(n)?
    half(h)
}

pub fn main() -> Result<Unit, io.Error> {
    io.println(quarter(12))?
    io.println(quarter(6))?
    ok(())
}
```

```text
ok(3)
err(3 is odd)
```

| Rule | Diagnostic |
|---|---|
| The enclosing function or lambda must return `Result` | E0314 |
| `?` on something that is neither a `Result` nor an `Option` | E0313 |
| The error must be the same type, or convert by one `From` (next section) | E0333 |

`?` returns from the *nearest* function or lambda, so `?` inside a
callback leaves the callback. It runs any pending cleanup on the way out.

`?` on an `Option` returns an `OptionMissing` error for `none`, converted
like any other: give the function's error a variant for it.

```badger fragment
@derive(Error, From)
pub enum AppError {
    missing(source: OptionMissing),
    invalid(message: String),
}

fn id_of(params: Map<String, String>) -> Result<Int, AppError> {
    ok(text.parse_int(params.get("id")?)?)
}
```

To report something more specific, name the error with `ok_or`:

```badger fragment
let first = items.first().ok_or("no items")?
```

## Error conversion with `From`

`?` converts the error when the function's error type has exactly one
direct `From` implementation for it. `@derive(From)` on an enum generates
one for each variant with a single payload.

```badger
import std.io

@derive(Error)
pub enum ParseError {
    empty,
    not_a_number(text: String),
}

@derive(Error, From)
pub enum AppError {
    parse(source: ParseError),
    range(source: NumericError),
    io(source: io.Error),
}

fn parse_int(s: String) -> Result<Int, ParseError> {
    if s.is_empty() {
        return err(ParseError.empty)
    }
    match s.parse_int() {
        .some(n) => ok(n),
        .none => err(ParseError.not_a_number(s)),
    }
}

fn parse_port(s: String) -> Result<U16, AppError> {
    let n = parse_int(s)?
    let port = U16.try_from(n)?
    ok(port)
}

pub fn main() -> Result<Unit, AppError> {
    io.println(parse_port("8080"))?
    io.println(parse_port(""))?
    io.println(parse_port("eighty"))?
    io.println(parse_port("70000"))?
    ok(())
}
```

```text
ok(8080)
err(parse: empty)
err(parse: not a number: eighty)
err(range: value out of range for the target type)
```

In `parse_port`, the first `?` converts `ParseError` to `AppError.parse`,
and the second converts `NumericError` to `AppError.range`. In `main`, `?`
on `io.println` converts `io.Error` to `AppError.io`.

Conversion is deliberately narrow:

- One direct step only. `?` does not chain conversions or build unions.
- Two variants with the same payload type make the conversion ambiguous:
  `derive(From)` skips that type and warns (E0203). Map it explicitly.
- A private function with no declared result type infers its error type
  from a single source. Several different error types flowing into one
  `?` target need a declared result type (E0325).
- `map_err` converts by hand: `read(path).map_err(fn(e) => AppError.io(e))?`.

A struct error implements `From` directly:

```badger
import std.io

pub struct ConfigError {
    pub message: String,
}

impl Display for ConfigError {
    fn display(self: &Self) -> String { $"config: {self.message}" }
}

impl Error for ConfigError {}

impl From<NumericError> for ConfigError {
    fn from(value: NumericError) -> ConfigError { { message: $"{value}" } }
}

fn port(n: Int) -> Result<U16, ConfigError> {
    let p = U16.try_from(n)?
    ok(p)
}

pub fn main() -> Result<Unit, io.Error> {
    io.println(port(8080))?
    io.println(port(70000))?
    ok(())
}
```

```text
ok(8080)
err(config: value out of range for the target type)
```

## The `Error` protocol

`Error` is the prelude protocol for expected failures. It extends
`Display` and adds three methods, each with a default:

| Method | Default |
|---|---|
| `code(self) -> Symbol` | the enum variant's name, or the type's name in snake case |
| `cause(self) -> Option<dyn Error>` | `none`; with `derive(Error)`, the payload of a single-field variant that is itself an `Error` |
| `details(self) -> Data` | the value's structured encoding |

`derive(Error)` on an enum provides `Display` and `Error`. The derived
text is the variant name (underscores as spaces) followed by its
payload: `not a number: eighty`. `error_chain(e)` lists the displayed
errors from the outermost cause inwards.

```badger
import std.io

pub struct ParseFailure {
    pub line: Int,
}

impl Display for ParseFailure {
    fn display(self: &Self) -> String { $"bad input on line {self.line}" }
}

impl Error for ParseFailure {
    fn details(self: &Self) -> Data { { line: self.line } }
}

@derive(Error)
pub enum LoadError {
    parse(source: ParseFailure),
    missing(path: String, reason: String),
}

pub fn main() -> Result<Unit, io.Error> {
    let e = LoadError.parse({ line: 7 })
    io.println($"{e.code()}: {e}")?
    match e.cause() {
        .some(c) => io.println($"cause {c.code()}: {c} {c.details()}")?,
        .none => io.println("no cause")?,
    }
    io.println(error_chain(e))?
    let m = LoadError.missing("a.txt", "gone")
    io.println($"{m.code()}: {m}")?
    ok(())
}
```

```text
parse: parse: bad input on line 7
cause parse_failure: bad input on line 7 {"line":7}
["parse: bad input on line 7", "bad input on line 7"]
missing: missing: a.txt, gone
```

## When `main` returns an error

If `main` returns `err(e)`, the program prints a report to standard error
and exits with status 1. The report has the error's code, its message,
each cause, its details, and the source location where the error started
propagating.

```badger
import std.io

@derive(Error)
pub enum ParseError {
    not_a_number(text: String),
}

@derive(Error, From)
pub enum AppError {
    parse(source: ParseError),
    io(source: io.Error),
}

fn parse_int(s: String) -> Result<Int, ParseError> {
    match s.parse_int() {
        .some(n) => ok(n),
        .none => err(ParseError.not_a_number(s)),
    }
}

pub fn main() -> Result<Unit, AppError> {
    let n = parse_int("eighty")?
    io.println(n)?
    ok(())
}
```

```text
$ badger run port.bdg
error[parse]: parse: not a number: eighty
  caused by: not a number: eighty
  details: {"parse":{"source":{"not_a_number":{"text":"eighty"}}}}
  at port.bdg:15:18
$ echo $?
1
```

The location is the `err(...)` that created the error (here inside
`parse_int`), or the first `?` that propagated an error from the standard
library. A `String` error is reported as `error: MESSAGE` followed by the
location.

## Defects

A defect means the program is wrong. It stops the current task with a
message and a source location. A program whose main task fails exits with
status 101.

| Cause | Message starts with |
|---|---|
| integer overflow, invalid shift | `integer overflow` |
| integer division by zero | `division by zero` |
| `xs[i]` out of range | `index out of bounds` |
| `assert(cond)` with a false `cond` | `assertion failed` |
| `panic(message)` | the message |
| a negative `Duration` | `negative duration` |
| a failed `requires`/`ensures` contract | the clause ([chapter 9](09-testing-and-contracts.md)) |

```badger
pub fn main() -> Unit {
    let mode = "fast"
    let delay: Int = match mode {
        "slow" => 100,
        "fast" => 10,
        _ => panic($"unknown mode {mode}"),
    }
    assert(delay > 50)
}
```

```text
$ badger run check.bdg
panic: assertion failed at check.bdg:8:5
$ echo $?
101
```

- `panic` returns `Never`, so it fits in any branch. When the other
  branches are bare integer literals, annotate the binding
  (`let delay: Int = ...`); without it the checker rejects the literals
  (E0326).
- Use `get(i)` (returning `Option`) instead of `xs[i]` when an index may
  be invalid; use `checked_add` and friends when overflow is expected.
- `?` and `match` never see a defect. Scoped cleanup still runs.
- Defects are contained at task boundaries. `scope.capture` runs a child
  task and reports how it ended as a `task.Exit`:

```badger
import std.io
import std.task

fn average(xs: List<Int>) -> Int {
    assert(!xs.is_empty())
    xs.fold(0, fn(a, b) => a + b) / xs.len()
}

pub fn main() -> Result<Unit, io.Error> {
    let outcome = task.scope(fn(s) -> Result<task.Exit<Int>, io.Error> {
        let child = s.capture(fn() => average([]))
        ok(child.join())
    })?
    match outcome {
        .returned(n) => io.println($"average {n}")?,
        .failed(info) => io.println($"defect: {info}")?,
        .cancelled(reason) => io.println($"cancelled: {reason}")?,
    }
    io.println("still running")?
    ok(())
}
```

```text
$ badger run average.bdg
defect: assertion failed at average.bdg:5:5
still running
```

Task scopes, supervision, and restart policies are in
[chapter 6](06-tasks-and-channels.md).

## Combinators

`std.option` and `std.result` hold the helpers. Most are also methods, so
`o.unwrap_or(0)` works without an import. The two marked "module" need
`import std.option` or `import std.result`.

### Option

| Call | Result |
|---|---|
| `o.is_some()`, `o.is_none()` | `Bool` |
| `o.unwrap_or(default)` | the value, or `default` |
| `o.unwrap_or_else(fn() => ...)` | the value, or the fallback computed only when absent |
| `o.map(fn(x) => ...)` | `Option` of the mapped value |
| `o.and_then(fn(x) => ...)` | chain a step that returns `Option` |
| `o.filter(fn(x) => ...)` | the value only if the test accepts it |
| `o.or(other)`, `o.or_else(fn() => ...)` | this option, or the other one |
| `o.zip(other)` | `some((a, b))` when both are present |
| `o.ok_or(error)`, `o.ok_or_else(fn() => ...)` | `Result`: absence becomes `err` |
| `o.to_list()` | a list of zero or one items |
| `option.flatten(o)` (module) | `Option<Option<T>>` to `Option<T>` |
| `option.transpose(o)` (module) | `Option<Result<T, E>>` to `Result<Option<T>, E>` |

### Result

| Call | Result |
|---|---|
| `r.is_ok()`, `r.is_err()` | `Bool` |
| `r.unwrap_or(default)` | the value, or `default` |
| `r.unwrap_or_else(fn(e) => ...)` | the value, or a fallback computed from the error |
| `r.map(fn(x) => ...)` | map the success value |
| `r.map_err(fn(e) => ...)` | map the error |
| `r.and_then(fn(x) => ...)` | chain a step that returns `Result` |
| `r.or_else(fn(e) => ...)` | recover; may change the error type |
| `r.value()`, `r.error()` | `Option` of the value or of the error |
| `result.flatten(r)` (module) | `Result<Result<T, E>, E>` to `Result<T, E>` |
| `result.transpose(r)` (module) | `Result<Option<T>, E>` to `Option<Result<T, E>>` |

There is no `unwrap` or `expect`. When absence or failure would be a bug,
say so with `match` and `panic`.

```badger
import std.io
import std.option

fn half(n: Int) -> Option<Int> {
    if n % 2 == 0 { some(n / 2) } else { none }
}

pub fn main() -> Result<Unit, io.Error> {
    io.println(some(8).and_then(half).and_then(half))?
    io.println(some(6).and_then(half).and_then(half))?
    io.println(some(3).filter(fn(x) => x > 2).map(fn(x) => x * 10))?
    io.println(option.flatten(some(some(7))))?
    let parsed: Result<Int, String> = "42".parse_int().ok_or("not a number")
    io.println(parsed.map(fn(n) => n + 1))?
    io.println(parsed.and_then(fn(n) => if n > 100 { ok(n) } else { err("too small") }))?
    ok(())
}
```

```text
some(2)
none
some(30)
some(7)
ok(43)
err(too small)
```

For lists of results (`try_map`, `all_ok`, `validate`,
`partition_results`), see [chapter 4](04-collections.md#fallible-processing).

---

Next: [Collections](04-collections.md)
