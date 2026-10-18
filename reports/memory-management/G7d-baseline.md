# G7d baseline before sweep prefetch edits

Thirty ordinary measurements were saved before production changes: five coverage
cases, workers 1/8, three repetitions, immutable `fc759e3`. All oracles pass.
The G7b/G7c production prototypes were reverted; `git diff fc759e3 d62ea36 --
crates` is empty. [Raw runs and hashes](G7d-baseline/measurements.json.gz).

Test x86-64 lookahead hints in sweep. Diagnostic builds expose disabled/8/16/32/64/128
object distances so memory-latency effects can be compared in the same executable
and source, with default policies unchanged. Only a measured ordinary candidate
will be retained; instruction hints are no justification for general speed claims.
