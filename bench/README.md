# Benchmarks

Measurement programs for the specification's proof/measurement gates
([spec/08](../spec/08-worked-examples.md#proofmeasurement-gates-before-implementation-claims)).
Build the toolchain in release mode and run each with optimized code:

```text
cargo build --release
target/release/lang run --release bench/records.lang
target/release/lang run --release bench/collections.lang
target/release/lang run --release bench/messages.lang
target/release/lang run --release bench/heap.lang
target/release/lang run --release bench/runtime.lang
target/release/lang run --release --workers 4 bench/parallel.lang
```

| Program | Measures |
| --- | --- |
| `records.lang` | closed-record, row-generic (specialized), and struct field access; record update |
| `collections.lang` | paged list build/iteration vs contiguous buffer/array; persistent map insert/lookup |
| `messages.lang` | prompt vs batch channel throughput; prompt round-trip latency |
| `heap.lang` | allocation with a retained window, collections and pause times, task spawn, cancellation latency under backpressure |
| `runtime.lang` | call cost with scalar frames vs frames holding rooted pointers, deep recursion, and shutdown under failure (a body defect with 200 parked tasks and 8 resources: time to settle and run every halt) |
| `parallel.lang` | CPU-bound tasks across worker threads (compare `--workers 1` and `--workers 4`) |

| `independent_channels.lang` | four producer/consumer pairs sharing nothing: runtime contention between workers |
| `parallel_alloc.lang` | four allocating tasks with bounded retained windows: allocation and collection contention |

`cargo test` only checks that these programs compile.

## Baseline

`python3 bench/baseline.py --out reports/benchmarks/NAME` records a
reproducible baseline (writes `NAME.json` and `NAME.md`): compiler phase
times (`LANG_TIMINGS=1`) for every program and generated programs of 50,
200, and 800 functions; binary MIR size and load time; and every program's
`--bench` run time on 1, 2, 4, and 8 workers, as medians of fresh runs.

## Application workload: telemetry indexer

[`telemetry.lang`](telemetry.lang) mixes bounded MPMC ingestion, 256-byte
payload hashing, spawned enrichment tasks with one-use reply channels, a hot/cold
cache actor, sharded aggregation, rolling counts, and a concurrent summary sink.
[`telemetry.py`](telemetry.py) checks an independent oracle and writes throughput,
worker scaling, latency histograms, peak RSS, heap snapshots, and process counters.
See [the design and measurement guide](telemetry.md) for profiles and metric limits.

```sh
python3 bench/telemetry.py --smoke --out /tmp/telemetry-smoke
python3 bench/telemetry.py --workers 1 2 4 8 --out reports/telemetry/run
```

This workload has its own runner; `baseline.py` keeps the existing microbenchmark
suite. `cargo test` checks `telemetry.lang` along with the other benchmark sources.
