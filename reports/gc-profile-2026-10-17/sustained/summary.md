| Case | Workers | Variant | Work ms median [min, max] | GC ms median | Peak RSS MiB |
|---|---:|---|---:|---:|---:|
| balanced | 1 | control | 2719.100 [2668.335, 2881.001] | 650.535 | 84.2 |
| balanced | 1 | young64m | 1887.186 [1883.502, 2232.239] | 361.913 | 143.4 |
| balanced | 4 | control | 1141.799 [1135.631, 1288.437] | 449.635 | 99.7 |
| balanced | 4 | young64m | 984.581 [972.968, 1121.662] | 302.754 | 160.0 |

Diagnostic GC phases summed across measured repetitions (wall time):

- ('balanced', 1, 'control'): sweep_finalize 2454.495 ms, trace 522.474 ms, weak_pages 112.492 ms, prepare 100.707 ms, trim 43.294 ms, roots 6.089 ms, finish 0.709 ms, accounting 0.459 ms, resume_world 0.298 ms, verify 0.086 ms, stop_world 0.046 ms
- ('balanced', 1, 'young64m'): sweep_finalize 1598.046 ms, trace 132.919 ms, weak_pages 47.575 ms, prepare 40.912 ms, trim 10.164 ms, roots 1.827 ms, finish 0.219 ms, accounting 0.156 ms, resume_world 0.124 ms, verify 0.026 ms, stop_world 0.015 ms
- ('balanced', 4, 'control'): sweep_finalize 1461.559 ms, trace 497.910 ms, weak_pages 119.371 ms, trim 93.380 ms, prepare 93.316 ms, roots 6.654 ms, stop_world 1.039 ms, finish 0.773 ms, accounting 0.536 ms, resume_world 0.223 ms, verify 0.096 ms
- ('balanced', 4, 'young64m'): sweep_finalize 1297.214 ms, trace 148.297 ms, weak_pages 53.910 ms, prepare 43.186 ms, trim 6.454 ms, roots 1.949 ms, finish 0.625 ms, stop_world 0.220 ms, accounting 0.187 ms, resume_world 0.086 ms, verify 0.027 ms
