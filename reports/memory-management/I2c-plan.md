# I2c: remove standard map-helper intermediate entry lists

Status: implemented and retained at 2bed9af. [Results, controls and limits](I2c-summary.md).
Projection time falls 62–68%; serial composition about 21–22%, with exact
same-native tuple elimination. Original application controls remain mostly mixed;
initial larger regressions do not reproduce in focused repeats. The plan and
acceptance criteria below have been evaluated, not left as architectural scaffolding.

Starting main/source revision: 9b23d40. Work stays on the existing isolated
perf/gc-reclaim-2026-10-18 worktree. Fresh ordinary/diagnostic binaries are built
before production edits as map-helpers-baseline[-diagnostic]. Standard sources
are embedded in the compiler, so old binaries retain the original helpers.

Current std.map keys/values each build a List<(K,V)> then project it into their
observable result list. map_values/filter/merge_with similarly materialize entry
lists before looping. Their public results must remain materialized; their
private intermediate lists/tuples can be avoided using existing typed map cursors.
Direct for-loops already preserve the runtime's stable entry order and bind
key/value separately. No collector, object ABI or escape-analysis change is needed.

Before changing std.map, add independent-oracle coverage for projections,
map_values, filter, overlapping merge_with, their composition, an explicitly
stored-entry control, and eight fixed parallel composition tasks. Retain results
across later rounds and dereference them after collections. Save ordinary plus
diagnostic-off/phase/census baselines before edits. Original application contracts
remain unchanged and serve as broader controls; they need not gain from APIs
they do not call.

Correctness: retain ordered public projections, source snapshots, overwrite and
combine behavior, callback order/count, reentrant callbacks, pointer keys/values,
packed U8/F32 values, empty maps, suspension, callback failure and old/new saved
MIR under both GC generations, reuse/quarantine/chaos and workers 1/2/4/8.
Live/unoptimized modes must keep semantics. Cursor traversal may retain input
trie nodes until callbacks finish, where an eager entry list could release trie
nodes earlier; avoid claiming a universal peak-live-memory improvement.

Keep only measured allocation reductions with reproducible helper-workload wins
and no unresolved substantial broader regressions. Record GC/RSS/ranges as well
as elapsed time; do not equate fewer bytes with an identical percentage time win.
