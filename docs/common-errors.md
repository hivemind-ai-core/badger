# Common errors

The diagnostics you are most likely to meet in your first hour, grouped by
what you were probably trying to do rather than by code number. Every code
has a fixed meaning and a fixed explanation: `badger explain E0301` prints
it, and `badger explain diagnostic CODE` does the same for any code in the
catalogue.

The groups are:

- [The name is not in scope](#the-name-is-not-in-scope)
- [The value is not the type I expected](#the-value-is-not-the-type-i-expected)
- [The call does not match](#the-call-does-not-match)
- [The pattern does not fit the value](#the-pattern-does-not-fit-the-value)
- [The value does not live long enough](#the-value-does-not-live-long-enough)
- [The effect row is missing an effect](#the-effect-row-is-missing-an-effect)
- [The error does not fit the `?`](#the-error-does-not-fit-the-)

## The name is not in scope

| Code | Meaning |
|---|---|
| E0127 | unknown name |
| E0122 | unknown type |
| E0114 | unknown module |
| E0112 | private item |

The compiler is looking for a name it cannot find. Either the name is
spelled wrongly, the module that holds it was never imported, the type is
in another module, or the item is not `pub` and you are outside its
defining module. Project modules live under the entry directory, so
`src/a/b.bdg` is the module `a.b`; the standard library modules are
`std.io`, `std.text`, `std.list`, and so on. The fix is to import the
module, or to mark the item `pub` in the module that defines it.

```badger fragment
import std.io

pub fn main() -> Result<Unit, io.Error> {
    io.println(greet("Ada"))?   // E0127: no `greet` in this scope
    ok(())
}
```

```badger
import std.io

pub fn greet(name: String) -> String {
    $"Hello, {name}"
}

pub fn main() -> Result<Unit, io.Error> {
    io.println(greet("Ada"))?
    ok(())
}
```

## The value is not the type I expected

| Code | Meaning |
|---|---|
| E0300 | type mismatch |
| E0301 | missing field |
| E0316 | missing struct field |
| E0338 | literal out of range |
| E0341 | missing implementation |

The value you have does not fit the type the context wants. The common
causes are a record that lacks a field the code reads, a struct literal
that leaves a field out, and a literal that does not fit the type it is
given. Records are structural and never drop fields implicitly, so a
closed record type `{ x: Int }` rejects `{ x: 1, y: 2 }` as well as
rejecting a record that has no `x`. Write the type as an open row
(`{ x: Int, ..Rest }`) to accept extra fields, use `records.project` to
build a closed subset, or give the struct literal all of its fields.

`E0341` is the same idea for a protocol: the type does not implement the
one the context needs. A route handler's error type, for instance, must
implement `HttpStatus` — the protocol that says which status an error
is answered with.

```badger fragment
pub fn main() -> Result<Unit, io.Error> {
    let p = { name: "Ada" }
    io.println(p.age)?      // E0301: the record has no field `age`
    let q: Point = { x: 1 } // E0316: `Point` also has a field `y`
    let n: U8 = 300         // E0338: `300` does not fit in `U8`
    ok(())
}
```

```badger
import std.io

pub struct Point {
    pub x: Int,
    pub y: Int,
}

fn label<Rest: Row>(item: { name: String, ..Rest }) -> String {
    $"<{item.name}>"
}

pub fn main() -> Result<Unit, io.Error> {
    let p: Point = { x: 1, y: 2 }
    io.println($"{p.x} {p.y}")?
    io.println(label({ name: "Ada", age: 36 }))?
    ok(())
}
```

## The call does not match

| Code | Meaning |
|---|---|
| E0309 | argument count |
| E0305 | function arity |
| E0312 | unknown method |

The function you called exists, but not with the arguments or the receiver
you gave it. Badger calls are positional and have no default or named
arguments; when a call needs several values, pass a record. E0305 is the
same mistake seen from the other side: you assigned a function to a
variable whose type says a different number of parameters. E0312 means the
receiver's type has no such method; methods are resolved by type, so check
the name in the [API reference](api/README.md).

```badger fragment
pub fn main() -> Result<Unit, io.Error> {
    io.println(area(3, 4, 5))?     // E0309: `area` takes 2 arguments
    io.println(5.to_upper())?       // E0312: no method `to_upper` on `Int`
    ok(())
}
```

```badger
import std.io

pub fn area(width: Int, height: Int) -> Int {
    width * height
}

pub fn main() -> Result<Unit, io.Error> {
    io.println(area(3, 4))?
    io.println("hello".to_upper())?
    ok(())
}
```

## The pattern does not fit the value

| Code | Meaning |
|---|---|
| E0342 | non-exhaustive match |
| E0318 | refutable binding |
| E0322 | unknown variant |

The pattern cannot cover the value, or the value cannot hold the pattern.
A `match` must list every variant; guarded arms do not count towards
coverage, so add a plain arm for the rest. A `let` or a parameter needs
an irrefutable pattern: use `match`, `if let`, or
`let PATTERN = VALUE else { ... }` for a pattern that might not match.
E0322 means the enum has no variant with that name; check the spelling in
the definition of the enum.

```badger fragment
pub fn main() -> Result<Unit, io.Error> {
    let o = some("ada")
    let .some(name) = o            // E0318: `some` may not hold
    match o {
        .some(v) => io.println(v)?, // E0342: `.none` is not covered
    }
    ok(())
}
```

```badger
import std.io

enum Colour {
    red,
    green,
    blue,
}

fn describe(c: Colour) -> String {
    match c {
        .red => "red",
        .green => "green",
        .blue => "blue",
    }
}

pub fn main() -> Result<Unit, io.Error> {
    io.println(describe(Colour.green))?
    let found = some("ada")
    if let .some(name) = found {
        io.println(name)?
    }
    ok(())
}
```

## The value does not live long enough

| Code | Meaning |
|---|---|
| E0404 | use after move |
| E0412 | implicit borrow of an owned value |
| E0413 | linear value not consumed |
| E0414 | move-only value copied |
| E0401 | region escape |
| E0414 | region value outlives its region |

Some values have one owner. Passing a builder, a buffer, a lease, or a
`once` closure by value moves it, and the old binding is dead: E0404.
Ordinary function arguments borrow explicitly, so a function that takes
`&T` or `&mut T` needs `&x` or `&mut x` when `x` is an owned affine
value: E0412. A value tied to a scope (a transaction, a task handle, a
running system) cannot outlive the callback that introduced the scope:
E0401, so return plain data from such a callback instead of the scoped
value itself. A record of a `region` block's region (`Node<Scratch>`)
cannot leave the block either, by its result, a `return`, or a store
outside it: E0414; build it in a region the caller gives (`R: Scope`)
instead. A resource (an open file, a batch lease) must also be
consumed: a file that is opened and dropped without `close` (or written
to and dropped) is E0413, on any path that leaves its scope other than
an error exit (`?`). A value of a struct or enum marked `@move_only` moves
the same way, and using it again after the move would copy it: E0414.
Borrow it (`&x`, a `self: &Self` method) to read it and keep it.

```badger fragment
pub fn main() -> Result<Unit, io.Error> {
    let buf = buffer.new<Int>()
    let array = buf.freeze()
    buf.push(1)        // E0404: `buf` was moved by `freeze`
    io.println(size(buf))?  // E0412: pass `&buf`
    ok(())
}
```

```badger
import std.buffer
import std.io

fn size(b: &Buffer<Int>) -> Int {
    b.len()
}

pub fn main() -> Result<Unit, io.Error> {
    let buf = buffer.new<Int>()
    buf.push(1)
    io.println(size(&buf))?
    let array = buf.freeze()
    io.println(array.len())?
    ok(())
}
```

## The effect row is missing an effect

| Code | Meaning |
|---|---|
| E0501 | effect bound violation |
| E0137 | effectful module value |
| E0138 | mutable module value |

The function does more than its effect row allows. A bound written
`!{...}` is an upper bound: doing less is fine, doing more is E0501. The
effects are `Io`, `Wait`, `State`, and `Foreign`; `badger explain effect
NAME` prints the row a definition was given. Module-level values are
pure: one that reads the environment is E0137, and one that holds a cell,
a buffer, a channel, or another resource is E0138. Compute such values
inside a function and pass them along.

```badger fragment
fn double(n: Int) -> Int !{} {
    let _ = io.println(n)  // E0501: effect `Io` is not permitted
    n * 2
}
```

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
    let counter = Cell.new(0)
    counter.set(counter.get() + 1)
    io.println(counter.get())?
    io.println(report(21))?
    ok(())
}
```

## The error does not fit the `?`

| Code | Meaning |
|---|---|
| E0333 | error conversion |
| E0313 | `?` on non-Result |
| E0314 | `?` outside a Result function |
| E0325 | ambiguous error type |
| E0337 | unused Result |

`?` takes the value out of `ok` and returns the `err` from the enclosing
function, so the function must return a `Result` (E0314) and the value
must be a `Result` (E0313). The error type must match the function's
error type or convert into it by one direct `From` implementation
(E0333); [the type system page](type-system.md) explains how `?` resolves
`From`. A private function that infers its error type from `?` needs one
source error type: several different ones need an annotation or an
explicit map (E0325). A `Result` you never look at is E0337; handle it
with `?` or `match`, or discard it on purpose with `let _ = ...`.

```badger fragment
pub fn main() -> Result<Unit, io.Error> {
    io.println("hi")   // E0337: this `Result` is ignored
    ok(())
}
```

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

## What to do next

`badger explain CODE` prints the catalogue entry for any code, including
the ones that are not on this page. The [language guide](guide/README.md)
covers the rules behind these diagnostics: rows and records in
[chapter 2](guide/02-types-and-records.md), `?` and error conversion in
[chapter 3](guide/03-errors.md), effects and ownership in
[chapter 7](guide/07-effects-and-ownership.md).
