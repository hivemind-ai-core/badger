# Optimisation survey — MIR, collections, GC, runtime scaling, type-system memory management

This is the index for the six new research notes (09–13) added to this
directory on 2026-10-19. The original README remains the index for the
clojure-toolbox survey (files 01–08).

| File | Track | Length |
| --- | --- | ---: |
| `09-mir-superoptimisation.md` | MIR-level superoptimisation (above Cranelift) | 1200 |
| `10-persistent-data-structures.md` | Optimised persistent data structures | 1286 |
| `11-gc-and-memory-management.md` | GC and memory management (incl. arenas, Immix, MMTk) | 690 |
| `12-runtime-scaling.md` | Runtime / scheduler scaling for stackful M:N runtimes | 423 |
| `12-runtime-scaling-extra.md` | Runtime / scheduler — extras (deques, NUMA, formal models, OSR) | 596 |
| `13-type-system-memory-management.md` | Type-system-driven memory management (avoid GC) | 1230 |

Each note is a literature / prior-art survey of its topic, written against
`main` at `f46d9bb` on 2026-10-19. Each note:

- opens with a one-section recap of badger's current state on the topic
  (with `file_path:line_number` references to the source);
- surveys the prior art, organised by technique class, with paper / system
  citations (author, year, venue, URL) and an honest verdict
  (very promising / interesting but expensive / mature in production /
  unrelated);
- closes with a "What this means for badger" section that ranks the
  concrete recommendations for this codebase;
- has a "Sources" appendix with the canonical URLs.

## Reading order

For the highest-leverage short path, read in this order:

1. **`12-runtime-scaling.md`** — the single biggest measured cost today
   (BACKLOG: `bench/messages.bdg` 78 ms at 4 workers / 36.6 ms at 1) is
   in this layer. Five of the eight BACKLOG "Runtime synchronization"
   slices (1, 2, 4, 6, 7) are in this domain.
2. **`11-gc-and-memory-management.md`** — GC takes 12–38 % of elapsed
   time in ordinary release runs (`reports/2026-10-17-gc-profiling.md`);
   the R1/R2/R3/R5 BACKLOG slices live here.
3. **`10-persistent-data-structures.md`** — collections are the second
   biggest cost on the I3/I5 workloads; the R2/R3 perf slices are
   collections-shaped.
4. **`09-mir-superoptimisation.md`** — a code-quality pass on top of
   whatever the runtime does; not the place to start when runtime cost
   dominates.

## Headline findings

- **MIR superoptimisation** (`09`): badger's existing `superopt.rs`
  (algebraic + boolean identities with brute-force verification up to 16
  bits of freedom, at `crates/badger_mir/src/opt/superopt.rs:1-736`) is
  at parity with LLVM's `InstCombine` for the narrow subset it covers.
  The cheapest next step is **SMT-validate every algebraic rule with Z3**
  in the style of Alive/Alive2; the second-cheapest is **lift straight-
  line MIR regions into egg's equality-saturation engine**. Full STOKE /
  AlphaDev-style stochastic search over assembly is not justified at
  badger's scale — Cranelift is the right layer for that, and the
  language IR would gain little.

- **Persistent data structures** (`10`): badger's CHAMP-style HAMT
  (`crates/badger_runtime/src/hamt.rs`) is the right data structure; the
  small-map flat-array fallback (Clojure's `PersistentArrayMap`, n ≤ 8)
  is the single biggest realistic win, the second is to formalise the
  CHAMP invariants (sorted entries, disjoint bitmaps), and the third is
  to switch the hasher to AHash. RRB-vectors, ropes, and `OrdMap` are
  deferred with reasons.

- **GC and memory management** (`11`): the runtime is on the
  *stop-the-world mark-sweep + sticky-mark generational* line, which is
  a reasonable baseline. The realistic improvements are: (a) keep the
  mark stack and remembered buffer across collections (R1); (b) account
  channel queues and staging as external memory (R1); (c) add the
  Immix-style block-reclamation pattern as a sub-strategy for the bump
  pools; (d) add escape-analysis-guided stack allocation for short-lived
  records at MIR level (already partially in `simplify.rs` / `sra.rs`).
  Full concurrent marking is *not* recommended yet — the stop-the-world
  rendezvous is < 0.16 % of collector time on the measured workloads.

- **Runtime scaling** (`12`): badger is on the standard M:N path
  (stackful coroutines, M:N workers, poll-based preemption, structured
  scopes, credit-based admission, producer-owned batch staging). The
  literature-consistent next steps are: per-worker Chase-Lev deques
  instead of the shared `VecDeque`; a per-channel state-machine model
  for selection (Go's `runtime.chan.go`); RCU/EBR for the per-worker
  grace period; the HotSpot polling-page trick for the GC handshake;
  and a per-worker segment of the heap to match the per-worker deques.
  None of these are architectural; all are 100–500 line changes with
  independent measurement.

- **Runtime scaling — extras** (`12-extra`): five follow-on sections
  fill in the parts of `12` that were thin: lock-free deques in depth
  (Chase-Lev, HP, EBR, RCU, Hazard Eras, Vyukov MPMC, flat-combining);
  NUMA-aware scheduling (Scheduler Activations, EEVDF, Carrefour, Lozi
  SOSP 2019, Apple M1 memory ordering); formal state-machine modelling
  (TLA+, SPIN, CertiKOS, seL4, linearizability, STM); the
  cooperative-vs-pre-emptive trade-off (Go 1.14, GHC, HotSpot, V8,
  Project Loom, BEAM); and on-stack replacement (Self, HotSpot, V8,
  Cranelift, B3, Mono). The ranking in `12` is unchanged; the
  additional evidence is concrete (urcu-qsbr for the GC handshake;
  HotSpot polling-page for `audit/gc-profile-2026-10-17` G3; TLA+
  PlusCal for the BACKLOG slice 4 lead).

- **Type-system memory management** (`13`): the dedicated companion to
  `11` from the language-side. Covers the classic taxonomy (stack,
  region/arena, linear/affine, ownership, borrowing, escape analysis);
  the Rust borrow checker line (Rustonomicon, Polonius, Oxide, Stacked
  Borrows, RustBelt, Creusot, Prusti, Kani, Aeneas, Linear Haskell,
  ATS); the Swift ownership line (SE-0366, SE-0377, SE-0382, the
  Ownership Manifesto, `~Copyable`, ARC cost, SE-0414 Region Based
  Isolation); the linear/affine research world (Wadler 1990, Vault,
  Cyclone, Austral, Verona, Cone, Roc, F\*, Dafny, Idris 2, Mezzo);
  production escape analysis (Choi 1999, Whaley-Rinard 1999, OpenJDK,
  V8, LLVM, GraalVM, RyuJIT, WebKit); region inference and arena
  allocators (Tofte-Talpin, ML Kit, Aiken-Fähndrich-Levien, Cyclone,
  bmalloc, PartitionAlloc, jemalloc, mimalloc, snmalloc, rpmalloc);
  effect systems (Plotkin-Pretnar, Koka, Cats Effect, ZIO, Kyo, Effekt,
  Unison, HVM, Nim ARC/ORC); and the no-GC-by-design languages
  (Austral, Cone, Verona, Roc, Gleam, F\*, Idris 2, Lean, Coq). The
  ranked recommendations for badger are: (1) region/arena for the
  typeck and AST (no language change); (2) MIR escape analysis to drive
  stack allocation for short-lived records (extends the existing
  `sra.rs`); (3) a formal region annotation on selected builtin types;
  (4) linear types for resources (channels, files, sockets); (5) a
  move-only marker for short-lived records.

## Cross-references to existing reports

The four notes build on the recent perf work and do not duplicate it. The
most important prior readings are:

- `reports/2026-10-17-gc-profiling.md` — measured GC time per phase
  (sweep 61–89 %, trace 7–27 %, stop-world 0.07–0.16 %).
- `reports/2026-10-19-mir-optimization-architecture.md` — current MIR
  optimisation architecture.
- `reports/2026-10-15-compiler-codegen-optimization-audit.md` —
  language-aware MIR opportunities (root slots, immediate closure
  allocations, comparison specialisation).
- `reports/2026-10-19-stdlib-transformations.md` and
  `reports/2026-10-19-eager-list-map-and-map-entries.md` — recent
  collection wins.
- `reports/2026-10-15-runtime-concurrency-review.md` — runtime
  concurrency baseline and the four-worker scaling anomaly
  (independent channel pairs *4.48× slower* at 4 workers than at 1).
- `reports/2026-10-19-select-fast-path-fragility.md` — the BACKLOG
  Slice 4 lead bug (a parked selector not woken by an unlocked fast-path
  send).
- `BACKLOG.md` — current state of all perf slices (C1, C2, C3, C4; R1,
  R2, R3, R4, R5).
- `ARCHITECTURE.md:105-108` — HM inference with rows / effects /
  regions at typeck; the typeck layer that `13` recommends extending.
- `ARCHITECTURE.md:117-174` — the existing ownership (`ownership.rs`)
  and region (`regions.rs`) checks in the typeck layer; the basis
  that `13` builds on.
- `crates/badger_mir/src/sra.rs` and
  `crates/badger_mir/src/opt/simplify.rs` — the existing MIR-side
  pieces that `13`'s MIR escape-analysis recommendation composes
  with.

## How the notes were produced

Each note was drafted by a research subagent from a single, structured
prompt that named the badger source files, the prior reports to
reference, the topic classes to cover, the citation requirements
(authors, year, venue, URL, one-paragraph summary), and the
deliverable shape (length budget, "Sources" section, ranked
recommendations at the end). The subagent was given web access to
verify URLs and pull abstracts. The subagent was instructed to name
sources it could not verify and not to invent references.

The notes were collected in the worktree branch
`research/optimisation-survey-2026-10-19`; they have not been merged to
`main` yet. The plan is to merge after the four notes are read and
any non-badger-specific synthesis (e.g. which recommendation to land
first) is recorded.
