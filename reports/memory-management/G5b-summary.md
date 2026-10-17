# G5b: bounded inline Buffer element storage

## Decision

Retain the 256-byte ordinary implementation at `a0a9838`. It avoids a native
payload allocation for small preallocated buffers and finalizer dispatch once
they freeze. Repeated balanced/backpressure applications improve at 4/8 workers.
Peak RSS commonly increases by a few MiB; this is a throughput optimization,
not a universal memory or tail-latency reduction. G5b is complete; G5, targeted
pool reservation and task/message arenas remain open.

## Implementation and accounting

Positive initial capacities up to 256 bytes live after BufData in the existing
nonmoving GC object. Packed widths and pointer tracing remain precise. The
inline flag fits existing metadata padding; generated pointer/length offsets,
BufData size and MIR format 6 remain compatible. A real G5a format-6 file runs
on this candidate; old format-5 files still require recompilation.

Growth copies only initialized elements into a new native allocation, charges
its entire capacity, and never reallocates/frees the interior managed pointer.
The former inline capacity stays in the managed cell until reclamation. Native
Vec adoption remains native. Freeze changes to a precisely traced descriptor
without a finalizer only when no native allocation is owned; mutation after
freeze remains forbidden. Rooting, write barriers and collection coordination
are unchanged. This uses normal GC ownership for arbitrary sharing/fanout and
requires no uniqueness, escape or topology inference.

A 32-word payload moves 256 native bytes into its managed cell: a 64-byte cell
plus 256 native bytes becomes a 320-byte cell. Object count and nominal payload
bytes do not disappear. Native allocator overhead/calls are avoided, but larger
pooled cells can reserve more memory. These objects still participate in GC.

A separate ordinary-binary proof creates 1,024 retained 32-word arrays before
any collection and verifies exactly 262,144 fewer externally charged bytes.
The diagnostic census independently validates allocation/survival conservation
and native buffer ownership. At one worker:

| Workload | Native allocation calls, inline0 / inline256 | Inline objects |
|---|---:|---:|
| Exact 32-word arrays | 65,537 / 1 | 65,536 |
| Eight-pointer arrays | 32,769 / 1 | 32,768 |
| Sustained integer messages | 262,145 / 1 | 262,144 |

Growth 32→65 words keeps native allocation count unchanged, but replaces one
reallocation with an inline-to-native copy per buffer. This tests fallback,
not an allocation-count improvement.

The new optional `buffer_storage` census separates native allocation, growth,
Vec ownership adoption, free, inline placement and growth out of inline storage.
Its remaining native bytes/blocks must conserve ownership. Adoption is not a
malloc call; inline bytes are already included in managed bytes. This measures
Items-owned capacity, not every temporary/native allocation or language escape.
It completes only part of G1b.

## Measurements

Immutable baseline `b8b6935` versus ordinary candidate `a0a9838`; diagnostic
policies use a separate feature build and are never ordinary timing evidence.
Main `56f5983` was integrated before the baseline; main advances independently.
[Revisions, binaries, source hashes and every matrix](G5b-binaries.json).

Saved before production changes: 12 smoke, 68 coverage and 32 application runs.
Final comparisons: 96 application, 408 coverage, 216 adjacent-pair application
follow-up, 80 identical-binary controls, 84 CPU confirmation, 16 storage census,
96 application policy, 54 storage policy and four real-application census runs.
All checks pass. The 1,166 saved measured intervals are nonoverlapping; own
builds/tests/experiments ran separately. Raw data, all outliers, source/executable
hashes, shuffled orders/seeds and host pressure are retained. Shared-host noise
remains, and paired counts/ranges are not a significance test.

### Applications: nine adjacent pairs per cell

Negative changes mean less work time or RSS. Values compare group medians.

| Profile | Workers | Work change | RSS MiB change |
|---|---:|---:|---:|
| backpressure | 1 | +0.1% | +3.6 |
| backpressure | 4 | -12.1% | +4.8 |
| backpressure | 8 | -9.6% | -6.5 |
| balanced | 1 | +0.2% | +4.1 |
| balanced | 4 | -6.0% | +1.5 |
| balanced | 8 | -10.3% | -0.6 |
| cpu | 1 | -1.2% | +4.2 |
| cpu | 4 | -2.5% | +6.2 |
| cpu | 8 | +5.3% | +2.6 |
| fanout | 1 | -2.3% | +0.2 |
| fanout | 4 | -6.4% | +3.0 |
| fanout | 8 | +0.8% | +4.4 |

[Full follow-up, ranges, paired ratios and sampled latency](G5b-app-followup/summary.md).
The initial three-repeat matrix also improves balanced/backpressure at 4/8;
CPU/8 is flat there. The CPU/8 +5.3% therefore triggered a new 21-pair repeat:
CPU/4 -5.1%, GC -13.5%, RSS +3.6 MiB; CPU/8 -4.7%, GC -10.6%, RSS +1.9 MiB.
The per-pair CPU/8 median is -3.4%, range -16.0% to +11.7%, with 11/21 faster.
The slowdown does not reproduce consistently; neither is a stable CPU/8 win
established. [Confirmation](G5b-cpu-confirmation/summary.md).

Single-worker application changes are mostly small. Fanout/8 is flat and the
follow-up includes large latency outliers; no general tail-latency claim.
The unchanged-binary controls and unaffected coverage cases also vary, including
persistent-map/1 -6.2%. Do not attribute every small delta to inline storage.

### Coverage and reservation tradeoffs

One-worker exact 31/32-word buffers improve 24.8%/23.8%; at eight workers,
25.2%/26.2%. The 33-word control does not inline. Grow-out coverage improves
5.0%/7.4%, pointer arrays 3.5%/4.7%, sustained integer messaging 12.5%/14.4%.
Exact 31/32-word RSS rises about 4–7 MiB. Empty/native-only controls cannot
establish a payload-inline benefit. [All 17 cases at 1/2/4/8](G5b-coverage/summary.md),
[identical-binary control](G5b-self-control/summary.md).

Diagnostic inline128/256 and inline256 with a 256-byte global pool limit test
alternatives. Routing 320-byte cells through the system allocator largely
removes the RSS increase but sacrifices most pure-storage speed: exact32 arrays
become 5–7% slower than inline0 instead of 25–27% faster. The global pool cutoff
also reroutes HAMT cells, so application effects cannot be attributed solely to
Buffer allocation. Keep the ordinary 512-byte cutoff and 2 MiB refill; targeted
size-class reservation remains a useful follow-up. No claim that all memory
changes are caused by huge pages or that pool reservation equals RSS.
[Application policies](G5b-diagnostic-app-policies/summary.md),
[storage policies](G5b-diagnostic-storage-policies/summary.md).

A separate balanced-application allocation census ranks MapNode, Option,
Buffer, Map and Record sites highly. Removing native payload calls leaves
managed record/collection/option churn; inline storage does not solve those.
[Allocation ranking and native ledger](G5b-validation/application-allocation-ranking.json).

## Validation

10 ordinary and 12 diagnostic runtime tests; all 66 CLI/program tests; 42
source/binary-MIR buffer stress checks, workers 1/4/8, both generational modes,
GC verification and preserved old-format rejection/new-format compatibility;
15 harness tests; strict runtime Clippy with all targets in both modes; scoped
Rust formatting. Expanded native tests exercise inline0/128/256, pool256/512,
generational on/off, verification on/off, capacities around the boundary,
32→128-pointer growth, promoted-owner/young-child tracing, packed I16/U32/F32/F64
bit patterns, native adoption and final external-byte reclamation. Verification
off covers actual managed-cell freeing; verification on covers quarantine.
[All logs and ordinary proof](G5b-validation/).

## Next priority

G6a: task entry closures and explicit entry arguments are rooted until scope
release even after the child finishes. Test whether dropping only those input
roots at safe completion points releases large captured graphs earlier. Keep
result aliases and cleanup captures rooted. The lock-free path must clear while
still counted in compiled execution, before its worker permits GC; clearing in
`finish_free` after decrement would race root tracing. Add scoped task retention,
returned aliases, cancellation/failure and cleanup coverage before acceptance.
