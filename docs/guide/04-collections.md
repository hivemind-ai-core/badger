# 4. Collections

The collection types, how to build them, and the operations you use every
day. For choosing between them and what each operation costs, see
[Collections: choosing and costs](../collections.md).

## The types

| Type | What it is | Literal |
|---|---|---|
| `List<T>` | immutable sequence, indexable | `[1, 2, 3]` |
| `Map<K, V>` | immutable key-value map | `map { "a" => 1 }` |
| `Set<T>` | immutable set | `set { :get, :post }` |
| `Array<T>` | immutable, fixed length, contiguous | `[1, 2]` where `Array<Int>` is expected |
| `Buffer<T>` | mutable, contiguous, one owner | `[1, 2]` where `Buffer<Int>` is expected |
| `Bytes` | immutable bytes | `"text".to_bytes()` |
| `Seq<T>` | lazy sequence: work happens when consumed | `1..10`, `xs.seq()` |
| `Stream<T>` | items that arrive over time; reading may wait | `stream.from_receiver(rx)` |

All of them are values: "changing" a list, map, or set returns a new one
and leaves the old one as it was. `Buffer` and the builders are the
exceptions; they are mutable and have a single owner
([chapter 7](07-effects-and-ownership.md)).

A collection's elements share one type: `[1, "two"]` is a type error.

An empty literal needs its type from context: `let names: List<String> =
[]`, or `map.empty<String, Int>()`.

## Lists

```badger
import std.io

pub fn main() -> Result<Unit, io.Error> {
    let xs = [3, 1, 4, 1, 5]
    io.println(xs[0])?
    io.println(xs.get(10))?
    io.println($"{xs.len()} {xs.first()} {xs.last()} {xs.contains(4)}")?
    io.println(xs.append(9))?
    io.println(xs.sort())?
    io.println(xs.slice(1, 3))?
    io.println(xs)?
    ok(())
}
```

```text
3
none
5 some(3) some(5) true
[3, 1, 4, 1, 5, 9]
[1, 1, 3, 4, 5]
[1, 4]
[3, 1, 4, 1, 5]
```

- `xs[i]` with an invalid index is a defect. `xs.get(i)` returns
  `Option` and never fails.
- `append`, `concat`, `reverse`, `slice`, and `sort` return new lists.
- Lists have no `push` and cannot be assigned through an index.

### Transforming lists

Each operation is a method (`xs.map(f)`) and a function in `std.list`
(`list.map(xs, f)`, convenient in pipes). List operations are eager and
return a new list.

```badger
import std.io
import std.list

type Person = { name: String, age: Int, team: String }

pub fn main() -> Result<Unit, io.Error> {
    let people: List<Person> = [
        { name: "ada", age: 36, team: "core" },
        { name: "bob", age: 25, team: "web" },
        { name: "cy", age: 41, team: "core" },
    ]
    let names = people
        |> list.filter(fn(p) => p.age > 30)
        |> list.map(fn(p) => p.name)
    io.println(names)?
    io.println(people.sort_by(fn(p) => p.age).map(fn(p) => p.name))?
    io.println(people.fold(0, fn(total, p) => total + p.age))?
    io.println(people.find(fn(p) => p.team == "web").map(fn(p) => p.name))?
    let teams = people.group_by(fn(p) => p.team)
    io.println(teams.get_or("core", []).map(fn(p) => p.name))?
    io.println([1, 2, 3, 4, 5].chunks_of(2))?
    io.println(list.sum([1, 2, 3]))?
    ok(())
}
```

```text
["ada", "cy"]
["bob", "ada", "cy"]
102
some(bob)
["ada", "cy"]
[[1, 2], [3, 4], [5]]
6
```

| Group | Operations |
|---|---|
| Map and filter | `map`, `filter`, `filter_map`, `flat_map`, `enumerate`, `zip`, `scan` |
| Slice | `take`, `skip`, `take_while`, `skip_while`, `slice`, `chunks_of`, `partition` |
| Search | `find`, `find_map`, `position`, `any`, `all`, `contains`, `count` |
| Reduce | `fold`, `min_by`, `max_by`, `list.sum` (lists of `Int`) |
| Order | `sort`, `sort_by(key)`, `sort_with(compare)`, `reverse`, `dedup`, `unique` |
| Group | `group_by(key)` (a `Map` from key to the items, in input order) |
| Fallible | `try_map`, `try_fold`, `try_each`, `validate` (see [below](#fallible-processing)) |
| Convert | `seq()`, `to_array()` |

`sort` and `sort_by` are stable. `sort_with(compare)` takes a function
returning a negative, zero, or positive `Int`. `dedup` removes
consecutive duplicates; `unique` removes all duplicates and keeps first
occurrences.

Every function and its signature is in the [`std.list`
reference](../api/list.md).

### Building lists

To build a list item by item, use a builder instead of repeated `append`.

```badger
import std.io
import std.list

pub fn main() -> Result<Unit, io.Error> {
    let squares = list.build(fn(out) {
        for i in 1..=5 {
            out.append(i * i)
        }
    })
    io.println(squares)?

    let b = list.builder<String>()
    b.append("first")
    b.append("second")
    let words = b.finish()
    io.println(words)?

    io.println(list.range(0, 5))?
    io.println((0..5).map(fn(n) => n * 10).collect())?
    ok(())
}
```

```text
[1, 4, 9, 16, 25]
["first", "second"]
[0, 1, 2, 3, 4]
[0, 10, 20, 30, 40]
```

- `list.build(fill)` hands `fill` a builder and finishes it when `fill`
  returns.
- `list.builder<T>()` gives a builder you finish yourself. `finish`
  consumes it: using the builder afterwards is E0404.
- `list.collect(seq)` or `seq.collect()` turns a sequence into a list.

## Maps

```badger
import std.io
import std.map

pub fn main() -> Result<Unit, io.Error> {
    let stock = map { "apple" => 3, "pear" => 5 }
    io.println(stock.get("apple"))?
    io.println(stock.get("kiwi"))?
    io.println(stock.get_or("kiwi", 0))?

    let more = stock.insert("kiwi", 2).remove("pear")
    io.println(more.keys().sort())?
    io.println(stock.len())?

    let bumped = stock.update("apple", fn(v) => v.map(fn(n) => n + 10))
    io.println(bumped.get_or("apple", 0))?

    var total = 0
    for (name, count) in stock {
        total = total + count
    }
    io.println(total)?

    let counts = map.from_list([("a", 1), ("b", 2)])
    io.println(counts.merge_with(map { "b" => 5 }, fn(x, y) => x + y).get("b"))?
    ok(())
}
```

```text
some(3)
none
0
["apple", "kiwi"]
2
13
8
some(7)
```

| Operation | Result |
|---|---|
| `m.get(k)` | `Option<V>` |
| `m.get_or(k, default)`, `m.contains(k)`, `m.len()`, `m.is_empty()` | |
| `m.insert(k, v)`, `m.remove(k)` | a new map |
| `m.update(k, fn(Option<V>) -> Option<V>)` | a new map; returning `none` removes the key |
| `m.keys()`, `m.values()`, `m.entries()` | lists, in the map's order |
| `m.merge(other)` | both; `other` wins on shared keys |
| `m.merge_with(other, combine)` | both; `combine(mine, theirs)` on shared keys |
| `m.map_values(f)`, `m.filter(fn(k, v) => ...)` | a new map |
| `map.from_list(pairs)`, `map.collect(seq)`, `map.build(fill)`, `map.builder<K, V>()` | construction |

- Iteration order is unspecified. Sort keys (`m.keys().sort()`) when
  output order matters.
- `for (k, v) in m` iterates entries as tuples.
- Keys need `Hash`. Strings, integers, symbols, and records and enums of
  them qualify; floats do not (E0341).
- A record literal where a `Map<Symbol, V>` is expected builds a map:
  `let prices: Map<Symbol, Int> = { apple: 3, pear: 5 }`.

## Sets

```badger
import std.io
import std.set

pub fn main() -> Result<Unit, io.Error> {
    let a = set { 1, 2, 3, 4 }
    let b = set.from_list([3, 4, 5])
    io.println(a.contains(2))?
    io.println(a.union(b).to_list().sort())?
    io.println(a.intersection(b).to_list().sort())?
    io.println(a.difference(b).to_list().sort())?
    io.println(set { 3, 4 }.is_subset(a))?
    io.println(set { :get, :post } == set { :post, :get })?
    ok(())
}
```

```text
true
[1, 2, 3, 4, 5]
[3, 4]
[1, 2]
true
true
```

Sets have `insert`, `remove`, `contains`, `len`, `is_empty`, `union`,
`intersection`, `difference`, `is_subset`, `filter`, and `to_list`, and
are built with `set.from_list`, `set.collect`, `set.build`, or
`set.builder<T>()`. Members need `Hash`, and iteration order is
unspecified.

## Arrays and buffers

`Array<T>` is immutable and contiguous. `Buffer<T>` is mutable and
contiguous; freezing it gives an `Array`.

```badger
import std.array
import std.buffer
import std.io

pub fn main() -> Result<Unit, io.Error> {
    let offsets: Array<Int> = [0, 8, 16]
    io.println(offsets[2])?
    let squares = array.build(4, fn(i) => i * i)
    io.println(squares.to_list())?
    let from_list = [5, 6, 7].to_array()
    io.println(from_list.len())?

    let buf = buffer.new<Int>()
    buf.push(1)
    buf.push(2)
    buf.set(0, 10)
    io.println(buf.pop())?
    let frozen = buf.freeze()
    io.println(frozen.to_list())?

    let fixed = buffer.fixed<Int>(1)
    io.println(fixed.push(1).is_ok())?
    io.println(fixed.push(2).is_ok())?
    ok(())
}
```

```text
16
[0, 1, 4, 9]
3
some(2)
[10]
true
false
```

| Constructor | Gives |
|---|---|
| `array.build(len, fn(i) => ...)` | an `Array` |
| `array.try_build(len, f)` | `Result<Array, E>`; the first error stops construction |
| `array.from_list(xs)`, `xs.to_array()`, `array.collect(seq)` | an `Array` copied from a list or sequence |
| `buffer.new<T>()`, `buffer.with_capacity<T>(n)` | a growable `Buffer` |
| `buffer.fixed<T>(n)` | a `FixedBuffer` that never grows: `push` returns `err(Full)` when full |

- Buffers have `push`, `pop`, `get`, `set`, `len`, `capacity`, and
  `freeze`. `freeze` consumes the buffer.
- A buffer has one owner. Passing it by value moves it; using it again is
  E0404. Ownership and borrowing are in [chapter 7](07-effects-and-ownership.md).
- Arrays and lists convert explicitly (`to_array`, `to_list`); there is no
  hidden copy between them.

## Bytes

`Bytes` is immutable byte storage.

```badger
import std.bytes
import std.io

pub fn main() -> Result<Unit, io.Error> {
    let request = "GET / HTTP/1.1\r\nHost: example.test\r\n\r\n".to_bytes()
    io.println(request.len())?
    io.println(request.starts_with("GET".to_bytes()))?
    io.println(request.index_of("Host".to_bytes()))?
    let lines = request.split("\r\n".to_bytes())
    io.println(lines.len())?
    io.println(request.slice(0, 3).map(fn(b) => b.hex()))?
    io.println(bytes.from_list([104, 105]).to_text())?
    io.println(bytes.from_list([255]).to_text().is_err())?
    ok(())
}
```

```text
38
true
some(16)
4
some(474554)
ok(hi)
true
```

| Operation | Result |
|---|---|
| `s.to_bytes()`, `bytes.from_text(s)` | UTF-8 bytes of a string |
| `b.to_text()` | `Result<String, String>`: fails on invalid UTF-8 |
| `bytes.from_list(list)`, `b.to_list()` | from and to `List<Byte>` |
| `bytes.freeze(buffer)` | a `Buffer<Byte>` as `Bytes` |
| `b.len()`, `b.get(i)`, `b.slice(start, end)` | `get` and `slice` return `Option` |
| `b.index_of(needle)`, `b.starts_with(p)`, `b.ends_with(s)`, `b.split(sep)`, `b.concat(other)`, `bytes.join(parts, sep)` | |
| `b.hex()` | lowercase hex text |

`Bytes` is not iterable and has no `Display`; use `to_list()` or
`to_text()`. Base64 and other encodings are in
[`std.encoding`](../api/encoding.md).

## Sequences (`Seq`)

A `Seq<T>` describes a sequence without computing it. Adapters (`map`,
`filter`, `take`, ...) build a new description; nothing runs until a
consumer (`collect`, `fold`, `count`, `find`, a `for` loop) pulls items.

```badger
import std.io

pub fn main() -> Result<Unit, io.Error> {
    let pulled = Cell.new(0)
    let squares = (1..=1000000).map(fn(n) {
        pulled.set(pulled.get() + 1)
        n * n
    })
    io.println(pulled.get())?
    let first = squares.filter(fn(n) => n % 7 == 0).take(3).collect()
    io.println($"{first} after {pulled.get()} items")?
    let again = squares.take(2).collect()
    io.println($"{again} after {pulled.get()} items")?
    ok(())
}
```

```text
0
[49, 196, 441] after 21 items
[1, 4] after 23 items
```

- Building `squares` ran nothing. Taking three multiples of 7 pulled only
  21 items.
- A `Seq` is reusable: each consumer starts a fresh iteration, re-running
  the work (and any effects) from the start.
- Effects happen during iteration, in order.

Where sequences come from:

| Source | Example |
|---|---|
| a range | `0..n`, `1..=n` |
| a list or array | `xs.seq()`, `list.of(xs)`, `array.of(a)` |
| text | `text.lines(s)` |
| a counter | `seq.range(start, end)` |
| a function | `seq.from_fn(start)`: `start` returns a fresh "next item" function |
| a resource | `seq.scoped(open, next, close)`: `close` runs exactly once, also on early exit |

Adapters: `map`, `filter`, `filter_map`, `flat_map`, `take`, `skip`,
`take_while`, `skip_while`, `enumerate`, `zip`, `scan`, `chunks`.
Consumers: `collect`, `fold`, `try_fold`, `try_each`, `count`, `find`,
`find_map`, `any`, `all`, `first`, `last`, `min_by`, `max_by`,
`partition`, `group_by`, and `seq.sum`, `seq.try_collect`. A consumer that
stops early (`find`, `take`, `break`, `?`) closes a scoped source.

Keep a pipeline lazy until the end, then collect once. The
[`std.seq` reference](../api/seq.md) lists every function.

## Streams

A `Stream<T>` is a source whose items arrive over time, typically from a
channel. Reading may suspend the task until the next item. Unlike a
`Seq`, a stream is consumed as it is read: the rest stays readable after
`take`, and it cannot be replayed.

```badger
import std.io
import std.stream

pub fn main() -> Result<Unit, io.Error> {
    let s = stream.from_list([1, 2, 3, 4, 5, 6, 7])
    let head = s.take(2) |> stream.collect
    io.println(head)?
    io.println(s.batches(3) |> stream.collect)?
    io.println(stream.from_list([1, 2, 3]).fold(0, fn(a, x) => a + x))?
    io.println(stream.from_list([1, 2, 3]).collect_at_most(2))?
    ok(())
}
```

```text
[1, 2]
[[3, 4, 5], [6, 7]]
6
none
```

- Operations: `map`, `filter`, `take`, `take_while`, `batches`,
  `try_map`, `each`, `try_each`, `fold`, `drain`, `collect`,
  `collect_at_most`, `try_collect`, `close`.
- `collect` reads until the stream ends; `collect_at_most(n)` bounds it
  and returns `none` past the limit.
- Closing a stream, or an operation that stops at an error, closes its
  source, so a producer sending into the channel learns to stop.

Streams over channels are covered in [chapter 6](06-tasks-and-channels.md).

## Fallible processing

| Operation | Behavior |
|---|---|
| `xs.try_map(f)` | `ok(list)`, or the first error (stops there) |
| `xs.try_fold(init, f)`, `xs.try_each(f)` | stop at the first error |
| `list.all_ok(results)` | a list of results to a result of a list |
| `xs.validate(f)` | visits every item; `err` holds every error, in order |
| `list.partition_results(results)` | `(successes, failures)` |

```badger
import std.io
import std.list

fn parse(s: String) -> Result<Int, String> {
    match s.parse_int() {
        .some(n) => ok(n),
        .none => err($"bad number: {s}"),
    }
}

pub fn main() -> Result<Unit, io.Error> {
    let good = ["1", "2", "3"]
    let mixed = ["1", "x", "3", "y"]
    io.println(good.try_map(parse))?
    io.println(mixed.try_map(parse))?
    io.println(mixed.validate(parse))?
    let (numbers, errors) = list.partition_results(mixed.map(parse))
    io.println($"{numbers} {errors}")?
    io.println(good.try_fold(0, fn(acc, s) => ok(acc + parse(s)?)))?
    ok(())
}
```

```text
ok([1, 2, 3])
err(bad number: x)
err(["bad number: x", "bad number: y"])
[1, 3] ["bad number: x", "bad number: y"]
ok(6)
```

Use `validate` to report every problem in an input. Avoid `filter_map`
when failures matter: it drops them silently.

## Text

`String` is not a collection type, but many collection-shaped operations
exist for it: `s.chars()` (`List<Char>`), `s.lines()` and `s.words()`
(`List<String>`), `text.lines(s)` (the same lines as a `Seq<String>`, for
pipelines), `s.split(sep)`, and `text.join(parts, sep)`. See the [`std.text`
reference](../api/text.md). To build a long string, collect the pieces
and `text.join` them once.

## Ordering, equality, costs

- Equality is structural for every collection; maps compare by entries and
  sets by membership.
- Sorting uses one ordering for every type; it is described in
  [ordering and equality](../collections.md#ordering-and-equality).
- Operation costs are in the [cost table](../collections.md#costs).

---

Next: [Modules and projects](05-modules-and-projects.md)
