# I3a before-edit baseline

216 measured processes: nine cases, workers 1/8, three repetitions per ordinary, diagnostic-off, phase-only and allocation-census mode. Seed 2026101942; one discarded warmup per cell. Ordinary elapsed times below are workload medians; allocation traffic is process-wide diagnostic census, including startup/output. Diagnostic timings are separate.

| Case | w1 ms | w8 ms | w1 managed objects | w1 cell bytes |
|---|---:|---:|---:|---:|
| visible | 0.715 | 0.626 | 60 | 4,512 |
| stored | 17.057 | 17.241 | 1,048,716 | 33,562,160 |
| stored-reused | 9.395 | 9.787 | 524,404 | 16,784,080 |
| stored-list | 6.186 | 6.469 | 262,321 | 8,663,200 |
| stored-pointers | 9.450 | 10.460 | 295,089 | 11,233,888 |
| stored-escape | 4.266 | 4.278 | 262,244 | 8,394,768 |
| stored-collected | 4.630 | 4.546 | 306,080 | 10,171,216 |
| collected | 17.746 | 21.580 | 1,223,948 | 40,609,234 |
| eager | 10.904 | 11.423 | 527,100 | 26,775,472 |

Production source is main `78cc5a4`; immutable baseline binaries were built for `2bed9af`, with an empty production-source diff from that revision to baseline main. Coverage was committed at `57b7589` before compiler edits. [Binary and harness hashes](I3a-baseline-binaries.json), [raw measurements](I3a-baseline/measurements.json.gz).
