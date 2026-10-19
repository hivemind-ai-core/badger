# G8d: cheaper buffer pushes against corrected October 19 main

Retain the cached-representation/capacity-sufficient push implementation from
`820dd56`. Growth and frozen-buffer failure remain rooted cold paths; pointer
barriers, initialization order, length publication, budgeting and callback
behavior stay intact. The runtime push function shrinks from 1,408 to 611 bytes
in the initial refreshed-main build. There is no layout, ABI or GC policy change.

Measurements below compare corrected main **b7cf709** with **7c2fb36** in the
same source/build directory. Both include the list directory alignment fix
found by the broad validation. The baseline temporarily restores only the old
`buffer.rs` while building; immutable binaries are hashed. Main subsequently
gained builder-node headroom; it is integrated separately, rather than attributed
to this change. [Build provenance](G8d-fixed-binaries.json),
[recheckable evidence audit](G8d-audit.json).

## Throughput and limits

Seven adjacent randomized pairs per 1/2/4/8-worker cell run **byte-identical
saved MIR** in the two native runtimes. Twelve coverage cases include packed
byte/word widths, inline/native boundaries, capacity, growth, pointer values,
records and a stored-sequence control.

- Byte churn: approximately **15–21% faster** across the measured widths and
  worker settings. Word churn: **16–19% faster**.
- Fixed scalar capacity cases: approximately **9–16% faster**; growth cases
  about **4–12% faster**.
- Pointer buffers: near flat to slightly faster; small positive cells remain.
  The preliminary older-base pointer regression does not reproduce consistently.
- Allocation/cell traffic for buffers and native storage allocation/growth/adoption
  events are unchanged. This removes CPU work, not GC allocations. Whole-process
  RSS is usually slightly higher; no memory reduction is claimed.

See [every native cell, range and pair](G8d-paired-summary.md), including the
same-binary controls. Short controls are noisy: the byte/one-worker control's
median is +5.9%, and stored/eight has a wide range. Controls are independent
cohorts, not confidence bounds or adjustments to the measured deltas.

The unchanged nine application contracts have seven pairs at 1/2/4/8 workers,
15 warmups and 15 measured iterations per process. Messaging/pipeline keep
eight consumers at every worker setting. Messaging improves roughly **8–18%**
paired medians; CPU, pipeline, state, snapshots, records and reconciliation are
mostly close to flat. Initial JSON/one is +6.1%, and batches/two's block mean is
+47%. Neither is omitted. [Original application matrix](G8d-current-applications/summary.md).

Longer repeats use 15 pairs, 15 warmups and 50 measured iterations, with an
independent 15-pair same-binary cohort:

| Case/workers | Paired median / block mean | Same-binary median / block mean |
|---|---:|---:|
| JSON/1 | +1.51% / +1.53% | +0.93% / +0.38% |
| JSON/2 | -1.08% / -0.40% | +0.69% / +0.20% |
| batches/1 | +1.64% / +2.74% | -1.48% / -1.53% |
| batches/2 | +0.64% / +4.25% | +0.03% / +1.28% |
| messaging/1 | -7.32% / -7.62% | -0.76% / -1.61% |
| messaging/2 | -14.66% / -9.17% | +0.21% / -3.72% |

Messaging/one improves all 15 pairs and messaging/two 14/15. Batches retains a
possible small throughput/tail tradeoff; the original +47% block-mean shift
shrinks substantially. Keep this open rather than declaring every application
faster. No p99 or universal speedup claim is made.
[Repeats](G8d-app-repeat/summary.md), [controls](G8d-app-control/summary.md).

## Validation and provenance

The corrected baseline/candidate pass 136 focused checks each in ordinary and
diagnostic builds (272 combined): all packed representations, capacity and
growth, pointer lifetimes/barriers, both GC modes, verification/reuse/quarantine,
workers 1/2/4/8, chaos, callbacks, suspension/failure, invalid frozen mutation,
and bidirectional saved-MIR execution. The saved MIR is identical.

Corrected-main validation passes runtime (11 ordinary/13 diagnostic unit tests,
plus integration tests), 14 MIR tests, the 76-test CLI program target, Option
niche, packed arrays, scalar enums, sequence fusion, standard-library caching,
transient maps and service suites; 27 Python harness tests; formatting; strict
runtime/CLI all-target Clippy with `--no-deps`. A broader lint attempt records
seven pre-existing MIR optimizer warnings. The initial broad CLI run passed
adversarial/docs/formatter/GC-retention/LSP/MCP/MIR tests, then found the list
misaligned store; that failure and its correction remain in the logs.

Integration with the subsequent builder-headroom change passes release runtime/
MIR tests and the program, Option, packed-array, sequence and transient-map
targets. Its two new manual ceiling-division expressions are expressed with
`div_ceil` for strict runtime linting; that merge cleanup is not a measured G8d
speedup. A final runtime/MIR recheck and scoped lint logs record that adjustment.

The audit checks **2,020 G8d measured processes** and **426 initial map/collector
baseline processes**, all with disjoint accepted intervals, binary/source hashes,
independent outputs and allocation conservation. Own compilation and tests
precede the timing blocks; other actors on this shared host are uncontrolled.
The first older calibration overlapping a fixture compile is excluded, as are
the older-main baseline/trial and correctness-only map smoke runs. Those raw
artifacts are retained and not substituted for corrected-main acceptance.

The next work follows the [map/GC baseline findings](P1-baseline-findings.md),
not the old sweep profile alone. Snapshot reference retention is cheap already;
private pointer-map allocation and immutable-query bookkeeping offer larger
remaining opportunities than small scratch-buffer changes in these cases.
