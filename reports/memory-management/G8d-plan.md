# G8d — buffer push hot path

Baseline: current main `f1b1737`, including streaming JSON and packed bulk copies.
Work is isolated on `perf/gc-reclaim-2026-10-18`.

First evaluate the buffer push path identified by the I3a byte profiles. Reuse
Items' existing cached element representation, separate capacity-sufficient
stores from rooted growing/collection work, and preserve frozen-buffer checks,
length publication, pointer barriers, budget accounting and inline-to-native
growth. No callback elision or public ABI/layout change in this slice.

Save immutable ordinary/diagnostic baseline binaries and before-edit raw data.
Cover U8 inline/native boundary (256/257 bytes), U64 words, growable and fixed
capacity buffers, pointer elements, surviving arrays, and unrelated stored
sequences/maps/records. Compare 1/2/4/8 workers with randomized adjacent pairs,
same-binary controls, unchanged applications and independent oracles. Retain
regressions. Count allocations separately; faster push need not allocate less.

Validation includes all element representations, reuse/quarantine, minor/major
collections, pointer growth and old-to-young barriers, invalid frozen mutation,
callback ordering/suspension/failure and saved-MIR compatibility. Default GC
policy remains unchanged. After this slice, select the next measured allocation
elimination opportunity from the remaining tracker.
