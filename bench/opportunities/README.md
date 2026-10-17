These benchmarks cover the six strongest opportunities from the October 16
audit. They measure local scalar enums (`compute/options`), range/map/filter/fold
versus direct loops (`compute/sequence`), typed array access and iteration
(`compute/array_reads`, `arrays`), bounded ASCII/Unicode slices (`text`), buffer
storage/list prefix/pool/old-generation retention (runtime `opportunities`), and
unreferenced generated functions (saved MIR plus `mir_audit`). `lifetimes.lang`
captures heap snapshots after an array's last use and subsequent allocation churn.

Baseline production revision: `babfdab` (pointer Option niches already included).
Benchmark additions contain no production changes. Build each revision separately:

```
cargo build --release --offline -p lang_cli
cargo build --release --offline -p lang_runtime --example opportunities
cargo build --release --offline -p lang_codegen --example mir_audit
```

Copy `target/release/lang`, `target/release/examples/opportunities`, and
`target/release/examples/mir_audit` into an immutable directory per revision. Run:

```
python3 bench/opportunities/run.py --bin-dir /tmp/opportunity-before --out /tmp/opportunity-baseline
```

The runner validates fixture checksums, clears inherited `LANG_*` settings, warms
each cell, shuffles five repetitions, records raw ranges, source/binary hashes,
1/2/4/8 worker configurations, per-process peak RSS, and generated code/MIR sizes.
GC probes run both generational modes in fresh processes. Pool RSS is current RSS;
process peak RSS includes the deliberately retained peak. Code counts use the
compiler's generated function sizes; MIR dumps and pre-optimization CLIF alone
are not evidence of final machine-code costs. Execution timers exclude JIT;
process resource numbers include it. No CPU affinity/frequency isolation is imposed.

After implementation, interleave the original immutable binary with the new one:

```
python3 bench/opportunities/run.py --bin-dir /tmp/opportunity-after --peer-bin-dir /tmp/opportunity-before --out /tmp/opportunity-comparison
python3 bench/opportunities/compare.py /tmp/opportunity-comparison/measurements.json
```

`current` labels the optimized binary and `baseline` labels the original binary.
The final run measures both versions again in shuffled order; initial baseline
results are retained separately. Runtime/compilation diagnostics in the final
run use `current`, so compare those with the initial baseline diagnostics.

For application workloads run the existing independently checked telemetry suite
against the same immutable binary, using all four profiles, fixed eight parsers,
and workers 1/2/4/8. Keep input settings identical across revisions:

```
python3 bench/telemetry.py --lang /tmp/opportunity-before/lang --events 8192 --reps 3 --out /tmp/opportunity-baseline/telemetry
```

8192 events provides a manageable throughput/regression control, not enough
latency samples for confident tail-latency claims. Repeat at larger event counts
for application conclusions. Rerun the baseline binary interleaved with the final
binary after implementation to check host drift. Retained-object counts, RSS,
external storage, cumulative allocations, and managed pressure are distinct
measurements; do not interpret `inspect.heap().bytes` as total process memory.

The paired application runner reuses that oracle and shuffles both binaries
across all profiles and worker counts. It saves raw histograms compressed:

```
python3 bench/opportunities/telemetry_pair.py --before /tmp/opportunity-before/lang --after /tmp/opportunity-after/lang --out /tmp/opportunity-comparison/telemetry.json.gz
```
