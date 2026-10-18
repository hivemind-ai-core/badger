# I5b proof: private mutation between observable scalar-map snapshots

Status: the bounded scalar-map implementation is retained at `49f281c`.
[Final validation, benchmarks and limits](I5b-summary.md). The following records
its proof boundary; parameter/pointer-map and arena generalizations remain open.
I5a already eliminates private fresh-map path copies. It conservatively rejects
an entire group if any version is published before a reachable later update.
Realworld state and compare snapshots publish every 256/64 updates: they must
retain old values, but need not path-copy on every update within those windows.
The runtime already uses CHAMP-layout HAMT nodes; this is ownership tracking,
not a conversion to CHAMP or an arena/GC-bypass claim.

## Compiler proof boundary

Start with I5a's one static fresh-empty origin, consistent scalar K/V descriptors,
copy/insert-only producers, parameter/environment/unknown-producer rejection and
all-or-none group transformation. Pointer-bearing keys/values remain excluded:
transitive aliases and self-insertion are not covered. Keep I5a's cheaper existing
helper for groups that never need a snapshot; do not add epoch costs there.

For other eligible groups, transform all inserts to an internal owned-update
helper. Pass whether any old local alias is live after that insert, using CFG
liveness/backedges and excluding the call's result destination. Uncertain uses
freeze the wrapper BEFORE observation/publication, including stores, captures,
unknown calls, return, tuple/list/builder insertion and cursor creation. Known
scalar get/get_split/contains/len/index reads can retain ownership. A final
publication also freezes; this avoids depending on future interprocedural proofs.
Calls returning a map do not introduce a fresh origin. Reject groups containing
legacy transient mutation helpers unless their exclusive construction phase is
proved separately: freezing before an unknown mutator does not make mutation
safe. Keep this separate from the already transformed all-private I5a groups.
Preserve source order,
checked behavior, budget/poll points and ordinary debug/live/no-inline behavior.

A live old alias requires freezing the old wrapper before forking even if it
has never escaped. Otherwise a later update through that old alias could mutate
nodes still shared with the new version. Freezing on ALL unknown publication
paths is essential even when the use looks read-only or another path updates.
Unknown calls can retain arguments; cursors retain trie roots. Captured/hidden
aliases are publications, not just local liveness. Compiler-internal helpers are
not a source mutation API or a security boundary for hand-authored MIR.

## Runtime ownership model

A wrapper with a nonzero globally unique owner token is private. Freezing sets
its owner to zero without changing root/length/content. Updating an unfrozen
wrapper with no old live aliases may reuse it. Updating a frozen wrapper or one
whose old alias must survive first freezes the original and creates a new wrapper
with a fresh token. Reuse a trie node only if its owner equals the active token;
otherwise copy it and stamp the copy. Every reused node that receives a new
managed child still needs the existing write barrier. A new wrapper also needs
ordinary rooting/barrier treatment. Existing builder/private paths retain their
current exclusivity contract.

An owner must never identify two active wrappers. Freezing does not clear each
node's old token: globally unique tokens make such nodes uneditable by the next
epoch. Ordinary persistent insert/remove must strip owner tags on nodes they
allocate, preserve all shared descendants, and never invoke the owned mutation
path. Source groups freeze before passing a map to ordinary/unknown operations.
Legacy transient builders must never be handed these shared nodes. Audit every
map/builder constructor before relying on that property.

Keep ordinary 16-byte map payloads unchanged. An internal owned-map wrapper can
have a 24-byte payload (root, length, token); existing map reads/tracing use the
same first two words and ignore the scalar token. The helper can fork a legacy
16-byte empty map on its first update. A compiler-only owned-empty helper is an
optional later optimization. Check payload size before reading the token. Old
saved MIR calls ordinary helpers and remains compatible; newly optimized MIR
will require any newly introduced helper symbols.

## Candidate compact node tokens (audit before implementing)

Avoid growing every trie node or adding branches to entry-count/offset loads.
Node flags currently use only bits 0/1 for managed key/value words. GC Header.meta
uses low bits 0/1/2 for mark/immortal/remembered and high 32 bits for payload size.
The G7b per-object finalizer-bit prototype was reverted. A possible 59-bit token
uses node flags bits 2..31 (30 low token bits) plus Header.meta bits 3..31 (29 high
token bits). Keep node layout and stored entry count unchanged. Mask Node.flags()
to return ONLY pointer flags to legacy allocation/copy/trace/cursor code; a
separate owner accessor reads the full flags/header bits. Newly allocated legacy
nodes have no owner. Stamp only newly allocated owned nodes; existing owner tags
are never reassigned while shared. GC mark/remembered clearing must preserve the
client bits, and payload_size() must remain exactly the high 32 bits.

This requires an explicit reserved client-bit mask in gc.rs and a complete audit
of metadata masks/stores, flags propagation, node cloning/constructors, collision
buckets, pool reuse and verifier quarantine. Do not silently share those bits
with future GC features. If this packing cannot be proved simple, use an explicit
owner word in a separate prototype and measure the space/access cost first.
Do not repurpose entry count without accounting for unbounded collision buckets.

Use a relaxed global checked owner allocator, never wrapping/reusing IDs. At
exhaustion return zero and fall back to immutable persistent insertion, after
freezing the input wrapper. This retains semantics indefinitely; tokens need not
be reclaimed. A checked 59-bit space avoids a short-running 30-bit epoch limit.
Only allocate an epoch when starting/forking an owned wrapper, not every insert.
Do not introduce new mutator/collector synchronization. Existing stop-the-world,
worker publication, roots and barriers remain authoritative.

## Acceptance before retaining

- Independently verify every retained key/value, not only aggregate checksums.
  Exercise publication every 1/2/64/256 updates, multiple generations of forks,
  alternation through old aliases, loops/branches and old cursors across updates.
- Hidden stores, closure/task handoff, unknown functions, empty/final maps,
  nonmatching input origins and pointer-bearing maps keep immutable semantics.
- Packed/negative scalar keys, float values, hash collisions (including buckets
  larger than 64 entries), depth changes, deletion after freezing, builder and
  legacy private-map interaction, and token exhaustion need regression coverage.
- Run source plus old/new saved MIR, real reuse and quarantine, both GC generation
  modes, workers 1/2/4/8 and chaos; verify old-to-young edges under forced GC.
- Save immutable current-main baseline binaries and unchanged realworld/compare
  sources before edits. Census exact Map/MapNode object and cell-byte traffic.
  Expect genuine state/snapshot reductions, while I5a private-map, pointer-map,
  stored-entry and record controls should not gain allocations or lose semantics.
- Measure adjacent shuffled ordinary process pairs and fixed-topology worker
  scaling. Diagnostic counts/phases are separate, and source/binary hashes and
  all samples remain. Require an application win, not only a new microbenchmark.

No arena, zero-copy handoff, task isolation or pointer graph ownership is proved
by this design. Those remain separate roadmap items with separate tracing and
reclamation contracts.
