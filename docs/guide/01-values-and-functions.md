# 1. Values and functions

The building blocks: primitive values and their literals, bindings,
operators, functions, lambdas, pipes, and control flow.

## A complete program

```badger
import std.io

pub fn main() -> Result<Unit, io.Error> {
    io.println("hello")?
    ok(())
}
```

```text
$ badger run hello.bdg
hello
```

- `main` is the entry point. It returns `Unit` or a `Result`.
- `io.println` returns `Result<Unit, io.Error>`; `?` passes an error up
  (chapter 3).
- `ok(())` is the successful result carrying the unit value `()`.

`badger check FILE` type-checks without running. `badger explain CODE`
prints the explanation of any diagnostic code mentioned below.

## Integers

| Type | Range |
|---|---|
| `I8`, `I16`, `I32`, `I64` | signed, 8 to 64 bits |
| `U8`, `U16`, `U32`, `U64` | unsigned, 8 to 64 bits |
| `Int` | alias of `I64` |
| `Byte` | alias of `U8` |
| `Size` | alias of `U64` |

```badger
import std.io

pub fn main() -> Result<Unit, io.Error> {
    let count = 42
    let mask = 0xFF
    let flags = 0b1010
    let big = 1_000_000
    let small = 200U8
    let port: U16 = 8080
    io.println($"{count} {mask} {flags} {big} {small} {port}")?
    ok(())
}
```

```text
42 255 10 1000000 200 8080
```

- Literals are decimal, `0x` hex, or `0b` binary, with optional `_`
  separators.
- A suffix fixes the type: `12U8`, `-1I32`.
- An unsuffixed literal takes the expected type (`port: U16`); otherwise it
  is `Int`.
- A literal that does not fit its type is rejected (E0338).

### No implicit conversion

Mixing integer types is a type error (E0300). Convert explicitly:

| Conversion | Result | Example |
|---|---|---|
| `Target.from(x)` | `Target`; only where every value fits | `Int.from(x_i32)` |
| `Target.try_from(x)` | `Result<Target, NumericError>` | `U8.try_from(300)` is an error |

```badger
import std.io

pub fn main() -> Result<Unit, io.Error> {
    let small: I32 = 40000
    let wide = Int.from(small) * 100000
    io.println(wide)?
    io.println(U8.try_from(300))?
    io.println(U8.try_from(255))?
    ok(())
}
```

```text
4000000000
err(value out of range for the target type)
ok(255)
```

### Arithmetic is checked

Overflow, division by zero, and invalid shifts stop the task with a
defect in every build. A defect is a bug, not an expected error
(chapter 3).

```badger
import std.io

pub fn main() -> Result<Unit, io.Error> {
    let level: U8 = 250
    io.println(level + 10)?
    ok(())
}
```

```text
$ badger run overflow.bdg
panic: integer overflow at overflow.bdg:5:16
$ echo $?
101
```

When overflow is an expected outcome, use the explicit helpers:

| Method | Returns |
|---|---|
| `checked_add`, `checked_sub`, `checked_mul`, `checked_div`, `checked_rem` | `Result<T, NumericError>` |
| `wrapping_add`, `wrapping_sub`, `wrapping_mul` | `T`, modulo 2^bits |
| `abs` (signed types), `min`, `max` | `T` |

```badger
import std.io

pub fn main() -> Result<Unit, io.Error> {
    let max: I32 = 2147483647
    io.println(max.checked_add(1))?
    io.println(max.wrapping_add(1))?
    io.println(10.checked_div(0))?
    io.println((-5).abs())?
    io.println(-7 / 2)?
    io.println(-7 % 2)?
    ok(())
}
```

```text
err(arithmetic overflow)
-2147483648
err(division by zero)
5
-3
-1
```

Integer division truncates toward zero. The remainder has the sign of the
dividend.

## Floats

`F32` and `F64` (`Float` is `F64`) follow IEEE 754. Literals: `0.5`,
`1e3`, `0.5F32`.

```badger
import std.io

pub fn main() -> Result<Unit, io.Error> {
    io.println(7.0 / 2.0)?
    io.println(0.1 + 0.2)?
    io.println(1.0 / 0.0)?
    let nan = 0.0 / 0.0
    io.println(nan == nan)?
    io.println(Float.from(3I32))?
    io.println(Int.try_from(3.0))?
    io.println(Int.try_from(2.7))?
    ok(())
}
```

```text
3.5
0.30000000000000004
inf
false
3.0
ok(3)
err(value out of range for the target type)
```

- Float division by zero gives `inf` or `nan`; it is not a defect.
- Floats have IEEE equality and ordering only. They are not `Eq` or
  `Hash`, so they cannot be map keys or set members (E0341).
- `Int.try_from(f)` succeeds only for a float with an exact integer value.
- `1 + 2.0` is a type error (E0300): write `1.0 + 2.0`.

## Bool and Char

`Bool` is `true` or `false`. Conditions must be `Bool`; there is no
truthiness (`if n { ... }` with an integer is rejected).

`Char` is one Unicode scalar value: `'A'`, `'\n'`, `'\u{1F600}'`.
Characters compare by code point.

## Strings

`String` is immutable UTF-8.

```badger
import std.io

pub fn main() -> Result<Unit, io.Error> {
    let name = "Ada"
    let jobs = 3
    io.println($"{name} has {jobs * 2} jobs")?
    io.println($"braces: {{ and }}")?
    io.println("tab:\tquote:\" snowman:\u{2603}")?
    io.println("""raw: \n is not an escape here""")?
    io.println(name.len())?
    io.println(name.to_upper())?
    io.println("héllo".slice(1, 3))?
    ok(())
}
```

```text
Ada has 6 jobs
braces: { and }
tab:	quote:" snowman:☃
raw: \n is not an escape here
3
ADA
some(él)
```

- `$"...{expr}..."` interpolates. Each value needs `Display` (E0331).
  `{{` and `}}` are literal braces.
- Escapes: `\n \r \t \0 \\ \" \'` and `\u{...}`.
- `"""..."""` is raw multiline text without escapes; `$"""..."""`
  interpolates.
- `len`, `slice`, and `index_of` count characters, not bytes.
  `byte_len` counts bytes.
- Strings cannot be indexed (`s[0]` is E0336). Use `s.char_at(i)` or
  `s.slice(start, end)`, which return `Option`.

The text operations are methods on `String` and functions in `std.text`;
see the [reference](../api/text.md).

## Symbols

A symbol is an interned name: `#get`, `#database`, or qualified
`#app/database`, written without a space after the `#`. Symbols compare by
name and are distinct from strings. Printed on their own they show just the
name; inside a list or record they keep the `#` (`[#a, #b]`).

```badger
import std.io

pub fn main() -> Result<Unit, io.Error> {
    let method = #post
    io.println(method == #post)?
    io.println(#app/database)?
    ok(())
}
```

```text
true
app/database
```

## Unit and tuples

`()` is the only value of `Unit`. A tuple groups values by position:
`(1, "two")`. Read fields with `.0`, `.1`, or destructure with
`let (a, b) = pair`. A one-element tuple is written `(item,)`.

## Durations, instants, timestamps

| Type | What it is | Get one |
|---|---|---|
| `Duration` | a non-negative length of time | literals `10ns`, `250us`, `5ms`, `2s`, `3min`, `1h`; `time.millis(n)` |
| `Instant` | a point on the monotonic clock | `time.now()` |
| `Timestamp` | wall-clock time (nanoseconds since the Unix epoch) | `time.timestamp()`, `datetime.parse_timestamp(text)` |

```badger
import std.datetime
import std.io
import std.time

pub fn main() -> Result<Unit, io.Error> {
    let timeout = 1500ms
    io.println(timeout)?
    io.println(2min + 30s)?
    io.println(timeout * 2 > 2s)?
    io.println(time.as_millis(timeout))?

    let start = time.now()
    let deadline = start + 5s
    io.println(time.now() - start < 1s)?
    io.println(deadline > start)?

    match datetime.parse_timestamp("2026-10-17T09:30:00+01:00") {
        .ok(ts) => io.println(datetime.rfc3339(ts))?,
        .err(e) => io.println(e)?,
    }
    ok(())
}
```

```text
1.500s
150s
true
1500
true
true
2026-10-17T08:30:00Z
```

- Durations add, subtract, multiply and divide by integers, and compare.
- Durations are never negative: `-5s` is rejected (E0327), and `1s - 2s`
  is a defect. Keep signed time differences as `Int` nanoseconds
  (`time.as_nanos`).
- `Instant - Instant` is a `Duration`; `Instant + Duration` is an
  `Instant`. Instants have no `Display`.
- Timestamps compare but have no arithmetic and no `Display`; calendar
  work (dates, zones, formatting, spans) is in
  [`std.datetime`](../api/datetime.md).
- `time.now()` and `time.timestamp()` carry the `Io` effect, and
  `std.test` can control the clock (chapter 9).

## Bindings

```badger
import std.io

const limit = 4 * 1024

pub fn main() -> Result<Unit, io.Error> {
    let name = "Ada"
    let name = $"{name} Lovelace"
    var count = 0
    count = count + 1
    io.println($"{name} {count} {limit}")?
    ok(())
}
```

```text
Ada Lovelace 1 4096
```

| Form | Meaning |
|---|---|
| `let x = e` | immutable binding; a later `let x` shadows it |
| `var x = e` | the name can be reassigned (`x = e2`); the type stays the same |
| `const x = e` | evaluated when the program is checked |

- Assigning to a `let` binding is E0130.
- `var` makes the *binding* reassignable, not the value mutable. Records,
  strings, and lists stay immutable; "changing" one makes a new value.
- `const` accepts literals, constructors, aggregates (records, tuples,
  lists, maps, sets), operators, and other constants. A function call is
  E0136; overflow in a constant is E0139.
- At module level only `let` and `const` are allowed (E0110). Module values
  are pure data: a cell, buffer, channel, or other resource there is E0138.
  Create those inside functions.
- Assignment is a statement, not an expression: `let y = (x = 3)` is a
  syntax error (E0001). Targets are a `var`, a field of one (`p.x = 2`), or
  an index of one (`buf[i] = v`, which calls the collection's `set`
  method; frozen lists have none).
- `x += e` is `x = x + e`, and likewise `-=`, `*=`, `/=`, and `%=`, for
  the same targets. An index in the target is read and written, so it
  must be a name or a literal (E0002).

## Operators

Strongest first:

| Operators | Notes |
|---|---|
| calls `f(x)`, fields `.x`, index `[i]`, `?` | |
| unary `-`, `!`, `&`, `&mut` | `&` borrows (chapter 7) |
| `*` `/` `%` | |
| `+` `-` | numbers and durations; `"a" + "b"` is E0339 (use interpolation or `text.concat`) |
| `<<` `>>` | integers |
| `&`, then `^`, then `\|` | bitwise, integers |
| `..` `..=` | ranges |
| `==` `!=` `<` `<=` `>` `>=` | do not chain |
| `&&` | short-circuits |
| `\|\|` | short-circuits |
| `\|>` | pipe |
| `=` | assignment (a statement) |

- `a < b < c` is rejected (E0003). Write `a < b && b < c`.
- `==` is structural for records, tuples, lists, maps, sets, and enums.
  Functions and cells have no equality (E0330).
- `<` orders numbers, characters, strings (by code point), durations,
  instants, timestamps, and, structurally, tuples, records, lists, and
  `Option`/`Result` values. See [ordering](../collections.md#ordering-and-equality).
- Operands and arguments evaluate left to right.

## Ranges

`start..end` excludes `end`; `start..=end` includes it. Ranges are lazy
and work for every integer type. A range is a `Seq<Int>` where one is
expected, so it has the sequence methods (chapter 4).

```badger
import std.io

pub fn main() -> Result<Unit, io.Error> {
    var total = 0
    for i in 1..=4 {
        total = total + i
    }
    io.println(total)?
    io.println((0..10).filter(fn(n) => n % 3 == 0).collect())?
    ok(())
}
```

```text
10
[0, 3, 6, 9]
```

## Functions

```badger
import std.io

fn double(value) {
    value * 2
}

pub fn area(width: Int, height: Int) -> Int {
    width * height
}

fn sign(n: Int) -> String {
    if n < 0 {
        return "negative"
    }
    "non-negative"
}

pub fn main() -> Result<Unit, io.Error> {
    io.println(double(21))?
    io.println(area(3, 4))?
    io.println(sign(-2))?
    ok(())
}
```

```text
42
12
negative
```

- The last expression of the body is the result. `return` exits early.
- A body of one expression can follow `=>`: `fn double(n: Int) -> Int =>
  n * 2` (methods too).
- `_` where an expression goes is a hole: `badger check` reports it
  (E0345) with the type expected there, and checks the rest.
- Private functions may omit types; they are inferred from the body and
  the calls.
- `pub` functions must declare every parameter and the result type
  (E0118).
- Calls are positional; there are no default or named arguments. Pass an
  options record (`{ retries: 3, verbose: true }`) when several arguments
  need names.
- A wrong argument count is E0309.
- A function that calls itself in tail position runs as a loop. Other tail
  calls are ordinary calls.

A signature can also state the function's effects, such as `!{}` (no
modeled effects) or `!{Io, Wait}`. Omitted effects are inferred. Effects
are covered in [chapter 7](07-effects-and-ownership.md).

## Generics

```badger
import std.io

fn identity<T>(value: T) -> T {
    value
}

fn largest<T>(a: T, b: T) -> T where T: PartialOrd {
    if a < b { b } else { a }
}

pub fn main() -> Result<Unit, io.Error> {
    io.println(identity("same"))?
    io.println(identity<Int>(42))?
    io.println(largest(3, 9))?
    io.println(largest("pear", "apple"))?
    ok(())
}
```

```text
same
42
9
pear
```

- Type parameters are uppercase and go in `<...>`.
- Type arguments are inferred; write them explicitly with `f<Int>(...)`.
- Bounds (`where T: PartialOrd`) are covered in
  [chapter 2](02-types-and-records.md#generics-and-bounds).

## Lambdas

```badger
import std.io

fn twice(f: fn(Int) -> Int, x: Int) -> Int {
    f(f(x))
}

pub fn main() -> Result<Unit, io.Error> {
    let increment = fn(n) { n + 1 }
    let square = fn(n) => n * n
    let add = fn(a: Int, b: Int) -> Int => a + b
    let adder = fn(n: Int) => fn(k: Int) => k + n
    io.println(twice(increment, 5))?
    io.println(twice(square, 3))?
    io.println(add(2, 3))?
    io.println(adder(10)(1))?
    io.println([1, 2, 3].map(fn(x) => x * 10))?
    ok(())
}
```

```text
7
81
5
11
[10, 20, 30]
```

- `fn(x) => e` is the same as `fn(x) { e }`. Use `=>` for one expression
  and a block for statements.
- Function types are written `fn(A, B) -> R`.
- `return` and `?` inside a lambda leave the lambda, not the enclosing
  function.

### Captures are values

A lambda captures the *value* a variable had when the lambda was created.
It cannot assign to a captured variable (E0131).

```badger
import std.io

pub fn main() -> Result<Unit, io.Error> {
    var n = 1
    let show = fn() => n * 10
    n = 5
    io.println(show())?
    ok(())
}
```

```text
10
```

For state that a lambda updates, use a `Cell` (one task) or an `Atom`
(shared between tasks):

```badger
import std.io

pub fn main() -> Result<Unit, io.Error> {
    let calls = Cell.new(0)
    let tick = fn() { calls.set(calls.get() + 1) }
    tick()
    tick()
    io.println(calls.get())?
    ok(())
}
```

```text
2
```

### `once fn`

A `once fn` may be called at most once, so it may consume what it
captures. A second call is E0404 (use after move).

```badger fragment
let take = once fn() => label
let a = take()
let b = take()   // E0404: use of moved value `take`
```

## Local functions

A `fn` declared inside a body is a helper for that block: it may be
generic, call itself, and call the block's other local functions (they
are visible in the whole block, also before their declaration).

```badger fragment
fn parse_all(lines: List<String>) -> Result<List<Int>, AppError> {
    fn lift<X>(r: Result<X, String>) -> Result<X, AppError> => r.map_err(AppError.parse)
    fn digits(n: Int) -> Int => if n < 10 { 1 } else { 1 + digits(n / 10) }
    ...
}
```

Unlike a closure, a local function does not capture the enclosing
function's variables (an unknown name there says so): pass what it needs,
or use a closure. It is private to its block, and it is reported as
`parse_all.lift`.

## Pipes

`x |> f(a, b)` means `f(x, a, b)`: the left side becomes the first
argument. `x |> f` is `f(x)`.

```badger
import std.io
import std.list
import std.text

fn double(n: Int) -> Int { n * 2 }

pub fn main() -> Result<Unit, io.Error> {
    io.println(5 |> double |> double)?
    io.println("  shout  " |> text.trim() |> text.to_upper())?
    let names = ["cy", "ada", "bob"] |> list.sort() |> list.map(fn(s) => text.to_upper(s))
    io.println(names)?
    ok(())
}
```

```text
20
SHOUT
["ADA", "BOB", "CY"]
```

- The left side is evaluated once, before the stage runs.
- `|>` binds more loosely than arithmetic and comparisons: `a + 1 |> f` is
  `f(a + 1)`.
- There are no placeholders and no last-argument pipe.
- A newline before `|>` continues the expression.

### `?` in a pipe

A stage written `f(args)?` applies `?` to that stage's result:
`x |> f()?` is `(x |> f())?`. Fallible stages chain.

```badger
import std.io

fn parse(s: String) -> Result<Int, String> {
    match s.parse_int() {
        .some(n) => ok(n),
        .none => err($"not a number: {s}"),
    }
}

fn positive(n: Int) -> Result<Int, String> {
    if n > 0 { ok(n) } else { err($"not positive: {n}") }
}

fn read(s: String) -> Result<Int, String> {
    let n = s |> parse()? |> positive()?
    ok(n * 2)
}

pub fn main() -> Result<Unit, io.Error> {
    io.println(read("21"))?
    io.println(read("-4"))?
    io.println(read("x"))?
    ok(())
}
```

```text
ok(42)
err(not positive: -4)
err(not a number: x)
```

`(make())?` in parentheses is not a call stage: it computes the function
the value is passed to.

## Continuation binding: `use`

`use x <- f(args)` passes the rest of the block to `f` as its final
argument, a callback taking `x`. It is plain syntax for callbacks; scoped
resources (transactions, task scopes, systems) are its main users.

```badger
import std.io

fn with_label<T>(label: String, body: fn(String) -> T) -> T {
    body($"[{label}]")
}

pub fn main() -> Result<Unit, io.Error> {
    use tag <- with_label("job")
    io.println($"{tag} started")?
    ok(())
}
```

```text
[job] started
```

This is the same as `with_label("job", fn(tag) { ... })`. `return` and `?`
after `use` belong to the callback. To end the callback early, put the
`use` in its own block. The right side must be a call, not a `?`
expression (E0129).

## Blocks are expressions

A block `{ ... }` evaluates its statements and has the value of its last
expression. A trailing `;` discards it, and an empty block is `()`.

```badger
import std.io

pub fn main() -> Result<Unit, io.Error> {
    let total = {
        let base = 10
        let bonus = 5
        base + bonus
    }
    io.println(total)?
    ok(())
}
```

```text
15
```

A brace that starts with `label:`, `..spread`, or a lone name followed by
`,` or `}` is a record, not a block (chapter 2).

## `if`

`if` is an expression. With `else`, both branches must have the same
type; without `else`, it is `Unit`.

```badger
import std.io

pub fn main() -> Result<Unit, io.Error> {
    let n = 7
    let size = if n > 10 { "large" } else if n > 5 { "medium" } else { "small" }
    io.println(size)?
    if n % 2 == 1 {
        io.println("odd")?
    }
    ok(())
}
```

```text
medium
odd
```

## `match`

`match` tries arms in order. It must cover every value (E0342). A guard
(`if ...`) makes an arm conditional; guarded arms do not count toward
coverage.

```badger
import std.io

fn describe(n: Int) -> String {
    match n {
        0 => "zero",
        1 => "one",
        _ if n < 0 => "negative",
        _ => "many",
    }
}

fn locate(point: (Int, Int)) -> String {
    match point {
        (0, 0) => "origin",
        (x, 0) => $"on the x axis at {x}",
        (_, y) => $"off the axis at height {y}",
    }
}

pub fn main() -> Result<Unit, io.Error> {
    io.println(describe(-3))?
    io.println(describe(1))?
    io.println(locate((4, 0)))?
    io.println(locate((1, 2)))?
    ok(())
}
```

```text
negative
one
on the x axis at 4
off the axis at height 2
```

Every pattern form is listed in [chapter 2](02-types-and-records.md#patterns).

## Loops

```badger
import std.io

pub fn main() -> Result<Unit, io.Error> {
    for (i, word) in ["a", "b", "c"].enumerate() {
        if i == 1 {
            continue
        }
        io.println($"{i}: {word}")?
    }

    var n = 0
    while n < 5 {
        n = n + 2
    }
    io.println(n)?

    let first_square = loop {
        n = n + 1
        if n * n > 50 {
            break n
        }
    }
    io.println(first_square)?
    ok(())
}
```

```text
0: a
2: c
6
8
```

| Loop | Value |
|---|---|
| `for PATTERN in EXPR { ... }` | `Unit` |
| `while COND { ... }` | `Unit` |
| `loop { ... }` | the value of `break value` (all breaks agree on a type) |

- `for` iterates lists, arrays, sets, maps (as `(key, value)` tuples),
  ranges, sequences, and streams. `String` and `Bytes` are not iterable
  (E0334): use `s.chars()`, `s.lines()`, or `b.to_list()`.
- `break` and `continue` act on the nearest loop. There are no loop labels.
- `break` and `continue` outside a loop are E0125.

## `if let` and `while let`

A pattern condition tests one pattern. Names bind only inside the
matching block.

```badger
import std.io

fn greet(name: Option<String>) -> String {
    if let .some(n) = name {
        $"hello {n}"
    } else {
        "hello stranger"
    }
}

pub fn main() -> Result<Unit, io.Error> {
    io.println(greet(Option.some("Ada")))?
    io.println(greet(Option.none))?

    var stack = [1, 2, 3]
    while let .some(top) = stack.last() {
        io.println(top)?
        stack = stack.slice(0, stack.len() - 1)
    }
    ok(())
}
```

```text
hello Ada
hello stranger
3
2
1
```

- `if let P = e { ... } else { ... }` is a `match` with that arm and `_`.
  `else if let` chains.
- `while let P = e { ... }` evaluates `e` before each iteration and stops
  when it does not match.
- An `if let .ok(x) = r` that ignores `.err` drops the error silently.
  Match the `Result` when the error matters.
- `let P = e else { ... }` keeps the names `P` binds for the rest of the
  block; when `e` does not match, the block runs and must leave
  (`return`, `break`, `continue`, or a panic):

```badger fragment
let .some(user) = users.get(id) else {
    return err(Error.unknown_user(id))
}
greet(user)
```

---

Next: [Types and records](02-types-and-records.md)
