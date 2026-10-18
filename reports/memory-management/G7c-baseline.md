# G7c baseline: descriptor-only finalizer classification

Reuse the immutable production `fc759e3` binaries and the committed before-edit
[G7b baseline](G7b-baseline.md), plus all baseline samples in its paired cohorts.
Reverting the G7b object-header prototype at `690637a` restores exactly the same
production crates as `fc759e3` (empty `git diff fc759e3 690637a -- crates`).
No fresh or more favorable baseline is substituted after the experiment.

G7c initializes the finalizer classification once per descriptor, refreshes it
at kind backpatch sites, and reads it during sweep. It retains the existing
header size, allocation path, buffer freeze transitions and saved-MIR format.
This avoids G7b's per-allocation cost, but must still earn its place in ordinary
application comparisons. The old G7b measurements remain a separate experiment.
