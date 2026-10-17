# Cross-language application benchmarks

Run comparable, ordinary programs on this language and Python, Node.js, Go,
Rust, Lua, LuaJIT, C++, Java, Clojure JVM, Babashka, and jank. The runner writes a
standalone **HTML report with interactive SVG charts and tables**, plus a JSON
evidence file. There are no Python package or browser-network dependencies.
This suite lives separately from the existing language-only microbenchmarks.

The [2026-10-16 retained comparison](../../reports/language-comparison/2026-10-16/README.md)
includes the interactive report, raw evidence, tool download identities, and
interpretation of the first complete run against all requested runtimes.

```sh
cargo build --release --locked
python3 bench/compare/run.py --list
python3 bench/compare/run.py --smoke --out bench/compare/results/smoke
python3 bench/compare/run.py --out bench/compare/results/comparison
# Open bench/compare/results/comparison.html in a browser.
```

Missing runtimes are listed, never silently omitted or assigned a timing. A
wrong answer, bad timer, compilation failure, or timeout excludes that cell and
returns a nonzero exit status. Use `--require-all` to also fail on missing tools.
Unimplemented concurrency ports are explicitly **unsupported**, not failures.
Smoke runs validate correctness at 257 items and are labeled unsuitable for
performance conclusions. The runner never downloads or installs anything.

```sh
# Narrow a run by family or exact mode; both are accepted.
python3 bench/compare/run.py --languages lang python node go rust --out /tmp/comparison
python3 bench/compare/run.py --languages lang-mir-w1 cpp-native --workloads records snapshots
# More samples / longer warmup; all counts are recorded in the report.
python3 bench/compare/run.py --fresh 7 --warmups 10 --samples 15 --build-samples 3
# Regenerate an existing report without rerunning measurements.
python3 bench/compare/run.py --report-only /tmp/comparison.json --out /tmp/refreshed
python3 -m unittest discover -s bench/compare -p 'test_*.py' -v
```

## What is being compared

The contract fixes the input, operations, retained observable versions,
concurrency topology, queue budget, batch boundaries, and output. Each port uses
its normal collections and concurrency primitives. It does not force Rust into
boxed objects or require Python to simulate contiguous native structs.

| Workload | Default input | Contract and language feature |
| --- | ---: | --- |
| `cpu` | 200,000 events | For each ID, start at `(id + seed) % 1000003`, apply `x = (17*x + 23) % 1000003` eight times, sum. CPU control, calls/loops/arithmetic. |
| `records` | 100,000 records | Materialize `{id, value: (id+seed)%1000, active: id%3 != 0}`; retain originals; materialize enriched copies with `value = value*3+7`; sum active enriched values plus all original values. Records, closures, collections, allocation, immutable updates. |
| `snapshots` | 16,384 updates | Set key `id%256` to `id+seed`; retain a complete immutable snapshot every 64 updates and one final snapshot, even at an exact boundary. After all updates, sum `(key+1)*value` over every entry of every version. Persistent maps/value semantics. |
| `handoff` | 100,003 values | One producer, one consumer; send `id+seed` once, in order, through a bounded queue of 256 scalar values; drain and sum; join producer. Tasks, scheduling, message passing, backpressure. |
| `batches` | 100,003 values | Same stream and topology; fixed 64-item materialized batches, four queued batches; drain partial final batch; sum and join. Collection transfer and amortization of scheduling. |

Mutable-map languages mutate their current working map and copy **only when
retaining a snapshot**. Persistent-map languages retain roots. Both preserve
all historical versions. Copying a mutable map on every update would unfairly
penalize those languages. This workload is intentionally small enough that
full-copy snapshots are a reasonable everyday choice; it is not a claim that
one collection representation wins at every map size or retention frequency.

All calculations stay within exact integer range in JavaScript and LuaJIT and
within signed 64-bit range. Arithmetic overflow never occurs. Rust/C++ release
integer checks and the language's checked arithmetic are not identical
semantics outside this domain. Java uses immutable records, Rust/C++/Go use
plain structs, Python/JS/Lua use normal objects/tables, Clojure/jank use maps.
Compilers may eliminate intermediate work when legal; there are no special
optimization barriers, hand-written SIMD, unchecked indexing, custom
allocators, FFI workload kernels, or language-specific benchmark tuning.

The independent oracle uses a closed-form modular recurrence for CPU results,
direct arithmetic for record/queue checks, and computes the historical writer
of each map entry without constructing versioned maps. Each port first runs
at `n=1, seed=0` and `n=65, seed=991`. Every stdout record, including warmups, is
checked. Checksums detect these workload mistakes; they are not a general
proof that arbitrary programs or concurrency runtimes are correct.

## Modes and idiomatic choices

| Family | Execution modes | Queue implementation |
| --- | --- | --- |
| Lang | Source and saved `.bdgmir`, each at 1 and 4 workers; optimized JIT in every mode | Standard channel and scoped task; fixed immutable-list batches |
| Python | `.py` and explicit `.pyc` | `queue.Queue` and `threading.Thread` |
| Node | Source, ordinary V8 JIT | Async producer/consumer on object-mode `PassThrough` with backpressure |
| Go | `go run` and native `go build` | Buffered channels, goroutine; `GOMAXPROCS=4` |
| Rust | Native `rustc -O` | `sync_channel` and scoped OS thread |
| Lua | Source and matching `luac` bytecode | Unsupported: no standard parallel tasks/channels |
| LuaJIT | Source and `luajit -b` bytecode | Unsupported: no standard parallel tasks/channels |
| C++ | Native `g++ -O2`, C++20 | Conventional mutex/condition-variable bounded queue and `jthread` |
| Java 21+ | Source launcher and saved `javac` classes | Virtual producer and `ArrayBlockingQueue`; scheduler parallelism 4 |
| Clojure JVM | Source and AOT JVM classes | `future` producer and Java `ArrayBlockingQueue` |
| Babashka | SCI source execution | Same Clojure port as JVM |
| jank | Source JIT and native static AOT, `-O2` | Unsupported: no Java bounded queue; same unmodified core Clojure workloads |

`.pyc` and `.bdgmir` are **not native executables**. Python bytecode skips
parsing; saved MIR skips parsing/type checking but still invokes Cranelift.
**Current compiler-mode limit:** `badger compile` has no `--release` switch and
saves MIR without the release-only inlining/receive-splitting passes applied by
`badger run --release SOURCE`. Artifact launch does not rerun those passes. Both
ports use optimized Cranelift code generation, but the MIR input differs. These
are the real public execution modes, not a controlled comparison of identical
optimized MIR; a slower saved-MIR workload must not be blamed on artifact loading.
Java and Clojure classes also still JIT. Rust/C++ have no standard source
launcher, and Node/Babashka have no equivalent standalone saved-workload mode
in this suite; their normal native or source mode and preparation time are
reported without inventing an interpreter or compilation mode.

The common batch test deliberately uses explicit fixed lists, not the
language's adaptive `open_batch` API: adaptive flush boundaries would change
the work. See `bench/messages.bdg` for the specialized API benchmark.

Concurrency compares the **application contract**, not identical schedulers.
OS threads, goroutines, virtual threads, futures, and async streams have
different costs. The budget is queued payloads, excluding values currently
held by the producer and consumer. Node's `PassThrough` has two buffers, each
configured to half the budget; stream high-water marks are thresholds, so it
can temporarily exceed the nominal bound. Its result should not be read as an
exact-capacity queue implementation benchmark. This is called out in the HTML.

## Timing and reproducibility

1. Prepare artifacts with ordinary release options. Measure three invocations
   by default, using normal warm build caches; retain each command and output.
   Record artifact bytes/hashes. Runtime/compiler installation, the Rust host
   compiler build, and Lua clock-module setup are outside these measurements.
2. Verify the two small cases for every supported port/workload. Failed cells
   cannot enter rankings. Builds and measurements are sequential; the runner
   does not run competing benchmark jobs concurrently.
3. Run five fresh processes per cell, shuffling cell order for each round with
   a recorded deterministic seed. Record end-to-end wall time and each first
   internal execution time separately. Fresh process means fresh process state,
   **not** a flushed OS page cache or cleared compiler disk cache.
4. In a separate fresh process, run five warmups then seven measured iterations
   per cell. Retain all warmup times and outputs. Warm samples share a heap,
   allocator, JIT, and possible thread pool; these are correlated observations,
   not seven independent process trials or proof of steady-state convergence.
5. Report median, range, sample count, and coefficient of variation (CV).
   Highlight warm CV above 10%. Keep all raw samples in JSON. Ratios always
   use the same workload and timing boundary; there is no overall winner score.

Internal timing uses each runtime's monotonic wall clock and covers input
generation, allocation, computation, queue creation, task submission/start,
drain, and join. Argument parsing/module loading are outside that timer, but
inside fresh-process wall time. Lazy JIT work may happen inside the first timer.
Warmups are **inside** each process, not just discarded process launches.
Printing happens after timing. Python refcount destruction and C++/Rust
destructors run at function return and can be included; tracing GC work can
be deferred into later iterations or untimed gaps. Default GC stays enabled.
These timing boundaries are documented rather than normalized with forced GC.

Lua has no standard monotonic wall clock. A tiny `benchclock.c` module supplies
only `clock_gettime(CLOCK_MONOTONIC)` to Lua/LuaJIT. It contains no workload
implementation. jank similarly uses C++ interop only for clock instrumentation.
Lua CPU time from `os.clock` would not be an apples-to-apples wall-time measure.

Source hashes, compiler source hashes, revision, dirty status, tool versions,
commands, explicit runtime environment overrides, machine/CPU/affinity, and
host load before/after are retained. Source changes during a run fail the run.
jank JIT and AOT use separate target directories to prevent AOT objects being
loaded in place of source JIT compilation. `BADGER_*` profiling/tuning variables and
common runtime option injection variables are removed from child environments.
Other inherited OS configuration remains part of the recorded machine context.
For publishable measurements, stop unrelated builds/workloads and use a stable
power policy; rerun noisy cells. No cross-machine or tail-latency claim is made.

## Tool discovery and isolated setup

`run.py` checks explicit overrides, then the suite's ignored `.tools/` directory
for locally downloaded JVM/Babashka/jank tools, then PATH. It never alters main,
the global PATH, or system packages. Babashka discovery validates that `bb` is
actually Babashka rather than a different executable with that name.

| Override | Meaning |
| --- | --- |
| `COMPARE_LANG` | Release `badger` binary; defaults to this worktree's `target/release/badger` |
| `COMPARE_PYTHON`, `COMPARE_NODE`, `COMPARE_GO`, `COMPARE_RUSTC`, `COMPARE_CXX` | Runtime/compiler executable paths |
| `COMPARE_LUA`, `COMPARE_LUAC`, `COMPARE_LUAJIT` | Matching Lua runtime/bytecode compiler paths |
| `COMPARE_CC`, `COMPARE_LUA_INCLUDE`, `COMPARE_LUAJIT_INCLUDE` | C compiler and Lua header directories for clock module |
| `COMPARE_JAVA`, `COMPARE_JAVAC` | Matching JDK 21+ binaries |
| `COMPARE_CLOJURE_CP` | Classpath containing Clojure, spec.alpha, core.specs.alpha JARs; enables JVM source and AOT modes |
| `COMPARE_CLOJURE` | Clojure CLI executable for source mode when no JAR classpath is supplied |
| `COMPARE_BB`, `COMPARE_JANK` | Babashka and jank executable paths |

On this Linux worktree, optional runtimes were downloaded into `.tools/` only.
They are ignored and not part of the source change. Exact versions and hashes
for a retained run are in its evidence JSON and companion run notes. Reinstall
from the official distributions when reproducing on another machine:

- [Python bytecode compilation](https://docs.python.org/3/library/py_compile.html)
- [Babashka releases](https://github.com/babashka/babashka/releases)
- [Temurin JDK releases](https://github.com/adoptium/temurin21-binaries/releases)
- [Clojure downloads](https://clojure.org/releases/downloads)
- [jank installation](https://book.jank-lang.org/getting-started/01-installation.html),
  [AOT output](https://book.jank-lang.org/project/aot.html),
  [native function interop](https://book.jank-lang.org/cpp-interop/native-functions.html)

## What this does not establish

This is a first, bounded performance comparison of implemented primitives.
Records/collections/history and lightweight bounded tasks are core features,
but performance is only one selling point. Static error/effect checking, typed
systems/interceptors, cancellation and cleanup on failures, live reload,
record/replay, capability boundaries, and AI-facing inspection need separate
semantic acceptance tests or application benchmarks. Timing a success-path
queue is not proof of those guarantees. HTTP, disk, database, JSON parsing,
peak RSS, deployment size, sustained multicore CPU scaling, and latency tails
are outside this version. Do not infer them from these tables.
