# Four-language baseline including Java — 2026-10-18

[Dashboard](index.html) · [stamped raw evidence](runs/2026-10-18T09-40-12.584488Z-411e9e628b1c.json) ·
[suite guide and pinned-library setup](../../bench/realworld/README.md)

Clean measured revision: `411e9e628b1c764271006a7559d3bc027bc6f7b3`. Badger was built from the worktree's
compiler sources at main revision `4b3b5e2a30e1`; the measured commit changes only
the benchmark suite and documentation. The later evidence commit leaves those
measured sources unchanged. The original three-language baseline is retained
unchanged in a separate selectable cohort, with no fabricated Java history.

## Reproduction and evidence

```sh
# Same source worktree/revision, explicit isolated compiler build:
cargo build --release --locked --offline -p badger_cli --target-dir /tmp/badger-realworld-java-target
# Install/verify pinned Gson as documented in bench/realworld/README.md first.
python3 bench/realworld/run.py --badger /tmp/badger-realworld-java-target/release/badger
```

- All six workloads at their original input sizes; four languages, 48 cells.
- 192 small-case validation processes at 1/65/257/1,025 items with independent oracles.
- 240 fresh performance processes: five per cell, 3 warmups + 5 measured iterations.
- 1,920 full-size iteration outputs checked, including warmups; zero failures.
- Three measured preparations each for Badger saved MIR, Go native code and Java classes.
- 17 harness/Badger-report/native-metrics tests passed. Java source passed
  `javac --release 21 -encoding UTF-8 -Xlint:all -Werror`.
- Dashboard tests cover four-language bars/legend/history, all eight metrics,
  worker selection, noise/exclusions and backwards compatibility with old reports.
- Retained HTML's embedded data exactly matches both raw JSON reports. Its real
  data passes rendering-logic checks for every metric and both cohorts. Visual
  browser QA remains unverified; these checks use the DOM harness.

Java runtime/compiler: `openjdk 25.0.4.1 2026-08-18` /
`javac 25.0.4.1`. Classes target Java 21. JSON library:
[Gson 2.14.0](https://github.com/google/gson/releases/tag/gson-parent-2.14.0),
SHA-256 `2cbd119bf1961c28788310963dc80ba65f58cdeec1dd139c8bdb1240faa2c36f`.
The jar is an ignored local setup artifact; the runner never downloads it.
JDK release/module/shared-library hashes, compiled-class hashes, jar identity,
commands, removed option/classpath variables and all raw timings are retained.
Runtime and library identities were verified unchanged after measurement.

Host: AMD Ryzen 7 8745HS w/ Radeon 780M Graphics, affinity to 16 logical CPUs.
Load averages before/after: `[0.63623046875, 2.97705078125, 6.82861328125]` / `[1.81494140625, 2.89208984375, 6.5751953125]`.
No dedicated quiet host or controlled power policy is claimed. 8/48 warmed
kernel cells exceed 10% process CV and are flagged in the dashboard and below.

## Directional warmed workload results

Milliseconds; median of five per-process warm medians. `*` means CV > 10%.
Do not infer JVM steady state or general language rankings from these short
application-shaped jobs. Full min/max/CV and first-execution timing remain visible.

| One consumer / serial workload | Badger | Go | Node.js | Java |
|---|---:|---:|---:|---:|
| cpu | 51.609 | 50.815 | 51.266 | 48.072 |
| reconcile | 18.913 | 3.000 | 8.189 | 2.229 * |
| json | 18.878 * | 14.588 | 6.649 | 16.264 |
| state | 8.491 | 2.069 | 3.343 * | 2.365 |
| messaging | 0.889 | 0.400 * | 11.192 | 1.945 |
| pipeline | 52.939 | 51.239 | 72.749 | 50.807 |

| CPU pipeline consumers | Badger | Go | Node.js | Java |
|---|---:|---:|---:|---:|
| 1 | 52.939 | 51.239 | 72.749 | 50.807 |
| 2 | 26.274 | 28.535 | 36.055 | 25.461 |
| 4 | 13.465 | 19.207 | 18.614 | 13.233 |
| 8 | 7.757 | 9.787 | 10.495 | 6.745 |

Java uses records, standard collections/streams, normal Gson record decoding and
serialization, and a persistent fixed thread pool. Input/result queues are both
bounded at four batches; batches contain at most 64 IDs. There are N consumers,
one producer, and a main-thread reducer. Java/Node therefore have coordinator
threads in addition to the consumer count. Default JVM compiler and GC threads
are not capped to N. This is not a strict equal total-CPU budget comparison.

## Java startup, compilation and memory

Java preparation is `javac --release 21` to `.class` files, not native AOT code.
Its median preparation duration was 428.336 ms.
JVM class loading and tiered JIT continue into startup/first/warm execution;
no separate per-workload Java JIT duration is invented. READY occurs before
fixture reads, Gson construction and pool creation. Source/JIT startup and
preparation are separate dashboard choices with documented different products.

The first pipeline kernel creates/starts the pool; subsequent iterations reuse
it. Each iteration includes bounded queue creation, task submission, drain and
future joins. Final pool shutdown is in whole-process time, like Node termination.
The same 3 warmups and 5 samples apply to every language and do not prove JVM
steady-state convergence. Longer warmup is an explicit separate cohort/study.

Median Java whole-process peak RSS (MiB), including JVM/JIT, input, allocations,
all threads and all iterations with default GC:

| One-consumer workload | Java peak RSS MiB |
|---|---:|
| cpu | 46.2 |
| reconcile | 99.1 |
| json | 508.7 |
| state | 79.5 |
| messaging | 63.9 |
| pipeline | 75.4 |

In particular, JSON's high-water RSS is not live retained heap or per-order
allocation. Tracing-GC scheduling and default heap ergonomics affect this metric;
no forced GC or heap cap was applied to normalize the result. The clean native
launcher excludes Python parent-memory inheritance from all workload RSS readings.
