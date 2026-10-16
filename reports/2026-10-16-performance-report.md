# Performance report: correctness baseline to optimized runtime

Dated 16 October 2026. Compares the baseline recorded after the
correctness work (`reports/benchmarks/2026-10-15-baseline`, revision
`f0914d61f0`) with the current tree (`reports/benchmarks/2026-10-16-optimized`,
revision `297cc75222`). Both were produced by `bench/baseline.py` on the same
machine (AMD Ryzen 7 8745HS, 16 logical CPUs), each figure the median of three
fresh processes after a warmup. The intermediate measurement after the
runtime-lock work is `reports/benchmarks/2026-10-16-after-lock`.

Every change below landed as its own commit on `main` after the full suite
(build, all tests, clippy `-D warnings`, rustfmt, `lang format --check`);
changes to rooting, memory reuse, or scheduling were additionally run over
the whole program corpus under `LANG_GC_STRESS=1` (debug and optimized code),
under `LANG_GC_VERIFY=1` with chaos scheduling on 4 workers, under chaos
scheduling on 8 workers, and through randomized stress soaks.

## Summary

| Area | Before | After | Change |
|---|---|---|---|
| Check an empty program | 126 ms | 20 ms | 6.3x |
| Check a 5,600-line program (`synthetic_800`) | 591 ms | 180 ms | 3.3x |
| Paged list iteration | 6 ns/element | 1 ns/element | 6x |
| Persistent map insert | 2,201 ns | 442 ns | 5.0x |
| Map lookup | 152 ns | 67 ns | 2.3x |
| Allocation-heavy loop (persistent append of records) | 105 ns/record | 40 ns/record | 2.6x |
| Collector pause total for that loop | 96 ms | 23 ms | 4.1x |
| Task spawn + join | 10.9 us | 0.57 us | 19x |
| Prompt channel send/receive, 8 workers | 362 ns | 53 ns | 6.8x |
| Prompt round trip, 8 workers | 5,489 ns | 332 ns | 16.5x |
| Four independent producer/consumer pairs, 8 workers | 130 ms | 8.4 ms | 15.5x |
| Four allocating tasks, 8 workers | 320 ms | 39 ms | 8.1x |
| CPU-bound generated code (`parallel`) | 656 / 167 ms (1 / 8 workers) | 654 / 169 ms | unchanged |

The headline result for concurrency is that adding workers no longer makes
programs slower: before, every benchmark that communicated or allocated got
slower with more workers (messages 45 ms on 1 worker, 234 ms on 8); now they
are flat or faster.

## Compiler

Wall time of `lang compile` (parse, resolve, type check, lower to MIR):

| Input | lines | before | after | parse | typecheck | lower |
|---|---|---|---|---|---|---|
| minimal | 1 | 126.2 | 20.0 | 26.6 -> 12.9 | 91.8 -> 3.2 | 0.04 -> 0.04 |
| records | 52 | 129.8 | 23.9 | 27.8 -> 14.9 | 95.7 -> 3.8 | 0.49 -> 0.45 |
| runtime | 116 | 135.1 | 36.7 | 27.5 -> 20.7 | 96.6 -> 7.1 | 2.9 -> 2.7 |
| synthetic_50 | 357 | 147.7 | 30.2 | 31.6 -> 18.5 | 106.7 -> 5.4 | 2.0 -> 1.6 |
| synthetic_200 | 1,407 | 217.7 | 57.7 | 45.0 -> 31.5 | 150.1 -> 12.2 | 10.6 -> 6.3 |
| synthetic_800 | 5,607 | 591.3 | 180.3 | 100.1 -> 88.2 | 369.9 -> 40.2 | 95.7 -> 29.8 |

(milliseconds)

What changed:

- **Only reachable standard modules are loaded** (`ffa5cf8`): a program
  parses and checks the standard modules its imports can reach, not all of
  them.
- **Internal-constraint partitioning is skipped when nothing is pending**
  (`b1c4ef8`): every binding group used to resolve the type of every module
  value still being inferred, O(groups x values); this was most of the type
  checking time for every program, including the standard library's.
- **Line/column lookups use a line-start index** (`e0c47d6`): MIR lowering
  computes a source location for each checked operation and each lookup
  scanned the file from its start, making lowering quadratic in file size.

Parsing (pest) is now the largest phase: about 15 us per line, spread thinly
across the generated parser. A hand-written parser is the remaining large
compiler-speed item. Binary MIR still loads a program without any of these
phases (2.3 ms for the minimal program).

## Runtime and generated code

`--bench` run time (after compilation), milliseconds, 1 / 2 / 4 / 8 workers:

| Program | before | after |
|---|---|---|
| records | 118 / 119 / 118 / 119 | 112 / 113 / 113 / 113 |
| collections | 538 / 630 / 642 / 635 | 167 / 186 / 188 / 190 |
| messages | 45 / 128 / 156 / 234 | 27 / 31 / 32 / 37 |
| heap | 545 / 453 / 459 / 507 | 140 / 143 / 141 / 147 |
| runtime | 53 / 56 / 55 / 56 | 45 / 46 / 47 / 47 |
| parallel | 656 / 330 / 169 / 167 | 654 / 332 / 167 / 169 |
| independent_channels | 17 / 38 / 76 / 130 | 11 / 8 / 9 / 8 |
| parallel_alloc | 92 / 188 / 318 / 320 | 35 / 46 / 53 / 39 |

Per-operation figures (1 worker unless noted):

| Operation | before | after |
|---|---|---|
| closed record field access | 1,049 ps | 1,026 ps |
| row-generic call, 6-field shape | 2,834 ps | 2,635 ps |
| record update (allocates) | 1,078 ps | 910 ps |
| call, scalar frame | 3,503 ps | 3,056 ps |
| call, 4 rooted pointers | 6,970 ps | 5,588 ps |
| paged list append (builder) | 10 ns | 11 ns |
| paged list iterate | 6 ns | 1 ns |
| paged list iterate by chunk | 7 ns | 10 ns |
| array iterate | 2 ns | 3 ns |
| persistent map insert | 2,201 ns | 442 ns |
| map lookup | 152 ns | 67 ns |
| prompt channel, 1 / 8 workers | 87 / 362 ns | 54 / 53 ns |
| batch channel, 1 / 8 workers | 104 / 281 ns | 54 / 88 ns |
| prompt round trip, 1 / 8 workers | 350 / 5,489 ns | 269 / 332 ns |
| persistent append of a record | 105 ns | 40 ns |
| spawn + join, 1 / 8 workers | 10,865 / 4,712 ns | 571 / 584 ns |
| GC pause, max / total (heap bench) | 9.0 / 96 ms | 1.8 / 23 ms |
| GC pause, max / total (parallel_alloc, 8 workers) | 11.0 / 32 ms | 1.1 / 4.2 ms |

### What changed

Runtime lock (step 4 of the plan; see `2026-10-16-after-lock`):

- Per-task temporary roots and a current-task pointer (`8ceea4a`, `c91d5b8`),
  allocation without the lock into per-worker object lists (`5e51d2d`), and
  runtime entries generated code calls without the lock (`eacb71b`): pure
  reads, builders, allocation, list and (later) map/set operations
  (`3b8081c`). A shared list's tail is extended by an atomic slot claim.
- Prompt and batch channel fast paths that take the lock only when a
  transfer must park or wake (`1368466`, `27d3d6a`), O(1) batch occupancy,
  and per-worker run-next slots so a woken task runs on its waker's worker
  (`0cee2ae`).
- Ready work wakes one idle worker at a time instead of all of them
  (`abc1798`): lock parking in the spawn benchmark fell from 187 ms to 47 ms.

The lock still serializes parking, waking, spawning, `select`, timers and the
collector's stop-the-world; removing it entirely needs a lock-free scheduler
and was deferred. With these changes no measured program slows down with more
workers.

Memory management (memory plan F1-F3, F6, F8, P1-P3, P7, P8; plan R1, R2, R7):

- The collector marks children while tracing and sweeps worker lists in
  place, without per-object scratch vectors or a survivor copy (`7951a26`).
- Maps and sets are CHAMP tries whose nodes are managed objects (`8095496`):
  an update copies only its path (no reference counts), old versions cost
  nothing to drop, and memory pressure is the nodes' real size instead of a
  512-byte estimate per version. This replaced `imbl`.
- Lists are managed headers over a managed page directory shared between
  versions (`78f4e03`): persistent append is one 32-byte allocation in the
  common case, and no list needs a finalizer.
- Small objects come from per-worker 16-byte size-class pools refilled by
  the sweep (`a34f5e4`), instead of one `calloc`/`free` per object.
- `ok(())` and other variants whose payload is all unit share one static
  value (`489b71b`); each channel send used to allocate its result.

Scheduling:

- A task's stack is mapped when it first runs, so a burst of spawns reuses
  the stacks of tasks that finished meanwhile (`a14f718`).

Generated code (codegen audit G1, G3; plan C4):

- Root slots only for pointers live across a safe point, and no shadow frame
  for functions without any (`ad191a8`): smaller frames, fewer stores, and
  dead temporaries become collectible earlier.
- `for` over a list fetches each page once and loads its elements directly
  (`5128348`).

Tooling added along the way: `LANG_PROFILE=FILE` (a sampling profiler writing
flamegraph-compatible collapsed stacks, covering compilation and execution),
`LANG_PERF_MAP=1` (a perf symbol map for generated code), and
`LANG_DUMP_ASM` (generated machine code).

## Regressions and costs

- **Heap size after the allocation benchmark is larger**: 13.7 MB live versus
  9.0 MB before. List versions share a page directory that can hold page
  slots past a version's end, and small cells are rounded up to 16 bytes.
  Small-object cells are reused but never returned to the operating system,
  so a program's resident size stays at its peak small-object footprint.
- **Iteration by chunk is slower** (7 -> 10 ns/element): `chunks()` copies
  each page into an array through per-element reads that now validate the
  list header. Plain iteration, which is the common form, is 6x faster.
- **The messaging benchmark on 8 workers is about 10% slower** than with the
  previous wake-everyone policy (measured A/B: 45 versus 41 ms, noisy), the
  cost of waking one worker at a time. Spawn-heavy work is 5x faster for it.
- **`parallel` is unchanged**: its loop is a multiply/add/remainder
  dependency chain that Cranelift already compiles to native-quality code
  (remainder by a constant is a multiply), so it is not a target.

## Not done

From the optimization documents, deferred with reasons:

- **MIR inlining and closure devirtualization (G2, G4, C5)**: the largest
  remaining generated-code opportunity (helper calls and the temporary
  `Option`/record allocations they force), and the largest change; it needs
  live-reload invalidation and a growth budget. (Since done; see the
  [closing report](2026-10-16-closing-report.md).)
- **Range facts for checks (G5), specialized comparisons (G6), tag-aware
  aggregate elimination (G7)**: depend on G2.
- **Incremental and cached checking (C1 steps 3-5, C3)**: wanted by the LSP
  and REPL more than by one-shot builds; to be revisited with the LSP.
- **A lock-free scheduler**: see above.
- **Streaming map entries (P9), inline buffer storage (P10), header
  compression (P14)**: small expected gains next to the items above.
- **Clearing dead root slots** (the next step of C4) so long-running frames
  stop retaining dead objects.

## Reproduction

```sh
cargo build --release --offline
python3 bench/baseline.py --out reports/benchmarks/NAME
LANG_PROFILE=/tmp/p.folded target/release/lang run --release bench/heap.lang
```

Build with `RUSTFLAGS="-C force-frame-pointers=yes"` for profiles that show
callers of runtime functions.
