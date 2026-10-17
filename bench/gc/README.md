GC/application attribution and coverage
=====================================

`coverage.lang` adds construction-inclusive byte/word buffer churn, a larger
survivor window, persistent-map snapshot histories, and equivalent visible,
stored, collected and eager pipelines. Checksums include retained data. The
Python oracle independently checks every workload; telemetry reuses its full
payload/routing/cache oracle. Timings include setup, mutation and consumption,
so they complement the earlier read-only microbenchmarks.

Build an ordinary fixed-revision compiler and copy it to an immutable path.
Build a separate diagnostic compiler (frame pointers and symbols matter):

```
CARGO_TARGET_DIR=/tmp/gc-diagnostic-target CARGO_PROFILE_RELEASE_DEBUG=1 RUSTFLAGS='-C force-frame-pointers=yes' cargo build --offline --release -p lang_cli -p lang_runtime --features lang_runtime/gc-profile
```

`gc-profile` adds per-collection phase timings with `LANG_GC_PROFILE=path.json`.
It records stop-world rendezvous, preparation, root enumeration, tracing,
weak-page clearing, verification, sweep/finalization, accounting, pool trimming
and finishing. Counts include examined objects, survivors, remembered objects,
external pressure/live bytes and pool reservation. Phase timings are disjoint
wall-clock intervals; parallel sweep CPU can exceed its wall time.

Only that feature accepts experimental `LANG_GC_MINOR_BYTES`,
`LANG_GC_MAX_MINORS` and `LANG_GC_TRIM=0`. They are attribution tools, not proposed
defaults. The young-budget override applies after the first collection. Normal
builds contain neither instrumentation nor these policy controls. Diagnostics
are buffered and written at exit. `LANG_PROFILE_SCOPE=runtime` excludes frontend
and JIT samples; its default retains existing whole-process profiling. With
sampling active, diagnostic sweep helper threads are sampled too.

```
python3 bench/gc/run.py --baseline /tmp/base/lang --diagnostic /tmp/diag/lang --suite telemetry --out /tmp/gc-telemetry
python3 bench/gc/run.py --baseline /tmp/base/lang --suite coverage --out /tmp/gc-coverage
python3 bench/gc/run.py --baseline /tmp/base/lang --diagnostic /tmp/diag/lang --suite policy --out /tmp/gc-policy
python3 bench/gc/run.py --baseline /tmp/base/lang --diagnostic /tmp/diag/lang --suite samples --workers 1 4 --reps 1 --out /tmp/gc-samples
python3 bench/gc/run.py --baseline /tmp/base/lang --diagnostic /tmp/diag/lang --suite smoke --workers 1 4 --reps 1 --out /tmp/gc-smoke
python3 -m unittest discover -s bench/gc -p 'test_*.py'
```

Each cell gets a warmup followed by shuffled fresh processes; the runner clears
inherited LANG settings, fixes parser topology, saves binary/source hashes and
raw runs in gzip JSON, and fails on oracle errors or timeout. Oracle outputs
are cached beside output directories by input and oracle-source hash; delete
the `oracles` directory to regenerate them. `baseline` and
`diagnostic-off` calibrate diagnostic build overhead; `diagnostic-off` versus
`diagnostic` additionally calibrate phase counters. Sampling plus lock statistics
is a separate perturbed diagnostic suite, not a throughput comparison. The
ordinary timing matrix contains no sampler or lock-stat overhead.

CLI CPU/RSS includes compilation; language elapsed time excludes it. GC phase
files span runtime execution, including configuration and output encoding;
filter collection indices to `(heap_before.collections, heap_after.collections]`
for workload attribution. CPU samples sum CPU across workers, whereas pauses
are wall time. They must not be added or treated as interchangeable percentages.
Worker sweeps keep eight parser tasks fixed. Shared-host runs are directional;
publish ranges and memory costs alongside policy comparisons.
