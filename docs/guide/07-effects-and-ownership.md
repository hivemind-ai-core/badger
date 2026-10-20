# Effects and ownership

Every function has an inferred effect summary, and every value has an ownership kind that decides whether it can be copied, moved, or sent to another task.
This chapter covers effect rows and bounds, `Transfer` and `Share`, affine values, and scoped regions.

## Effects

| Effect | Meaning |
|---|---|
| `Io` | Filesystem, network, processes, environment, clocks |
| `Wait` | Parking: sleeps, joins, channel waits, timers |
| `State` | Mutable state observable outside the function (an `Atom`, a `Cell` received as a parameter) |
| `Foreign` | Unsafe foreign (`extern "c"`) calls |

There is no `async`. Any function may suspend its task; effects only
record what it may do.

### See what was inferred

```badger
import std.io
import std.task

fn double(n: Int) -> Int { n * 2 }

fn greet(name: String) -> Result<Unit, io.Error> { io.println($"hello {name}") }

fn nap() -> Unit { task.sleep(1ms) }

fn counter() -> Int {
    let c = Cell.new(0)
    c.set(c.get() + 1)
    c.get()
}

fn bump(a: &Atom<Int>) -> Unit { a.set(a.get() + 1) }

fn apply(f: fn(Int) -> Int, x: Int) -> Int { f(x) }

pub fn main() -> Result<Unit, io.Error> {
    greet("x")?
    ok(())
}
```

`badger explain effect NAME FILE` prints a definition's effects:

```text
double: !{}
greet: !{Io}
nap: !{Wait}
counter: !{}
bump: !{State}
apply: !{} plus the effects of its callbacks
```

`counter` is `!{}`: a cell created and used only inside the function is
private computation, not observable state.

For a function whose body has effects, each is followed by where it first
comes from, so an unexpected `State` or `Wait` leads to its call:

```text
work: !{Io, State, Wait}
  Io: app.bdg:10:5 `io.println("start")`
  State: app.bdg:11:5 `c.set(helper())`
  Wait: app.bdg:11:11 `helper()`
```

### Bounds are upper bounds

```badger
import std.io

fn double(n: Int) -> Int !{} {
    n * 2
}

fn report(n: Int) -> Result<Unit, io.Error> !{Io, Wait} {
    io.println($"{double(n)}")
}

pub fn main() -> Result<Unit, io.Error> {
    report(21)
}
```

```text
42
```

`!{...}` after the result type is a bound: the body may do at most that.
Doing less is fine (`report` waits for nothing). Callers see the declared
bound, so `badger explain effect report` prints `!{Io, Wait}`.

Doing more is E0501:

```badger fragment
fn double(value: Int) -> Int !{} {
    let _ = io.println(value)   // E0501: effect `Io` is not permitted
    value * 2
}
```

`!{}` means no modeled effects. It does not promise the function
terminates or never panics.

### Effect variables

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

fn pure_sum(x: Int) -> Int !{} {
    twice(fn(n) { n + 1 }, x)
}

pub fn main() -> Result<Unit, io.Error> {
    let _ = logged(fn(n) { n * 3 }, pure_sum(1))
    ok(())
}
```

```text
3 -> 9
```

- `!Eff` names the callback's effects. `twice` does exactly what `f` does.
- `!{Io, Eff}` extends a variable: `logged` does `Io` plus whatever `f` does.
- `pure_sum` passes a pure callback, so `twice` is pure there and the `!{}`
  bound holds. A callback that printed would make it E0501.

You can usually omit all this. A function-typed parameter written without
`!` is already effect-polymorphic: the callback's effects are charged to
the caller. That is why `apply` above reports "plus the effects of its
callbacks".

The exception is a function type stored in a struct field, enum payload,
or type alias, or returned as a result. Written without `!`, nothing
determines its effects, so calling it counts as any effect. Annotate such
fields to call them under a tight bound:

```badger
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

Without the `!{}` on `on_start`, `start` is E0501.

### Effect-generic types

```badger
import std.io
import std.seq

struct Source<Eff: Effect> {
    next: fn() -> Int !Eff,
}

fn take_two<Eff>(s: Source<Eff>) -> Int !Eff {
    s.next() + s.next()
}

fn constant() -> Source<!{}> {
    { next: fn() { 21 } }
}

fn sum_pure() -> Int !{} {
    take_two(constant())
}

fn lines() -> Seq<Int, !{Io}> {
    seq.range(1, 3).map(fn(s) {
        let _ = io.println($"reading {s}")
        s
    })
}

pub fn main() -> Result<Unit, io.Error> {
    io.println($"{sum_pure()}")?
    for l in lines() {
        io.println(l)?
    }
    ok(())
}
```

```text
42
reading 1
1
reading 2
2
```

A type parameter declared `Eff: Effect` carries an effect row. Write the
argument as `Source<!{}>`, `Source<!{Io}>`, or `Source<Eff>`. The standard
`Seq<T, Eff>`, `Stream<T, Eff>`, and pipeline stages work this way: the
effects happen while iterating, and the row says which. In a signature, a
trailing effect argument may be omitted and is inferred.

### Module values are pure

Top-level `let` values are computed without effects and hold no mutable
state:

```badger fragment
import std.env

let home = env.get("HOME")   // E0137: a module-level value cannot have effect `Io`
let counter = Cell.new(0)    // E0138: module-level value `counter` holds a `Cell`
```

Importing a module never reads the environment or does I/O. Compute such
values in `main` and pass them along. Constants, records, and lists of
plain data are fine at the top level.

## What may cross tasks: `Transfer` and `Share`

| Value | `Share` (copy freely, use from many tasks) | `Transfer` (may cross a task boundary) |
|---|---|---|
| `Int`, `String`, records, lists, maps of shareable values | yes | yes |
| Closures capturing shareable values | yes | yes |
| `Atom<T>` | yes | yes |
| Channel endpoints | yes | yes |
| `Buffer<T>`, `ListBuilder<T>` (affine) | no | yes, by moving |
| `Cell<T>` | no | no (task-confined) |

A spawned closure, its captures, its result, and channel payloads must be
`Transfer`. Generic code states the requirement as a bound:
`where T: Transfer` or `where T: Share`. The compiler derives both from a
value's representation; code cannot implement them (E0217).

### `Atom` for state shared between tasks

```badger
import std.io
import std.task

pub fn main() -> Result<Unit, io.Error> {
    let hits = Atom.new(0)
    use scope <- task.scope()
    let workers = [1, 2, 3, 4].map(fn(_) {
        scope.spawn(fn() {
            for _ in 0..100 {
                let _ = hits.update(fn(n) { n + 1 })
            }
        })
    })
    for w in workers {
        w.join()
    }
    io.println($"hits {hits.get()}")?
    ok(())
}
```

`badger run --workers 4` prints:

```text
hits 400
```

`Atom` operations are atomic and carry the `State` effect. `update`
applies a function and returns the new value. Every user of one atom
serializes on it, so keep data a task works on owned by that task and send
it rather than sharing it.

### `Cell` for state inside one task

```badger
import std.io

fn make_counter() -> fn() -> Int {
    let count = Cell.new(0)
    fn() {
        count.set(count.get() + 1)
        count.get()
    }
}

pub fn main() -> Result<Unit, io.Error> {
    let next = make_counter()
    let _ = next()
    io.println($"{next()}")?
    ok(())
}
```

```text
2
```

A `Cell` may have many aliases in its own task and cannot leave it. The
same holds for everything that contains or captures one: records holding
a cell, closures capturing it, and functions returning such closures.
Sending one to another task is E0402:

```badger fragment
let count = Cell.new(0)
use scope <- task.scope()
let child = scope.spawn(fn() { count.set(1) })
// E0402: `fn() -> Unit` cannot be transferred to another task
```

Database transactions (`sqlite.Tx`, `postgres.Tx`) are confined the same
way, and so are borrows: a task spawned with `&b` or `&mut b` could race
the owner's writes or outlive it, so a closure capturing a borrow (or a
value holding one) stays on its task. Move the value into the task, or
share it through an `Atom` or a channel.

## Affine values

An affine value has one owner. Using it by value moves it, and the old
binding is dead.

| Affine value | Made by | Consumed by |
|---|---|---|
| `ListBuilder<T>` | `list.builder<T>()` | `finish()` |
| `Buffer<T>` | `buffer.new<T>()`, `buffer.with_capacity(n)` | `freeze()` (to `Array<T>`) |
| `FixedBuffer<T>` | `buffer.fixed(n)` | `freeze()` |
| `channel.Batch<T>` lease | `receiver.recv_batch()` | `release()` |
| `once fn` closure | a closure that consumes an affine capture | calling it |

Aggregates follow their fields: a record or list holding a buffer is
affine too.

```badger
import std.buffer
import std.io
import std.list

fn fill(b: &mut Buffer<Int>, n: Int) -> Unit {
    for i in 1..=n {
        b.push(i)
    }
}

fn size(b: &Buffer<Int>) -> Int { b.len() }

pub fn main() -> Result<Unit, io.Error> {
    let names = list.builder<String>()
    names.append("ada")
    names.append("grace")
    let frozen = names.finish()

    let buf = buffer.new<Int>()
    fill(&mut buf, 4)
    io.println($"{frozen} {size(&buf)}")?
    let array = buf.freeze()
    io.println($"{array.len()} items frozen")?
    ok(())
}
```

```text
["ada", "grace"] 4
4 items frozen
```

- Method calls borrow their receiver automatically (`buf.push`, `b.len()`).
- Ordinary functions borrow explicitly: `&buf` for read access, `&mut buf`
  for exclusive access. Passing `buf` where the function takes `&Buffer<Int>`
  is E0412.
- `let` stops rebinding, not mutation through the owner: `buf` above is a
  `let` and still grows.

Using a value after it moved is E0404:

```badger fragment
let buf = buffer.new<Int>()
let array = buf.freeze()
buf.push(1)   // E0404: use of moved value `buf`
```

| Code | Mistake |
|---|---|
| E0404 | Use after move (passing by value, storing, returning, or capturing moves) |
| E0405 | Moving or mutating an owner while a borrow of it is still used |
| E0407 | Returning or storing a borrow that would outlive its owner |
| E0408 | `&mut` of something not assignable: a plain `let` value (builders and buffers are fine), a temporary, or a shared borrow |
| E0412 | Passing an owned affine value where a function takes `&T` |
| E0413 | A resource (file, batch lease) dropped without being closed, released, or moved on |
| E0414 | A value of a `@move_only` type used again after it moved (a copy) |

### Resources are used exactly once

A file and a batch lease are *linear*: affine, and each must be consumed on
every path out of its scope, by `close` (a file) or `release` (a lease), or
by moving it on (passing it by value, returning it, storing it). Reads and
writes borrow it. So a file is closed deterministically, where the program
says, instead of by the finalizer once nothing can reach it.

```badger
import std.fs
import std.io
import std.path

fn save(f: fs.File, text: String) -> Result<Unit, io.Error> {
    f.write_text(text)?
    f.close()
}

pub fn main() -> Result<Unit, io.Error> {
    let dir = fs.make_temp_dir("guide-")?
    let p = path.join(dir, "notes.txt")
    let f = fs.create(p)?
    f.write_text("first\n")?
    save(f, "second\n")?
    io.print(fs.read_text(p)?)?
    fs.remove_all(dir)
}
```

```text
first
second
```

Leaving out the `save` call is E0413: the file is written (borrowed) but
never consumed. The diagnostic points at the binding and at where the value
is dropped: the end of its scope, a `return`, a `break` or `continue`, or an
assignment over it. Discarding one (`let _ = fs.open(p)?`, `.ok(_)`) is E0413
too. An error exit (`?`, `return err(..)`) is not checked: there the runtime
closes the file, as it does for a file held by a value that is dropped.
Channel endpoints are shared handles, not resources of this kind: see
[the specification](../../spec/04-memory-and-collections.md#linear-resources).

### Move-only types

Your own struct or enum can ask for the same discipline: mark it
`@move_only` and its values are moved, never copied. It suits a value that
stands for something that happens once, such as a ticket that is redeemed or
a draft that is sent.

```badger
import std.io

@move_only
pub struct Draft { pub to: String, pub body: String }

pub struct Sent { pub to: String }

impl Draft {
    pub fn preview(self: &Self) -> String { $"to {self.to}: {self.body}" }
}

// sending takes the draft: it cannot be sent twice
fn send(d: Draft) -> Sent { { to: d.to } }

pub fn main() -> Result<Unit, io.Error> {
    let d: Draft = { to: "ada", body: "hello" }
    io.println(d.preview())?
    let s = send(d)
    io.println($"sent to {s.to}")?
    ok(())
}
```

```text
to ada: hello
sent to ada
```

Binding, passing by value, returning, storing in a record or list, and
capturing in a closure that takes it all move the value; using the old
binding again would copy it, and is E0414. Field reads, `self: &Self`
methods, and `&d` arguments only borrow.

```badger fragment
let s = send(d)
let t = send(d)   // E0414: cannot copy `d`: `Draft` is move-only
```

Move-only is not linear: a draft may be dropped without being sent. A file
must be closed (E0413) *and* cannot be copied; a `@move_only` type only
promises the second. A type without the marker copies as it always has.

### Moving into a task

```badger
import std.buffer
import std.io
import std.task

pub fn main() -> Result<Unit, io.Error> {
    let buf = buffer.new<Int>()
    buf.push(1)
    buf.push(2)
    use scope <- task.scope()
    let child = scope.spawn(fn() {
        buf.push(3)
        buf.len()
    })
    io.println($"child saw {child.join()} items")?
    ok(())
}
```

```text
child saw 3 items
```

The spawned closure captures `buf` and takes ownership. Touching `buf` in
the parent after the spawn is E0404.

### `once` closures

```badger
import std.buffer
import std.io

fn run_once(f: once fn() -> Int) -> Int { f() }

pub fn main() -> Result<Unit, io.Error> {
    let buf = buffer.new<Int>()
    buf.push(5)
    let total = run_once(fn() { buf.freeze().len() })
    io.println($"{total}")?
    ok(())
}
```

```text
1
```

A closure that consumes an affine capture can run once. A parameter typed
`once fn` accepts it. Calling such a closure twice is E0404.

## Scoped regions

```badger
import std.io

pub struct Session<S> {
    name: String,
}

impl<S> Session<S> {
    pub fn name(self: &Self) -> String { self.name }
}

fn with_session<R>(name: String, body: for<S: Scope> fn(Session<S>) -> R) -> R {
    let _ = io.println($"open {name}")
    let r = body({ name })
    let _ = io.println($"close {name}")
    r
}

pub fn main() -> Result<Unit, io.Error> {
    let size = with_session("db", fn(s) { s.name().len() })
    io.println($"size {size}")?
    ok(())
}
```

```text
open db
close db
size 2
```

`for<S: Scope> fn(Session<S>) -> R` gives the callback a fresh region `S`.
The result type `R` is chosen outside that region, so it cannot mention
`S`. A value branded with `S`, or a closure capturing one, cannot leave the
callback:

```badger fragment
let leaked = with_session("db", fn(s) { s })
// E0401: a scoped value escapes its scope
```

Storing the value in an outer `Cell`, `Atom`, collection, or `var` escapes
too, including through helper functions; `dyn`, `Data`, and channels do not
erase the brand. All are E0401.

The standard library uses regions for every resource with a lifetime:

| Scope | Branded values |
|---|---|
| `task.scope` | `TaskScope<S>`, task handles |
| `sqlite.transaction`, `postgres.transaction` | `Tx<S>` |
| `sqlite.open`, `postgres.connect`, providers | `Pool<S>` |
| `system.run` | the running system and its node outputs |
| `http.scoped` | `RequestScope<S>`, the request's `BodyReader<S>` |

A trailing region parameter may be elided in annotations: `sqlite.Pool`
means `sqlite.Pool<_>`.

### Region blocks

A `region` block opens a region of your own. Its name is a type-level name
inside the block, and a struct whose last type parameter is a region
(`R: Scope`) is a *region record*: `Batch<Scratch>` is a batch of region
`Scratch`. Use one for scratch values that must not outlive a computation:

```badger
import std.io

struct Batch<R: Scope> { name: String, items: List<Int> }

fn batch<R: Scope>(name: String, items: List<Int>) -> Batch<R> { { name, items } }

fn total<R: Scope>(b: &Batch<R>) -> Int { b.items.fold(0, fn(a, x) { a + x }) }

// the caller gives the region of the result: it outlives the return
fn summary<R: Scope>(n: Int) -> Batch<R> {
    region Scratch {
        let scratch: Batch<Scratch> = batch("scratch", [n, n + 1])
        batch("summary", [total(&scratch)])
    }
}

pub fn main() -> Result<Unit, io.Error> {
    let sum = region Main {
        let s: Batch<Main> = summary(2)
        total(&s)
    }
    io.println($"sum {sum}")
}
```

```text
sum 5
```

`Scratch` ends with its block, so a `Batch<Scratch>` cannot leave it: not
as the block's result, not by `return` or `break`, not into an outer `var`,
cell, or collection, and not inside a closure or task. All are E0414:

```badger fragment
fn leak<R: Scope>(n: Int) -> Batch<R> {
    region Scratch {
        let scratch: Batch<Scratch> = batch("scratch", [n])
        scratch
        // E0414: a value of region `Scratch` outlives its region
    }
}
```

The brand is a static guarantee: region records are allocated and
collected like any other value, and a `region` block runs as an ordinary
block.

Next: [Systems and pipelines](08-systems-and-pipelines.md).
