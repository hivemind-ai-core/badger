# core.async

> Category on clojure-toolbox.com: **Asynchronous Programming**
> Repo: <https://github.com/clojure/core.async>
> Author: Rich Hickey and contributors (Clojure team)
> License: Eclipse Public License
> Latest stable: 1.9.865
> Dep coord: `[org.clojure/core.async "1.9.865"]`

## What it is

**core.async** is Clojure's standard library for asynchronous programming and coordination. It adds **channels** (CSP-style message queues) and **`go` blocks** (cooperative, state-machine-based "threads") to the language, without modifying Clojure itself. It is the bridge between "I have functions and atoms" and "I have producers and consumers that need to coordinate".

Its main tools are:

- **Channels** — many-to-many queues with optional buffering.
- **Put / take** — `>!` and `<!` (park) inside a `go` block, or `>!!` and `<!!` (block) on a real thread.
- **`go`** — a macro that converts its body into a state machine: whenever a blocking channel op is hit, the body parks there; when the operation completes, control resumes. Thousands of logical processes run on a small thread pool (or, in ClojureScript, on a single browser thread).
- **`alts!`** — wait on many channels at once. Crucial for timeouts and "whichever responds first".

It's used everywhere in Clojure: Pedestal uses it for event loops, Ring/Async uses it for non-blocking server logic, re-frame uses it for async flows, Datomic Cloud uses it for tx-report timeouts, and almost every websocket server uses it.

## Design

### The Rationale (paraphrased from Hickey's 2013 design doc)

> *"There comes a time in all good programs when components or subsystems must stop communicating directly with one another. … On the JVM, the j.u.c concurrent queues are a viable choice but require one or more actual threads per consumer, which limits scale. On the JS engine, there are no threads and no queues. … core.async is built on CSP. Channels are first-class blocking constructs. Bounded buffering with blocking gives back-pressure; unbounded buffering is discouraged."*

The architectural goals are:

- **Independent threads of activity** communicating via queue-like **channels**.
- **Two flavors of "threads"** — ordinary JVM threads *and* `go`-block "IOC threads" — interconvertible at channel boundaries.
- **CSP-derived semantics**, not actor semantics. Channels decouple producer from consumer: producer doesn't care who consumes; consumer doesn't know who produces.
- **ClojureScript support** — the *only* sane way to do concurrency at all in CLJS, which has no threads. `go` blocks compile down to a state machine on a single thread plus `setTimeout` for time-based ops.

### How CSP maps to core.async

| CSP / Go term | core.async |
|---|---|
| goroutine / process | `go` block / `thread` |
| channel | `chan` (with optional buffer) |
| `<-` / `->` | `<!` / `>!` (park) or `<!!` / `>!!` (block) |
| `select { case <-ch: ... }` | `(alts! [ch1 ch2 (timeout 1000)] :woken ...)` |
| `close(ch)` | `(close! ch)` |
| nil-value close sent | `nil` (cannot be sent through closed channel) |
| channel-of-channels | `chan` of `chan`s |

Crucial differences from Go:

- **All operations are expressions.** Every `>!`/`<!` returns a value (the channel it parked on, or a value), usable in `let`.
- **It's a library, not syntax.** No special keywords (`<-`); just Clojure macros.
- **`alts!` is a function.** Takes a runtime-variable number of clauses; supports priorities via position.
- **Library not runtime.** Clojure is hosted, so core.async reuses existing JVM/JS primitives rather than introducing a new runtime.

### Channels in detail

A channel is constructed with `(chan)` — unbuffered, also called a *rendezvous* — or `(chan n)` for fixed-size buffering, or `(chan (dropping-buffer n))` / `(chan (sliding-buffer n))` / `(chan (buffer 16))` for varying drop/slide policies.

Behaviors:

- An **unbuffered** channel: `>!` parks until a `take`, and vice-versa. The producer and consumer rendezvous.
- A **fixed buffer of n**: `>!` parks only when full; `<!` parks only when empty.
- A **dropping buffer**: `>!` to a full buffer is silently dropped (returns nil).
- A **sliding buffer**: takes from the *oldest* slot when the buffer is full.
- A **promise channel** (`promise-chan`): a channel that will deliver a single value then close.
- **Closing** a channel delivers `nil` to pending takers, releases blocked takers; further `>!` throws.

### The two thread flavors

- **IOC threads** (`go` blocks). User code appears sequential. The macro re-arranges the body into a state machine with explicit parking points. On parking, the engine releases the worker thread to the pool. On resume, an *arbitrary* pool thread picks it up. (Recent versions dispatch back to the originating thread when possible for better locality.) Supports JavaScript out of the box (the JS version uses a JS-side state machine + `setTimeout`).

- **Real threads** (`thread`, `thread-call`, `>!!`, `<!!`, `alts!!`). For when you actually need OS-thread semantics — usually to call into a blocking JDBC driver or a synchronous library. Newer versions use **virtual threads** when available (`io-thread`, `<!!` on JDK 21+).

You can mix freely: a `>!` from a real thread to a channel consumed by a `go` block is fine, as long as both ends agree on the buffer policy.

### `alts!` and friends

```clojure
(let [[v ch] (alts! [in-c out-c (timeout 1000)])]
  (cond
    (= ch in-c)  (handle-input v)
    (= ch out-c) (handle-output-ok)
    (= ch (timeout 0)) :timed-out))
```

Variants:

- `alts!` for `go` blocks (parking).
- `alts!!` for real threads (blocking).
- `alt!` / `alt!!` macros that fuse choice with conditional evaluation.
- `do-alts` — same as `alts!` but without the return.

### Why not actors?

From the rationale:

> *"I remain unenthusiastic about actors. They still couple the producer with the consumer. … Queues are more primitive."*

Channels are queues; queues are a fundamental decoupling primitive. Actors build on top of them.

### Other opinionated features

- **Transducers supported.** `map`, `filter`, `partition`, `transduce` work on channels.
- **`pipeline` and `pipeline-async`** — concurrent transducer pipelines with bounded parallelism over a stream from a source channel. Useful for "I have 10k events, do f on each, max 8 concurrent".
- **`mix`** — fan-out mixer that lets you route from many inputs to many outputs without threads.
- **`mult`/`tap`** — multicast (one source → N consumers).
- **Back-pressure by default.** Bounded buffers + blocking `>!` make over-load get absorbed cleanly into `>!` parks (with metrics), not crashes.
- **`unique!`** — guarantee only-the-first success on a channel (kind of `compareAndSet` for puts).
- **`flow` and `core.async.flow`** (newer, 1.10+): higher-level typed "dataflow" topologies built on top of `mix`/`mult`, with propagation semantics. *All APIs subject to change.* These are how newer Clojure dataflow projects wire things together.

## Implementation

Two halves:

- **ClojureScript side** — pure CLJS. State machine is hand-coded as a Clojure data structure (a vector of op-codes + locals) and rewritten into JavaScript. Park is a real wait (in browser or node).
- **Clojure (JVM) side** — `go` blocks are processed at macro-expand time by `clojure.core.async.impl.ioc-macros` (or, in CLJS, by `cljs.core.async.impl.ioc-macros`). The body is transformed into a JVM-state-machine-like data structure which is then `eval`'d at the call site.

### `go` macro mechanics

1. The macro looks at every expression in its body for `<!`, `>!`, `alt!`, `alts!`, `timeout`.
2. For each occurrence it inserts a state-machine "park" placeholder. Becomes roughly:
   ```
   (state id args locals continue-fn)
   ```
3. The state machine is a function with a giant `case` on state-id, each branch returning either:
   - a value (the op succeeded), or
   - a `(park fn)` — set the resume function, register on channel, return.
4. When the channel op completes, the channel delivers a `Continuation` to the state machine, which immediately re-invokes the same state function with the new locals.

The machinery is large but conceptually simple: it's a giant switch-loop manually implemented in bytecode.

### Park and dispatch

When a `go` block parks, its continuation is enqueued in the channel's waiter set. When a put or take happens on the channel, the waiting continuations are flushed:

- On a regular JVM thread: continuations go to the `clojure.core.async.dispatch` thread pool (recently, **virtual threads** via `clojure.core.async.executor-factory` if available — async/up to ~10 million logical threads).
- Older versions: a fixed-size pool (default 8 since 0.2.382, configurable via `clojure.core.async.pool-size` system property).
- ClojureScript: continuations are scheduled on the JS event loop or `setTimeout` for timeouts.

### Buffers

Buffers are protocols — `Buffer` with `full?`, `remove`, `add!` methods. The four built-in are unbuffered (rendezvous), fixed, dropping, sliding. Users can implement custom (e.g., LIFO, batching, sampling, …).

### Channels as protocols

A channel is itself a protocol — `WritePort`, `ReadPort`, `Channel` — so users can implement their own (e.g., a virtual channel that maps to a websocket, or one that logs every put). In ClojureScript, `cljs.core.async.impl.channels` provides the standard one.

### Errors

- `go` blocks have try/catch support. Exceptions are surfaced at the next `take` of the channel that the `go` block returns. As of 0.7.559 there is also `clojure.core.async.go-checking` that throws if blocking ops (`>!!`, `<!!`) appear inside `go`.

### Performance characteristics

- Channels are essentially concurrent linked queues with waiter sets. Park and wake are O(1) ops.
- A `go` block's state machine is a single function with a large `case`; closure capture for locals is only at park points, so memory usage per channel park is small.
- A modern core.async benchmark on a 64-thread JVM can park and resume millions of logical processes per second per core.

## API (in `clojure.core.async`)

Core channel & put/take:

```clojure
(chan) (chan buf-or-n)                        ; create a channel
(chan 1)                                      ; rendezvous
(chan (dropping-buffer n))
(chan (sliding-buffer n))
(chan (buffer n))

(>! ch v)     (>!! ch v)                     ; put (park / block)
(<! ch)       (<!! ch)                       ; take (park / block)
(put! ch v callback)                          ; async put w/ cb
(take! ch callback)                           ; async take w/ cb
(close! ch)

(offer! ch v)     (offer! ch v timeout-ms)   ; non-blocking w/ optional timeout
(poll! ch)        (poll! ch timeout-ms)      ; non-blocking take
```

Pipelines and combinations:

```clojure
(into-to-chan! coll ch)        ; move coll onto ch, close when done
(to-chan! coll)                 ; return a ch containing all of coll

(pipeline n out-ch xform from-ch)
(pipeline-async n out-ch afn from-ch)
(pipeline-blocking n out-ch afn from-ch)

(redduce rf init ch)
(transduce xform f init ch)
(map< xform from-ch)   ; returns new ch of (xform v) for each v from from-ch
(filter< pred from-ch)
(remove< pred from-ch)
(split< pred from-ch) ; -> [truthy-ch falsy-ch]
```

Coordinations:

```clojure
(alts! [ch1 ch2 (timeout-ms 1000)])  ; park until one is ready
(alts!! [ch1 ch2] {:default val :priority true})
(alt!! [[ch1 (do-thing1)]
        [ch2 (do-thing2)]])
(promise-chan)                       ; single-value ch
(timeout ms)                         ; auto-close channel after N ms
```

Multiplexing / fanning:

```clojure
(mult ch)                            ; one source, many consumers
(tap mult sink)                      ; subscribe
(untap mult sink)
(unsub-all mult)

(mix)                                ; merge many inputs to many outputs
(admix mix in-ch)
(unmix mix in-ch)
(toggle mix toggle?)
```

Threads and IOC:

```clojure
(go body...)                         ; spawn IOC
(go-loop bindings body...)           ; like go but re-entering
(thread body...)                     ; spawn real thread
(thread-call f a b c)
(io-thread body...)                  ; uses a virtual-thread-friendly executor
```

Transformation / error:

```clojure
(unique! ch op-fn)                   ; CAS-style first-success on ch
(map< xform from-ch)
(println "parked:" (clojure.core.async/buffers))
```

## How `go` works at the bytecode level

Quoting from a typical macroexpansion (slightly simplified):

```clojure
;; (defn fetch [url] (go (let [r (<! (http/get url))] (:body r))))
(macroexpand '(go (let [r (<! (http/get url))] (:body r))))
;; → (clojure.core.async/let
;;     [body (clojure.core.async/<! (http/get url))]
;;     (:body body))
;; The macro shadowed `let`, `<!`, etc. and arranged parking.
```

In JVM-land, the final form is roughly:

```
(fn [box state-id]
  (case state-id
    0 (let [b (clojure.core.async/<! (http/get url))]
        (state 1 {:body b}))
    1 (:body (clojure.core.async/return-chan box))))
```

When `(http/get url)` parks, the runtime saves `(state-id=0, locals={...})` inside the channel's waiter set and returns control to the engine. When the future inside `http/get` returns, it calls `(deliver-continuation ctx state-fn locals)` which re-invokes `state-fn` with the new state-id and locals.

This is essentially the same mechanism C#'s `async/await` uses (continuation monad), without language support; the macro does the work.

## How it's used in practice

### 1. Worker pool

```clojure
(def job-ch (chan 64))
(def done-ch (chan))

(defn worker [id]
  (go-loop []
    (when-let [job (<! job-ch)]
      (<! (do-job job))
      (>! done-ch {:worker id :job job})
      (recur))))

(defn start [n]
  (dotimes [i n] (worker i))
  (pipeline 8 done-ch (map :result) job-ch))   ; 8 concurrent in pipeline
```

### 2. WebSocket with timeouts

```clojure
(defn handle-socket [ws]
  (let [out (chan 32)]
    (go-loop [n 0]
      (if (< n 10000)
        (do (>! out (str "msg-" n))
            (<! (timeout 100))
            (recur (inc n)))
        (close! out)))
    (async/pipe out ws)))
```

### 3. Server request flow

```clojure
(defn handler [req]
  (go
    (let [data (<! (db/query req))
          html (<! (render data))]
      {:status 200 :body html})))
```

The `go` block converts this to a non-blocking handler that responds asynchronously via `alts!`/`<!`.

### 4. UI animation loop

```clojure
(go-loop [t 0]
  (q/clear)
  (q/point 200 200)
  (>! (q/sketch->chan) (str "frame " t))
  (<! (timeout 16)) ; 60 FPS
  (recur (inc t)))
```

## Why it matters

- It's the **standard answer** to "I need to coordinate concurrent things in Clojure/Script and I don't want actors, futures, or callbacks."
- It brought CSP — out of textbook CS — into mainstream everyday Clojure. The shape of "channels + alt" has leaked into how people design even the synchronous, single-threaded code in their head.
- It is the substrate for nearly every async DSL in the ecosystem: fwk 7 simultaneous UI animations, ring-async, manifold, missionary (which is "core.async but smarter").
- It enables the ClojureScript story. Without it, ClojureScript would be effectively single-threaded by design (which it is on the JVM too, but JVM has blocking ops as fallback).

## Caveats

- You can deadlock with ease: a `go` block waiting on each other's output is a classic bug.
- Debugging parked `go` blocks is unpleasant: call stacks are gone because the work is in a state machine. Use `flow` or a debugger.
- Park-time, not lap-time: a `go` block holding onto a 10 MB object for minutes prevents GC. Be parsimonious with closed-over state.
- Mixing blocking ops (`<!!`) inside `go` blocks silently defeats the point. Use `go-checking` to catch this.

## Sources

- <https://github.com/clojure/core.async> (README + CHANGELOG)
- <https://clojure.github.io/core.async/rationale.html> — Hickey 2013 design rationale
- <https://clojure.github.io/core.async/> — auto-generated API
- Tim Baldridge, "core.async: Patterns and Best Practices", Clojure/conj 2013 (YouTube link in README)
