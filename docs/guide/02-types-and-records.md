# 2. Types and records

How to model data: structural records, aliases, structs, enums, patterns,
methods, protocols, `derive`, and generic bounds.

## Records

A record is an immutable value with named fields. Its type is its shape;
no declaration is needed.

```badger
import std.io

pub fn main() -> Result<Unit, io.Error> {
    let person = { name: "Ada", age: 36, address: { city: "Dublin" } }
    io.println(person.name)?
    io.println(person.address.city)?
    let name = "Bob"
    let age = 25
    let other = { name, age }
    io.println(other.age)?
    io.println({ x: 1, y: 2 } == { y: 2, x: 1 })?
    ok(())
}
```

```text
Ada
Dublin
25
true
```

- `{ name, age }` is shorthand for `{ name: name, age: age }`.
- `{}` is the empty record.
- Labels are lowercase (an uppercase label is E0102).
- Equality is structural, and field order does not matter.
- Reading a field the record does not have is a type error (E0301), never
  a missing value.

## Updating records

`{ ..base, field: value }` makes a new record from `base` with the listed
fields replaced or added. The original is unchanged.

```badger
import std.io

pub fn main() -> Result<Unit, io.Error> {
    let person = { name: "Ada", age: 36, address: { city: "Dublin", country: "Ireland" } }
    let older = { ..person, age: person.age + 1 }
    let with_email = { ..person, email: "ada@example.test" }
    let moved = { ..person, address.city: "Cork" }
    io.println($"{person.age} {older.age}")?
    io.println(with_email.email)?
    io.println($"{moved.address.city}, {moved.address.country}")?
    ok(())
}
```

```text
36 37
ada@example.test
Cork, Ireland
```

| Rule | Diagnostic |
|---|---|
| One spread, and it comes first | E0104 |
| A label appears once | E0103 |
| A dotted path (`address.city`) needs a base record that has `address` | E0105 |

An update may also change a field's type: `{ ..r, id: "a1" }` turns an
`Int` id into a `String`.

## Functions over records

A function that reads fields accepts any record that has them. The rest of
the record passes through.

```badger
import std.io

fn greet(person) {
    $"Hello, {person.name}"
}

fn birthday(person) {
    { ..person, age: person.age + 1 }
}

pub fn label<Rest: Row>(item: { name: String, ..Rest }) -> String {
    $"<{item.name}>"
}

pub fn main() -> Result<Unit, io.Error> {
    let ada = { name: "Ada", age: 36, team: "core" }
    io.println(greet(ada))?
    io.println(greet({ name: "a cat" }))?
    let older = birthday(ada)
    io.println($"{older.age} {older.team}")?
    io.println(label(ada))?
    ok(())
}
```

```text
Hello, Ada
Hello, a cat
37 core
<Ada>
```

- `greet` and `birthday` infer an *open* record type: "has `name`" and
  "has a numeric `age`", plus any other fields.
- `{ name: String, ..Rest }` writes an open type explicitly. `Rest: Row`
  is a type parameter that stands for the other fields.
- A type without `..Rest`, such as `{ x: Int }`, is *closed*: passing
  `{ x: 1, y: 2 }` to it is E0301. Records never lose fields implicitly.
- `if` branches returning different shapes are E0301. Use an enum or
  `Option` fields instead.

## Destructuring

```badger
import std.io

pub fn main() -> Result<Unit, io.Error> {
    let person = { name: "Ada", age: 36, team: "core" }
    let { name, age, .. } = person
    let { team: group, .. } = person
    io.println($"{name} {age} {group}")?
    ok(())
}
```

```text
Ada 36 core
```

`..` ignores the other fields. Without it, the pattern must name every
field (E0301 for a record, E0319 for a struct).

## Record operations

`std.records` has typed operations on whole records:

| Operation | Result |
|---|---|
| `records.project<T>(r)` | the fields of `T` taken from a record that has them all (E0301 if one is missing) |
| `records.merge(a, b)` | both records' fields; `b` wins on overlap |
| `records.remove(r, #label)` | the record without that field |

Qualified labels let libraries add fields without name clashes:
`{ ..request, [#auth/user]: user }`, read back with
`request[#auth/user]`. The label must be a literal symbol: indexing a
record with a runtime `Symbol` is E0335 (use a `Map` for runtime keys).

```badger
import std.io
import std.records

type Contact = { name: String, email: String }

pub fn main() -> Result<Unit, io.Error> {
    let person = { name: "Ada", age: 36, email: "ada@example.test" }
    let contact = records.project<Contact>(person)
    let tagged = { ..contact, [#import/source]: "contacts.csv" }
    io.println(tagged[#import/source])?
    io.println(records.remove(tagged, #import/source) == contact)?
    let settings = records.merge({ retries: 2, verbose: false }, { retries: 4 })
    io.println(settings.retries)?
    ok(())
}
```

```text
contacts.csv
true
4
```

## Type aliases

`type` names a type. An alias is the same type as what it names; it adds
no identity.

```badger
import std.io

type Person = { name: String, age: Int }
type Pair<T> = (T, T)

fn swap<T>(p: Pair<T>) -> Pair<T> {
    let (a, b) = p
    (b, a)
}

pub fn main() -> Result<Unit, io.Error> {
    let ada: Person = { name: "Ada", age: 36 }
    let plain = { name: "Ada", age: 36 }
    io.println(ada == plain)?
    let swapped = swap((1, 2))
    io.println($"{swapped.0} {swapped.1}")?
    ok(())
}
```

```text
true
2 1
```

An alias that contains itself has no finite size (E0213).

## Structs

A `struct` is a nominal type: a record shape with its own identity.

```badger
import std.io

pub struct Point {
    pub x: Int,
    pub y: Int,
}

pub struct UserId {
    value: U64,
}

pub fn user_id(value: U64) -> UserId {
    { value }
}

pub fn main() -> Result<Unit, io.Error> {
    let p: Point = { x: 1, y: 2 }
    let fields = { y: 5, x: 4 }
    let q: Point = fields
    let moved = { ..p, x: 10 }
    io.println($"{p.x} {q.x} {moved.x}")?
    let id = user_id(42U64)
    io.println(id == user_id(42U64))?
    ok(())
}
```

```text
1 4 10
true
```

- Construct a struct with a record literal where a struct is expected
  (`let p: Point = { ... }`), or let a record of exactly the same shape
  convert (`let q: Point = fields`).
- A struct update `{ ..p, x: 10 }` makes a new struct.
- Fields are private unless marked `pub`. Outside the defining module, a
  private field cannot be read (E0308), and a struct with any private field
  cannot be constructed or updated (E0315). Its module provides
  constructor functions, such as `user_id` above, that establish the
  invariants. Modules are covered in [chapter 5](05-modules-and-projects.md).
- A constructor is an ordinary lowercase function. There is no special
  constructor syntax.
- Missing fields are E0316.

Structs can be generic: `struct Stack<T> { items: List<T> }`.

## Enums

An enum is a closed set of variants. Each variant may carry a payload.

```badger
import std.io

enum Command {
    greet(name: String),
    retry(attempt: Int),
    move(Int, Int),
    shutdown,
}

fn describe(c: Command) -> String {
    match c {
        .greet(name) => $"hello {name}",
        .retry(n) if n < 3 => $"retry {n}",
        .retry(_) => "give up",
        .move(dx, dy) => $"move by {dx},{dy}",
        .shutdown => "stop",
    }
}

pub fn main() -> Result<Unit, io.Error> {
    let commands = [
        Command.greet("Ada"),
        Command.retry(1),
        Command.retry(5),
        Command.move(2, -1),
        Command.shutdown,
    ]
    for c in commands {
        io.println(describe(c))?
    }
    ok(())
}
```

```text
hello Ada
retry 1
give up
move by 2,-1
stop
```

- Variants are lowercase. Payload fields may be named (`greet(name:
  String)`) or positional (`move(Int, Int)`).
- Construct with the type name: `Command.retry(1)`, `Command.shutdown`.
  A bare `.shutdown` is only a pattern, not an expression.
- `Option` and `Result` are enums from the prelude; `some`, `none`, `ok`,
  and `err` are helper functions for them (chapter 3).
- Enums can be generic, as `Option<T>` and `Result<T, E>` are.

### Recursive enums

A variant cannot contain its own enum directly (E0213). Put it behind a
`Box` or inside a collection.

```badger
import std.io

enum Expr {
    num(Int),
    add(left: Box<Expr>, right: Box<Expr>),
    neg(Box<Expr>),
}

fn eval(e: Expr) -> Int {
    match e {
        .num(n) => n,
        .add(l, r) => eval(l.get()) + eval(r.get()),
        .neg(inner) => -eval(inner.get()),
    }
}

pub fn main() -> Result<Unit, io.Error> {
    let e = Expr.add(Box.new(Expr.num(2)), Box.new(Expr.neg(Box.new(Expr.num(5)))))
    io.println(eval(e))?
    ok(())
}
```

```text
-3
```

## Patterns

Patterns appear in `match` arms, `let`, function parameters, `for`,
`if let`, `while let`, and `use`.

| Pattern | Matches |
|---|---|
| `x` | anything; binds it to `x` |
| `_` | anything; binds nothing |
| `42`, `-1`, `"text"`, `'c'`, `#sym`, `true`, `0s` | that literal |
| `()` | the unit value |
| `(a, b)` | a tuple, element by element |
| `{ x, y: 0 }` | a record with exactly these fields; `x` binds the field, `y` must match `0` |
| `{ x, .. }` | a record with at least these fields |
| `.variant(p1, p2)` | an enum variant; the enum comes from the value being matched |
| `Type.variant(p1)` | the same, naming the enum |
| `.variant` | a variant without payload |
| `p \| q` | either (at the top of a `match` arm); both bind the same names |

Patterns nest. A guard `PATTERN if COND =>` adds a condition (to every
alternative of an or-pattern).

```badger
import std.io

enum Shape {
    circle(radius: Float),
    rect(w: Float, h: Float),
    dot,
}

type Event = { kind: Symbol, shape: Shape, retries: Int }

fn describe(e: Event) -> String {
    match e {
        { kind: #draw, shape: .dot, .. } => "draw a dot",
        { kind: #draw, shape: .circle(r), .. } if r > 10.0 => "draw a big circle",
        { kind: #draw, shape: Shape.circle(r), .. } => $"draw a circle of {r}",
        { kind: #draw, shape: .rect(w, h), retries: 0 } => $"draw {w}x{h} once",
        { kind: #draw, shape: .rect(_, _), retries } => $"draw a rectangle, {retries} retries",
        { kind, .. } => $"ignore {kind}",
    }
}

fn nested(r: Result<Option<Int>, String>) -> String {
    match r {
        .ok(.some(-1)) => "minus one",
        .ok(.some(n)) => $"value {n}",
        .ok(.none) => "nothing",
        .err(message) => $"failed: {message}",
    }
}

pub fn main() -> Result<Unit, io.Error> {
    io.println(describe({ kind: #draw, shape: Shape.circle(12.0), retries: 0 }))?
    io.println(describe({ kind: #draw, shape: Shape.rect(2.0, 3.0), retries: 2 }))?
    io.println(describe({ kind: #erase, shape: Shape.dot, retries: 0 }))?
    io.println(nested(ok(Option.some(-1))))?
    io.println(nested(err("timeout")))?
    ok(())
}
```

```text
draw a big circle
draw a rectangle, 2 retries
ignore erase
minus one
failed: timeout
```

Common mistakes:

| Mistake | Diagnostic |
|---|---|
| A `match` that misses a case (guarded arms do not count) | E0342 |
| A refutable pattern in `let` or a parameter (`let .some(x) = o`) | E0318: use `match`, `if let`, or `let ... else` |
| A variant the enum does not have | E0322 |
| The wrong number of payload patterns | E0323 |
| A struct pattern that omits fields without `..` | E0319 |

## Methods

An `impl` block adds methods to a struct or enum. A method takes the value
as `self: &Self`; a function without `self` is called on the type.

```badger
import std.io

pub struct Stack<T> {
    items: List<T>,
}

impl<T> Stack<T> {
    pub fn empty() -> Stack<T> { { items: [] } }
    pub fn push(self: &Self, item: T) -> Stack<T> { { items: self.items.append(item) } }
    pub fn peek(self: &Self) -> Option<T> { self.items.last() }
    pub fn size(self: &Self) -> Int { self.items.len() }
}

pub enum Shape {
    circle(radius: Float),
    square(side: Float),
}

impl Shape {
    pub fn area(self: &Self) -> Float {
        match self {
            .circle(r) => 3.0 * r * r,
            .square(s) => s * s,
        }
    }
}

pub fn main() -> Result<Unit, io.Error> {
    let s = Stack.empty().push(1).push(2)
    io.println($"{s.size()} {s.peek()}")?
    io.println(Shape.square(3.0).area())?
    ok(())
}
```

```text
2 some(2)
9.0
```

- `value.method(args)` calls a method. The receiver is borrowed as the
  signature says.
- `Type.function(args)` calls a function of the type (`Stack.empty()`,
  `Box.new(x)`).
- A method is private to its module unless marked `pub` (E0112 from
  another module). Methods of a protocol implementation are as visible as
  the protocol.
- Methods are resolved by type. There are no extension functions: other
  modules use plain functions and pipes.
- A record field that holds a function is called like a method but does
  not receive the record: `store.save(order)` calls the field `save` with
  `order`.

## Protocols

A protocol is a set of methods a type can implement.

```badger
import std.io

pub protocol Shape {
    fn area(self: &Self) -> Float
    fn name(self: &Self) -> String
    fn summary(self: &Self) -> String { $"{self.name()} with area {self.area()}" }
}

pub struct Circle {
    pub r: Float,
}

pub struct Rect {
    pub w: Float,
    pub h: Float,
}

impl Shape for Circle {
    fn area(self: &Self) -> Float { 3.0 * self.r * self.r }
    fn name(self: &Self) -> String { "circle" }
}

impl Shape for Rect {
    fn area(self: &Self) -> Float { self.w * self.h }
    fn name(self: &Self) -> String { "rect" }
    fn summary(self: &Self) -> String { $"a {self.w} by {self.h} rect" }
}

fn report<T>(shape: &T) -> String where T: Shape {
    shape.summary()
}

pub fn main() -> Result<Unit, io.Error> {
    let c: Circle = { r: 1.0 }
    let r: Rect = { w: 2.0, h: 3.0 }
    io.println(report(&c))?
    io.println(report(&r))?
    let shapes: List<dyn Shape> = [c, r]
    for s in shapes {
        io.println(s.area())?
    }
    ok(())
}
```

```text
circle with area 3.0
a 2.0 by 3.0 rect
3.0
6.0
```

- A method with a body in the protocol is a default; an `impl` may
  override it. An `impl` must provide every method without a default
  (E0219).
- `where T: Shape` makes a generic function accept any type implementing
  `Shape`.
- `dyn Shape` is an explicit dynamic value, for example a list of
  different types behind one protocol. A protocol with generic methods
  cannot be used as `dyn` (E0216).

### Other protocol forms

| Form | Example |
|---|---|
| Supertrait: implementors must also implement `Display` | `protocol Greet: Display { ... }` |
| Parameterized protocol, implemented several times | `protocol Conv<T> { fn conv(self: &Self) -> T }`, then `impl Conv<Int> for N` and `impl Conv<String> for N` |
| Associated type | `protocol Container { type Item ... }`; `impl` writes `type Item = String`; use as `C.Item` |
| Blanket implementation | `impl<T> Loud for T where T: Display { ... }` |

### Implementation rules

| Rule | Diagnostic |
|---|---|
| An `impl` belongs to the package that defines the protocol or the type (no `impl Display for Int` in your code) | E0211 |
| One implementation per protocol and type; a `derive` and an `impl` of the same protocol overlap | E0212 |
| Records and aliases of records cannot implement protocols; wrap them in a struct | E0218 |

### Protocols in the prelude

| Protocol | Purpose |
|---|---|
| `Display` | `display(self) -> String`; used by interpolation and `io.println` |
| `Error` | expected failures (chapter 3) |
| `From<T>`, `TryFrom<T>` | conversions: `Target.from(x)`, `Target.try_from(x)` |
| `PartialEq`, `Eq` | `==`; `Eq` excludes floats |
| `PartialOrd` | `<` and sorting |
| `Hash` | map keys, set members |
| `Codec` | encoding through a schema (JSON, `Data`) |

`Share`, `Transfer`, and `Unscoped` are compiler-checked capabilities, not
protocols you implement (E0217). See [chapter 7](07-effects-and-ownership.md).

## `@derive`

`@derive(...)` on a struct or enum asks the compiler to implement
protocols.

```badger
import std.io

@derive(Display, Eq, Hash)
pub struct Point {
    pub x: Int,
    pub y: Int,
}

@derive(Display)
pub enum Shape {
    circle(radius: Float),
    rect(w: Float, h: Float),
    dot,
}

pub fn main() -> Result<Unit, io.Error> {
    let p: Point = { x: 1, y: 2 }
    io.println(p)?
    io.println(Shape.circle(1.5))?
    io.println(Shape.rect(1.0, 2.0))?
    io.println(Shape.dot)?
    ok(())
}
```

```text
Point { x: 1, y: 2 }
circle: 1.5
rect: 1.0, 2.0
dot
```

| Derivation | Gives |
|---|---|
| `Display` | the text shown above |
| `Eq`, `Hash`, `Codec` | a checked statement that every field supports it |
| `Error` | `Display` plus the `Error` protocol (chapter 3); a variant's `@message("no book {id}")` is its display, `{field}` interpolating a field |
| `From` | (enums) a conversion from each variant's single payload type (chapter 3) |
| `Default` | (structs) `S.default()`: every field's default (`0`, `0.0`, `false`, `""`, `0s`, an empty collection, `none`, or the field type's own `Default`) |
| `HttpError` | (enums) `HttpStatus` from each variant's `@status(code)`, 500 for the others (chapter 10) |

- Equality, hashing, and `Codec` encoding are structural and available
  without `derive` whenever every field supports them. Deriving them
  documents the intent and makes the compiler check it: `@derive(Eq)` on a
  struct with a `Float` or function field is E0204.
- `Default` pairs with a record update for settings:
  `{ ..Settings.default(), port: 8080 }`. `default_of()` is the default
  of whatever type is expected there.
- Only these eight can be derived (E0204). There are no user-defined
  derivations or macros.

### Wrapper structs in JSON: `@flatten`

A one-field struct gives a value its own type (`BookId` is not any
`Int`). In JSON it nests like any struct, `{"id":{"value":7}}`, unless
it is marked `@flatten`: then it encodes, decodes, reads from
configuration, and appears in schemas as its field's value.

```badger fragment
@flatten
pub struct BookId { value: Int } derive(Codec)

pub struct Book { id: BookId, title: String } derive(Codec)
// {"id":7,"title":"Dune"}
```

`@flatten` on anything other than a one-field struct is E0192.

### Options and sets in JSON

An `Option` field is its value or `null`, and may be left out (it
decodes as `none`; so may a `@flatten` struct over an `Option`). When the
value can be `null` itself -- `Option<Option<T>>`, `Option<Unit>` --
`some(v)` is written `[v]`, so a patch can say "clear it" (`[null]`)
apart from "leave it" (`null`). A `Set` is an array, in sorted order.

```badger fragment
@derive(Codec)
pub struct Patch { title: Option<Option<String>> }
// none: {"title":null}   some(none): {"title":[null]}   some(some("x")): {"title":["x"]}
```

## Generics and bounds

Structs, enums, aliases, functions, and `impl` blocks take type
parameters. A bound requires a protocol.

```badger
import std.io

fn tag<T: Display + PartialOrd>(a: T, b: T) -> String {
    if a < b { $"{a} first" } else { $"{b} first" }
}

fn show_all<T>(items: List<T>, sep: String) -> String where T: Display {
    var out = ""
    for item in items {
        out = if out == "" { $"{item}" } else { $"{out}{sep}{item}" }
    }
    out
}

pub fn main() -> Result<Unit, io.Error> {
    io.println(tag(3, 1))?
    io.println(tag("b", "a"))?
    io.println(show_all([1, 2, 3], ", "))?
    ok(())
}
```

```text
1 first
a first
1, 2, 3
```

- Inline (`<T: A + B>`) and `where T: A + B` mean the same.
- `where` can bound an associated type: `where C: Container, C.Item: Display`.
- `Rest: Row` is the bound for record-row parameters (see
  [functions over records](#functions-over-records)).
- Type parameters are uppercase identifiers. Higher-kinded parameters
  (`F<Int>` with `F` a parameter) are not supported (E0121).
- Generic functions are specialized for each type they are used with.

---

Next: [Errors](03-errors.md)
