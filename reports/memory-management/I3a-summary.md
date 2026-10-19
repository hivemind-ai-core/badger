# I3a retained: fuse nonescaping stored finite sequence pipelines

Keep `10dde6a`. Immutable local range/list sequence descriptions used only by
supported `fold`, `count` and `for` consumers now lower to ordinary loops before
MIR dumping/saving. Construction-time arguments and callbacks are evaluated
once at the declaration; each traversal has fresh position/take counters.
Callbacks remain lazy and reusable. Typed source/callback operands remain roots.

Only two compiler production files change: `lower.rs` and `lower/fusion.rs`.
Captured/escaping/reassigned descriptions, stored aliases, unknown/scoped
sources and unsupported consumers retain their iterator objects/protocol.
Explicit `collect`, eager collection transformations and composed stored
bindings are follow-ups; I3 is not complete. Runtime/GC policy, object layouts,
public sequence ABI and saved-MIR format do not change.

## Target results

Ordinary builds: seven randomized adjacent pairs at each worker count.
Percentages are medians of paired workload elapsed ratios; negative is faster.
Every targeted pair is faster at every worker setting. These shared-host ranges
are not confidence intervals. [Full paired results](I3a-paired-summary.md).

| Coverage | w1 | w2 | w4 | w8 |
|---|---:|---:|---:|---:|
| Stored scalar range | -96.8% | -96.5% | -96.4% | -96.3% |
| Stored range, two traversals | -96.5% | -96.8% | -96.8% | -96.7% |
| Stored scalar list | -74.8% | -75.6% | -74.8% | -76.3% |
| Stored String list, mapped to length | -56.1% | -47.2% | -45.1% | -45.7% |

The scalar case uses 65,536 elements and eight rounds; added cases use 32,768
and four rounds. Repeat traversal checks the same description after a yield.
Pointer inputs are independently allocated Strings with varying lengths.
Python oracles check every result, including small/odd configurations.

At workers 1/8, scalar kernel medians change 17.262/17.975 to 0.553/0.644 ms;
peak RSS changes 58.215/57.938 to 35.594/36.016 MiB. Two kernel collections
become zero, and GC pause counters change 2.194/2.241 ms to zero. List kernels
can improve without a collection occurring in the measured window: per-element
allocation and iterator/call overhead are also removed. Process RSS includes
frontend/JIT/native memory; it is not a live-heap measure or a stack-allocation
proof. A serial kernel does not establish worker scaling.

[Old/new MIR in the same native runtime](I3a-native-mir/summary.md), five pairs,
reproduces scalar -96.8%/-96.4%, scalar-list -75.0%/-75.6% and pointer-list
-56.8%/-46.2% at workers 1/8. Every targeted pair is faster. Generated code and
its layout change too, so the entire elapsed reduction is not attributed to GC.

## Managed allocation proof

[Same-native diagnostic census](I3a-native-census/summary.md), three repetitions
at workers 1/8, removes exactly 1,048,576 per-element Option objects in the
scalar case, 524,288 in the repeat case, and 262,144 in each list case. One
process-startup Option remains. This is not natural-null ABI lowering or a
claim that every Option in the language is allocation-free.

| Case | Managed objects before → after (w1) | Cell bytes before → after |
|---|---:|---:|
| Stored scalar | 1,048,716 → 60 | 33,562,160 → 4,512 |
| Repeated traversal | 524,404 → 60 | 16,784,080 → 4,544 |
| Scalar list | 262,321 → 137 | 8,663,200 → 272,992 |
| String list | 295,089 → 32,905 | 11,233,888 → 2,843,680 |

Counts include startup/output. Cell bytes include managed headers/rounding;
native allocations, spare capacity and allocator metadata are excluded. Output
number lengths can change a few bytes; full ranges remain in the manifest.
Visible, escaping, stored-collected, collected and eager controls retain exactly
their object counts and Option counts in every same-native census process.
Diagnostic timings are not ordinary-build speed claims.

## Retained regressions and control attribution

This is a targeted optimization, not a general application performance win.
In the combined coverage program the already-fused visible pipeline changes
-25.2% at one worker but +18.8–19.4% at 2/4/8. Byte churn is +6.4–12.4%; escaped,
collected/eager, map and record controls are mixed. All initial samples remain.

Standalone controls isolate the unchanged work from changed neighboring
branches. Both compilers produce **byte-identical saved MIR** for visible,
byte-churn and escaping programs. Fifteen adjacent pairs per control/worker:

| Control | Different executables, source w1/w8 | Same executable, same MIR w1/w8 | Different executables, same MIR w1/w8 |
|---|---:|---:|---:|
| Visible | +0.45% / -0.31% | -0.62% / +2.18% | +0.00% / -0.01% |
| Byte churn | +13.91% / +10.64% | -1.20% / -2.11% | +3.66% / +5.52% |
| Escaping | -1.24% / +0.75% | +0.84% / -2.67% | +0.19% / +4.47% |

The isolated visible regression disappears. The byte regression remains:
**none of its fifteen one-worker source pairs is faster**. Do not dismiss it
as ordinary timing noise or subtract the separate control ratios. Results point
to executable/frontend/layout sensitivity beyond the identical MIR; the exact
cause remains unresolved. `buffer::push`, its intrinsic wrapper and `gc::alloc`
have matching instruction sequences after resolving relocation targets, but
addresses/alignment differ. That does not prove a hardware/cache explanation.
[Instruction check](I3a-validation/runtime-instructions.json),
[standalone source](I3a-isolated-source/summary.md),
[same-native control](I3a-isolated-native/summary.md),
[cross-native same-MIR](I3a-isolated-cross-native/summary.md).

The unchanged nine-case [application matrix](I3a-applications/summary.md) uses
workers 1/2/4/8, seven pairs, and 15 warmups/15 inner samples. Most differences
are small/mixed. Initial batches/two block-mean +12.73% becomes +3.98% in a
[fifteen-pair repeat](I3a-app-repeat/summary.md); the [same-binary control](I3a-app-control/summary.md)
is +4.15%. Snapshots/two median +9.95% becomes +1.43%, control -0.50%.
Snapshots/four repeats at +4.70% median/+2.62% block mean, control
+0.95%/-3.30%; messaging/eight retains its original +7.32% median/+2.93% block
mean. There is no universal no-regression claim, and no samples are discarded.
Original realworld/compare workloads and peer-language contracts are unchanged.

## Profile evidence and next opportunity

Twelve additional, separately perturbed profiles run isolated byte churn at
1,048,576 arrays of 256 bytes, one worker, three shuffled pairs for source and
three for identical saved MIR. Their 21,659 CPU samples attribute 67–71% self
samples to `badger_runtime::buffer::push`, with 17–22% in the generated fill
loop. GC entry/collection functions have about 1% self samples; whole-kernel
pause counters are about 0.4–0.5% of profiled elapsed time. Native caller chains
are incomplete without force-frame-pointer builds, and helper-thread CPU is
not established by these self samples. Profiling changes timing/RSS and is
separate from ordinary results. [Profiles](I3a-byte-profile/profiles.json.gz).

`array.build` currently calls buffer push for every item. Bounded scalar stores
or proven constant bulk fills are a more direct next target for this shape
than collector tuning. Preserve callback order/count, suspension/failure,
length checks, pointer roots/barriers and growth semantics; save fresh baselines
before implementation. G8d and the executable-sensitivity follow-up remain open.

## Validation and provenance

Passes: 14 MIR unit tests; all CLI test targets (121 tests, including the 76-test
program suite, source/release/saved-MIR/multicore corpus and four fusion tests);
strict MIR/CLI all-target Clippy in ordinary/diagnostic modes; cargo fmt; 27
Python harness tests. The first broad CLI run was blocked by sandbox localhost
port reservation; the remaining targets pass with local-port access. Both logs
are retained. No new sanitizer or external database coverage is claimed.

The reusable checker passes 24 MIR probes and 152 GC/semantic checks in each
ordinary/diagnostic binary (48/304 combined). It covers generations 0/1,
reuse/quarantine, chaos, workers 1/2/4/8, no-inline/unoptimized modes, bidirectional
saved-MIR compatibility, constructor/callback order, mutable snapshots,
repeated/nested traversal, take/empty/Int-max limits, break/continue/return,
U8/F32/String values, suspension, child failure and scoped-resource fallback.

The [before-edit baseline](I3a-baseline.md) is committed at `57b7589` before
compiler edits; main was fetched at `78cc5a4`. Immutable compiler binaries and
source hashes, exact arguments, raw data and validation paths are recorded in
[I3a-binaries.json](I3a-binaries.json). All 2,756 ordinary/diagnostic measured
process intervals and the twelve profile intervals are disjoint. Own builds,
tests and MIR compilation precede the timed blocks; other shared-host actors
are uncontrolled. Baseline-versus-itself controls are independent cohorts,
not confidence bounds or causal corrections. No tail/p99 claim is made.
