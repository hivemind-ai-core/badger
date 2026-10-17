# G3a: sweep helper profiling and reuse evaluation

## Decision

Retain scoped helpers in ordinary builds. The bounded reusable-helper prototype
remains available only in diagnostic builds with `BADGER_GC_SWEEP_REUSE=1`.
The ordinary cached-helper candidate is **rejected as a default**: fewer native
thread starts and some strong allocation-heavy gains did not translate into
consistent application improvements. The eight-worker CPU slowdown repeats at
4.1% in a separate 15-repeat comparison. This is a completed evaluation, not a
completed production G3 optimization. G3b and the broader G3 box remain open.

The ordinary scoped sweep body is restored verbatim from main `36091e7`;
[normal-policy-restoration.json](G3-validation/normal-policy-restoration.json)
records its source-body hash. Experimental threads, queue state and shutdown
hooks compile only with `gc-profile` or unit tests. Ordinary builds keep the
existing threshold, per-worker ownership, finalizer order and helper lifetime.

## Integration and provenance

- Completed G1a/G2 was merged to main at `5d25647` after integrating `35bf751`.
  A guard preserved all 236 then-current dirty/untracked paths and the complete
  staged diff. No G3 candidate was merged to main.
- Main subsequently committed its archive cleanup at `36091e7`; it was merged
  into this branch at `b3602ac` before the ordinary comparisons. The rebuilt
  baseline is byte-identical to `5d25647`, as expected from the documentation
  and example-only changes.
- Ordinary baseline: `36091e71e8b22525747bc9bec4b6a4401cf535b0`; rejected cached
  candidate: `564b49238886d38543db0360878d0ce7f7a937bc`. Immutable executable
  hashes, diagnostic revisions and build details are in
  [G3-binaries.json](G3-binaries.json). The rejected candidate remains in Git
  history; its timings are not claims about the final scoped default.
- Runtime artifacts were cleaned/rebuilt across worktrees. Procfs behavior
  probes independently distinguish the cached candidate from the baseline and
  reject diagnostic-feature executables in ordinary comparisons.
- One warmup/cell, shuffled fresh processes, independent checksums/payload
  validation, cleared inherited `BADGER_*`, fixed task topology. Work elapsed
  excludes frontend/JIT; clean-launcher peak RSS and process CPU include them.
  Shared-host directional results retain outliers and full ranges.

| Matrix | Measured runs | Evidence |
|---|---:|---|
| Initial application baseline | 48 | [G3-baseline/summary.md](G3-baseline/summary.md) |
| Sweep work census and build controls | 144 | [G3-sweep-telemetry/summary.md](G3-sweep-telemetry/summary.md) |
| Scoped/reused diagnostic policies | 160 | [G3-helper-policies/summary.md](G3-helper-policies/summary.md) |
| Stronger parallel coverage baseline | 24 | [G3-parallel-baseline/summary.md](G3-parallel-baseline/summary.md) |
| Ordinary application comparison | 224 | [G3-applications/summary.md](G3-applications/summary.md) |
| 18-case ordinary coverage comparison | 720 | [G3-coverage/summary.md](G3-coverage/summary.md) |
| CPU/fanout ordinary follow-up | 120 | [G3-applications-followup/summary.md](G3-applications-followup/summary.md) |

All 1,440 measured runs pass their independent oracles. Raw gzip reports
pin sources, executables, host/toolchain, exact arguments and every run. Validation
smokes after restoring the default are separate from these performance claims.

## Profiling and candidate

The fresh phase-only census records 825 helper jobs during workload collections;
only six have fewer than 4,096 objects. Most application work is reasonably
balanced across active workers. Suppressing tiny jobs is therefore not the
leading opportunity in this data.

The prototype lazily caches one native helper per worker slot, bounded by
`MAX_WORKERS`. Capacity-one job/result channels permit one outstanding job per
helper. All results are received before serial collector-thread finalizers or
worker resumption. Idle helpers hold no managed pointers. Finish closes and
joins the cache; late collections fall back to scoped helpers and cannot reopen
it. The isolated regression tests sparse worker indices, old prefixes, sticky
and cleared marks, exact finalizer order/thread, reuse and shutdown/fallback.

Five-repeat diagnostic policy comparisons use one executable, both settings,
262,144 events/profile and 1/2/4/8 workers. Diagnostic timings are perturbed and
do not substitute for the ordinary comparison:

| Profile | Workers | Median native threads created: scoped / cached |
|---|---:|---:|
| backpressure | 8 | 154 / 7 |
| balanced | 8 | 154 / 7 |
| cpu | 8 | 154 / 7 |
| fanout | 8 | 287 / 7 |

These are successful native thread creations, not object/allocation counts.
Managed allocation pressure and collection cadence are unchanged by the policy.

## Ordinary application comparison

Seven repetitions, 262,144 events/profile; negative changes mean less time/RSS.

| Profile | Workers | Work change | GC pause change | RSS MiB base / candidate |
|---|---:|---:|---:|---:|
| backpressure | 1 | -1.8% | +0.3% | 56.8 / 57.1 |
| backpressure | 2 | +0.4% | -11.1% | 63.7 / 63.4 |
| backpressure | 4 | -1.8% | -7.9% | 68.1 / 68.0 |
| backpressure | 8 | -3.2% | -6.4% | 78.7 / 78.9 |
| balanced | 1 | +1.3% | +6.2% | 85.3 / 85.4 |
| balanced | 2 | +1.1% | +6.1% | 95.3 / 97.3 |
| balanced | 4 | +0.0% | +2.1% | 99.4 / 97.7 |
| balanced | 8 | +0.3% | -1.7% | 105.2 / 99.3 |
| cpu | 1 | -3.2% | +0.5% | 80.7 / 81.0 |
| cpu | 2 | -1.7% | -0.7% | 89.4 / 89.0 |
| cpu | 4 | +1.6% | +2.6% | 99.6 / 98.3 |
| cpu | 8 | +4.1% | +3.3% | 126.9 / 124.5 |
| fanout | 1 | +0.7% | +0.8% | 88.2 / 88.1 |
| fanout | 2 | +4.4% | -0.7% | 107.4 / 106.1 |
| fanout | 4 | -4.2% | -9.4% | 125.0 / 122.3 |
| fanout | 8 | +5.3% | +0.6% | 108.3 / 116.8 |

Backpressure shows modest gains; balanced results are nearly flat. CPU/fanout
regressions motivated a separate 15-repeat follow-up at 2/8 workers:

| Profile | Workers | Work change | GC pause change | RSS change |
|---|---:|---:|---:|---:|
| cpu | 2 | +1.2% | -3.9% | +1.6% |
| cpu | 8 | +4.1% | -2.6% | -1.3% |
| fanout | 2 | -1.2% | -1.2% | +0.5% |
| fanout | 8 | -3.5% | -6.5% | +0.6% |

Fanout regressions in the first matrix do not repeat, but CPU/8 remains +4.1%.
Its GC pause median is 2.6% lower in the follow-up, demonstrating that less GC
wall time does not by itself guarantee lower application elapsed time. The
counters do not isolate the remaining slowdown. Process counters also show
substantial kernel/scheduling costs beyond helper creation; source inspection
alone cannot attribute all of those to particular locks or wake paths.

Latency bounds and RSS are mixed. In the first fanout/8 cell, ending reserved
pools are 36 versus 42 MiB; in the follow-up RSS is only 0.6% higher. Process RSS
is not an isolated native-cache memory measurement. There is no accepted broad
memory or tail-latency improvement.

## Coverage improvement

Added fixed-eight-task `parallel-maps` and `parallel-snapshots`; the earlier
collection cases execute on one language task. The snapshot case now validates
every retained key/string against its capture step after churn. Root lengths
alone could miss reclaimed descendants. Setup, joins and validation are timed.
An initial shallow exploratory baseline was replaced before accepted comparisons;
`G3-parallel-baseline/` uses the stronger checks and the same workload source as
the ordinary pair. Historical application/coverage baselines retain their own
workload hashes and must not be compared as though source revisions were equal.

| Case | Workers | Work change | GC pause change | RSS MiB change |
|---|---:|---:|---:|---:|
| parallel-maps | 1 | -7.2% | -14.3% | +0.2 |
| parallel-maps | 2 | -1.8% | -13.0% | +0.1 |
| parallel-maps | 4 | -5.4% | -13.9% | -0.5 |
| parallel-maps | 8 | -12.1% | -32.5% | +5.8 |
| parallel-snapshots | 1 | -1.9% | +3.0% | -0.2 |
| parallel-snapshots | 2 | -7.2% | -16.2% | +0.6 |
| parallel-snapshots | 4 | -3.1% | -7.1% | +0.9 |
| parallel-snapshots | 8 | -1.5% | -6.1% | -0.4 |

The cached candidate helps concurrent allocation, especially maps/8, but serial
controls also move by several percent despite creating no helpers. Those serial
changes cannot be attributed to helper reuse. Keep M3 open; do not generalize
a favorable isolated case into a universal application claim.

## Validation and next steps

Both the rejected candidate and restored-default tree are checked; details and
logs are in [G3-validation/](G3-validation/). Runtime tests exercise the prototype
in isolated children independently of whether it is compiled into ordinary builds.
The ordinary restoration probe confirms zero cached helpers and absent diagnostic
files, and the original normal sweep-body hash is checked against main.

G3b should evaluate cheaper batch publication/completion and bounded sweep scratch
reuse before trying another default. G1b native-buffer attribution and G2c pool
reservation remain priorities. Allocation elimination, proven descendant ownership
and request/message regions target pressure itself; helper reuse does not.
