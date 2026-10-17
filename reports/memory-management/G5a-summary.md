# G5a: direct Buffer/Array metadata in managed storage

## Decision

Keep the direct 64-byte managed-cell implementation at `b8b6935`. It removes
one native metadata allocation/free per buffer and an array-access pointer load.
Broader applications improve in the ordinary comparison and an independent
adjacent-pair repeat. This is a throughput/GC improvement with mixed peak RSS,
not an across-the-board memory or tail-latency guarantee. G5a is complete;
G5 and the wider memory roadmap remain open.

## Implementation and accounting

BufData occupies the managed payload directly. Objects remain nonmoving, packed
element storage stays native, pointer values retain precise tracing/barriers,
and finalization drops native element storage in place. Generated array length
and indexing load fields directly through the rooted owner. No task arena,
channel specialization, escape inference or element-storage pooling is added.

On this 64-bit target, excluding element storage and native allocator overhead:

| Layout | Managed cell | Native metadata | Nominal combined bytes |
|---|---:|---:|---:|
| Baseline | 32 | 48 | 80 |
| Embedded with compatibility pointer, intermediate `75f9c98` | 80 | 0 | 80 |
| Direct metadata, final `b8b6935` | 64 | 0 | 64 |

Managed object counts are unchanged. Managed requested bytes increase by 40 per
buffer and pooled cell bytes by 32; the former 48-byte native metadata Box is
gone. The nominal combined reduction is 16 bytes after pool rounding (8 before
rounding). Do not interpret the managed-byte increase as total allocation growth
or the nominal reduction as a process-RSS prediction. Native element storage,
GC finalizer dispatch and other native allocation sources remain.

The census independently checks all object/byte/survival conservation invariants.
For sustained integer messaging at one worker, both binaries allocate 262,209
Buffer objects. Their managed cells total 8,390,688 versus 16,781,376 bytes,
while 262,209 former native metadata allocations (12,586,032 nominal bytes) are
removed. Empty-buffer coverage allocates 524,288 managed objects in both cases.
The native unit test checks that only element capacity is externally charged;
this remains a managed census plus a scoped metadata accounting proof, not full
native malloc interception or language allocation-site/escape attribution.
[Exact counts](G5a-validation/buffer-census-comparison.json).

## ABI and correctness

Array field loads change, so saved MIR advances from format 5 to 6. Former files
are refused with E0011 and a compile-again message before code execution; users
must rebuild them. The checker compiles a real format-5 file with the baseline
and verifies rejection. A native test also guards rejection before payload decode.

Validation: 9 default and 11 diagnostic runtime tests; 3 MIR tests; all 66 CLI/
program tests including source and compiled corpora; 24 dedicated packed/
empty/growing/pointer-buffer GC stress and verification checks with source and
binary MIR, workers 1/4/8 and both generational modes; strict runtime/MIR Clippy
in both modes; scoped Rust formatting; 15 Python harness tests. The isolated
native regression covers stable metadata, growth, promoted-buffer/young-string
barriers, precise descendants, packed access offsets, Vec adoption and eventual
external storage reclamation even while cells remain quarantined. The ordinary
checker rejects diagnostic-feature artifacts via an invalid diagnostic policy.

The first sandboxed program test run could not bind loopback/use PTY/debugging.
Its complete rerun with the required local permissions passes; no test is skipped.
[Validation logs](G5a-validation/).

## Provenance and measurement boundaries

Committed main `56f5983` was integrated at `1b47a02` before the fixed baseline.
Main continues independently; these changes remain on the isolated worktree branch.
Only G2c diagnostic policy code, Buffer metadata, its MIR lowering and format
guard differ in production source. Ordinary refill remains 2 MiB, cell cutoff
512 bytes, scoped GC helpers. [Binaries and source hashes](G5a-binaries.json).

Every run uses a fresh process, fixed workload/task topology, cleared inherited
BADGER settings, independent checks, source/executable hashes and a clean native
RSS launcher. Work time excludes frontend/JIT; process RSS includes them.
No own builds/tests overlap the accepted measurements. Shared-host noise remains;
host CPU pressure/load and process CPU/context switches are saved separately.
Ratios, ranges and all outliers are retained; no statistical significance claim.

| Matrix | Measured runs | Evidence |
|---|---:|---|
| Fresh added-case/coverage baseline | 6 + 88 | [Coverage](G5a-coverage-baseline/summary.md) |
| Intermediate compatibility-pointer candidate | 160 + 440 | [Applications](G5a-applications/summary.md), [coverage](G5a-coverage/summary.md) |
| Final ordinary application matrix, 7 reps, 1/2/4/8 workers | 224 | [Applications](G5a-direct-applications/summary.md) |
| Final ordinary coverage, 3 reps, 11 cases, 1/2/4/8 workers | 264 | [Coverage](G5a-direct-coverage/summary.md) |
| Identical-baseline control, 5 reps, 4/8 workers | 80 | [Control](G5a-self-control/summary.md) |
| Fresh adjacent randomized pairs, 9 reps, 4/8 workers | 144 | [Follow-up](G5a-paired-isolated/summary.md) |
| Separate before/after diagnostic controls/censuses | 32 + 32 | [Before](G5a-census-before-isolated/summary.md), [after](G5a-census-after-isolated/summary.md) |

One attempted census briefly overlapped the end of an earlier paired repeat.
That runner was interrupted; the entire affected repeat and both attempted
census directories are retained but excluded from accepted performance claims.
Fresh `-isolated` datasets run sequentially and have checked nonoverlapping
measured intervals. [Exclusion record](G5a-overlap-exclusion.json),
[fresh boundaries](G5a-validation/isolated-run-boundaries.json). The GC runner now
has an exclusive per-user invocation lock to prevent this error. It does not
control unrelated tools or host workloads.

## Final ordinary application matrix

Negative changes mean less work time, GC pause time or RSS; seven repetitions.

| Profile | Workers | Work change | GC pause change | RSS MiB change |
|---|---:|---:|---:|---:|
| backpressure | 1 | -3.4% | -11.7% | -1.3 |
| backpressure | 2 | -5.1% | -14.3% | -0.8 |
| backpressure | 4 | -7.6% | -16.0% | -2.5 |
| backpressure | 8 | -11.6% | -22.1% | -2.7 |
| balanced | 1 | -4.5% | -9.5% | -1.5 |
| balanced | 2 | -12.6% | -18.4% | -0.6 |
| balanced | 4 | -8.4% | -20.1% | -2.4 |
| balanced | 8 | -12.1% | -24.4% | +7.5 |
| cpu | 1 | -2.9% | -12.3% | -3.2 |
| cpu | 2 | -2.6% | -20.3% | -3.6 |
| cpu | 4 | -6.3% | -22.2% | -4.8 |
| cpu | 8 | -1.0% | -19.2% | +1.0 |
| fanout | 1 | -4.1% | -8.8% | -2.0 |
| fanout | 2 | -3.5% | -13.5% | -6.0 |
| fanout | 4 | -3.3% | -14.7% | +2.3 |
| fanout | 8 | -2.5% | -16.4% | +4.8 |

## Adjacent-pair follow-up, 9 repetitions per cell

Ratios below use group medians; the linked analysis additionally reports
per-pair ratios/ranges and counts without dropping outliers.

| Profile | Workers | Work change | GC pause change | RSS MiB change |
|---|---:|---:|---:|---:|
| backpressure | 4 | -7.8% | -16.3% | +0.5 |
| backpressure | 8 | -9.4% | -19.6% | -0.1 |
| balanced | 4 | -11.0% | -22.7% | -1.3 |
| balanced | 8 | -11.8% | -23.0% | +3.4 |
| cpu | 4 | -6.3% | -20.7% | -2.3 |
| cpu | 8 | -1.6% | -21.4% | +2.9 |
| fanout | 4 | -5.2% | -14.9% | -3.5 |
| fanout | 8 | -4.1% | -17.7% | -0.8 |

The isolated follow-up repeats backpressure/4 −7.8%, balanced/4 −11.0%,
CPU/4 −6.3% and fanout/4 −5.2%. CPU/8 is only −1.6%, within the scale of the
identical-binary control (up to about 5% work-time differences); do not claim
a demonstrated CPU/8 throughput win. GC pause medians improve in all these
cells. RSS remains mixed: balanced/8 +3.4 MiB and CPU/8 +2.9 MiB in the repeat;
the first full matrix has a +7.5 MiB balanced/8 cell. The self-control also has
large RSS variation, including +12.2 MiB fanout/4, so exact small memory deltas
are uncertain. This is not a universal RSS reduction or a p99 latency claim.

Coverage one-worker medians: empty buffers −48.2% elapsed, growing buffers
−15.1%, sustained integer messages −11.6%; persistent-map control −1.5%.
The managed count is unchanged; strong empty-buffer gains do not generalize
directly to workloads dominated by payload creation, scheduling or IO.

## Next priority

G5b should evaluate small fixed-capacity element storage in the managed object.
Common messages use 32 U64/Int words (256 bytes). That can remove their remaining
native payload allocation, and frozen storage with no native resources can avoid
finalizer dispatch. Preserve GC tracing, packed widths, stability, mutation
barriers, inline-to-native growth, budgets, handoff and cancellation. This uses
existing GC ownership and does not require proving unique messages or arenas.
Bounded task/message regions remain later work with their ownership prerequisites.
