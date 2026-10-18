# Badger / Go / Node.js / Java application benchmarks

A separate Linux suite with a Python standard-library runner for repeatable **application-shaped batch jobs**.
It writes a unique UTC-stamped JSON evidence report for each run. A **Badger
program** reads every report and generates a standalone HTML dashboard with:

- Latest head-to-head comparisons, with workload, worker and metric controls.
- History for Badger, Go, Node and Java, separated into comparable cohorts.
- Startup, first and warmed execution, throughput, whole-process duration,
  preparation/compilation, CPU time in raw evidence, and peak process RSS.
- Raw ranges, CV, revisions, tool/source hashes and machine context.

The initial retained baseline is in [`reports/realworld/`](../../reports/realworld/README.md).
This suite complements [`bench/compare`](../compare/README.md), which covers more
languages and execution modes, and [`telemetry`](../telemetry.md), which exercises
a more elaborate Badger service topology. It does not replace either suite.

## Run it

Build from the same clean revision you intend to measure. Requires Python 3.10+,
Linux, a C compiler (`cc`, instrumentation only), Go 1.21+, Node.js 22+, a matching
JDK/JRE 21+, pinned Gson 2.14.0, and the release Badger executable. The runner
downloads or installs nothing; Gson is an explicit one-time setup step. Run
commands from the repository root:

```sh
cargo build --release --locked -p badger_cli

# Java JSON library, from Maven Central; verify the exact pinned artifact.
mkdir -p bench/realworld/.tools
curl -fL -o bench/realworld/.tools/gson-2.14.0.jar \
    https://repo.maven.apache.org/maven2/com/google/code/gson/gson/2.14.0/gson-2.14.0.jar
echo '2cbd119bf1961c28788310963dc80ba65f58cdeec1dd139c8bdb1240faa2c36f  bench/realworld/.tools/gson-2.14.0.jar' | sha256sum -c -

# Correctness and plumbing only; excluded from performance charts.
python3 bench/realworld/run.py --smoke --reports /tmp/realworld-smoke/runs --html /tmp/realworld-smoke/index.html

# Full baseline: 48 cells, 5 fresh processes/cell, 3 warmups + 5 samples/process.
python3 bench/realworld/run.py
# Open reports/realworld/index.html.

# Later, rebuild the compiler and repeat exactly the same command to extend history.
# The CLI regenerates HTML by invoking this Badger program after a successful run:
target/release/badger run --release bench/realworld/report.bdg -- \
    reports/realworld/runs reports/realworld/index.html

# A shorter exploratory matrix (different config => different history cohort).
python3 bench/realworld/run.py --workloads json reconcile pipeline --workers 1 4 \
    --processes 3 --warmups 2 --samples 3 --reports /tmp/exploration/runs --html /tmp/exploration/index.html

# Meaningful measurement/reporting regression checks.
REALWORLD_BADGER="$PWD/target/release/badger" python3 -m unittest discover -s bench/realworld -p 'test_*.py' -v
node bench/realworld/test_dashboard.js
```

`--badger`, `--go`, `--node`, `--java`, and `--javac` select explicit tool executables; the default
Badger is this checkout's `target/release/badger`. The runner records their hashes;
when overriding Badger, you are responsible for building it from the recorded
source revision. `--scale N` multiplies all normal input sizes, keeping the same
contract. `--timeout` bounds each process, including compilation. A nonzero result
or oracle failure rejects the entire run, retains a stamped failed report and
excludes it from charts. Missing tools fail before a measurement run starts.
`--no-html` keeps only the evidence file. `--gson PATH` selects another location
for the same pinned jar; a wrong or missing artifact fails before measurement.
The jar is ignored in Git. The version, SHA-256 and download URL are retained in
every Java report. Use `--languages badger go node` when Java is unavailable, or
`--languages java --no-html` for a focused Java run without a Badger executable.
The report directory is dedicated to
this schema; malformed or incompatible JSON is rejected by the Badger generator.
It walks subdirectories, so archived runs in that tree are still included.

## Workload contracts

All integers remain exact in JavaScript's safe-integer domain and within signed
64-bit range, including supported `--scale`/`--seed` limits. Ports use ordinary
language collections, JSON libraries and concurrency; there are no FFI workload
kernels, forced GCs, hand SIMD, benchmark-specific compiler switches, custom
allocators or optimizations that change the work. Legal compiler elimination
is allowed. These are useful service/job components, not full deployed services.

| Workload | Default size | Shared observable work |
|---|---:|---|
| `cpu` | 120,000 IDs | Integer scoring control: start `(id+seed)%1000003`, apply `(17*x+23)%1000003` 128 times, sum every result. Models moderately expensive per-record computation; no claim about crypto, SIMD or floating point. |
| `reconcile` | 60,000 orders | Generate and materialize orders; join a 1,024-customer discount table; filter inactive orders; materialize enriched copies with integer price adjustment; group revenue by customer; weighted sum plus group cardinality. |
| `json` | 8,000 orders | Read the same generated UTF-8 NDJSON fixture outside kernel timing; split, parse into orders, enrich price by 20%, serialize and reparse every order; sum active enriched values, IDs and UTF-8 note lengths. Quotes and non-ASCII notes exercise string handling. |
| `state` | 16,384 updates | Update 1,024 keyed values; retain a complete immutable snapshot every 256 updates and a final version even on a boundary; after all writes, digest all keys/values of every retained version. Persistent maps retain roots; mutable maps copy only at snapshot boundaries. |
| `messaging` | 65,539 IDs | Producer → bounded 64-ID batches → N worker consumers → reducer. Two scoring rounds/ID, four input batches queued, partial final batch preserved. Lightweight work exposes scheduling/message overhead. |
| `pipeline` | 120,003 IDs | Same batch pipeline with 128 scoring rounds/ID. Mixes allocation, queueing, scheduling and CPU work. Workers 1/2/4/8 show whether useful work scales on this host. |

Orders have `{id, customer, amount, active, note}`. Customer is `id%1024`,
amount is `(id*31+seed)%100000`, active is `id%7!=0`. Reconciliation discounts
are `(customer*7+seed)%20`; enriched amount is
`amount*(100-discount)//100`. Groups sum enriched amounts, and the digest is
`sum((customer+1)*amount)+group_count`. State values use the same amount formula.
JSON amounts become `amount+amount//5`; the active-order digest includes
`(customer+1)*amount+id+utf8_byte_length(note)`. JSON property order is immaterial.

Badger uses paged lists/arrays, immutable records and persistent maps; Go uses
slices, structs and mutable maps; Node uses arrays, objects and Maps. Java uses
records, ArrayLists/streams, arrays and standard HashMaps with boxed keys/values. Go's
`encoding/json` typed decoding, Badger's schema decoder, Java's Gson record
decoding and JS's explicit input field checks have different invalid-input guarantees. This suite measures valid
shared inputs, including output parsing. It does not infer validation equivalence.

Java runs classes produced by `javac --release 21` on the selected JVM. The JSON
port uses [Gson 2.14.0](https://github.com/google/gson/releases/tag/gson-parent-2.14.0)
with normal `fromJson`/`toJson` record support ([official guide](https://google.github.io/gson/UserGuide.html)).
Gson construction and fixture reads are outside kernel timing; its lazy adapter
creation, parsing, serialization and UTF-8 note conversion remain inside it.
Default HotSpot tiered JIT and GC stay enabled, with no forced heap sizing or GC.
Class loading and JIT can continue into first/warm execution. Three warmups are
not a claim of JVM steady state; increase `--warmups` and `--samples` for that study.

### Parallel implementation boundaries

Badger uses scoped tasks and bounded input/result channels (capacity 4 each).
Go uses goroutines, buffered channels of capacity 4 and a wait group.
Both start N consumers and one producer per iteration. Badger `--workers` and
Go `GOMAXPROCS` equal N. One worker can run the full topology by parking tasks.

Node uses **real worker threads**, not synchronous promises presented as parallel
CPU work. A main-thread dispatcher keeps at most four materialized input batches
queued and one batch in flight per worker; result events are reduced immediately.
`postMessage` structured-clones normal arrays. Go/Badger pass references. Node's
pool starts inside the first kernel execution and is reused by later iterations;
Go/Badger reuse runtime threads but recreate tasks/channels. Node has N worker threads plus its coordinating main thread; this is a consumer
count sweep, not a strict equal total-thread or total-CPU cap. The pipeline measures
normal application choices, not identical queues, memory budgets or task APIs.
It fixes the batch boundary, per-ID work and digest, not internal scheduling.

Java uses a persistent `Executors.newFixedThreadPool(N+1)`: one producer task,
N consumer tasks, and `ArrayBlockingQueue` input/result queues of capacity 4.
Each batch is a materialized `long[]` of at most 64 IDs, passed by reference.
Empty arrays signal shutdown after all data; they are not counted as payloads.
The main thread reduces exactly the expected number of batch results, then joins
all futures. Pool construction/lazy thread starts are in the first kernel and
reuse persists through warmups/samples; final shutdown is in whole-process time.
The producer and main coordinator are additional platform threads, and JVM
compiler/GC threads keep their normal ergonomics. As with Node, N is consumer
count, not an equal total CPU/thread cap.

## Measurement boundaries and correctness

1. Measure three ordinary Badger `compile` commands (source → saved MIR) and
   three `go build -trimpath` commands and three `javac --release 21` commands. Normal
   disk/build caches stay enabled;
   first invocation can be slower. Record each command, output, wall time, peak
   RSS, and artifact hash/size. Badger's host Rust compiler build and tool
   installation are excluded. Node has no separate preparation command (N/A).
2. Independently validate every selected port/workload/worker at `n=1,65,257,1025` with
   seeds `0,991,37,7`, covering partial batches, snapshot and key boundaries.
   CPU expected values use an affine closed form; state derives last writers
   without copying maps; reconciliation/JSON use scalar direct arithmetic.
   Every warmup and measured output is also validated at the full input size.
3. Shuffle all cells for each of five fresh-process rounds, with recorded seed.
   Jobs run sequentially, including preparation. The suite holds a host-local
   lock against a second instance of itself; it does not stop other host work.
4. The parent times **process launch to observed READY** via stdout, after
   Badger compilation/module loading and before fixture reads/Node/Java pool thread
   creation. It also measures the **whole process**. Each child uses its native
   monotonic clock for the kernel and prints only after stopping that timer.
5. Each process runs three warmups plus five measured iterations. Report the
   median of the five per-process warm medians, not 25 independent observations.
   First execution, min/max and CV across processes are retained separately.
   All warmups/raw iteration values remain in JSON. CV > 10% is flagged; short
   samples/noisy cells warrant a longer run, not a precise universal ranking.
6. `wait4` records **per-process peak RSS**, user/system CPU time and context
   switches. A tiny clean native launcher measures the actual workload child,
   preventing Python's retained report/hash allocations becoming an inherited
   RSS floor. No workload logic runs in the launcher. Its setup/build identity
   is retained but excluded from workload preparation timings. Startup and whole-
   process wall time include this common extra fork/exec (usually sub-millisecond).
   There are no cumulative-child RSS maxima or sampled-process memory guesses.
   Timeout kills the whole process group. Source/tool hash changes reject the run.

Kernel timing includes input generation (except the JSON fixture), collection
construction, transformation, serialization, queue/task setup, computation,
drain and join. Normal GC stays on; Go/Node GC or pool teardown can happen later.
Badger/Go task joins are included; Node/Java final thread termination is in whole-
process time. Warm samples share allocator/heap/JIT/worker state and do not prove
steady-state convergence. Fresh process does not mean flushed OS disk caches.

**Compilation is not one comparable number.** Badger source execution is
`badger run --release --bench`; the CLI's full front-end + Cranelift duration is
retained and charted separately, within launch-to-READY. Saved MIR preparation
is only the front end and is not a native AOT build or the measured mode. Its
MIR optimization path can differ from source `--release`. Go's preparation
produces native code. Java preparation writes `.class` bytecode; it does not time
HotSpot native JIT compilation.
No separate per-workload Java JIT duration is available in this harness, so its
launch-compilation metric is N/A. Java loading and JIT remain in startup/first/warm
execution. Node parsing/compilation remains in startup and V8's lazy
JIT remains in first/warm execution; separate compile time is unavailable.

Peak RSS covers the complete process, all threads, compiler/JIT memory, fixtures,
and all iterations. It is neither live heap nor bytes allocated per operation.
For startup/compile/memory, keep these product differences visible. The dashboard
charts all metrics individually and offers no combined score.

## Honest history

Each successful JSON report is exclusively created as
`UTC-TIMESTAMP-REVISION.json`; existing files are never overwritten. A cohort
hash includes the workload and runner sources, configuration, host identity,
CPU/affinity/quota/OS, and Go/Node/Java/javac executable hashes plus the JVM runtime/library and Gson
hashes. It **excludes Badger's
compiler revision/hash**, allowing compiler improvements to extend a series.
Changing workloads, runner semantics, inputs, worker sweep, repetition counts,
control runtimes, or host makes a separate cohort. The HTML selects the latest
complete non-smoke report and displays its cohort. Failed/smoke runs are counted
and excluded. Select another cohort to inspect another environment/configuration.

Compiler/std-library source hashes, binary identity, git revision/dirty status,
exact commands, input hashes, tool versions, inherited-option removals, affinity,
load, CPU pressure, memory limit and CPU quota are retained. `BADGER_*`, `LANG_*`,
Node/Java option injection, inherited classpaths and Go runtime/build tuning are
cleared in children, including tool-version probes. Java reports fingerprint the
JDK release, module image and shared libraries, not just its small launcher; the
runner rejects runtime/library changes during measurement.
Other inherited environment/OS policy and unrelated host activity remain limits.
For publication, use a quiet machine and stable power policy; inspect raw ranges
and controls. Do not infer cross-machine regressions or sustained tail latency.

One baseline is one historical point. The suite never fabricates past runs.
HTTP, database/disk latency, durable integration effects, service availability,
latency percentiles and error recovery need separate end-to-end benchmarks.

Adding Java changes the workload/harness/control identity, so the four-language
baseline starts a new cohort. Earlier three-language JSON reports remain intact
and selectable; the dashboard shows only the languages actually measured in a
run. No Java data is backfilled into historical reports.
