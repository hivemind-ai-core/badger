# I5b runtime ownership core

Runtime core implemented on the worktree; compiler opt-in and application
validation are still pending. No source program invokes owned updates yet.
Do not mark I5b complete or claim application gains from this commit.

Ordinary wrappers remain 16 bytes; owned wrappers are 24. Node tokens use the
explicitly reserved GC client bits and pointer-flag spare bits, without enlarging
nodes or changing entry counts. Global 59-bit checked tokens never wrap; exhausted
allocation falls back to ordinary persistent insertion. Forks freeze the input,
so neither branch can later edit shared nodes under the old token. Reused nodes
and wrappers retain barriers. Ordinary insert/remove/cursor close an active epoch;
frozen shared wrappers incur no write, allowing concurrent readers.

Metadata audit: GC allocation/immortal allocation/pool reuse initializes client
bits to zero; mark/remembered masks and full-generation clearing preserve them;
payload size uses only high 32 bits. Generated barriers mask only GC flags. List
weak-page checks mask MARK/IMMORTAL, not client bits. Legacy node allocation/copy
uses masked pointer flags and strips tokens. Builders retain their exclusively
private contract and must not receive owned shared nodes.

11 runtime tests and strict runtime Clippy pass. Existing HAMT model test now
also checks 512 equal full hashes, all retained epochs and legacy clones. An
isolated runtime model checks snapshots every 1/2/64/256 updates, every saved
key/value, forks updated on both sides, ordinary sharing APIs, allocation reuse,
forced collections, high client-bit preservation across minor/full cycles and
complete helper exhaustion fallback. Eight child configurations cover generation
0/1, verification/reuse 0/1 and collection stress 0/1. Compiler proof, saved MIR,
worker/chaos validation and ordinary benchmark retention remain outstanding.

Baseline before production edits: immutable G7d `prefetch32-candidate` and its
ordinary halves in G7d-applications / G7d-coverage, plus the immediately preceding
G7e-trace-baseline ordinary runs. Exact hashes and raw data are retained in their
manifests. Reuse unchanged workloads and run fresh adjacent baseline/candidate
pairs after compiler integration; diagnostics are separate.
