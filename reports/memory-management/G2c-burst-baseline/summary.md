| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| pool-burst-1 | 1 | baseline | 65.124 [61.990, 68.257] | 16.211 | 61.8 |
| pool-burst-1 | 2 | baseline | 64.077 [63.008, 65.146] | 16.103 | 62.0 |
| pool-burst-1 | 4 | baseline | 68.243 [66.352, 70.135] | 18.494 | 63.5 |
| pool-burst-1 | 8 | baseline | 65.187 [63.767, 66.607] | 17.447 | 61.8 |
| pool-burst-16 | 1 | baseline | 65.414 [65.020, 65.808] | 17.798 | 63.0 |
| pool-burst-16 | 2 | baseline | 62.557 [62.157, 62.957] | 15.452 | 62.2 |
| pool-burst-16 | 4 | baseline | 64.553 [64.183, 64.924] | 16.636 | 62.7 |
| pool-burst-16 | 8 | baseline | 69.185 [69.085, 69.284] | 19.465 | 62.9 |
| pool-burst-256 | 1 | baseline | 69.262 [64.720, 73.804] | 17.548 | 62.0 |
| pool-burst-256 | 2 | baseline | 71.654 [64.456, 78.852] | 21.302 | 61.9 |
| pool-burst-256 | 4 | baseline | 66.216 [65.824, 66.609] | 17.152 | 63.2 |
| pool-burst-256 | 8 | baseline | 66.661 [64.236, 69.086] | 17.604 | 61.8 |

Burst checkpoints: pool reservation is distinct from live objects and process peak RSS.
Idle yields without allocating intentionally or forcing a collection.

| Case | Workers | Variant | Stage | Pool MiB median [min, max] | Managed live MiB median | Collection count median |
|---|---:|---|---|---:|---:|---:|
| pool-burst-1 | 1 | baseline | after churn | 18.25 [18.25, 18.25] | 0.02 | 15.0 |
| pool-burst-1 | 1 | baseline | burst | 18.25 [18.25, 18.25] | 15.55 | 1.0 |
| pool-burst-1 | 1 | baseline | idle | 18.25 [18.25, 18.25] | 0.02 | 15.0 |
| pool-burst-1 | 2 | baseline | after churn | 18.25 [18.25, 18.25] | 0.02 | 15.0 |
| pool-burst-1 | 2 | baseline | burst | 18.25 [18.25, 18.25] | 15.55 | 1.0 |
| pool-burst-1 | 2 | baseline | idle | 18.25 [18.25, 18.25] | 0.02 | 15.0 |
| pool-burst-1 | 4 | baseline | after churn | 18.25 [18.25, 18.25] | 0.02 | 15.0 |
| pool-burst-1 | 4 | baseline | burst | 18.25 [18.25, 18.25] | 15.55 | 1.0 |
| pool-burst-1 | 4 | baseline | idle | 18.25 [18.25, 18.25] | 0.02 | 15.0 |
| pool-burst-1 | 8 | baseline | after churn | 18.25 [18.25, 18.25] | 0.02 | 15.0 |
| pool-burst-1 | 8 | baseline | burst | 18.25 [18.25, 18.25] | 15.55 | 1.0 |
| pool-burst-1 | 8 | baseline | idle | 18.25 [18.25, 18.25] | 0.02 | 15.0 |
| pool-burst-16 | 1 | baseline | after churn | 34.25 [34.25, 34.25] | 0.02 | 15.0 |
| pool-burst-16 | 1 | baseline | burst | 18.25 [18.25, 18.25] | 15.55 | 1.0 |
| pool-burst-16 | 1 | baseline | idle | 34.25 [34.25, 34.25] | 0.02 | 15.0 |
| pool-burst-16 | 2 | baseline | after churn | 34.25 [34.25, 34.25] | 0.02 | 15.0 |
| pool-burst-16 | 2 | baseline | burst | 18.25 [18.25, 18.25] | 15.55 | 1.0 |
| pool-burst-16 | 2 | baseline | idle | 34.25 [34.25, 34.25] | 0.02 | 15.0 |
| pool-burst-16 | 4 | baseline | after churn | 34.25 [34.25, 34.25] | 0.02 | 15.0 |
| pool-burst-16 | 4 | baseline | burst | 18.25 [18.25, 18.25] | 15.55 | 1.0 |
| pool-burst-16 | 4 | baseline | idle | 34.25 [34.25, 34.25] | 0.02 | 15.0 |
| pool-burst-16 | 8 | baseline | after churn | 34.25 [34.25, 34.25] | 0.02 | 15.0 |
| pool-burst-16 | 8 | baseline | burst | 18.25 [18.25, 18.25] | 15.55 | 1.0 |
| pool-burst-16 | 8 | baseline | idle | 34.25 [34.25, 34.25] | 0.02 | 15.0 |
| pool-burst-256 | 1 | baseline | after churn | 34.25 [34.25, 34.25] | 0.09 | 15.0 |
| pool-burst-256 | 1 | baseline | burst | 18.25 [18.25, 18.25] | 15.55 | 1.0 |
| pool-burst-256 | 1 | baseline | idle | 34.25 [34.25, 34.25] | 0.09 | 15.0 |
| pool-burst-256 | 2 | baseline | after churn | 34.25 [34.25, 34.25] | 0.09 | 15.0 |
| pool-burst-256 | 2 | baseline | burst | 18.25 [18.25, 18.25] | 15.55 | 1.0 |
| pool-burst-256 | 2 | baseline | idle | 34.25 [34.25, 34.25] | 0.09 | 15.0 |
| pool-burst-256 | 4 | baseline | after churn | 34.25 [34.25, 34.25] | 0.09 | 15.0 |
| pool-burst-256 | 4 | baseline | burst | 18.25 [18.25, 18.25] | 15.55 | 1.0 |
| pool-burst-256 | 4 | baseline | idle | 34.25 [34.25, 34.25] | 0.09 | 15.0 |
| pool-burst-256 | 8 | baseline | after churn | 34.25 [34.25, 34.25] | 0.09 | 15.0 |
| pool-burst-256 | 8 | baseline | burst | 18.25 [18.25, 18.25] | 15.55 | 1.0 |
| pool-burst-256 | 8 | baseline | idle | 34.25 [34.25, 34.25] | 0.09 | 15.0 |

Diagnostic GC phases summed across measured repetitions (wall time):
