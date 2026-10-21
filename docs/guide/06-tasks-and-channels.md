# Tasks and channels

Concurrent work runs in tasks owned by a scope; tasks talk through typed, bounded channels.
This chapter covers scopes, cancellation and timeouts, channels, replies, broadcast, and deadlock reports.

## Run work in a task scope

```badger
import std.io
import std.task

fn square(n: Int) -> Int { n * n }

pub fn main() -> Result<Unit, io.Error> {
    use scope <- task.scope()
    let a = scope.spawn(fn() { square(3) })
    let b = scope.spawn(fn() { square(4) })
    io.println($"{a.join() + b.join()}")?
    ok(())
}
```

```text
25
```

`use scope <- task.scope()` opens a scope for the rest of the function. The
rest of the function is the scope's body, and it returns a `Result`.

- `scope.spawn(f)` starts a child task running `f` and returns a handle.
- `handle.join()` waits for the child and returns its value unchanged. A
  child that returns a `Result` gives you that `Result`; nothing is flattened.
- When the body returns `ok`, the scope waits for every child.
- When the body returns `err`, the scope cancels its unfinished children,
  waits for them to settle, then returns the error.

Every task belongs to a scope. There is no global spawn, and dropping a
handle never detaches a task. A handle cannot leave its scope: returning
one from the scope's callback is E0401.

A task whose value is a `Result` must be used. Spawning one and ignoring
the handle is E0337.

What a spawned closure may capture and return is limited to values that can
cross tasks. [Chapter 7](07-effects-and-ownership.md) explains `Transfer`.

## Combine fallible children

```badger
import std.io
import std.task

fn parse(text: String) -> Result<Int, String> {
    match text.parse_int() {
        .some(n) => ok(n),
        .none => err($"not a number: {text}"),
    }
}

fn sum(x: String, y: String) -> Result<Int, String> {
    use scope <- task.scope()
    let a = scope.spawn(fn() { parse(x) })
    let b = scope.spawn(fn() { parse(y) })
    let (m, n) = scope.try_join(a, b)?
    ok(m + n)
}

pub fn main() -> Result<Unit, io.Error> {
    io.println($"{sum("20", "22")}")?
    io.println($"{sum("20", "x")}")?
    ok(())
}
```

```text
ok(42)
err(not a number: x)
```

| Method | Waits for | On the first error |
|---|---|---|
| `scope.try_join(a, b)` | Two tasks returning `Result` | Cancels and settles the other task, returns the error |
| `scope.try_all(tasks)` | A `List` of tasks returning `Result<T, E>` | Cancels and settles the rest, returns the error |

"First" means first in completion order. `try_join` accepts two different
error types when the target error type converts from both (`From`, see
[chapter 3](03-errors.md)). `try_all` returns the values in task order.

`task.map_bounded(items, limit, f)` runs `f` over a list with at most
`limit` calls at once (in a scope of its own) and returns the results in
item order. The first error cancels the calls still running, and items
not started yet are not run.

## Contain a failure with `capture`

A defect (a `panic`, a failed assertion or contract, overflow) fails its
task. Without containment it fails the scope and the owning task:

```badger
import std.io
import std.task

pub fn main() -> Result<Unit, io.Error> {
    use scope <- task.scope()
    let t = scope.spawn(fn() -> Int { panic("boom") })
    io.println($"{t.join()}")?
    ok(())
}
```

Saved as `boom.bdg`, it prints:

```text
panic: child task failed: boom at boom.bdg:6:39
```

The process exits with status 101. To observe the failure instead, spawn
with `capture`. Joining a captured task returns a `task.Exit<T>`:

```badger
import std.io
import std.task

fn risky(n: Int) -> Int {
    if n > 2 {
        panic("too big")
    }
    n * 10
}

pub fn main() -> Result<Unit, io.Error> {
    use scope <- task.scope()
    for n in [1, 5] {
        let t = scope.capture(fn() { risky(n) })
        match t.join() {
            .returned(v) => io.println($"returned {v}")?,
            .failed(info) => io.println($"failed: {info.message}")?,
            .cancelled(reason) => io.println($"cancelled: {reason}")?,
        }
    }
    ok(())
}
```

```text
returned 10
failed: too big
```

| `Exit<T>` variant | Meaning |
|---|---|
| `.returned(value)` | The task finished and returned `value` |
| `.failed(info)` | A defect; `info.message`, `info.location`, and `info.notes` (cleanup failures) |
| `.cancelled(reason)` | The task was cancelled; `reason` is a `task.CancelReason` |

A defect cannot be caught inside the task that raised it. Containment is
per task. [Workers](08-systems-and-pipelines.md#workers) build restart
policies on `capture`.

## Cancel and time out

```badger
import std.io
import std.task

fn slow() -> Int {
    task.sleep(10s)
    1
}

pub fn main() -> Result<Unit, io.Error> {
    match task.timeout(50ms, slow) {
        .ok(v) => io.println($"done: {v}")?,
        .err(e) => io.println($"{e}")?,
    }
    match task.timeout(1s, fn() { 2 + 2 }) {
        .ok(v) => io.println($"done: {v}")?,
        .err(e) => io.println($"{e}")?,
    }
    ok(())
}
```

```text
timed out after 50.000ms
done: 4
```

`task.timeout(limit, f)` runs `f` as a child task. If `f` is still running
after `limit`, it is cancelled, its cleanup runs, and the call returns
`err(task.TimedOut)`. A defect in `f` still propagates.

To cancel a task yourself, call `handle.cancel()`. `scope.defer(f)`
registers cleanup that runs when the scope closes, after its children have
settled, in reverse registration order, also when the scope fails:

```badger
import std.io
import std.task

pub fn main() -> Result<Unit, io.Error> {
    use scope <- task.scope()
    scope.defer(fn() { let _ = io.println("cleanup runs last") })
    let t = scope.capture(fn() -> Int {
        task.sleep(10s)
        1
    })
    t.cancel()
    match t.join() {
        .cancelled(reason) => io.println($"cancelled: {reason}")?,
        _ => io.println("finished")?,
    }
    ok(())
}
```

```text
cancelled: requested
cleanup runs last
```

| `CancelReason` | Cause |
|---|---|
| `requested` | `handle.cancel()` was called |
| `timeout` | `task.timeout` expired |
| `scope_exited` | The owning scope ended (its body returned an error, or it halted) |
| `sibling_failed` | A sibling task failed with a defect |
| `sibling_error` | A sibling joined with `try_join` / `try_all` returned an error |

Rules that follow from the design:

- Cancellation is a request. A task observes it at its next safe point:
  a wait, a loop iteration, or a recursive call (the compiler inserts the
  checks, so a busy loop is cancelled too).
- Cancellation is not a `Result` error. It cannot be swallowed by `?` or
  `match` inside the cancelled task.
- A timeout does not undo work. A request already sent may have taken
  effect on the remote side.

## What cancellation does to a waiting task

```badger
import std.channel
import std.io
import std.process
import std.task

pub fn main() -> Result<Unit, channel.ConfigError> {
    // the subprocess is killed when the timeout cancels the task
    let r = task.timeout(100ms, fn() { process.run("sleep", ["5"]).is_ok() })
    let _ = io.println($"sleep: {r}")
    // a waiting receive is withdrawn; the channel still works
    let ch = channel.create<Int>(channel.defaults)?
    let r = task.timeout(100ms, fn() { ch.receiver.recv() })
    let _ = io.println($"recv: {r}")
    let _ = ch.sender.send(5)
    let _ = io.println($"afterwards: {ch.receiver.recv()}")
    ok(())
}
```

```text
sleep: err(timed out after 100.000ms)
recv: err(timed out after 100.000ms)
afterwards: some(5)
```

| The task waits on | When it is cancelled |
|---|---|
| A subprocess (`std.process`) | The process is killed; the task waits for it to exit, then leaves |
| A SQLite query | The query is interrupted; the pool stays usable |
| A native module's async call | The call is withdrawn (the module's cancel hook runs); a late result's handles are released |
| A blocking file call, or a blocking native call that owns its arguments | The task leaves at once; the result is dropped when the call ends. An external effect, such as a write, may still happen |
| A blocking `extern "c"` call | The task waits for the call to return, because the call may use the task's memory |
| A channel send, receive, or select | The wait is withdrawn; an uncommitted reservation is released. A message already committed stays sent |

Under `badger run --record`, every operation is waited for, so the bundle
holds its outcome. `tests/run/cancel_matrix.bdg` exercises each row.

## Channels

```badger
import std.channel
import std.io
import std.task

@derive(Error, From)
pub enum AppError {
    config(source: channel.ConfigError),
    send(source: channel.SendError<Int>),
}

pub fn main() -> Result<Unit, AppError> {
    let jobs = channel.create<Int>({ ..channel.defaults, capacity: 2 })?
    use scope <- task.scope()
    let consumer = scope.spawn(fn() {
        var total = 0
        loop {
            match jobs.receiver.recv() {
                .some(n) => { total = total + n },
                .none => break,
            }
        }
        total
    })
    for n in 1..=5 {
        jobs.sender.send(n)?
    }
    jobs.sender.close()
    let _ = io.println($"total {consumer.join()}")
    ok(())
}
```

```text
total 15
```

`channel.create<T>(options)` returns `{ sender, receiver }` or a
`channel.ConfigError`. Options are a complete record, usually
`{ ..channel.defaults, capacity: N }`.

| Operation | Behavior |
|---|---|
| `sender.send(v)` | Waits for capacity, then publishes |
| `sender.try_send(v)` | Never waits |
| `sender.close()` | Stops admission; receivers drain what was sent |
| `receiver.recv()` | Waits for a message; `none` once closed and drained |
| `receiver.try_recv()` | Never waits: `.value(v)`, `.empty`, or `.closed` |
| `receiver.close()` | Stops consuming; pending messages are released, senders get `channel/closed` |
| `receiver.len()` | Messages waiting |

Each message goes to exactly one receiver. Several receivers on one
channel share the work; they do not each get a copy (use
[broadcast](#broadcast) for that). Messages from one producer arrive in
the order sent. Messages from different producers have no order between
them.

Close channels explicitly. A sender that is garbage collected is not an
end-of-stream signal. `channel.open<T>(options, body)` is the scoped form:
it closes the channel when `body` returns, also on failure, and is used
with `use`: `use ends <- channel.open<Job>(channel.defaults)`.

### Capacity, byte budgets, and send errors

```badger
import std.channel
import std.io

fn report(label: String, r: Result<Unit, channel.SendError<String>>) -> Unit {
    match r {
        .ok(_) => { let _ = io.println($"{label}: sent") },
        .err(e) => { let _ = io.println($"{label}: {e.reason}, kept {e.value}") },
    }
}

pub fn main() -> Result<Unit, channel.ConfigError> {
    let ch = channel.create<String>({ capacity: 1, byte_budget: 16 })?
    report("first", ch.sender.try_send("short"))
    report("second", ch.sender.try_send("also short"))
    let _ = ch.receiver.recv()
    report("big", ch.sender.send("this message is far too long"))
    ch.sender.close()
    report("after close", ch.sender.try_send("late"))
    let bad = channel.create<String>({ ..channel.defaults, capacity: -1 })
    let _ = io.println($"{bad.is_err()}")
    ok(())
}
```

```text
first: sent
second: channel/full, kept also short
big: channel/too_large, kept this message is far too long
after close: channel/closed, kept late
true
```

| Option | Default | Meaning |
|---|---|---|
| `capacity` | 64 | Messages the channel holds. `0` makes a rendezvous channel: a send completes when a receiver takes it |
| `byte_budget` | 0 (none) | Bytes of queued messages. A single message larger than the budget fails with `channel/too_large` instead of waiting forever |

A failed send returns the unsent value in `SendError.value`, with
`reason` set to `#channel/full`, `#channel/closed`, or
`#channel/too_large`. Negative limits are a `ConfigError`.

The byte budget limits the queue, not everything the program keeps after
delivery. Heap limits are separate (`[runtime] heap_max_bytes`).

### Batch channels

`channel.batch` (and the scoped `channel.open_batch`) stage appended
messages and publish them in sealed batches. `sender.append(v)` stages,
`sender.flush()` publishes, `receiver.recv_batch()` returns a leased
`Batch<T>`, and `batch.release()` returns its page. The lease is linear:
every path that keeps it must release it or move it on (E0413). `examples/09_batch_channels.bdg`
is a complete program; [std.channel](../api/channel.md) has every option.

## Select one of several operations

```badger
import std.channel
import std.io

pub fn main() -> Result<Unit, channel.ConfigError> {
    let fast = channel.create<String>(channel.defaults)?
    let slow = channel.create<String>(channel.defaults)?
    let _ = slow.sender.send("from slow")
    let ops = [channel.Op.recv(fast.receiver), channel.Op.recv(slow.receiver)]
    for _ in 0..2 {
        match channel.select(ops, some(10ms)) {
            .received(i, v) => { let _ = io.println($"op {i}: {v}") },
            .timeout => { let _ = io.println("timeout") },
            _ => (),
        }
    }
    ok(())
}
```

```text
op 1: some(from slow)
timeout
```

`channel.select(ops, timeout)` waits until exactly one operation commits.
Operations are `Op.recv(r)`, `Op.send(s, v)`, and `Op.recv_batch(r)`; all
carry the same message type. The timeout is `some(duration)` or `none`.

| `Selected<T>` | Meaning |
|---|---|
| `.received(index, value)` | A receive committed; `value` is `none` if that channel closed |
| `.sent(index)` | A send committed |
| `.closed(index)` | A send's channel is closed; nothing was sent |
| `.too_large(index)` | A send's value exceeds the channel's byte budget |
| `.timeout` | Nothing committed in time |

| Variant | Use it when |
|---|---|
| `channel.select` | Ready operations rotate fairly. Requires `T: Share` |
| `channel.select_priority` | The earliest listed ready operation wins |
| `channel.select_keep` | Payloads are affine: losing sends are handed back |
| `channel.choose` | Operations carry different message types |

`channel.choose` takes cases built with `on_recv`, `on_send`,
`on_send_keep`, and `on_recv_batch`. Each case maps its outcome into one
type you choose, usually an enum:

```badger
import std.channel
import std.io

enum Event {
    tick(n: Int),
    word(w: String),
    closed,
}

fn from_int(m: Option<Int>) -> Event {
    match m {
        .some(n) => Event.tick(n),
        .none => Event.closed,
    }
}

fn from_text(m: Option<String>) -> Event {
    match m {
        .some(w) => Event.word(w),
        .none => Event.closed,
    }
}

pub fn main() -> Result<Unit, channel.ConfigError> {
    let ticks = channel.create<Int>(channel.defaults)?
    let words = channel.create<String>(channel.defaults)?
    let _ = words.sender.send("hi")
    let next = channel.choose([
        channel.on_recv(ticks.receiver, from_int),
        channel.on_recv(words.receiver, from_text),
    ], some(10ms))
    match next {
        .some(.word(w)) => { let _ = io.println($"word {w}") },
        .some(.tick(n)) => { let _ = io.println($"tick {n}") },
        .some(.closed) => { let _ = io.println("closed") },
        .none => { let _ = io.println("timeout") },
    }
    ok(())
}
```

```text
word hi
```

`choose` returns `none` on timeout, and also when no case can commit any
more (every send case's channel is closed).

## Replies and `channel.call`

```badger
import std.channel
import std.io
import std.task

type Lookup = { key: String, reply: channel.ReplySender<Int> }

fn serve(requests: channel.Receiver<Lookup>) -> Unit {
    loop {
        match requests.recv() {
            .some(req) => { let _ = req.reply.send(req.key.len()) },
            .none => break,
        }
    }
}

pub fn main() -> Result<Unit, channel.ConfigError> {
    let lookups = channel.create<Lookup>(channel.defaults)?
    use scope <- task.scope()
    let server = scope.spawn(fn() { serve(lookups.receiver) })
    let answer = channel.call(lookups.sender, fn(r) { { key: "badger", reply: r } })
    let _ = io.println($"{answer}")
    lookups.sender.close()
    server.join()
    ok(())
}
```

```text
some(6)
```

A request carries its own reply sender. `channel.call(to, make)` creates a
one-shot reply, sends `make(reply_sender)`, and waits for the answer.

- It returns `none` if the request cannot be sent, or if the server closes
  the reply sender without answering.
- `channel.reply<T>()` makes a reply by hand: `{ sender, receiver }`. The
  sender sends once; a second send, or one after `close`, is a `SendError`.
- To have many requests in flight from one task, send each with its own
  reply first, then receive the replies. No task per request is needed.
- `channel.route(senders, key)` picks a shard's sender by key, so one key
  always reaches the same shard.

## Broadcast

```badger
import std.broadcast
import std.io

pub fn main() -> Result<Unit, broadcast.Error> {
    let news = broadcast.create<String>(broadcast.defaults)?
    let first = news.subscribe()?
    let second = news.subscribe()?
    let reached = news.publish("hello")
    news.close()
    let _ = io.println($"reached {reached}")
    let _ = io.println($"first: {first.recv()}, then {first.recv()}")
    let _ = io.println($"second: {second.recv()}")
    ok(())
}
```

```text
reached 2
first: some(hello), then none
second: some(hello)
```

Every subscriber receives each message published after it subscribed.
`publish` returns how many subscribers it reached.

| Option | Default | Meaning |
|---|---|---|
| `backlog` | 64 | Messages kept per subscriber |
| `policy` | `#drop_oldest` | When a backlog is full: `#drop_oldest` drops the oldest message and counts it in `subscription.lagged()`; `#block` makes `publish` wait |

- `broadcaster.close()` ends the broadcast: subscribers drain what they
  have, then `recv` returns `none`. Subscribing afterwards is an error.
- `subscription.close()` leaves: later publishes skip it and, under
  `#block`, stop waiting for it. A subscription that is only dropped stays
  subscribed.
- Payloads must be `Share` and `Transfer`.

## Read a channel as a stream

```badger
import std.channel
import std.io
import std.stream
import std.task

pub fn main() -> Result<Unit, channel.ConfigError> {
    let words = channel.create<String>(channel.defaults)?
    use scope <- task.scope()
    let producer = scope.spawn(fn() {
        for w in ["alpha", "beta", "gamma"] {
            let _ = words.sender.send(w)
        }
        words.sender.close()
    })
    for w in stream.from_receiver(words.receiver) {
        let _ = io.println(w)
    }
    producer.join()
    ok(())
}
```

```text
alpha
beta
gamma
```

`stream.from_receiver(r)` yields messages until the channel is closed and
drained. A stream is consumed as it is read; a loop that stops early leaves
the rest readable. Closing the stream, or a stream operation that stops at
an error (`try_map` then `try_collect`), closes the receiver, so the
producer's next send fails and it can stop. [Chapter 4](04-collections.md)
compares `Stream` with `Seq`; [std.stream](../api/stream.md) lists the
operations.

## Deadlocks

The compiler warns about three patterns that block on every run:

| Code | Pattern | Fix |
|---|---|---|
| E0450 | A task receives until a channel is closed, and is joined before that channel's sender is closed | Close the sender before joining |
| E0451 | A task waits to receive, and is joined before anything is sent on that channel | Send before joining |
| E0452 | More messages are sent on a fresh channel than it holds, before any task could receive | Start the receiving task first, or use `try_send` |

This program checks with an E0450 warning:

```badger
import std.channel
import std.io
import std.task

pub fn main() -> Result<Unit, channel.ConfigError> {
    let jobs = channel.create<Int>(channel.defaults)?
    use scope <- task.scope()
    let worker = scope.spawn(fn() {
        var total = 0
        loop {
            match jobs.receiver.recv() {
                .some(n) => { total = total + n },
                .none => break,
            }
        }
        total
    })
    let _ = jobs.sender.send(1)
    let total = worker.join()
    jobs.sender.close()
    let _ = io.println($"{total}")
    ok(())
}
```

```text
[E0450] Warning: `worker` receives from `jobs` until it is closed, but `jobs.sender.close()` comes after this join
```

Running it ends with the runtime's deadlock report. When every task is
blocked, the program fails and names what each task waits for:

```text
panic: deadlock: all tasks are blocked
  note: task 1 (main) waits on join (joining task 2)
  note: task 2 (child of task 1) waits on channel receive (channel #1)
```

What the compiler guarantees regardless of warnings:

- No data races: values crossing tasks must be `Transfer`, and shared
  mutable state exists only as `Atom` (see [chapter 7](07-effects-and-ownership.md)).
- No lock-order deadlocks: there are no locks in the language.
- No leaked children: every scope waits for or cancels its tasks.

`join` hands the task's value to the caller. A task whose value is affine
(a buffer, a builder, a `@move_only` value) is consumed by its join, so
only one caller owns the value; joining it again is E0404 (E0415 for a
`@move_only` value). A task with a shareable value may be joined any
number of times.

## Workers

Tasks run on worker threads. The count comes from `badger run --workers N`,
`[runtime] workers` in `badger.toml`, or the `BADGER_WORKERS` environment
variable; the default is 1. One worker is a supported mode, not a different
semantics: the transfer rules apply either way. `task.worker_count()`
returns the count.

Next: [Effects and ownership](07-effects-and-ownership.md).
