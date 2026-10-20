# Learn Badger in 30 minutes

Badger is a statically typed language for services and automation. The
`badger` tool checks a program, compiles it to native code, and runs it.
This page walks through the language from values to tasks in one pass
(about 4,500 words, half of them code; roughly 30 minutes).

Every `badger` program below is complete: the docs test checks each one,
and each output shown is what `badger run` prints. Copy any of them into a
`.bdg` file and run it.

Contents: [running](#run-a-program) · [values](#values-and-literals) ·
[bindings](#bindings-and-control-flow) · [functions](#functions-lambdas-and-pipes) ·
[types](#records-structs-and-enums) · [errors](#option-result-and-) ·
[collections](#collections-and-sequences) · [modules](#modules-and-projects) ·
[effects](#effects) · [`use`](#use-the-rest-of-the-block-as-a-callback) ·
[tasks](#tasks) · [channels](#channels) · [HTTP](#http-in-a-few-lines) ·
[tests](#tests-and-contracts) · [next](#where-to-go-next)

## Run a program

```badger
// hello.bdg
import std.io

// `main` is where the program starts. It returns a Result, so a failed
// write to the console is reported instead of ignored.
pub fn main() -> Result<Unit, io.Error> {
    io.println("Hello, Badger!")
}
```

```sh
badger run hello.bdg     # compile and run
badger check hello.bdg   # type-check only; prints nothing when all is well
```

Output:

```text
Hello, Badger!
```

The commands you will use most:

| Command | What it does |
|---|---|
| `badger run FILE` | Compile and run; arguments after `--` go to the program |
| `badger check FILE` | Parse and check without running |
| `badger test FILE [FILTER]` | Run the `test_*` functions |
| `badger repl` | Interactive session: definitions accumulate, expressions print |
| `badger format FILE` | Rewrite a file in the canonical layout |
| `badger explain E0337` | Explain a diagnostic code |
| `badger init NAME` | Start a project: `badger.toml` and `src/main.bdg` with a test |

Without a file, `run`, `check`, and `test` use the project in the current
directory (see [modules and projects](#modules-and-projects)).

## Values and literals

```badger
import std.io

pub fn main() -> Result<Unit, io.Error> {
    // ---- Numbers ----
    let count = 42          // Int: a 64-bit signed integer
    let big = 1_000_000     // `_` separates digits
    let mask = 0xFF         // hexadecimal; `0b1010` is binary
    let small = 12U8        // a suffix picks the type: U8, I32, U64, F32, ...
    let ratio = 0.5         // Float: a 64-bit float
    io.println($"{count} {big} {mask} {small} {ratio}")?

    // Integer division truncates toward zero.
    io.println($"{7 / 2} {7 % 2} {7.0 / 2.0}")?

    // No silent conversions. Widen with `T.from`; narrow with `T.try_from`,
    // which returns a Result.
    io.println(Int.from(small) + count)?
    io.println(U8.try_from(300))?

    // ---- Text ----
    let name = "Ada"        // String: immutable UTF-8
    let initial = 'A'       // Char: one Unicode scalar value
    // `$"..."` interpolates any expression inside `{}`; `{{` and `}}` are
    // literal braces. There is no `+` for strings.
    io.println($"{name} has {name.len()} letters and starts with {initial}")?
    io.println($"{{braces}} and {count + 1}")?

    // Triple quotes keep raw multi-line text exactly as written (a newline
    // right after the opening quotes would be kept too). No escapes.
    let poem = """roses are "red"
  \n stays as two characters
"""
    io.print(poem)?

    // ---- Symbols ----
    // Interned names for tags and keys, distinct from strings. They can be
    // namespaced with `/`. Printed on their own they show just the name;
    // inside a list or record they keep the `#` (`[#a, #b]`).
    let key = #database
    let namespaced = #app/database
    io.println($"{key} {namespaced} {key == #database}")?

    // ---- Durations ----
    // Literal units: ns, us, ms, s, min, h.
    let timeout = 5s
    let short = 250ms
    io.println($"{timeout} {short} {timeout + short} {2min > 90s}")?

    // ---- Bool, Unit, tuples ----
    let ready = true && !false
    let pair = (count, name)
    let (n, s) = pair       // destructure a tuple
    io.println($"{ready} {pair.0} {n} {s} {()}")?
    ok(())
}
```

Output:

```text
42 1000000 255 12 0.5
3 1 3.5
54
err(value out of range for the target type)
Ada has 3 letters and starts with A
{braces} and 43
roses are "red"
  \n stays as two characters
database app/database true
5s 250.000ms 5.250s true
true 42 42 Ada ()
```

Integer arithmetic is checked: overflow and division by zero stop the
task and report the source line. They never wrap silently.

## Bindings and control flow

```badger
import std.io

// Module-level constants: literals and simple constant expressions only.
const max_retries = 3

pub fn main() -> Result<Unit, io.Error> {
    // `let` binds an immutable name.
    let width = 10
    // Shadowing is allowed: this is a new binding, not a mutation.
    let width = width * 2

    // `var` makes a binding assignable. The value itself stays immutable.
    var attempts = 0
    attempts = attempts + 1

    io.println($"{width} {attempts} {max_retries}")?

    // `if` is an expression. Conditions must be Bool: no truthiness.
    let size = if width > 15 { "large" } else { "small" }

    // Ranges: `a..b` excludes b, `a..=b` includes it.
    var total = 0
    for i in 1..=4 {
        total = total + i
    }

    // `while` loops while a condition holds.
    var n = 1
    while n < 100 {
        n = n * 3
    }

    // `loop` runs until `break`; `break value` makes it an expression.
    var tries = 0
    let found = loop {
        tries = tries + 1
        if tries * tries > 50 {
            break tries
        }
    }
    io.println($"{size} {total} {n} {found}")?

    // Closures capture values, so they cannot assign to a captured `var`.
    // For shared mutable state, use a Cell (one task) or an Atom (shared
    // between tasks).
    let clicks = Cell.new(0)
    let click = fn() { clicks.set(clicks.get() + 1) }
    click()
    click()
    io.println(clicks.get())?
    ok(())
}
```

Output:

```text
20 1 3
large 10 243 8
2
```

## Functions, lambdas, and pipes

```badger
import std.io
import std.list
import std.seq

// A function's last expression is its value. No `return` needed.
fn add(a: Int, b: Int) -> Int {
    a + b
}

// Private functions may leave out types; the compiler infers them.
fn double(x) {
    x * 2
}

// Public functions (`pub`) must spell out parameter and result types.
pub fn clamp(value: Int, low: Int, high: Int) -> Int {
    if value < low {
        return low      // early exit
    }
    if value > high { high } else { value }
}

// Generics use `<T>`. Function types are written `fn(A) -> B`.
fn apply_twice<T>(value: T, f: fn(T) -> T) -> T {
    f(f(value))
}

// Functions are values: this returns a closure over `prefix`.
fn labeler(prefix: String) -> fn(Int) -> String {
    fn(n) => $"{prefix}-{n}"
}

pub fn main() -> Result<Unit, io.Error> {
    io.println($"{add(2, 3)} {double(21)} {clamp(150, 0, 100)}")?

    // Lambdas: `fn(x) { block }`, or `fn(x) => expression` for one expression.
    let inc = fn(x) { x + 1 }
    let square = fn(x: Int) -> Int => x * x
    io.println($"{apply_twice(3, inc)} {apply_twice(3, square)}")?

    let job = labeler("job")
    io.println(job(7))?

    // Calls are positional. There are no named or default arguments; pass
    // a record when you need labels.

    // The pipe `x |> f(a)` means `f(x, a)`: the value goes in first.
    let result = 10 |> add(5) |> double()
    io.println(result)?

    // Pipes read well with the Seq functions (lazy sequences, more later).
    let evens = 1..=10
        |> seq.filter(fn(n) => n % 2 == 0)
        |> seq.map(fn(n) => n * n)
        |> list.collect()
    io.println(evens)?

    // Methods on built-in types use dot syntax.
    io.println([3, 1, 2].sort().map(fn(n) => n * 10))?
    ok(())
}
```

Output:

```text
5 42 100
5 81
job-7
30
[4, 16, 36, 64, 100]
[10, 20, 30]
```

## Records, structs, and enums

Three ways to define data:

| Kind | Identity | Use it for |
|---|---|---|
| Record `{ name: "Ada" }` | Structural: same fields, same type | Everyday data |
| `struct` | Nominal: its own type, private fields possible | Domain types with invariants |
| `enum` | Nominal, one of several variants | Choices, states, errors |

Names that start uppercase are types (`Int`, `Point`, `Shape`). Values,
fields, functions, and enum variants start lowercase.

```badger
import std.io

// ---- Records: structural, immutable, inferred ----

// A type alias names a record shape. It adds no new identity.
type Point = { x: Int, y: Int }

// This accepts ANY record with a `name` field; the rest of the record's
// fields are carried through unchanged.
fn greet(person) {
    $"Hello, {person.name}"
}

// `{ ..r, field: value }` copies `r` with a field replaced or added.
fn birthday(person) {
    { ..person, age: person.age + 1 }
}

// ---- Structs: nominal types ----

// A struct has its own identity. Fields are private unless marked `pub`.
pub struct UserId {
    pub value: Int,
}

// Methods live in `impl` blocks. `self: &Self` borrows the receiver.
impl UserId {
    pub fn next(self: &Self) -> UserId {
        { value: self.value + 1 }   // a record literal builds the struct
    }
}

// Protocols are interfaces. Implementing Display lets a value print and
// appear in `$"..."`.
impl Display for UserId {
    fn display(self: &Self) -> String {
        $"user-{self.value}"
    }
}

// ---- Enums: closed sets of variants, with optional payloads ----

// Variants are lowercase. `Shape.circle(1.0)` constructs one.
pub enum Shape {
    circle(radius: Float),
    rect(w: Float, h: Float),
    dot,
}

// `match` must cover every case. `.variant` patterns take the enum from
// the value being matched.
fn area(s: Shape) -> Float {
    match s {
        .circle(r) => 3.0 * r * r,
        .rect(w, h) => w * h,
        .dot => 0.0,
    }
}

// Patterns can be literals, bindings, `_`, tuples, records, and variants.
// `if` adds a guard.
fn describe(n: Int) -> String {
    match n {
        0 => "zero",
        n if n < 0 => "negative",
        n if n < 10 => "small",
        _ => "large",
    }
}

pub fn main() -> Result<Unit, io.Error> {
    let p: Point = { x: 1, y: 2 }
    let moved = { ..p, x: 10 }
    io.println($"({p.x}, {p.y}) -> ({moved.x}, {moved.y})")?

    // Records compare by value; field order does not matter.
    io.println(p == { y: 2, x: 1 })?

    let ada = { name: "Ada", age: 36, city: "London" }
    let older = birthday(ada)
    io.println($"{greet(older)}: {older.age}, still in {older.city}")?

    // Destructure a record; `..` ignores the other fields.
    let { name, age, .. } = older
    io.println($"{name} {age}")?

    // Nested updates use a dotted path.
    let office = { name: "HQ", address: { city: "Dublin", zip: "D01" } }
    let moved_office = { ..office, address.city: "Cork" }
    io.println($"{moved_office.address.city} {moved_office.address.zip}")?

    // A struct is built from a record literal where its type is expected.
    let id: UserId = { value: 41 }
    io.println(id.next())?

    io.println($"{area(Shape.rect(2.0, 3.0))} {area(Shape.dot)}")?
    io.println($"{describe(0)} {describe(-5)} {describe(7)} {describe(99)}")?

    // `if let` tests one pattern; the names bind only inside the block.
    for s in [Shape.circle(1.5), Shape.dot] {
        if let .circle(r) = s {
            io.println($"circle with radius {r}")?
        } else {
            io.println("not a circle")?
        }
    }
    ok(())
}
```

Output:

```text
(1, 2) -> (10, 2)
true
Hello, Ada: 37, still in London
Ada 37
Cork D01
user-42
6.0 0.0
zero negative small large
circle with radius 1.5
not a circle
```

Records do not print as a whole: print their fields, or implement
`Display` on a struct as above.

## Option, Result, and `?`

```badger
import std.io
import std.text

// There is no null and there are no exceptions. Two prelude enums cover
// absence and failure:
//
//     enum Option<T>    { none, some(value: T) }
//     enum Result<T, E> { ok(value: T), err(error: E) }
//
// `some`, `none`, `ok`, and `err` are available everywhere.

// An error type is an ordinary enum. `derive(Error)` makes it printable as
// an error; `derive(From)` lets `?` convert each wrapped error type into it.
@derive(Error, From)
pub enum AppError {
    bad_number(input: String),
    too_big(value: Int),
    io(source: io.Error),
}

fn parse(input: String) -> Result<Int, AppError> {
    // text.parse_int returns an Option; turn absence into an error.
    match text.parse_int(input) {
        .some(n) => ok(n),
        .none => err(AppError.bad_number(input)),
    }
}

fn check_size(n: Int) -> Result<Int, AppError> {
    if n > 100 {
        return err(AppError.too_big(n))
    }
    ok(n)
}

// `?` unwraps an `ok`, or returns the `err` from the current function.
fn parse_small(input: String) -> Result<Int, AppError> {
    let n = parse(input)?
    let checked = check_size(n)?
    ok(checked * 2)
}

// `?` also works on pipe stages: each stage's result is checked.
fn parse_small_piped(input: String) -> Result<Int, AppError> {
    let n = input |> parse()? |> check_size()?
    ok(n * 2)
}

// main returns a Result too. Here `?` after io.println converts io.Error
// into AppError.io through the derived From.
pub fn main() -> Result<Unit, AppError> {
    for input in ["21", "x", "500"] {
        match parse_small(input) {
            .ok(n) => io.println($"ok {n}")?,
            .err(e) => io.println($"error: {e}")?,
        }
    }
    io.println(parse_small_piped("7"))?

    // Helpers on Option and Result.
    io.println(parse_small("x").unwrap_or(-1))?
    io.println(text.parse_int("4").map(fn(n) => n * 2))?
    io.println(text.words("").first().is_none())?
    let missing: Result<String, String> = text.words("").first().ok_or("no words")
    io.println(missing)?

    // A Result cannot be ignored silently. `let _ =` discards it on purpose.
    let _ = check_size(1000)
    ok(())
}
```

Output:

```text
ok 42
error: bad number: x
error: too big: 500
ok(14)
-1
some(8)
true
err(no words)
```

When `main` returns an `err`, the program prints it on stderr and exits
with status 1:

```badger
// fail.bdg
pub fn main() -> Result<Unit, String> {
    err("something went wrong")
}
```

`badger run fail.bdg` prints:

```text
error: something went wrong
  at fail.bdg:3:5
```

A task *defect* is different from an error: a failed `assert`, `panic`,
an out-of-range `list[i]`, or an overflow. It stops the task (and its
scope) instead of returning a value. An uncaught defect in `main` exits
with status 101.

## Collections and sequences

| Type | What it is |
|---|---|
| `List<T>` | Immutable ordered list; `append` returns a new list |
| `Map<K, V>` | Immutable key-value map; `insert` returns a new map |
| `Set<T>` | Immutable set |
| `Seq<T>` | Lazy sequence: work happens when it is consumed |
| `ListBuilder<T>` | Builds a list in place; `finish` turns it into a `List` |

```badger
import std.io
import std.list
import std.map
import std.seq
import std.set

type Person = { name: String, age: Int, team: String }

pub fn main() -> Result<Unit, io.Error> {
    // ---- List<T>: immutable, ordered ----
    let people: List<Person> = [
        { name: "ada", age: 36, team: "core" },
        { name: "bob", age: 25, team: "web" },
        { name: "cy", age: 41, team: "core" },
    ]
    io.println($"{people.len()} {people[0].name}")?   // `[i]` stops the task if i is out of range
    io.println(people.get(9).map(fn(p) => p.name))?  // `get` returns an Option instead

    // "Changing" a list returns a new list; the old one is untouched.
    let more = people.append({ name: "dee", age: 25, team: "ops" })
    io.println($"{people.len()} {more.len()}")?

    // Eager list methods.
    io.println(more.map(fn(p) => p.age).fold(0, fn(sum, a) => sum + a))?
    io.println(more.sort_by(fn(p) => p.age).map(fn(p) => p.name))?
    io.println(more.filter(fn(p) => p.team == "core").len())?

    // To build a list item by item, use a builder. `finish` consumes it.
    let builder = list.builder<Int>()
    for i in 0..5 {
        builder.append(i * i)
    }
    io.println(builder.finish())?

    // ---- Map<K, V>: lookup by key ----
    let ages = map { "ada" => 36, "bob" => 25 }
    let ages2 = ages.insert("cy", 41)      // a new map; `ages` is unchanged
    io.println($"{ages2.get("cy")} {ages2.get_or("zed", 0)} {ages.contains("cy")}")?

    // Iteration order of maps and sets is unspecified: sort for stable output.
    for name in ages2.keys().sort() {
        io.println($"{name}: {ages2.get_or(name, 0)}")?
    }

    // ---- Set<T>: membership ----
    let tags = set { #urgent, #billing }
    io.println($"{tags.contains(#urgent)} {tags.insert(#urgent).len()}")?

    // ---- Seq<T>: lazy sequences ----
    // Nothing runs until the sequence is consumed (by collect, count,
    // fold, sum, first, a `for` loop, ...). Ranges are sequences too.
    let names = more.seq()
        |> seq.filter(fn(p) => p.age > 30)
        |> seq.map(fn(p) => p.name)
        |> list.collect()
    io.println(names)?
    io.println(seq.sum(1..=100))?

    // Only the first three squares are ever computed here.
    io.println(seq.range(1, 1_000_000).map(fn(n) => n * n).take(3).collect())?

    let by_team = people.group_by(fn(p) => p.team)
    io.println(by_team.get_or("core", []).map(fn(p) => p.name))?
    ok(())
}
```

Output:

```text
3 ada
none
3 4
127
["bob", "dee", "ada", "cy"]
2
[0, 1, 4, 9, 16]
some(41) 0 false
ada: 36
bob: 25
cy: 41
true 2
["ada", "cy"]
5050
[1, 4, 9]
["ada", "cy"]
```

Using a builder after `finish` is a compile error (`use of moved value`):
some values, like builders, can be used only once.

## Modules and projects

One file is one module. A project is a directory with a `badger.toml` and
sources under `src/`; `src/greet.bdg` is the module `greeter.greet`.

```text
greeter/
├── badger.toml
└── src/
    ├── main.bdg
    └── greet.bdg
```

`badger.toml`:

```toml
[package]
name = "greeter"
entry = "src/main.bdg"
```

`src/greet.bdg`:

```badger
// src/greet.bdg is the module `greeter.greet`.

/// `///` documents the next declaration.
pub fn greeting(name: String) -> String {
    $"Hello, {shout(name)}!"
}

// Not `pub`: private to this module.
fn shout(name: String) -> String {
    name.to_upper()
}
```

`src/main.bdg` (an excerpt: it only checks inside the project):

```badger fragment
import std.io                  // a standard module, used as `io.println`
import std.text.{join}         // import single items by name
import std.list as lists       // rename a module
import greeter.greet           // this project's src/greet.bdg

pub fn main() -> Result<Unit, io.Error> {
    io.println(greet.greeting("ada"))?
    io.println(join(["a", "b", "c"], "-"))?
    io.println(lists.empty<Int>().len())?
    ok(())
}
```

```sh
cd greeter
badger run       # runs the entry module
badger check     # checks the entry module and everything it imports
badger test      # runs test_* functions in the entry module and the project modules it imports
```

`badger run` prints:

```text
Hello, ADA!
a-b-c
0
```

Rules to remember:

- Everything is private unless marked `pub`.
- `import std.io` makes `io.println` available; `import std.text.{join}`
  imports one name; `import std.list as lists` renames.
- `pub` functions must declare parameter and result types; private ones
  may leave them to inference.
- Dependencies go in `[dependencies]`, for example
  `textutil = { path = "../textutil" }`.

## Effects

```badger
// effects.bdg
import std.io
import std.task

// Effects say what a function may do besides compute. They are inferred;
// writing `!{...}` after the result type states an upper bound that the
// compiler checks.
//
//   Io       files, network, processes, environment, console
//   Wait     sleeping, joining tasks, channel waits
//   State    mutable state visible outside the function
//   Foreign  unsafe foreign calls

// `!{}`: no modeled effects. Calling io.println in here is a compile error.
fn area(w: Int, h: Int) -> Int !{} {
    w * h
}

fn report(label: String, value: Int) -> Result<Unit, io.Error> !{Io} {
    io.println($"{label}: {value}")
}

fn pause() -> Unit !{Wait} {
    task.sleep(1ms)
}

// A callback's effects are charged to the caller, so `twice` itself stays
// generic over whatever `f` does.
fn twice(f: fn() -> Unit) {
    f()
    f()
}

// There is no `async`: any function may suspend (sleep, wait on I/O)
// and callers just call it.
pub fn main() -> Result<Unit, io.Error> {
    pause()
    twice(fn() { let _ = report("area", area(2, 3)) })
    ok(())
}
```

Output:

```text
area: 6
area: 6
```

Ask the compiler what it inferred:

```sh
badger explain effect twice effects.bdg
```

```text
twice: !{} plus the effects of its callbacks
```

Breaking a bound is a compile error. This program is rejected:

```badger fragment
// bad_area.bdg
import std.io

fn area(w: Int, h: Int) -> Int !{} {
    let _ = io.println("computing")
    w * h
}

pub fn main() -> Result<Unit, io.Error> {
    io.println(area(2, 3))
}
```

`badger check bad_area.bdg` reports two diagnostics; the first:

```text
[E0501] Error: effect `Io` is not permitted by this function's effect bound
   ╭─[ bad_area.bdg:5:13 ]
   │
 5 │     let _ = io.println("computing")
   │             ───────────┬───────────  
   │                        ╰───────────── this operation has effect `Io`
   │
   ├─[ bad_area.bdg:5:13 ]
   │
 4 │ fn area(w: Int, h: Int) -> Int !{} {
   │                                ─┬─  
   │                                 ╰─── declared bound `!{}`
   │ 
   │ Note: in fn bad_area.area
───╯
```

## `use`: the rest of the block as a callback

Tasks, channels, transactions, and other scoped resources are written
with `use`. It is plain syntax for a callback, nothing more:

```badger
import std.io

// A helper that takes a callback as its LAST argument...
fn with_label<T>(label: String, body: fn(String) -> T) -> T {
    body($"[{label}]")
}

pub fn main() -> Result<Unit, io.Error> {
    // ...can be called the ordinary way:
    let a = with_label("plain", fn(tag) => $"{tag} done")
    io.println(a)?

    // `use x <- helper(args)` passes the REST OF THE BLOCK as that callback.
    // This is the same call as above, written flat.
    use tag <- with_label("use")
    io.println($"{tag} done")
}
```

Output:

```text
[plain] done
[use] done
```

Inside a `use` body, `return` and `?` exit the callback. The helper's
result becomes the value of the enclosing block, so the helper can run
cleanup after the callback and report its own errors.

## Tasks

A task is a unit of concurrent work scheduled by the runtime; it is not
an OS thread. Code in a task runs sequentially; a call that waits (sleep,
I/O, a channel) suspends only that task. `badger run --workers N` spreads tasks over N threads.

```badger
import std.io
import std.task

@derive(Error, From)
pub enum AppError {
    failed(message: String),
    io(source: io.Error),
}

fn slow_square(n: Int) -> Int {
    task.sleep(20ms)
    n * n
}

fn fetch(n: Int) -> Result<Int, String> {
    if n < 0 { err($"negative input {n}") } else { ok(n + 100) }
}

pub fn main() -> Result<Unit, AppError> {
    // Every task belongs to a scope. The scope waits for its children
    // before it ends, so no task outlives it.
    use scope <- task.scope()

    // `spawn` starts a child task; `join` waits for its value. The two
    // children run concurrently.
    let a = scope.spawn(fn() => slow_square(3))
    let b = scope.spawn(fn() => slow_square(4))
    io.println(a.join() + b.join())?

    // `join` returns the child's value as is. For children that return
    // Results, `try_join` waits for both and, on the first error, cancels
    // the other child and returns that error.
    let x = scope.spawn(fn() => fetch(1))
    let y = scope.spawn(fn() => fetch(2))
    let (p, q) = scope.try_join(x, y)?
    io.println($"both: {p} {q}")?

    let bad = scope.spawn(fn() => fetch(-2))
    let good = scope.spawn(fn() => fetch(3))
    match scope.try_join(bad, good) {
        .ok(_) => io.println("unexpected")?,
        .err(e) => io.println($"failed: {e}")?,
    }

    // task.timeout runs a function as a task and cancels it after a limit.
    let slow = task.timeout(50ms, fn() {
        task.sleep(1s)
        "finished"
    })
    io.println(slow.is_err())?

    // The scope body's result decides what happens to unfinished
    // children: `ok` waits for them, `err` cancels them.
    ok(())
}
```

Output:

```text
25
both: 101 102
failed: negative input -2
true
```

## Channels

```badger
import std.channel
import std.io
import std.task

@derive(Error, From)
pub enum AppError {
    config(source: channel.ConfigError),
    send(source: channel.SendError<Int>),
    io(source: io.Error),
}

// A request that carries its own one-shot reply channel.
type Lookup = { key: Int, reply: channel.ReplySender<String> }

pub fn main() -> Result<Unit, AppError> {
    // A Channel<T> is a typed, bounded queue. `channel.open` creates one
    // and closes it when the block ends. Options are a record; update the
    // defaults with the fields you want to change.
    use jobs <- channel.open<Int>({ ..channel.defaults, capacity: 8 })
    use scope <- task.scope()

    // Three workers share one receiver. Each message goes to exactly one.
    let workers = [1, 2, 3].map(fn(_id) => scope.spawn(fn() {
        var done = 0
        // recv() waits for a message; it returns `none` once the channel
        // is closed and drained.
        while let .some(n) = jobs.receiver.recv() {
            done = done + n
        }
        done
    }))

    // send() waits while the channel is full (backpressure).
    for n in 1..=100 {
        jobs.sender.send(n)?
    }
    jobs.sender.close()     // tells the workers no more jobs are coming

    var total = 0
    for w in workers {
        total = total + w.join()
    }
    io.println($"total {total}")?

    // Request/response: channel.call sends a request built around a fresh
    // reply sender, then waits for the reply.
    use lookups <- channel.open<Lookup>(channel.defaults)
    let server = scope.spawn(fn() {
        while let .some(req) = lookups.receiver.recv() {
            let _ = req.reply.send($"value-{req.key}")
        }
    })
    let answer = channel.call(lookups.sender, fn(r) => { key: 42, reply: r })
    io.println(answer)?
    lookups.sender.close()
    server.join()
    ok(())
}
```

Output:

```text
total 5050
some(value-42)
```

The compiler checks what crosses between tasks: values are immutable,
shared mutable state is an explicit `Cell` (one task) or `Atom` (shared),
and a task cannot outlive its scope. If every task ends up blocked, the
program fails with `deadlock: all tasks are blocked` and names what each
task waits for.

## HTTP in a few lines

```badger
import std.http
import std.io
import std.text

// A handler is a function from a request to a Result of a response.
// Requests are records: { method, path, headers, body }.
fn route(request: http.Request) -> Result<http.Response, http.Error> {
    if request.method == #get && request.path == "/hello" {
        // http.json encodes any record (or other Codec value) as JSON.
        http.json(200, { message: "hi", path: request.path })
    } else {
        http.text(404, "not found")
    }
}

pub fn main() -> Result<Unit, io.Error> {
    let app = http.handler(route)

    // http.call runs a handler in-process: no socket, handy in tests.
    // http.listen(address, port, app) serves the same handler over TCP.
    let hello = http.call(app, http.request(#get, "/hello", ""))
    io.println($"{hello.status} {text.from_bytes(hello.body).unwrap_or("?")}")?
    let missing = http.call(app, http.request(#get, "/nope", ""))
    io.println(missing.status)?
    ok(())
}
```

Output:

```text
200 {"message":"hi","path":"/hello"}
404
```

## Tests and contracts

A test is a function named `test_*`. Put tests next to the code they test.

```badger
// pricing.bdg
import std.test

// Contracts: `requires` is checked when the function is called, `ensures`
// when it returns (`result` is the returned value). The compiler proves
// what it can and checks the rest at run time.
requires percent >= 0 && percent <= 100, "percent is 0..=100"
ensures result <= price
pub fn discount(price: Int, percent: Int) -> Int {
    price - price * percent / 100
}

fn parse_price(s: String) -> Result<Int, String> {
    s.parse_int().ok_or($"not a price: {s}")
}

// Any function named `test_*` is a test. It passes if it returns
// normally (or returns `ok`).
fn test_discount() {
    test.assert_eq(discount(200, 25), 150)
    test.assert_eq(discount(99, 0), 99)
}

fn test_parse_price() -> Result<Unit, String> {
    test.assert_eq(parse_price("120")?, 120)
    let e = test.expect_err(parse_price("cheap"))
    test.assert_eq(e, "not a price: cheap")
    ok(())
}

fn test_wrong_on_purpose() {
    test.assert_eq(discount(100, 10), 80)
}
```

```sh
badger test pricing.bdg
```

Output:

```text
test test_discount ... ok
test test_parse_price ... ok
test test_wrong_on_purpose ... FAILED (assertion failed: expected 80, found 90 at pricing.bdg:32:5)

test result: FAILED. 2 passed; 1 failed
```

A second argument filters tests by name:

```sh
badger test pricing.bdg parse
```

Output:

```text
test test_parse_price ... ok

test result: ok. 1 passed; 0 failed
```

`badger contracts` shows what the compiler decided about each clause:

```sh
badger contracts pricing.bdg
```

Output:

```text
pricing.bdg:7: discount requires `percent >= 0 && percent <= 100`: proven (holds at all 3 call(s))
pricing.bdg:8: discount ensures `result <= price`: runtime (not proven at every return)
2 clause(s): 1 proven, 1 checked at run time, 0 refuted
```

A call that can never satisfy a `requires` clause is a compile error
(E0145). A clause that fails at run time is a task defect; its message
names the clause and where it was broken.

## Where to go next

The guide, one topic per chapter:

1. [Values and functions](guide/01-values-and-functions.md)
2. [Types and records](guide/02-types-and-records.md)
3. [Errors](guide/03-errors.md)
4. [Collections](guide/04-collections.md)
5. [Modules and projects](guide/05-modules-and-projects.md)
6. [Tasks and channels](guide/06-tasks-and-channels.md)
7. [Effects and ownership](guide/07-effects-and-ownership.md)
8. [Systems and pipelines](guide/08-systems-and-pipelines.md)
9. [Testing and contracts](guide/09-testing-and-contracts.md)
10. [Services](guide/10-services.md)

Reference:

- [Command line](reference/cli.md), [REPL](reference/repl.md),
  [MCP server](reference/mcp.md), [configuration](reference/config.md)
- [Standard library](api/README.md), generated from the checked
  interfaces (also `badger doc std`)
- [Choosing collections](collections.md)
- [How the implementation works](internals.md)
- [The language specification](../SPEC.md)
