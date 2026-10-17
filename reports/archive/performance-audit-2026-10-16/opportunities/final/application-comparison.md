# Telemetry comparison: 65,536 events, five repetitions

Each cell uses eight parser tasks, one warmup per binary, and shuffled fresh processes. Ranges are observed minima/maxima, not confidence intervals.

| Profile | Workers | Before events/s [range] | After events/s [range] | Change | Before/after peak RSS MiB | Before/after pooled p99 upper bound ms |
|---|---:|---:|---:|---:|---:|---:|
| backpressure | 1 | 632,722 [254,813, 663,384] | 544,401 [194,289, 569,362] | -14.0% | 51.2 / 46.4 | 8.192 / 7.936 |
| backpressure | 2 | 877,326 [191,144, 892,219] | 773,550 [680,965, 832,301] | -11.8% | 54.4 / 48.9 | 10.752 / 4.608 |
| backpressure | 4 | 1,305,574 [1,160,669, 1,342,705] | 1,151,315 [1,066,545, 1,294,452] | -11.8% | 63.5 / 55.7 | 3.712 / 3.456 |
| backpressure | 8 | 1,627,973 [1,453,589, 1,769,977] | 1,588,117 [250,056, 1,710,564] | -2.4% | 67.4 / 67.7 | 3.328 / 8.704 |
| balanced | 1 | 603,281 [570,229, 635,537] | 525,849 [266,992, 566,544] | -12.8% | 59.1 / 62.2 | 21.504 / 36.864 |
| balanced | 2 | 661,299 [213,303, 860,773] | 720,139 [203,568, 791,955] | +8.9% | 64.3 / 67.0 | 65.536 / 53.248 |
| balanced | 4 | 1,120,400 [272,995, 1,294,923] | 1,246,362 [238,963, 1,298,171] | +11.2% | 71.9 / 77.8 | 45.056 / 51.200 |
| balanced | 8 | 1,699,752 [295,145, 1,788,030] | 1,507,623 [246,427, 1,705,382] | -11.3% | 81.2 / 80.0 | 40.960 / 51.200 |
| cpu | 1 | 270,221 [247,868, 302,683] | 281,626 [138,395, 301,346] | +4.2% | 60.2 / 61.5 | 51.200 / 90.112 |
| cpu | 2 | 430,462 [125,982, 468,254] | 472,816 [409,531, 489,175] | +9.8% | 62.7 / 64.7 | 90.112 / 25.600 |
| cpu | 4 | 775,426 [422,084, 839,619] | 825,982 [177,775, 856,779] | +6.5% | 76.8 / 77.2 | 36.864 / 61.440 |
| cpu | 8 | 802,269 [203,940, 912,877] | 737,086 [259,415, 756,906] | -8.1% | 96.8 / 92.2 | 65.536 / 45.056 |
| fanout | 1 | 206,892 [86,954, 239,441] | 193,918 [180,028, 204,846] | -6.3% | 80.4 / 75.0 | 118.784 / 69.632 |
| fanout | 2 | 272,713 [255,730, 275,323] | 236,334 [211,571, 257,867] | -13.3% | 88.5 / 82.9 | 47.104 / 63.488 |
| fanout | 4 | 311,381 [76,427, 352,950] | 286,578 [268,037, 309,117] | -8.0% | 93.9 / 97.6 | 147.456 / 49.152 |
| fanout | 8 | 305,335 [258,696, 312,783] | 270,597 [252,636, 315,868] | -11.4% | 94.2 / 93.5 | 51.200 / 45.056 |

Each cell pools 20,480 latency samples. These histogram bucket bounds are exploratory; p99.9 has too few tail samples for a strong conclusion. RSS includes compilation and runtime. No CPU affinity or frequency isolation was imposed.

All 160 measured runs passed the independent payload/hash/routing/cache oracle. Raw configurations, results, histograms, process counters, compiler timings, and hashes are in [telemetry-large.json.gz](telemetry-large.json.gz).
