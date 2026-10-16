# Cross-language comparison: 2026-10-16

[Open the interactive HTML report](comparison.html) · [Raw samples and provenance](comparison.json)

Completed all **98 supported cells across 22 execution modes and all 11 requested comparison language families**, plus Lang. There were **zero failures and zero missing runtimes**. Twelve queue cells are explicitly unsupported (Lua, LuaJIT, and jank modes).

The compiler/runtime baseline is main snapshot `cd92e94`; the benchmark implementation is commit `0540896358042c0f285c66adeea09a019d009b20`. The working tree was clean at measurement start, and all captured source hashes stayed unchanged during the run. Main continued receiving separate edits, which are outside this result.

## Measured warmed execution

Medians in milliseconds; lower is faster. These rows show one named mode per family. Every other mode, first-use timing, fresh-process wall time, compilation time, and raw warmup is in the HTML/JSON. An asterisk marks warmed CV above 10%; inspect the ranges before interpreting small differences.

| Mode | CPU | Records | Snapshots | Handoff | Batches |
| --- | ---: | ---: | ---: | ---: | ---: |
| Lang source · 1 worker(s) | 3.621 | 6.408 | 5.897* | 2.328 | 1.509 |
| Python source | 88.635 | 65.235 | 5.365 | 83.011 | 11.392 |
| Node.js source (V8 JIT) | 3.598 | 10.461* | 2.066* | 17.872 | 4.990 |
| Go native | 3.128 | 0.636* | 2.422* | 2.868 | 0.283* |
| Rust native (release) | 3.063 | 1.616 | 0.856 | 2.811* | 2.659 |
| Lua source | 12.701 | 39.890* | 2.881 | unsupported | unsupported |
| LuaJIT source | 4.244 | 23.302* | 0.456* | unsupported | unsupported |
| C++ native (-O2) | 2.272 | 1.622 | 2.027 | 7.311 | 2.411 |
| Java saved JVM classes | 2.982 | 1.380* | 2.250* | 11.243* | 1.883* |
| Clojure JVM AOT classes | 11.783* | 6.258* | 7.759 | 9.262 | 2.290 |
| Clojure · Babashka | 188.051 | 73.026* | 40.988* | 20.851* | 6.526* |
| jank native AOT (-O2) | 125.899 | 35.726 | 64.311* | unsupported | unsupported |

## What this run suggests

- **CPU:** Lang source is close to Node (3.621 vs 3.598 ms), about 1.59× the C++ time, and 24.5× faster than Python on this bounded-integer control workload.
- **Records:** Lang is 10.2× faster than Python in this run. It takes about 4.0× the Rust time and 10.1× the Go median. Java and Go allocation/GC variation is high; the table is exploratory evidence, not a precise ordering for those runtimes.
- **Immutable history:** Lang takes 6.9× the Rust time and is slightly slower than Python here. Persistent value semantics do not make this small integer-key snapshot workload faster: mutable ports copy only every 64 updates, and Lua tables naturally exploit the dense integer keys. Map size, key type, snapshot frequency, and retained history would need a parameter sweep before a general conclusion.
- **Bounded scalar handoff:** Lang is competitive with Go/Rust and 35.7× faster than Python here. This measures a single producer/consumer contract with different schedulers and queue implementations; it does not prove MPMC scaling or failure/cancellation behavior.
- **Batching:** Explicit 64-item batches reduce Lang’s warmed time from 2.328 to 1.509 ms. Go is substantially faster on this case. The language-specific adaptive batch API is deliberately outside this fixed-boundary comparison.

## Source and saved MIR

Both Lang modes use `--release` Cranelift code generation. However, the public `lang compile` command currently saves MIR without the release-only inlining/receive-splitting performed by source mode. Artifact launch does not rerun those MIR passes. This is an actual compiler-mode limitation, not a reason to infer that saved code intrinsically runs slower. No compiler/runtime changes were made for this benchmark.

| Lang, one worker | CPU warm ms | Scalar handoff warm ms | CPU fresh-process wall ms | Records fresh-process wall ms |
| --- | ---: | ---: | ---: | ---: |
| Lang source · 1 worker(s) | 3.621 | 2.328 | 54.374 | 58.580 |
| Lang saved MIR · 1 worker(s) | 4.093 | 3.885 | 18.661 | 23.725 |

Saving MIR reduced fresh-process wall time substantially in this program even though the MIR optimization level differs. `.langmir` is validated intermediate code, not an AOT native executable. Python `.pyc` and saved JVM classes are also labeled as bytecode rather than native binaries.

## Reproduction and validation

```sh
cargo build --release --locked --offline
python3 bench/compare/run.py --require-all --out reports/language-comparison/2026-10-16/comparison
python3 -m unittest discover -s bench/compare -p 'test_*.py' -v
```

- Five independent fresh processes per cell, five in-process warmups, seven measured warm iterations, three artifact preparation invocations. Cell order was shuffled with seed 20261016. Workload seed: 37.
- 490 full-size fresh-process results, 196 small-input validation processes, and 1,176 checked warmup/measurement outputs. All outputs matched the independent oracle.
- Seven harness regression tests passed; compiler checking/formatting and Node syntax checks passed. Rust and C++ ports also compiled with warnings treated as errors. The full run and smaller smoke runs executed every supported port.
- HTML tested in headless Chromium at actual viewport widths of 1440 and 500 pixels: 22 CPU bars, 16 handoff bars, workload switching, measurement switching, baseline ratios, unsupported-baseline handling, and responsive overflow checks. See [validation evidence](validation.json), [the screenshot](report.png), and [the repeatable browser check](check_report.py).
- Release host compiler built from this worktree, with its own target directory. No changes were merged into main.

Optional JDK, Clojure, Babashka, and jank distributions live only in the ignored
`bench/compare/.tools/` directory. Their download URLs and archive hashes are
retained in [tool-downloads.json](tool-downloads.json); they are not vendored
into Git or installed globally. The standard runner reports missing tools on a
machine without them; use the documented environment overrides to supply them.

This is a shared-host exploratory run. Tracing GC can defer work outside an individual timer, short loops may not reach JIT convergence, and warm samples share runtime state. Large variation is visible rather than removed. Peak memory, external I/O, sustained CPU fan-out, latency tails, cleanup/cancellation, live reload, replay, static guarantees, and AI tooling are not established here. See the [full methodology](../../../bench/compare/README.md).
