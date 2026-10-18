# I5b runtime ownership core

Historical runtime checkpoint at `0cc0d7e`, before compiler opt-in and application
validation. The final retained implementation is `49f281c`; current authority for
behavior, validation, results and remaining tradeoffs is [I5b-summary.md](I5b-summary.md).

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
0/1, verification/reuse 0/1 and collection stress 0/1. Subsequent compiler proof,
saved-MIR/worker/chaos validation and ordinary retention are recorded in the final
summary; the tests above describe this earlier runtime checkpoint.

Baseline before production edits: immutable G7d `prefetch32-candidate` and its
ordinary halves in G7d-applications / G7d-coverage, plus the immediately preceding
G7e-trace-baseline ordinary runs. Exact hashes and raw data are retained in their
manifests. Reuse unchanged workloads and run fresh adjacent baseline/candidate
pairs after compiler integration; diagnostics are separate.

Compiler integration: the second MIR pass handles scalar groups rejected by I5a
only for alias liveness/publication. It keeps origin/producer/descriptor rejection,
rejects legacy transient mutators, passes old-alias liveness to every owned insert,
and freezes every other uncertain read before the statement and every map-valued
terminator. Private-only groups retain I5a. Optimization stays gated by release,
inlining and non-live dispatch. Runtime helpers keep the 256-byte budget poll.
14 MIR tests and the new end-to-end fixture pass at integration. Broad validation,
static specialization and benchmark retention followed; see the final summary.
