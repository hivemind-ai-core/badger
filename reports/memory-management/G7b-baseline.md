# G7b baseline: cached finalizer presence

Before production edits, save 24 ordinary measurements at production revision
`fc759e3` (published in `8b84fe0`), using the immutable private-map-candidate.
Cases: records-window, scalar-snapshots, buffers-pointers and snapshots-major;
workers 1/8, three repetitions. All independent oracles pass.

[Raw commands, binary/source hashes and intervals](G7b-baseline/measurements.json.gz).
The proposed experiment preserves the 16-byte header and all collector policy.
It uses a spare metadata bit for finalizer presence and the existing lazily reset
descriptor shape cache, reducing descriptor lookups while reclaiming dead objects.
Ordinary and diagnostic costs will be compared separately; retain only if measured.
