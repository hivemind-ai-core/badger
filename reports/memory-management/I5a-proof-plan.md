# I5a: private scalar-map update reuse

The I2b diagnostic application census still attributes 897,160 MapNodes /
402,771,648 cell bytes to eight reconciliation iterations. Its map starts empty,
updates one current value, and observes no earlier versions. Evaluate reusing
the existing builder-only transient insertion path after a compiler proof.
This is separate from true snapshots, whose old nodes must remain immutable.

Proposed bounded proof (optimized non-live MIR after inlining):

1. Join pointer locals connected by plain copies or map-insert input/results.
   A group must have exactly one static, fresh map-empty origin and consistent
   map descriptors. Parameters/environments, other producers or merged origins
   disqualify it. Initially accept only scalar keys/values (neither is a pointer).
2. Every definition in the group must be that origin, a group copy or a known
   map insert. Every insert in the group must pass; never transform just one
   side of a persistent fork whose nodes could then be shared.
3. Backward CFG liveness at each insert must find no old group local live after
   the call, excluding the result destination. This preserves old aliases even
   if they stay entirely inside the function.
4. Get/get_split/len/contains/index may read the map during private construction;
   scalar results cannot expose its trie. Any other observation/publication,
   including cursor creation, stores, closure captures or unknown calls, is
   allowed only if no further group update is reachable. A retained cursor or
   published snapshot before later updates disqualifies the entire group.
5. Rewrite only the intrinsic target to a private insert helper returning the
   same map wrapper. The helper preserves the existing budget check and uses
   map::insert_mut with its existing roots, old-to-young barriers and descendant
   uniqueness requirement. After final publication the map is ordinary immutable
   data; callers with input maps do not obtain a fresh-origin proof.

Keep all loops/polls, checked arithmetic, ordinary fallback ABI and GC policy.
New optimized saved programs would require the helper's runtime symbol; existing
saved programs should continue to run. Live/no-inline modes retain ordinary calls.

Baseline coverage includes private source-form updates, a new scalar-snapshot
case that verifies every old key/value after churn, realworld reconcile/state,
compare records/snapshots, and fixed eight-task parallel maps. The correctness
fixture covers old/hidden/captured aliases, branched versions, map parameters,
cursors spanning updates, suspension, packed/float values, pointer fallbacks and
handing a returned map to another task. Add MIR regression tests for the proof's
rejected shapes before retaining any speedup. No ownership/region inference is
claimed beyond this conservative boundary.
