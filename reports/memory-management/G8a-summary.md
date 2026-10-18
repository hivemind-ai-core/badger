# G8a evaluated: small-cell clear specialization rejected

Do not retain prototype `e41aa39`. Revert `772d9ee` restores production allocation
exactly to C1 (`b080078`, integrated main `494b178`). Keep the experiment and raw
measurements. A valid assembly simplification did not become a broad application
win, and it made a byte-heavy control substantially slower.

## Hypothesis and validation

The recycled-cell path in ordinary release gc::alloc called dynamic-size memset,
then wrote both header words. Constant full clears for 32/48/64-byte cells become
inline stores; LLVM also removes overwritten header zeroes in the caller. The
allocator's source-level fully-zeroed contract, including padding, stayed intact.
Other sizes retained the generic path with extra dispatch comparisons.
[Baseline](G8a-validation/baseline-alloc.asm) and
[candidate](G8a-validation/candidate-alloc.asm) assembly remain. Function size is
1,992→2,046 bytes in these binaries. No GC policy, layout or allocation-count
reduction was intended.

The ordinary runtime's 11 unit/three integration tests and strict runtime/CLI
all-target Clippy pass. The existing pool test exhaustively checks payload sizes
0..1040, zeroing/reuse, retirement/quarantine and cross-worker returns. Formatting
passes. The prototype was rejected before broader integration/diagnostic testing;
those checks are not claimed for it. Retained C1 remains fully validated.

## Results

Before edits: 36 fresh coverage processes using the immutable C1 binary.
Final ordinary measurements: seven adjacent randomized pairs, warm applications
using 15 warmups/15 samples, plus 15-pair targeted repeats and same-binary controls.
Each process contributes a median and a block mean; all checksums are gated.
All raw samples remain; no outliers are removed.

| Application repeat | Workers | Paired median | Paired block mean |
|---|---:|---:|---:|
| Compare records | 1 | -0.6% | +0.8% |
| Compare records | 8 | +0.7% | +1.6% |
| Reconcile | 1 | -1.7% | -1.4% |
| Reconcile | 8 | +0.5% | -0.4% |

The [repeat](G8a-repeat/summary.md) stays small/mixed relative to its
[same-binary control](G8a-control/summary.md): control medians +0.2% to +1.2%
except reconcile/eight -0.5%, with wide ranges. Initial JSON/state controls are
also mixed. [All 112 initial application processes](G8a-applications/summary.md).
Do not turn the small one-worker reconcile improvement into a universal claim.

| Coverage, paired median | w1 | w2 | w4 | w8 |
|---|---:|---:|---:|---:|
| Record window | -4.1% | -4.3% | -7.2% | -5.4% |
| Transient Array<U8>, 256 elements | +12.7% | +9.1% | +17.1% | +17.4% |

The byte-heavy candidate is slower in all seven pairs at workers 2/4/8 and six
at one. Other map/buffer/snapshot controls are small/mixed.
[All 336 coverage processes, ranges, GC and RSS](G8a-coverage/summary.md).
The extra dispatch on non-specialized cell sizes is a plausible contributor,
not isolated causal proof. Record-window gains do not justify this regression.

724 measured processes in five disjoint datasets include the baseline, coverage,
initial applications, repeats and controls. Binaries and source/command hashes
remain. The first application command mistyped the candidate revision label as
`26`; its original report is preserved as `original-measurements.json.gz` and the
primary report carries an explicit metadata correction to `e41aa39`. Binary hashes
and every run/sample are identical; no measurements were edited.

Next G8b adds diagnostic allocator-path counts (recycled cell, fresh pooled bump
space, system allocation) and pool refill/trim counts before choosing another
allocator policy. This distinguishes where requests are served, not ownership,
liveness or escape. Counters are not an allocation or collection optimization.
