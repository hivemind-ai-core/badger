# Minimum runtime features to host these libraries

> This is a follow-up to the seven deep-dives in this folder. The question:
> *What runtime features does a programming language need if it wants to author libraries of these shapes as part of its blessed extended standard library?*

The seven reference libraries are: **Leiningen**, **Component**, **Ring**, **core.async**, **Instaparse**, **shadow-cljs**, **Reitit**. Below I extract the *minimal set of runtime capabilities* they require. I split the answer into three layers:

1. **Layer A — Runtime primitives** (things the *evaluator* must expose; not part of any library).
2. **Layer B — Minimal stdlib** (the smallest blessed library that makes the seven libraries feasible).
3. **Layer C — Optional ergonomic / performance features** (highly recommended but replaceable).

I close with a check: for each of the seven libraries, which layers are mandatory vs. optional.

---

## A. Runtime primitives (the evaluator must provide these)

These are not libraries — they're things the *language evaluation model* provides for free (or must be defined into the spec). If your language has them, the libraries are possible. If any is missing, you must either add a less ergonomic version of one library, or invent a different shape for it.

### A1. First-class functions with closures

**Why it's essential:** Every library uses `fn` as a value.

- Ring handlers: `(fn [request] response)`.
- Reitit handlers: `(fn [{:keys [params]}] ...)`.
- core.async `thread` / `go` blocks are functions (state machines) whose body is a closure capturing locals.
- Component constructors return functions (and use closures inside `start`/`stop`).
- Instaparse's `insta/transform` maps tags to transformer functions (`{:add +, :number read-string}`).

What specifically:

- `fn` is a value — it can be stored, returned, passed.
- Closures capture local environment by reference (for cyclic references like `defrecord`).
- Variadic arguments (Ring middleware has unknown arity).
- `apply`, `partial`, `comp`, `juxt`, threading macros are *convention*; the runtime only needs first-class functions.

### A2. Persistent / value-typed collections with structural sharing

**Why it's essential:** Half of the API surface of the seven libraries is *plain Clojure data*.

- Ring: request map, response map.
- Component: system map of components with assoc'd dependencies.
- Reitit: route tree (nested vectors/maps) — *the very API of the library*.
- core.async: a channel is a value; `(do-alts [ch1 ch2 timeout])` is data.
- Instaparse: a parse tree is `(tag child1 …)`.
- Leiningen: `(defproject …)` returns a *map*.
- shadow-cljs: `shadow-cljs.edn` is data.

What specifically:

- `map`, `vector`, `set`, `seq` are values, comparable by equality, with efficient structural sharing (so `(assoc m k v)` is O(log n)).
- All collections support `assoc`, `dissoc`, `get`, `get-in`, `update`, `merge` on maps; `conj` everywhere; `nth`/count on vectors.
- Iteration via lazy seq (concat/seq) so transformed views don't realize intermediate maps.
- Iteration **never** mutates the underlying collection (immutability by default).

### A3. Metadata

**Why it's essential:** Used pervasively to attach information *alongside* data without changing equality semantics.

- Component: stores dependency graph in metadata `(with-meta component {:dependencies [:db :scheduler]})`.
- Instaparse: `insta/span` returns `[:node start-idx end-idx]`, and `:instaparse.gll/start-index`, `:instaparse.gll/start-line` are attached as metadata to every tree node.
- Reitit: route data lives alongside path-tuple.
- core.async: `(meta ch)` carries buffer config (`{:buf … :ex-handler …}`).

Without metadata, every API has to either: (a) replace it with redundant maps (verbose), or (b) replace it with subclasses / records (lossy). Either way it's worse.

The minimum: a single attachment `meta → key → value` map, equality ignoring meta, `with-meta` / `vary-meta`.

### A4. Records (a.k.a. typed structs)

**Why it's essential:** Many of the libraries define a *specific* kind of value (Component, channels, parse failures).

- Component: `defrecord Database [host port connection]`. Lifecycle impls are methods.
- core.async: `ManyToManyChannel`, `Buffer` records. (Could be implemented as plain maps with metadata, but records give you type-tag dispatch.)
- shadow-cljs: server records.

Minimum: a fixed-key map with a type-tag that can be the dispatch value in a multi-method or protocol. Generality: it's an immutable map with positional field access + accessor fns.

### A5. Type-tag dispatch via protocols / typeclasses (open extension)

**Why it's essential:** A *lot* of these libraries are "extensible abstractions over data".

- Component's `Lifecycle` protocol — every user record extends it.
- Ring's `StreamableResponseBody` protocol — every new kind of response body just extends it.
- core.async's `Buffer`, `Channel`, `WritePort`, `ReadPort`.
- shadow-cljs's `RuntimeListener`.

Without protocols: you'd reinvent the wheel with methods on records (only work for sealed types) or multi-methods. With multi-methods only, it's OK but slower and less ergonomic.

Minimum: a "thing that dispatches on type" abstraction with `extend`/`extend-protocol`/`extend-type` for user code.

### A6. Reader, macros, and compile-time code-as-data

**Why it's essential:** Most of these libraries have a syntax (data shape) that has to be *parsed* into runtime data structures.

- Leiningen: `defproject` is read by the Clojure reader as a list, with a `def` symbol and a map literal.
- Reitit: route data is read straight from Clojure data, but you can also build it from string DSL.
- Instaparse: grammar strings are read at runtime; but the *grammar language* itself is essentially parsed by the Instaparse library.
- core.async: `go`, `<<!`, `let […]` are all macros. The whole library is gated on a macro expansion that turns channel ops into state-machine instructions.
- shadow-cljs: `defparser` macro pre-compiles a grammar at compile time.
- Component: `defrecord`, `component/using`.

Without a reader/macro system, the equivalent libraries would have to define their own parsers / builders, at significant cost. In Lisps this is "free"; in other languages you'd need to fake it.

Minimum:

- A way for user code to be loaded at compile-time producing a data tree.
- A macro system: input form → output form, executed at compile time.
- Compile-time eval capability (so macros can do work).
- Either homoiconicity (form-as-data) or a syntactic way to build & manipulate ASTs (Lisp macro / Rust proc-macro / Scala macro / C macro / Python ast module).

### A7. Exceptions with arbitrary data

**Why it's essential:** `ex-info` is the Clojure way to attach data to an exception.

- Component's `start-system` wraps thrown exceptions in `ex-info` with `:system` and `:component` keys. Cleanup code reads these to figure out what was started.
- Ring middlewares catch `ex-info` and turn it into proper HTTP responses.
- core.async: timeouts raise `ex-info` with `:channel` key.
- Instaparse failures are either exception objects with rich data or values carrying `:instaparse/failure` data.

Minimum: an exception type that carries a hashmap of arbitrary data + cause + message + stack.

### A8. Concurrency primitives

This is the largest single category.

#### A8.1. Threads (lightweight or OS)

Required by:

- Ring adapter (Jetty thread per request).
- core.async: dispatching `go` blocks on a thread pool. **Could be substituted by green threads.**
- shadow-cljs: watcher thread, REPL server thread.
- Component: each component's `start`/`stop` may spawn threads.

Virtual threads / goroutines / Erlang processes are even better than OS threads here because `go` blocks can spawn 100Ks of them.

#### A8.2. Mutex / lock primitives

Required by:

- core.async internals (channel state, buffer state).
- Component if multiple threads can start/stop the same system simultaneously.

Minimum: `lock` / `unlock` atomic CAS, `with-lock`, monitor, semaphore. Or STM.

#### A8.3. Atoms / refs / single-cell mutable state

Required by:

- Component's `(def system (atom nil))` workflow.
- Reitit's tour of route data is read-only; users sometimes add `:middleware` to a route atomically.

Minimum: a swap-with-cas primitive (atom, ref, or agent).

#### A8.4. Channels / queues

Required by:

- core.async literally: `chan`, `>!`, `<!`, `alts!`.
- Instaparse: `parses` returns a *lazy sequence* of all parses — almost-but-not-quite a channel.
- shadow-cljs: REPL ↔ browser is a single bidirectional channel.

Minimum: a bounded queue with blocking `put` / `take`, optional buffering modes (unbuffered, fixed, dropping, sliding).

#### A8.5. Promises / futures

Required by:

- core.async's `promise-chan`.
- Leiningen / shadow-cljs AOT-compilation steps.

Minimum: a single-cell one-shot future.

#### A8.6. Timeouts / timers

Required by:

- core.async's `(timeout 1000)` returns a channel that closes after N ms.
- Reitit request middleware often wants a hard timeout.

Minimum: `setTimeout` / `Future.get-with-timeout` / Erlang `after` / `(recv … timeout)`.

#### A8.7. Dynamic / fluid-let / thread-local binding

Required by:

- Component might want current system as a *dynamic* var (some users do this via Stuart Sierra's reloaded workflow) — though Component itself doesn't require it.
- Ring request-binding extensions.

Minimum: a way to bind a value globally for a thread / call / dynamic extent with `with-bindings` (or Lisp's `letf` / Python's `contextvars` / Scala's `Dynamic`).

#### A8.8. Event loop or async I/O

Required by:

- Ring's `:async?` handlers and Ring on top of Aleph (Netty event loop).
- shadow-cljs's persistent browser REPL websocket.
- core.async on ClojureScript (where *only* the JS engine's event loop exists).

Minimum: either threads + blocking I/O (Boa-style, blocking sockets), or async I/O over an event loop (Tokio-style), or both.

### A9. I/O / OS abstractions

#### A9.1. Byte / character I/O streams

Required by:

- Ring's `:body` is streamed to `OutputStream`.
- Leiningen reads `project.clj` from a stream, writes `(uber)jar`.
- core.async wraps streams into channels.

Minimum: `Reader`/`InputStream`/`Writer`/`OutputStream` with the standard interfaces (read bytes, read chars, write, flush, close).

#### A9.2. Filesystem access

Required by:

- Leiningen jar packing, file walking, configuration discovery.
- shadow-cljs build cache, asset server, source-path watch.
- Component writing out config files.

Minimum: `stat`, `exists?`, `mkdir`, `delete`, `walk`, `read-text`, `write-bytes`, `with-open`. Path abstraction (slash-portable).

#### A9.3. File change watching

Required by:

- shadow-cljs `watch` mode (must re-compile when source file changes).
- Leiningen `:checkouts` mode.

Minimum: an OS-level file watcher (FSEvents / inotify / ReadDirectoryChangesW wrapped in a portable polyfill).

#### A9.4. Subprocess control

Required by:

- Leiningen forks a child JVM for `test`, `run`, `repl`.
- shadow-cljs runs `npm install` (or yarn / pnpm / bun), invokes Node, etc.
- Component's typical integration with `pm2`/`systemd` or `pkill`-based restart hooks.

Minimum: `spawn(cmd, args, opts)` returning pid + stdio streams + exit code, plus `kill`, `signal`, `wait`.

#### A9.5. TCP / UDP sockets

Required by:

- Ring's adapter.
- shadow-cljs's REPL websocket.
- core.async over sockets (`.buffered`).

Minimum: client + server sockets, nonblocking optional but useful.

#### A9.6. Path abstraction & URL parsing

Required by:

- Reitit matches `(split path "/")` after a string parse.
- Leiningen's Maven coords look like URL strings.
- shadow-cljs's `:asset-path` is `/js/main.js`.

Minimum: a portable path type (POSIX + Windows) with `join`, `dir`, `base`, `ext`. URL parsing for query strings.

### A10. FFI (foreign function interface)

Required by:

- shadow-cljs running Clojure code on the JVM *and* on JS, depending on context.
- Instaparse's regex uses the host platform's regex engine.
- core.async uses each platform's `setTimeout`-style primitive.
- Ring uses the platform's HTTP server.

Minimum: a way for code in the language to call *out* to a host (JVM methods / C functions / JS objects). Without this, you can't talk to OS / browser / database.

### A11. Runtime introspection + REPL

Required by:

- `lein repl` needs a connectable REPL.
- shadow-cljs REPL needs the same.
- Component's `user.clj` `(def system (atom nil))` REPL workflow.
- core.async: exception throws inside `go` blocks; users need to inspect what went wrong.

Minimum:

- `eval` (running a form in a context).
- `read-string` / `read`.
- A REPL with a session-protocol (nREPL-like, with evaluation interruption).
- Stack trace: location, function, file, line.
- Doc / source lookup (or your runtime equivalent).

### A12. Logging, configuration, env

Required by:

- Leiningen reads `~/.lein/profiles.clj`, env vars, system properties.
- Component may log errors with stack traces.
- shadow-cljs dump cache state with logs.

Minimum: leveled logger (`info`, `warn`, `error`), env-variable access, some way to read OS sysprops.

---

## B. Minimal standard / blessed extended library

If you implement everything above, the seven reference libraries are *possible*. To make them pleasant and compact, ship a "blessed extended stdlib" with these modules. (You don't have to put them in the base lib.)

### B1. Collection helpers

`assoc`, `dissoc`, `get-in`, `update-in`, `merge-with`, `select-keys`, `zipmap`, `frequencies`, `group-by`. Plus sequence / transducer helpers: `mapcat`, `partition`, `filter`, `remove`, `take`, `drop`, `comp`, `transduce`, `eduction`, `volatile!`. Not strictly required (most are user-replaceable) but expected by ~every library.

### B2. Reader + data readers

`symbol`, `keyword`, EDN-style data: keywords as keys, numbers (`42`, `3.14`), strings, booleans, nil, vectors `[…]`, maps `{…}`, sets `#{…}`. Plus:

- `#inst "2024-01-01"` (parsed time)
- `#uuid "…"`
- Tagged literals for spec/Schema/Malli

This is what makes Reitit routes, shadow-cljs's `shadow-cljs.edn`, and Leiningen's `project.clj` so compact — data-driven config.

### B3. Reader for tagged literals + a registry

`default-data-readers!` / `*data-readers*` so libraries can register tagged literals (`#object[...]`, `#xp/…`).

### B4. URL / path parsing

`url-decode`, `url-encode`, `parse-url`, `URL(String)`. Plus query-string parsing.

### B5. Topological sort

For Component. ~20 lines if missing, but having it in stdlib saves every user the re-write.

### B6. Process / spawn

`sh` / `cljsh` analogues. spawn & pipe.

### B7. Filesystem walk + watcher

`file-walker`, `make-observer`, the equivalent of Java NIO WatchService / inotify.

### B8. HTTP server API

This isn't strictly required to host the *Router* libraries, but Reitit / Ring / Bidi all need an embedded HTTP server. A blessed one — Aleph, http-kit, Jetty — should ship in the blessed stdlib so it doesn't get reinvented per project.

### B9. JSON / EDN / Transit codec

`read-str`, `write-str`. Reitit's `muuntaja` makes content-negotiation trivial.

### B10. A native `regexp` API

Instaparse's terminal regex support assumes the language's `regexp` API is present.

### B11. Time / Duration types + arithmetic

`(timeout 1000)` style and middleware. Most runtimes have this.

### B12. A loop / go macro and go-loop helper

`go`, `go-loop`, `thread`, `<!`, `>!`, `chan`, `buffer`, `close!`, `alts!`. The actual core.async library.

This single module alone unlocks most of what we'd recognize as "the Clojure concurrency story". It's feasible on top of A1–A8.

### B13. Macros: cond->, some->, delay, future

Sweetens library code. The libraries themselves don't require them.

---

## C. Optional / performance / ergonomic

These aren't strictly necessary to make the *shapes* possible, but they're what makes them pleasant.

### C1. Virtual threads / goroutines / fibers

Without these, `go` works but exhibits thread-pool exhaustion at high concurrency. With them, a million park-and-resume cycles are fine.

### C2. Transducers

Channels, lazy seqs, transducers all interplay. With transducers, `map`, `filter`, `partition` are composable across all those substrates. If absent, you write the transforms by hand per substrate.

### C3. nREPL-style wire protocol

Connecting a REPL client to a running process. Without it, REPL is local-only. With it, REPL ↔ editor ↔ remote runtime.

### C4. Add-lib / add-classpath

Leiningen / shadow-cljs's ability to download JARs and add them to the running classpath at runtime (JVM supports this via `URLClassLoader`).

### C5. Source-tracking / locations everywhere

`(meta form)` carries `{:line 5 :column 12 :file "..." :ns 'foo}`. Most runtimes have this; few exploit it for great stack traces.

### C6. Pluggable IOFactory

The double-handler pattern (Ring's response body, channel buffer, file-stream) all improve when there's a single `IOFactory` interface that users can `extend`.

### C7. Stack walking / debug stepping

Without this, debugging nested core.async state machines is hell. With it, the library author can pretty-print the state machine to text.

---

## Mapping back: per-library minimum

| Library | Truly required from layer A | Required from layer B |
|---|---|---|
| **Leiningen** | A1, A2, A3, A5, A6, A7, A8.1/3/4, A9.1–6 (everything!), A10 (FFI for AOT/JAR), A11 (REPL), A12 | B2, B3 (config parsing), B6 (subprocess), B7 (file walker) |
| **Component** | A1, A2, A3, A4, A5 (Lifecycle), A7 | B1, B5 (topo-sort) |
| **Ring** | A1, A2, A5 (StreamableResponseBody), A8.1, A9.1/5, A11 | B1, B8 (HTTP server), B9 (codec), B4 (URL parsing) |
| **core.async** | A1, A2, A4, A5 (Buffer/Channel), A7, **A8.1–A8.7**, A8.8 | B12 (channel/buffer primitives) |
| **Instaparse** | A1, A2 (trees), A3 (spans), A6 (macro `defparser`), A7 (failures), A11 | B10 (regexp) |
| **shadow-cljs** | A1, A2, A6, A7, **A8.1, A9.2, A9.3, A9.4, A9.5**, A10, A11 | B6, B7, B8 (asset server), B11 (timeouts in REPL), optional C3 (REPL session protocol) |
| **Reitit** | A1, A2 (the entire data model — Reitit is data), A6 (no macro strictly required) | B1, B4 (URL parsing), B8 (HTTP server for Ring adapter), B9 (codec via muuntaja) |

**One-line summary per library:**

- Component, Reitit, Instaparse are mostly *data*. They need: A1, A2, A3, A5, A6, A7.
- Leiningen, shadow-cljs are *tools*. They need: everything I/O + spawn + REPL.
- Ring, core.async need *concurrency primitives*. They need: A8 (all of it) plus A8.5 (channels / queues).

So the order in which to grow a runtime:

1. **Implement A1, A2, A3, A4, A5, A6, A7** → Component, Reitit, Instaparse become possible.
2. **Implement A8 (threads, atoms, channels, futures, timeouts)** → Ring, core.async become possible.
3. **Implement A9 (streams, FS, sockets, subprocess, watcher)** → Leiningen, shadow-cljs become possible.
4. **Add B1–B12** → make the seven libraries feel ergonomic in < 1000 lines of code each.

---

## The non-obvious bits

A few features that *seem* like they should be optional but turn out to be load-bearing:

1. **Metadata.** Without it, Component's dependency graph leaks into the user's record definition (`{:dependencies {…}}` field) instead of `with-meta`, which makes `Component`'s core example verbose. Without metadata, Instaparse's `:instaparse.gll/start-index` is just `(idx tree)` calls — every user pays.
2. **Persistent maps with structural sharing.** If your runtime gives you immutable maps *without* structural sharing (e.g., persistent only in the sense of "you can't mutate"), you're paying a deep-copy per `assoc`. Either of these — Ring request dispatch, Reitit match table, Component system construction — becomes quadratic. Persistent + sharing is the magic.
3. **Reader + macro system.** This is the single biggest enabler for "data-shaped DSLs". It is the feature that makes Leiningen, Reitit, Instaparse, and shadow-cljs configs readable. If the runtime doesn't have it, every library author has to write a parser for their config.
4. **Channels.** Without channels you'd reinvent core.async over `Future + callbacks` (which gives you Promises but loses the CSP-style "rendezvous" and "alts!" of `go` blocks). Many modern runtimes lack channels; adding them is what makes core.async-style code possible.
5. **Filesystem watcher.** A real-time source watcher is "obviously" needed for shadow-cljs's `watch`. Without it you have to reimplement FSEvents/inotify wrapping, and you can't get correct cross-platform semantics for free.
6. **Compile-time eval.** For `defparser`, `defproject`, etc. Without compile-time eval, these macros can't do their work — runtime eval is too late in CLJS.

## What's *not* required

- Tail calls (none of these libraries use them).
- Type inference (everything is dynamic).
- Reflection metadata / JVM class scanning.
- A package manager baked into the runtime — that's the stdlib/plugin territory.
- A debugger — nice but not necessary.

## Concretely: smallest viable "Clojure-like" runtime

In 12 bullets, the absolute minimum runtime + library to make all seven projects viable:

1. Persistent maps, vectors, sets, seqs with structural sharing.
2. First-class functions + closures.
3. Protocols (open polymorphism over records).
4. Records with metadata.
5. Atom + future + channel primitives.
6. Threads (or virtual threads).
7. A Reader (parse s-expressions) + macro system + compile-time eval.
8. Stream + filesystem + subprocess.
9. File watcher + socket pair.
10. ex-info-style exception with data.
11. Optional: `go`, `defparser`, `defproject` macros in a blessed `extension.async` namespace.
12. Optional: HTTP server, JSON/EDN codec, topologic sort, regex helpers in blessed `extension.web` / `extension.io` / `extension.algo` namespaces.

If your language has 1–10 you can write Component / Reitit / Instaparse in a single file. Add 11 to make core.async & defparser macros viable. Add 12 to make Ring (or equivalent), Leiningen (or equivalent), and shadow-cljs (or equivalent) viable in a day each.

The non-Lisp-version of this list reads similarly: protocols → interfaces; reader + macros → proc-macros / type-stitching; persistent maps → immutable tree maps; channels → actor mailboxes or shared memory; etc.
