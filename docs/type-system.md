# The type system

How Badger decides the type of a value: structural records are rows, every
function has an effect row, and conversions between error types are
resolved through `From`. This page explains the rules; the
[language guide](guide/README.md) has the full tour, and
[the specification](../spec/02-types-and-records.md) states them formally.

## Records are rows

A record type is a row: a set of labelled fields plus, optionally, a
remainder written `..Rest`. A record literal has a closed type — the
fields it literally has — and a function that takes a record infers an
open one, so the fields it does not use still travel with the value.

### A literal has a closed type

The type of a record literal is its exact shape. Nothing is added to it
later.

```badger
import std.io

pub fn main() -> Result<Unit, io.Error> {
    let person = { name: "Ada", age: 36 }
    io.println(person.name)?
    io.println(person.age)?
    ok(())
}
```

### A function's parameter is inferred from what it reads

The fields a function reads constrain its parameter, and everything else
passes through. `greet` below reads `name`, so it accepts any record that
has one; `badger explain type greet` prints the inferred signature.

```badger
import std.io

fn greet(person) {
    $"Hello, {person.name}"
}

fn birthday(person) {
    { ..person, age: person.age + 1 }
}

pub fn main() -> Result<Unit, io.Error> {
    let ada = { name: "Ada", age: 36, team: "core" }
    io.println(greet(ada))?
    let older = birthday(ada)
    io.println($"{older.age} {older.team}")?
    ok(())
}
```

```text
greet: fn({name: A, ..B}) -> String
birthday: fn({age: A, ..B}) -> {age: C, ..B}
```

`birthday` keeps the whole remaining row, so `older` still has `team`.

### An annotation closes the type

`{ x: Int }` names exactly that record. Passing `{ x: 1, y: 2 }` to it is
E0301 (`unexpected field`), because records never drop fields implicitly.
Reading a field the record does not have is the other E0301 (`missing
field`).

```badger fragment
fn closed(item: { name: String }) -> String { item.name }

closed({ name: "Ada", age: 36 })   // E0301: unexpected field `age`
```

### An open type names its remainder

Write `..Rest` to say that other fields are allowed, and bound the row
variable with `Rest: Row`.

```badger
import std.io

fn label<Rest: Row>(item: { name: String, ..Rest }) -> String {
    $"<{item.name}>"
}

pub fn main() -> Result<Unit, io.Error> {
    let ada = { name: "Ada", age: 36, team: "core" }
    io.println(label(ada))?
    ok(())
}
```

`Rest` is a compile-time row, not a runtime bag of fields. A call keeps
the full concrete record; nothing is projected away or erased. Rows hold
unique labels, so an open row `{ x: T, ..Rest }` implies that `Rest` does
not contain `x` (E0303).

### `records.project` builds a closed subset

When you want a closed owned value from a wider record, project it
explicitly. The field must exist (E0301 otherwise).

```badger
import std.io
import std.records

type Contact = { name: String, email: String }

pub fn main() -> Result<Unit, io.Error> {
    let person = { name: "Ada", age: 36, email: "ada@example.test" }
    let contact = records.project<Contact>(person)
    io.println(contact.email)?
    ok(())
}
```

## Effect rows

Every function has an effect row: the set of effects its body may
perform. The effects are `Io` (filesystem, network, processes,
environment, clocks), `Wait` (parking, sleeps, joins, timers), `State`
(mutable state observable outside the function), and `Foreign` (unsafe
foreign calls).

### Effects are inferred

Leave the row out and the compiler works it out. `badger explain effect
NAME` prints the result.

```badger
import std.io
import std.task

fn double(n: Int) -> Int { n * 2 }

fn greet(name: String) -> Result<Unit, io.Error> { io.println($"hello {name}") }

fn nap() -> Unit { task.sleep(1ms) }

pub fn main() -> Result<Unit, io.Error> {
    greet("x")?
    ok(())
}
```

```text
double: !{}
greet: !{Io}
nap: !{Wait}
```

### A written bound is an upper bound

`!{...}` after the result type says what the body may do at most. Doing
less is fine; doing more is E0501. The bound is what callers see, so
`badger explain effect` prints the annotation, not the inference.

```badger
import std.io

fn double(n: Int) -> Int !{} {
    n * 2
}

fn report(n: Int) -> Int !{Io, Wait} {
    let _ = io.println($"{double(n)}")
    n
}

pub fn main() -> Result<Unit, io.Error> {
    io.println(report(21))?
    ok(())
}
```

`!{}` means no modelled effects. It does not promise that the function
terminates or never panics.

### A row can name a variable

`!Eff` stands for whatever effects a callback has, and a row can extend
one: `!{Io, Eff}` is `Io` plus the callback's effects.

```badger
import std.io

fn twice<Eff>(f: fn(Int) -> Int !Eff, x: Int) -> Int !Eff {
    f(f(x))
}

fn logged<Eff>(f: fn(Int) -> Int !Eff, x: Int) -> Int !{Io, Eff} {
    let y = f(x)
    let _ = io.println($"{x} -> {y}")
    y
}

pub fn main() -> Result<Unit, io.Error> {
    let _ = logged(fn(n) { n * 3 }, twice(fn(n) { n + 1 }, 1))
    ok(())
}
```

### A parameter without `!` is effect-polymorphic

A function-typed parameter written without an effect row is charged to
the caller, so a callback that prints makes the caller's function
effectful. A function type *stored* in a field, payload, alias, or
result is different: nothing determines its effects there, so calling it
counts as any effect, and the call is E0501 under a tight bound. Annotate
it.

```badger
import std.io

struct Hooks {
    on_start: fn() -> Unit !{},
}

fn start(h: Hooks) -> Unit !{} {
    (h.on_start)()
}

pub fn main() -> Unit {
    start({ on_start: fn() {} })
}
```

### A type can carry a row

A type parameter declared `Eff: Effect` holds an effect row, written
`Source<!{}>`, `Source<!{Io}>`, or `Source<Eff>`. The standard `Seq<T,
Eff>` and `Stream<T, Eff>` work the same way.

```badger
import std.io

struct Source<Eff: Effect> {
    next: fn() -> Int !Eff,
}

fn take_two<Eff>(s: Source<Eff>) -> Int !Eff {
    s.next() + s.next()
}

fn constant() -> Source<!{}> {
    { next: fn() { 21 } }
}

pub fn main() -> Result<Unit, io.Error> {
    io.println(take_two(constant()))?
    ok(())
}
```

## `derive(From)` and a manual `From`

`From` is the prelude protocol for conversions: `fn from(value: T) -> Self`.
There are two ways to write one.

### `derive(From)` writes conversions into an enum

`derive(From)` is for enums only (E0202). It generates one conversion for
each variant that has exactly one payload, from that payload's type into
the enum. Two variants with the same payload type make the choice
ambiguous, so no conversion is derived for that type and the compiler
warns (E0203); the other variants still convert.

```badger
import std.io

@derive(Error, From)
pub enum AppError {
    parse(text: String),
    range(value: NumericError),
}

pub fn main() -> Result<Unit, io.Error> {
    io.println(AppError.from("boom"))?
    io.println(AppError.from(NumericError.out_of_range))?
    ok(())
}
```

The conversions point one way: from the payload type into the enum. Use
`derive(From)` when your error enum is a wrapper around other error
types, which is the usual shape for an application error.

### A manual `From` writes one conversion yourself

`impl From<X> for Y` writes the conversion by hand. It works for any
nominal type, and it is the way to build a value that the source does not
already contain.

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

pub fn main() -> Result<Unit, io.Error> {
    io.println(ConfigError.from(NumericError.out_of_range))?
    ok(())
}
```

The conversion is called on the target: `ConfigError.from(x)`. A manual
`From` also has to be an implementation the package is allowed to write:
it must belong to the package that defines the protocol or the type
(E0211), and records and other structural types are not implementation
identities (E0218), so wrap them in a `struct` first.

## How `?` resolves `From`

`expr?` takes the value out of `ok` and returns the `err` from the
enclosing function. The error has to fit the function's error type, and
`From` is how it gets there.

### The same type, or one direct conversion

`?` on a `Result<T, E1>` inside a function returning `Result<R, E2>`
accepts `E1` equal to `E2`, or one direct `From<E1>` implementation for
`E2`. Nothing else: no chains, no erasing to `Data`, no invented union.

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

fn parse_port(s: String) -> Result<Int, AppError> {
    let n = parse_int(s)?
    ok(n)
}

pub fn main() -> Result<Unit, AppError> {
    io.println(parse_port("8080"))?
    io.println(parse_port(""))?
    ok(())
}
```

In `parse_port`, the `?` converts `ParseError` into `AppError.parse`. In
`main`, the `?` on `io.println` converts `io.Error` into `AppError.io`.

### One step only

A conversion is one direct `From` implementation. If `E1` converts to
`E2` and `E2` converts to `E3`, then `?` will not carry an `E1` into an
`E3`; it reports E0333. Convert the error yourself instead.

```badger fragment
let n = parse_int(s)?   // E0333: `?` cannot convert `ParseError` into `AppError`
```

### `?` needs a `Result` and a `Result` function

`?` does not work on `Option` (E0313): give the missing case an error
first. And the enclosing function must return a `Result` (E0314), because
that is where the error goes.

```badger
import std.io

fn first_or(items: List<String>, fallback: String) -> Result<String, String> {
    let first = items.first().ok_or(fallback)?
    ok(first)
}

pub fn main() -> Result<Unit, io.Error> {
    io.println(first_or(["a", "b"], "none"))?
    io.println(first_or([], "none"))?
    ok(())
}
```

### An inferred error type needs one source

A private function with no declared result type infers its error type
from a single source. When several different error types flow into one
`?` target, the compiler will not guess (E0325): annotate the result
type, or map the errors explicitly with `map_err`.

```badger fragment
fn c() {
    let x = a()?      // E0325: errors of types String, NumericError flow here
    let y = b()?
    ok(x + y)
}
```

## What to read next

- [Chapter 2](guide/02-types-and-records.md) of the guide: records, rows,
  structs, enums, protocols, and `derive`.
- [Chapter 3](guide/03-errors.md): `Option`, `Result`, `?`, and the
  `Error` protocol.
- [Chapter 7](guide/07-effects-and-ownership.md): effect rows, `Transfer`
  and `Share`, affine values, and scoped regions.
- [Common errors](common-errors.md): what to do when a diagnostic comes
  out.
