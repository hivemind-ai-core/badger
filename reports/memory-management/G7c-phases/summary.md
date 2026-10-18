# Diagnostic whole-process GC phases

Three paired processes per cell. Includes all warmups, inner iterations and runtime output; instrumentation perturbs time. Phase totals are not ordinary timing or isolated kernel attribution.

| Suite/case | Workers | Base sweep ms | Candidate sweep ms | Change |
|---|---:|---:|---:|---:|
| compare/batches | 1 | 0.000 | 0.000 | no collections |
| compare/batches | 8 | 0.000 | 0.000 | no collections |
| compare/records | 1 | 3.789 | 3.972 | +4.8% |
| compare/records | 8 | 3.796 | 3.925 | +3.4% |
| compare/snapshots | 1 | 5.633 | 5.242 | -6.9% |
| compare/snapshots | 8 | 5.284 | 5.524 | +4.5% |
| realworld/json | 1 | 2.129 | 2.495 | +17.2% |
| realworld/json | 8 | 2.515 | 2.491 | -1.0% |
| realworld/reconcile | 1 | 2.674 | 2.636 | -1.4% |
| realworld/reconcile | 8 | 2.542 | 2.472 | -2.8% |
| realworld/state | 1 | 8.703 | 8.645 | -0.7% |
| realworld/state | 8 | 8.822 | 8.616 | -2.3% |
