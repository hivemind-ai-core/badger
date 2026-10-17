| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| tasks-input | 1 | baseline | 90.335 [86.357, 96.639] | 18.893 | 58.8 |
| tasks-input | 2 | baseline | 81.879 [79.336, 82.865] | 14.271 | 60.7 |
| tasks-input | 4 | baseline | 82.298 [78.227, 84.679] | 14.378 | 61.3 |
| tasks-input | 8 | baseline | 85.585 [85.094, 97.925] | 16.455 | 62.9 |
| tasks-result | 1 | baseline | 86.199 [79.434, 87.120] | 15.145 | 58.6 |
| tasks-result | 2 | baseline | 79.274 [78.812, 92.192] | 12.456 | 58.3 |
| tasks-result | 4 | baseline | 80.847 [77.384, 82.253] | 13.377 | 60.2 |
| tasks-result | 8 | baseline | 83.715 [80.047, 84.991] | 14.416 | 60.5 |

Lifetime checkpoints: pool reservation is distinct from live objects and process peak RSS.
Idle yields without allocating intentionally or forcing a collection.

| Case | Workers | Variant | Stage | Pool MiB median [min, max] | Managed live MiB median | Native external MiB median | Collection count median |
|---|---:|---|---|---:|---:|---:|---:|
| tasks-input | 1 | baseline | after churn | 16.25 [16.25, 16.25] | 0.03 | 16.00 | 15 |
| tasks-input | 1 | baseline | idle | 16.25 [16.25, 16.25] | 0.03 | 16.00 | 15 |
| tasks-input | 1 | baseline | tasks done | 0.25 [0.25, 0.25] | 0.01 | 16.00 | 1 |
| tasks-input | 2 | baseline | after churn | 16.50 [16.50, 16.50] | 0.03 | 16.00 | 15 |
| tasks-input | 2 | baseline | idle | 16.50 [16.50, 16.50] | 0.03 | 16.00 | 15 |
| tasks-input | 2 | baseline | tasks done | 0.50 [0.50, 0.50] | 0.01 | 16.00 | 1 |
| tasks-input | 4 | baseline | after churn | 17.00 [16.75, 17.00] | 0.03 | 16.00 | 15 |
| tasks-input | 4 | baseline | idle | 17.00 [16.75, 17.00] | 0.03 | 16.00 | 15 |
| tasks-input | 4 | baseline | tasks done | 1.00 [0.75, 1.00] | 0.01 | 16.00 | 1 |
| tasks-input | 8 | baseline | after churn | 17.25 [16.75, 17.25] | 0.03 | 16.00 | 15 |
| tasks-input | 8 | baseline | idle | 17.25 [16.75, 17.25] | 0.03 | 16.00 | 15 |
| tasks-input | 8 | baseline | tasks done | 1.25 [0.75, 1.25] | 0.01 | 16.00 | 1 |
| tasks-result | 1 | baseline | after churn | 16.25 [16.25, 16.25] | 0.03 | 16.00 | 15 |
| tasks-result | 1 | baseline | idle | 16.25 [16.25, 16.25] | 0.03 | 16.00 | 15 |
| tasks-result | 1 | baseline | tasks done | 0.25 [0.25, 0.25] | 0.01 | 16.00 | 1 |
| tasks-result | 2 | baseline | after churn | 16.25 [16.25, 16.50] | 0.03 | 16.00 | 15 |
| tasks-result | 2 | baseline | idle | 16.25 [16.25, 16.50] | 0.03 | 16.00 | 15 |
| tasks-result | 2 | baseline | tasks done | 0.25 [0.25, 0.50] | 0.01 | 16.00 | 1 |
| tasks-result | 4 | baseline | after churn | 16.25 [16.25, 16.50] | 0.03 | 16.00 | 15 |
| tasks-result | 4 | baseline | idle | 16.25 [16.25, 16.50] | 0.03 | 16.00 | 15 |
| tasks-result | 4 | baseline | tasks done | 0.25 [0.25, 0.50] | 0.01 | 16.00 | 1 |
| tasks-result | 8 | baseline | after churn | 16.25 [16.25, 16.50] | 0.03 | 16.00 | 15 |
| tasks-result | 8 | baseline | idle | 16.25 [16.25, 16.50] | 0.03 | 16.00 | 15 |
| tasks-result | 8 | baseline | tasks done | 0.25 [0.25, 0.50] | 0.01 | 16.00 | 1 |

Diagnostic GC phases summed across measured repetitions (wall time):
