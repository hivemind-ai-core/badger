# Diagnostic whole-process GC phases

Three paired processes per cell. Includes all warmups, inner iterations and runtime output; instrumentation perturbs time. Phase totals are not ordinary timing or isolated kernel attribution.

| Suite/case | Workers | Base sweep ms | Candidate sweep ms | Change |
|---|---:|---:|---:|---:|
| compare/batches | 1 | 0.000 | 0.000 | no collections |
| compare/batches | 8 | 0.000 | 0.000 | no collections |
| compare/records | 1 | 3.941 | 3.617 | -8.2% |
| compare/records | 8 | 3.910 | 3.552 | -9.1% |
| compare/snapshots | 1 | 5.837 | 4.634 | -20.6% |
| compare/snapshots | 8 | 5.868 | 4.878 | -16.9% |
| realworld/json | 1 | 2.229 | 2.749 | +23.3% |
| realworld/json | 8 | 2.340 | 2.573 | +9.9% |
| realworld/reconcile | 1 | 2.713 | 2.708 | -0.2% |
| realworld/reconcile | 8 | 2.610 | 2.506 | -4.0% |
| realworld/state | 1 | 8.408 | 8.953 | +6.5% |
| realworld/state | 8 | 8.825 | 7.931 | -10.1% |
