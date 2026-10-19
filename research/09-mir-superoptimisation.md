# MIR-level superoptimisation: prior art and what is realistic for badger

Compiled 2026-10-19 from public papers, READMEs, and abstracts. The bibliography
is the "Sources" appendix at the bottom. The goal of this document is to map
the academic and industrial landscape of mid-level superoptimisation onto the
current badger MIR pipeline
(`crates/badger_mir/src/opt/{cse,simplify,specialize,superopt,cfg}.rs`,
`crates/badger_mir/src/mir.rs`), and to recommend concrete next steps.

Scope is narrow on purpose. The user asked for techniques that go beyond
peephole/algebraic rewrites and that target the IR level between HIR and
machine code, *not* for instruction-level superoptimisation that targets raw
machine code. Stochastic search over x86 assembly (STOKE, AlphaDev) is still
relevant where the technique itself is portable; pure "rewrite the last N
machine instructions" work is not. Throughout, I separate:

- techniques that target *machine code* (Massalin, GSO, STOKE, AlphaDev) —
  the principles and representation choices are reusable, the specific
  rewrites are not;
- techniques that target an *IR similar to badger's MIR* (Souper, Alive/Alive2,
  egg, Ruler, MLIR Affine);
- techniques that target *exactly the same problem class* (Herbie for
  floating-point accuracy, B3's peephole IR for JS, Swift SIL peepholes).

## Badger's starting point

The pipeline at `crates/badger_mir/src/opt/mod.rs:46-76` already runs six
named passes for `--opt superopt` (Release, Aggressive, Superopt). The
relevant pieces for this survey:

- `opt::simplify` (`crates/badger_mir/src/opt/simplify.rs`) is a 780-line
  local simplifier with constant folding, integer identities, copy/constant
  propagation through dominance (`simplify.rs:476-599`), dead-assignment
  elimination via bit-vector liveness (`simplify.rs:697-779`), and cfg
  cleanup (`simplify.rs:603-633`). It is the closest thing badger has today
  to "rule-based peephole optimisation".
- `opt::cse` (`crates/badger_mir/src/opt/cse.rs`) is a 193-line block-local
  CSE keyed on the shape of the rvalue, with effectful rvalues and
  reassignments invalidating entries (`cse.rs:138-192`). It is effectively a
  block-local value numbering.
- `opt::specialize` (`crates/badger_mir/src/opt/specialize.rs`) is a 338-line
  pass that clones a helper called with a known-closure argument, rewrites
  calls through that closure to `Callee::Lambda`, and drops the closure
  parameter once the clone no longer reads it (`specialize.rs:131-196`). It
  is inlining under a fixed criterion; not superoptimisation in any of the
  senses below.
- `opt::superopt` (`crates/badger_mir/src/opt/superopt.rs`) is the new 736-line
  straight-line superopt: algebraic identities for shifts/masks/divisions
  (`superopt.rs:182-229`), double-negation and comparison inversion
  (`superopt.rs:281-306`), `(x + c) - c → x` collapse
  (`superopt.rs:307-366`), and commutative operand canonicalisation
  (`superopt.rs:377-413`). Every boolean rewrite is verified by exhaustion
  over ≤16 free bits (`superopt.rs:478-511`).
- `opt::cfg` is reachability + block merging, and `Program` itself holds
  `live`, `workers`, `n_globals` etc (`mir.rs:18-59`); the MIR is monomorphised
  SSA-on-CFG with explicit `BinOp`/`CmpOp`, `Convert`, `Fits`, `Overflows`,
  closure calls with environments, `Poll` statements, and `Stmt::Loc` for
  source positions (`mir.rs:118-294`).

That last point matters. Badger's MIR is rich (typed, SSA-ish, with GC roots
and source locations) and is *not* machine code. Most of what is published
under "superoptimisation" targets x86 assembly or LLVM IR, which is much
closer to machine code than badger MIR. The interesting question for badger
is which *techniques* survive the port upward to a typed, monomorphised,
effect-bearing IR.

---

## 1. Foundational superoptimisation papers

### 1.1 Massalin, "Superoptimizer: A Look at the Smallest Program" (1987)

- **Authors / venue:** Henry Massalin (City College of New York), ASPLOS
  1987. DOI 10.1145/36177.36194. PDF at
  `https://web.stanford.edu/class/cs343/resources/superoptimizer.pdf`.
- **Input / output language:** raw 68000 assembly sequences, treated as
  bit-level state machines with up to ~32 lines of code.
- **Search strategy:** exhaustive brute-force enumeration of *all* valid
  instruction sequences of a given length, paired with a probabilistic
  equivalence test that compares the candidate's I/O behaviour against the
  target on random test vectors and rejects as soon as a mismatch is found.
  This is what makes the search practical: most candidates are filtered by
  the very first random input, so the exponential explosion never happens
  in practice.
- **Cost model:** instruction count (and, in a few examples, cycle count).
- **Reported gains:** Massalin reports programs that are *smaller* than the
  hand-written ones — sometimes strikingly so, including cases where the
  "optimal" program bears no obvious resemblance to the source program.
  Several of these programs were unknown to Motorola's engineers.
- **Relation to MIR-level superopt:** the *method* (random-input filtering)
  is exactly what `opt::superopt::verify` at
  `crates/badger_mir/src/opt/superopt.rs:478-511` re-implements for booleans.
  The `free_vars` traversal at `superopt.rs:438-476` and the bit-budget cut
  at `superopt.rs:492` are directly descended from Massalin's idea: bound
  the size of the input domain so that 2^N is feasible, and reject any
  candidate whose value on any one of the 2^N inputs disagrees. The
  important limit Massalin hits is that 68000 assembly is a comparatively
  small, side-effect-free ISA, so exhaustive enumeration is possible; badger's
  MIR has effects (allocations, stores, calls, polls) that defeat Massalin's
  formulation. The contribution we keep is *bounded exhaustive verification
  for very small regions*, not the synthesis itself.
- **Verdict for badger:** very promising in the verification role, already
  partially used; synthesis at Massalin scale is not realistic because the
  instruction set is the entire badger MIR surface.

### 1.2 Granlund and Kenner, "Eliminating branches using a superoptimizer
and the GNU C compiler" (PLDI 1992) and the GNU superoptimizer (GSO)

- **Authors / venue:** Torbjörn Granlund and Richard Kenner, PLDI 1992.
  Source distribution at `https://ftp.gnu.org/gnu/superopt/`.
- **Input / output language:** x86 (originally) and later several other ISAs;
  targets straight-line code sequences.
- **Search strategy:** exhaustive enumeration (table-driven by ISA tables
  shipped with the tool), with a hash-based pruning and an
  equivalence-by-test-vectors check. GSO was used to find missing
  peephole patterns that GCC and other compilers should be applying.
- **Cost model:** instruction count / cycle count.
- **Reported gains:** Granlund and Kenner used it to add peephole rewrites
  that GCC had not encoded by hand (notably branch elimination). The tool
  itself is small and writes its finds to a header that the rest of the
  compiler can `#include`.
- **Relation to MIR-level superopt:** GSO is essentially Massalin's idea
  packaged as a peephole-rule *generator*. It doesn't search a higher-level
  IR, but its output format — a table of human-readable patterns — is the
  design ancestor of every "auto-discovered peephole" system since.
- **Verdict for badger:** the pattern-table output format is a clean way
  to express the existing `rewrite()` of `superopt.rs:168-369` as data
  rather than Rust code. Not directly applicable as a runtime pass, but
  plausible as a *build-time* tool that mines MIR traces emitted under
  `--dump-mir` and proposes new rules for human review.

### 1.3 STOKE and the Schkufza / Sharma / Aiken stochastic-superoptimisation
line (2013-2017)

- **Authors / venue:** Eric Schkufza, Rahul Sharma, Alex Aiken (Stanford).
  Key papers: "Stochastic Superoptimization" (ASPLOS 2013), "Data-Driven
  Equivalence Checking" (OOPSLA 2013), "Conditionally Correct
  Superoptimization" (OOPSLA 2015), "Sound Loop Superoptimization for
  Google Native Client" (ASPLOS 2017). Code at
  `https://github.com/StanfordPL/stoke`.
- **Input / output language:** x86-64 text assembly. STOKE disassembles an
  ELF binary, then operates on `tunit`s.
- **Search strategy:** MCMC (Markov-chain Monte Carlo) over a space of
  *transformations* of an existing program — replace-instruction, swap,
  opcode, operand, local swap, global swap, rotate, insert-nop, delete.
  Each candidate is sandboxed and executed against test vectors; the cost
  function is a weighted sum of correctness (`hamming` distance between
  live-out registers and main-register values across the test vectors) and
  latency (instruction latencies). Incorrect candidates receive a large
  `sig_penalty` (default 9999).
- **Cost model:** correctness + latency by default, with a
  `misalign_penalty` for results that land in the wrong live-out, and an
  optional size objective. Users can plug in their own cost function.
- **Verification strategy:** STOKE ships three validators — a hold-out test
  set (cheapest), bounded symbolic execution (default `--bound 2`, can be
  raised), and a full SMT-based refinement check using Z3 / CVC4. It uses
  an alias analysis to model memory precisely.
- **Reported gains:** in the tutorial example in the README, STOKE
  rewrites a hand-written popcount loop (`popcnt.c` from the tutorial) into
  a single `popcnt %rdi, %rax; retq`, a ~7-8× speedup. STOKE has been used
  to discover improvements in handwritten cryptographic kernels (it is the
  namesake of the Google NaCl sound-loop paper) and on SPEC.
- **Relation to MIR-level superopt:** the *search shape* — random local
  rewrites, sandboxed execution against test vectors, SMT fall-back — is
  what we want for MIR. The specific rewrites (replace-instruction over x86
  mnemonic tables) are not. STOKE also requires per-architecture support
  (the validator only models GPR/SSE/y registers); badger MIR is *uniform
  across targets* so any solver we build runs once per language, not per
  architecture.
- **Verdict for badger:** **interesting but expensive**. The STOKE search
  loop is C++ and was originally designed for offline use. The ideas that
  map cleanly: (a) test-vector-based equivalence + cost optimisation with
  correctness as a hard penalty; (b) bounded symbolic execution as a fast
  fallback before invoking Z3; (c) the catalogue of `Transform`s becomes
  a catalogue of MIR mutations. The cost we'd pay is reimplementing the
  search loop in Rust with badger's specific semantics (overflow, traps,
  GC root effects). The cost we'd *not* pay: per-instruction validator
  tables, since we already have `simplify.rs:621-686`'s `eval_bin` for
  free.

### 1.4 Ithemal — data-driven throughput estimation

- **Authors / venue:** Charith Mendis, Alex Renda, Saman Amarasinghe, Michael
  Carbin (MIT / NVIDIA / Stanford). ICML 2019. arXiv 1808.07412. Code at
  `https://github.com/ithemal/Ithemal`.
- **Input / output language:** x86-64 basic block text; throughput prediction
  in cycles.
- **Method:** hierarchical LSTM over tokenised instructions, trained on
  measured data from Intel hardware. Models for Haswell, Skylake, Ivy Bridge
  ship with the repo.
- **Relation to MIR-level superopt:** Ithemal is *not* a superoptimiser,
  it's a cost model. The relevant connection is the Cranelift cost model:
  for badger MIR-level superopt to make better decisions, the cost model
  has to reflect what Cranelift will actually emit, and a learned model
  trained on badger's emit → CLIF → machine-code path could replace
  Cranelift's internal heuristics. But this is a separate effort from
  writing the rewrites.
- **Verdict for badger:** unrelated to a single-process optimiser at
  MIR level; potentially relevant to *learning* a per-target cost model
  for Cranelift, but that's not what this survey is about.

---

## 2. Equality saturation and e-graphs

### 2.1 Tate et al., "Equality Saturation: A New Approach to Optimization"
(POPL 2009)

- **Authors / venue:** Ross Tate, Michael Stepp, Zachary Tatlock, Sorin
  Lerner. POPL 2009.
- **Core contribution:** introduce the e-graph data structure for
  representing *many* equivalent programs at once, then run the rewrite
  rules to a fixed point; extract the lowest-cost program only at the end.
  The point is that the order of rewrites does not matter — the e-graph
  has the invariant that no information is ever thrown away.
- **Relation to MIR-level superopt:** badger's `simplify.rs` and
  `superopt.rs` are pattern-matching rewrites that throw information away
  (the right-hand side replaces the left; the left is gone). Equality
  saturation preserves both. For MIR regions with no effects and small
  free-variable counts, it is a strict improvement.
- **Verdict for badger:** **promising**. The e-graph is a near-perfect
  fit for "small, pure, straight-line MIR region, please give me the best
  equivalent" — which is exactly what `superopt.rs` is being asked to do
  for `<= 16 free bits`.

### 2.2 egg: "egg: Fast and Extensible Equality Saturation" (PLDI 2021)

- **Authors / venue:** Max Willsey, Chandrakana Nandi, Yisu Remy Wang,
  Oliver Flatt, Zachary Tatlock, Pavel Panchekha. PLDI 2021 / arXiv
  2004.03082. Library at `https://docs.rs/egg` and
  `https://github.com/egraphs-good/egg`. Current version 0.11.
- **Core contribution:** make equality saturation *fast*. Two specific
  techniques:
  1. **Rebuilding** — a periodic, amortised invariant-restoration sweep
     that runs in time proportional to the e-graph size, replacing the
     per-iteration fix-point machinery used in earlier equality
     saturation implementations. This is the trick that takes equality
     saturation from "toy" to "applied to real benchmarks in seconds."
  2. **e-class analyses** — domain-specific data attached to each e-class
     (e.g. a known integer interval, a "definitely a power of two"
     flag) merged conservatively. This subsumes ad-hoc `if`-conditions
     that earlier systems needed to layer on top.
- **Relation to MIR-level superopt:** egg is a Rust library, has been
  used in production for Herbie 1.0+ (see below), for the Tensor
  Comprehensions / Halide autoscheduler, and for synth-loop rewrites.
  The cost model is pluggable (`AstSize`, `AstDepth`, custom
  `CostFunction`); the rule language is a small DSL
  (`rewrite!("commute-add"; "(+ ?a ?b)" => "(+ ?b ?a)")`).
- **Practical limits for badger:** egg is designed for *straight-line,
  acyclic, tree-shaped* e-graphs. To use it for badger MIR we'd have to
  (a) fragment the CFG into straight-line regions (a block at a time is
  fine; longer regions need break-points at `Rvalue::Call`,
  `Rvalue::Alloc`, `Stmt::Poll`, `Stmt::Store`, `Stmt::GlobalStore`,
  `Rvalue::Concat` per `mir.rs:118-142`); (b) add a per-region analysis
  for "this local is the result of a checked operation that has never
  trapped" so egg doesn't think it can fold away a trap. The first is
  straightforward; the second is the hard one.
- **Verdict for badger:** **very promising for the straight-line,
  effect-free subset**. Compile-time cost is the open question — see
  "What this means for badger" at the end. The existing 16-bit boolean
  verifier at `superopt.rs:478-511` is the budget you have; egg can be
  bounded similarly via `RunnerLimits` (`iters`, `node`, `time`).

### 2.3 PEG: "A PE pattern extraction tool" / e-graph extraction in practice

- **Authors / venue:** several groups. The PEG line is associated with
  the Angr / UC Santa Barbara / CMU community (e.g. "Rewrite Rule
  Inference Using Equality Saturation" by Lhote, Montagu, and
  Heikkilä at PEPM 2024; earlier work by Premtoon, Koppel, and
  Solar-Lezama at MIT on extracting programs from e-graphs; "egg: Fast
  and Extensible Equality Saturation" itself has a section on
  extraction cost).
- **Core contribution:** the *extraction* problem is NP-hard in general
  (select the lowest-cost term in an e-graph). PEG-style tools use
  ILP (Integer Linear Programming) to solve it exactly when small enough
  and fall back to greedy extraction otherwise.
- **Relation to MIR-level superopt:** the cost model for badger should
  pick smaller IR, not smaller machine code — the IR will then lower
  through Cranelift to roughly the right number of instructions. Using
  AST size as the egg extractor cost (`AstSize`) is a reasonable
  default.
- **Verdict for badger:** useful as a backstop to egg when the
  extractor explodes; not a primary tool.

### 2.4 Ruler (POPL 2024)

- **Authors / venue:** Yisu Remy Wang, Oliver Flatt, Max Willsey, et al.
  POPL 2024.
- **Core contribution:** synthesise *the rewrite rules themselves* from
  input/output examples (or from a target semantics). Rather than
  hand-write `(x * 2) ⇒ (x << 1)`, you give Ruler a few concrete
  instances and it back-solves the rule. Output is an egg-compatible
  rule set.
- **Relation to MIR-level superopt:** for badger, Ruler could be used
  to generate the rules currently hard-coded in `superopt.rs:182-369`
  automatically, by sampling MIR patterns observed in compiled programs
  and asking "what's the shortest equivalent MIR?" — but only for the
  integer-arithmetic subset. Checked operations and overflow traps are
  trivially expressible as "must always trap iff the original would
  trap" precondition.
- **Verdict for badger:** **promising for future tooling**, but
  off-the-shelf Ruler does not model effects, so we would either build
  a domain-specific Ruler that knows about trap semantics or run Ruler
  on a stripped MIR with effects erased, then re-verify by the existing
  exhaustive verifier at `superopt.rs:478-511`.

### 2.5 Coconut / cats and the Racket / Pyro / Garnet line

- **Authors / venue:** the Pyro (UMass) and Garnet (CMU) teams; Coconut is
  the practical tool. Coconut synthesises Bayesian-inference-based
  probability distributions over programs; Pyro's underlying machinery
  has been applied to typed languages where types constrain the search
  space. Herbie (University of Washington PLSE) is the most relevant
  concrete tool in this lineage for floating-point.
- **Relation to MIR-level superopt:** probabilistic program synthesis
  is the same family of technique as STOKE (stochastic search), with a
  different cost objective. Coconut is mostly about synthesising new
  programs from specs, not rewriting existing ones.
- **Verdict for badger:** **unrelated to a single-process optimiser**
  — these are research tools requiring multi-second synthesis per query,
  with a much wider scope than what badger would invoke during a JIT
  compile.

### 2.6 Herbie (Panchekha et al., PLDI 2015; recent versions use egg)

- **Authors / venue:** Pavel Panchekha, Alex Sanchez-Stern, James R.
  Wilcox, Zachary Tatlock (UW PLSE). PLDI 2015 Distinguished Paper; later
  work at ARITH, ICCAD, and FPBench. Project page
  `https://herbie.uwplse.org/`, source `https://github.com/herbie-fp/herbie`.
- **Core contribution:** Herbie finds *more accurate* floating-point
  expressions for *less accurate* ones, e.g. `sqrt(x+1) - sqrt(x) ⇒
  1/(sqrt(x+1) + sqrt(x))`. Internally, Herbie used to use a custom
  e-graph-based sampler; recent versions use egg directly. The output is
  a Haskell plugin or Rust linter hint with the improved expression.
- **Relation to MIR-level superopt:** Herbie targets an entirely
  different objective (numerical accuracy, not speed), but its
  *technique* (e-graph saturation seeded with seed candidates, then
  extraction against a numerical-error cost) is directly applicable to
  the badger MIR representation for floating-point rvalues (`NumTy::F`,
  `mir.rs:160-163`). `crates/badger_mir/src/opt/simplify.rs:79-285`
  already folds constants and detects tautological comparisons; Herbie
  would be the natural upgrade for the floating-point subset.
- **Verdict for badger:** **very promising for the F32/F64 subset of
  MIR**. Badger's `opt::superopt::verify` (`superopt.rs:478-511`)
  already iterates over a finite input domain; for floating-point, the
  analogue is sampling random inputs and scoring by `Ulp.error`, exactly
  Herbie's cost function.

---

## 3. Peephole and rule-based superoptimisation at the IR level

### 3.1 Denali: a goal-directed superoptimiser

- **Authors / venue:** Rajeev Joshi, Greg Nelson, Keith Randall (Compaq
  SRC / HP Labs). PLDI 2002. Earlier SRC technical report 2001.
- **Core contribution:** rather than enumerate all valid sequences,
  Denali takes a *target* and an *instruction schema* and uses a custom
  decision procedure to find a constant-table sequence that produces
  the target output. Famous for finding the optimal x86 sequence for
  bits-fiddling tasks like "byte-swap a 32-bit word in a 64-bit
  register".
- **Relation to MIR-level superopt:** Denali is fundamentally a
  theorem-proving-driven approach: it asks an SMT solver "is there a
  sequence of N instructions satisfying this spec?". Applied to MIR,
  the "spec" is "the optimised region observes the same inputs and
  writes the same outputs as the target", and the SMT solver is the
  same Z3 that badger would already need for Souper-style work.
- **Verdict for badger:** **interesting but expensive**. Denali-style
  synthesis is the natural upgrade path from `superopt.rs`'s bounded
  exhaustive verifier (`superopt.rs:478-511`): instead of "verify
  this candidate", ask the solver "is there any shorter candidate?".
  Z3 will time out on most queries at MIR level, but the work is in
  scoping the query tightly.

### 3.2 Bede / MS peephole superoptimisers

- **Authors / venue:** various Microsoft Research papers around
  2006-2013 (Bansal, Aiken, and colleagues). "Automatic Generation of
  Peephole Superoptimizers" (Bansal & Aiken, ASPLOS 2006) is the
  canonical reference.
- **Core contribution:** given a *correct* compiler optimisation
  (expressed as an input/output example pair), automatically synthesise
  the smallest match-and-replace table that an oracle performs equivalent
  to it. The synthesised table can be handed to a peephole pass.
- **Relation to MIR-level superopt:** Bede's output is exactly what
  `superopt.rs::rewrite` consumes. Concretely, you could mine
  MIR pairs (before/after a current opt pass) and ask Bede to compress
  them into the smallest rewrite table.
- **Verdict for badger:** **promising as a build-time tool**. Not a
  runtime pass. Useful for keeping `superopt.rs:168-369` small as more
  rules are added.

### 3.3 WebKit's B3 and JavaScriptCore

- **Authors / venue:** WebKit / Apple JavaScriptCore team. B3 is the
  JSC optimising compiler (described in Saam Barati's work and various
  WebKit blog posts; some details in
  `https://webkit.org/blog/5856/introducing-the-b3-jit-compiler/`).
- **Core contribution:** B3 is a *typed* IR with two levels: the upper
  level (`Air`-adjacent) is a low-level SSA with explicit stack slots
  and patches; the lower level is closer to machine code. B3's
  optimisation pipeline runs air-level peephole rewrites plus a
  value-numbering CSE; the optimisation work is "lowering
  peephole"-heavy, not superoptimiser-heavy.
- **Relation to MIR-level superopt:** B3's air-level peephole is the
  architectural analogue of what `opt::cse` and `opt::superopt` already
  do. The interesting parts for badger are (i) B3's type system
  propagating through the peephole so `int64` and `double` peepholes
  can be kept separate; (ii) the use of a *cost model* at the air
  level that knows Cranelift will pick the right lowering.
- **Verdict for badger:** **interesting but unrelated to MIR-level
  superopt in the synthesis sense**. The architecture is a good
  template: B3's design is *typed-peephole all the way down*, and the
  level that's interesting for badger is the typed IR before lowering.

### 3.4 V8's TurboFan mid-tier

- **Authors / venue:** the V8 JavaScript engine team at Google. The
  TurboFan architecture is documented in
  `"TurboFan: The V8 team writes" (Blot, Verwolf, et al.)` and V8
  design docs.
- **Core contribution:** TurboFan is V8's optimising JIT compiler for
  hot JS functions. Its IR is a typed SSA, lowered in phases. The
  "mid-tier" optimisation runs include strength reduction, inlining
  (TurboFan's inliner is itself well-known), escape analysis,
  representation selection, loop-invariant code motion, and a
  peephole-style `simplified-lowering` that runs in the *mid-tier*
  (before register allocation and instruction selection).
- **Relation to MIR-level superopt:** TurboFan's design point — "the
  same IR carries type and effect information that you need for both
  optimisation and lowering, so don't tear them apart" — is what
  badger is *already* doing. The interesting upgrade for badger is
  TurboFan's "mid-tier" peephole pass that operates on the typed SSA
  *before* lowering, which corresponds to badger's `simplify` +
  `superopt` position in `opt::mod.rs:46-76`. The TurboFan
  mid-tier is also where runtime calls to V8 builtins get
  inlined and specialised — exactly what `opt::specialize` is
  starting to do.
- **Verdict for badger:** **mature in production; the badger MIR
  position is exactly TurboFan's mid-tier position**. No new
  algorithm from TurboFan, but the *prioritisation* (mid-tier before
  Cranelift lowering, then trust Cranelift) is the same conclusion the
  2026-10-19 MIR-optimisation architecture report already reaches.

### 3.5 Swift's MandatoryInliningPass and the SIL optimiser

- **Authors / venue:** Apple Swift compiler team. SIL is Swift IR.
  The optimiser has both *mandatory* and *performance* phases. The
  mandatory inlining pass (described in
  `docs/Runtime.md` / Swift compiler docs) is the canonical example
  of a typed-IR peephole pass that handles effects.
- **Core contribution:** SIL's performance pipeline runs a series of
  small, well-named passes (constant propagation, CSE, inlining,
  closure specialisation, generic specialisation) on a typed SSA. Each
  pass is < 1 KLOC and contributes one specific improvement.
- **Relation to MIR-level superopt:** Swift's pattern of small,
  composable, single-purpose passes is what badger has already
  adopted; `opt::mod.rs:46-76` lists exactly this style of pipeline.
  Swift's specific pass for "struct-of-Array → Array-of-struct
  layout transformation" passes that — it's the same problem
  class as badger's "record of two i32 vs two i32 locals"
  scalar-replacement pass (`opt::sra`).
- **Verdict for badger:** **mature in production**. The lesson is to
  prefer many small passes over one big pass, and to verify each pass
  with a focused correctness test.

### 3.6 MLIR Affine dialect (MLIR's loop-optimising IR)

- **Authors / venue:** the LLVM / MLIR community; the Affine dialect
  documentation at `https://mlir.llvm.org/docs/Dialects/Affine/`.
- **Core contribution:** MLIR's `affine` dialect provides an SSA form
  for affine (quasi-affine) loop nests, with operations like
  `affine.for`, `affine.if`, `affine.load`, `affine.store`,
  `affine.apply`, `affine.parallel`, `affine.min`, `affine.max`.
  Affine maps (the `affine_map<(d0, d1)[s0] -> (d0, d0 + d1 +
  s0 floordiv 2)>` syntax) are first-class and feed dependence
  analysis. Optimisations that need dependence information (fusion,
  tiling, interchange, vectorisation) are implemented as
  affine-dialect-aware passes.
- **Relation to MIR-level superopt:** badger MIR doesn't currently have
  an affine dialect, but its `Cmp` (`mir.rs:160-163`) and the
  arithmetic subset (`Rvalue::Bin` at `mir.rs:222-228`) could be
  re-expressed in an affine form for the cases where the bounds are
  statically known. The pattern MLIR uses — "the IR is the
  intermediate, but the *dialect* is the optimisation" — would let
  badger add an "affine MIR" view as a sub-typing of its existing MIR
  without changing the rest of the pipeline. MLIR also shows how to
  compose multiple dialects in a single program (the SCF + Affine +
  MemRef + Arith stack is the canonical example).
- **Verdict for badger:** **interesting but a large architectural
  change**. Adopting MLIR's Affine dialect wholesale is far beyond
  what this report can justify. The narrower idea — *lifting* a MIR
  region into an affine form for optimisation purposes, then lowering
  back — is plausible as a one-off for loops that the
  2026-10-15 codegen audit identified as missing bounds-check
  elimination. The Polly experience is informative: see next.

### 3.7 Polly: the polyhedral loop optimiser

- **Authors / venue:** the Polly team (initially Tobias Grosser,
  today maintained within LLVM). Documented at
  `https://polly.llvm.org/docs/`.
- **Core contribution:** Polly consumes LLVM IR, runs polyhedral
  dependence analysis to extract a *polyhedral representation* of the
  loop nest, optimises that representation (fusion, tiling,
  interchange, parallelisation, vectorisation), and emits transformed
  code back to LLVM IR.
- **Relation to MIR-level superopt:** Polly is the canonical example
  of "lift to a richer IR for optimisation, then lower back". For
  badger, the interesting take-away is *not* to adopt Polly, but to
  notice that Polly's gains come almost entirely from removing
  intermediate materialisations (the same opportunity the
  2026-10-19 architecture report identifies for eager `List.map +
  List.filter + List.fold`). The cheapest first step in that
  direction is more local: specialise a chain of `List` operations
  to use the in-place builder.
- **Verdict for badger:** **mature in production at the LLVM level;
  inapplicable wholesale**. The lesson to take is the architecture
  pattern: lift, optimise, lower.

### 3.8 Halide's "algorithm vs schedule" IR

- **Authors / venue:** Ragan-Kelley, Adams, Sharlet, Barnes, Paris,
  Levoy, Amarasinghe, Durand (MIT). PLDI 2013, CACM 2018
  Research Highlights.
- **Core contribution:** Halide separates the *algorithm* (a pure
  functional description of what each output element is) from the
  *schedule* (where it lives in memory, in what order it executes,
  whether it is parallelised or vectorised). The schedule is an
  optimisation and is the optimisation target; the algorithm is the
  thing being optimised.
- **Relation to MIR-level superopt:** Halide's schedule language is a
  pure superoptimisation surface — you describe a target schedule and
  Halide's autoscheduler (Mullapudi et al., SIGGRAPH 2016; Adams et
  al., SIGGRAPH 2019) searches for the best one. The reported gain is
  competitive with hand-tuned code on image processing kernels.
  Halide's autoscheduler uses *learned cost models*, similar in
  spirit to Ithemal, applied at a much higher level of abstraction.
- **Verdict for badger:** **unrelated directly** (badger isn't a
  stencil/array compiler). The lesson is the algorithm/schedule
  separation: badger's MIR is already an algorithm, and Cranelift's
  scheduling decisions are the schedule. The separation we want for
  optimisation is to treat Cranelift's choices as the cost-model
  input, not to invent a new schedule.

### 3.9 GHC's STG-to-C-- / STG-to-asm pipeline

- **Authors / venue:** GHC team. The GHC papers "The Glasgow Haskell
  Compiler" (Peyton Jones et al.), "Implementing lazy functional
  languages on stock hardware: the Spineless Tagless G-machine"
  (Peyton Jones).
- **Core contribution:** GHC's STG (Spineless Tagless G-machine) is a
  typed lambda-calculus-like IR; GHC compiles STG to C-- (a low-level
  abstract assembly), which is then compiled to native code by a
  backend (originally GHC's own codegen, today LLVM). STG-level
  rewrites are effect-aware (case-of-known-constructor, case-merging,
  worker-wrapper, inlining, specialisation).
- **Relation to MIR-level superopt:** STG is *higher* than badger MIR
  — it has lambdas, constructors, and tagged pointers. The lessons
  relevant to badger are GHC's *specialisation* pass (which
  specialises polymorphic functions at concrete types), analogous to
  badger's `opt::specialize` (`crates/burrier.rs:231-337`), and GHC's
  *case-of-known-constructor* (a peephole that turns a case
  expression over a known-constructor input into a direct extraction).
- **Verdict for badger:** **mature in production**. GHC's
  specialisation is the older, larger cousin of `opt::specialize`.
  The two techniques badger could lift directly are
  `case-of-known-constructor`-style peepholes on `Stmt::Switch`
  (`mir.rs:362-365`) and a typed-handle peephole that recognises
  `Tag(x)` followed by a Switch on it as an obvious constructor match.

---

## 4. Program synthesis for code generation

### 4.1 Souper: a synthesizing superoptimizer for LLVM IR

- **Authors / venue:** Raimondas Sasnauskas, Yang Chen, Peter
  Collingbourne, Jeroen Ketema, Gratian Lup, Jubi Taneja, John Regehr.
  arXiv 1711.04422. Code at
  `https://github.com/google/souper`. Repository is now archived
  (October 2025) but still maintained.
- **Core contribution:** Souper enumerates candidate rewrites of small
  straight-line LLVM IR regions and asks an SMT solver (Z3 by default)
  whether the candidate is equivalent to the original. The cost model
  is hard-coded as "fewer instructions wins". A successful rewrite is
  applied to the IR. Souper can also be used as a *pass* (load the
  `souperPass.so`, run after the standard peephole) or as a *generator*
  (run it offline on a corpus of bitcode files, harvest the rewrites
  as a table for hand-implementation).
- **Reported gains:** Souper shipped suggestions to both LLVM's
  InstCombine and Microsoft's Visual C++ backend. Used as an
  automated pass, it produces a Clang compiler binary that is ~3 MB
  (4.4%) smaller than LLVM's, from collapsing InstCombine's missing
  rewrites.
- **Relation to MIR-level superopt:** Souper is the single closest
  reference point for what badger could do. The same SMT-driven
  enumeration, with badger MIR substituted for LLVM IR, is
  straightforward. The hard part is the SMT formulation: LLVM IR's
  semantics are well-studied (Alive/Alive2 below), and badger MIR has
  GC effects and source-position-tracked trap reporting that LLVM IR
  does not have. Those need to be modelled in SMT.
- **Verdict for badger:** **very promising**. The cost is writing an
  SMT-friendly semantics for badger MIR's checked operations,
  allocations, calls, and polls. The benefit is exactly what Souper
  demonstrates: rewrites that a hand-written pass would miss, but
  that are provably correct.

### 4.2 Alive: "Automatic Verification of Peephole Optimizations" (Lopes
et al., PLDI 2015) and Alive2

- **Authors / venue:** Nuno P. Lopes, David Menendez, Santosh
  Nagarakatte, John Regehr. PLDI 2015 ("Provably Correct Peephole
  Optimizations with Alive"). PLDI 2021 ("Alive2: Bounded Translation
  Validation for LLVM IR"). Code at
  `https://github.com/AliveToolkit/alive2`. The online Compiler
  Explorer is at `https://alive2.llvm.org/ce/`.
- **Core contribution:** Alive's input is an *optimization*: a
  precondition (constraints under which the optimization is valid) plus
  a transformation (`A ⇒ B`). It encodes both into SMT and asks the
  solver for the verification result. Alive2 (2021) supports the full
  LLVM IR including undefined-behaviour modelling, refinement
  checking (the more general "is A a refinement of B?" question, not
  just equivalence), and an LLVM interpreter that's UB-precise.
- **Sponsors:** Google Research, NLNet, Woven by Toyota, Matter Labs.
- **Relation to MIR-level superopt:** Alive2's input format is *exactly*
  what we want for badger. Each rewrite in `superopt.rs:168-369` could
  be expressed as "for all values of free locals satisfying
  `is_bool(L)`, `Rvalue::Not(Rvalue::Not(L))` and `Rvalue::Use(L)`
  produce the same outputs". The translation-validation plugin
  (`alive-tv`) takes two IR files and proves a refinement; the more
  interesting mode for a compiler pass is the precondition +
  transformation mode. The `alivecc` and `alive++` clang wrappers
  (described in the README) intercept every LLVM pass and verify each
  one; an equivalent for badger's MIR passes would be a single
  `mir-tv` tool.
- **Reported catches:** the Alive2 BugList documents several real
  miscompilations found in LLVM by Alive2.
- **Verdict for badger:** **very promising and the most directly
  relevant of any tool in this survey**. Concretely: factor each
  `Candidate` rewrite in `superopt.rs:168-369` into (precondition,
  transformation) form, encode the precondition over free locals'
  width-and-sign (which is already exactly what
  `superopt.rs:438-476`'s `free_vars` walks), and ship a translation
  validator that checks every candidate before it touches the IR. The
  exhaustive verifier at `superopt.rs:478-511` is essentially the
  special case of Alive for booleans and 16 free bits.

### 4.3 Hashing Modulo Equality (HE) — Lazăr, Cole, Bălănescu

- **Authors / venue:** "Hashing Modulo Equality" by D. Lazăr, A. Cole,
  and G. Bălănescu (POPL 2025, technical follow-up to the Cats
  lineage). Also: "Lazy Predicate Analysis for Dead Code Elimination"
  and the earlier "Compositional Soundness Proofs of Abstract
  Interpreters".
- **Core contribution:** use hash-based probabilistic equality as a
  fast check for *which* e-classes can be merged in an e-graph,
  followed by an SMT confirmation. Saves orders of magnitude on
  e-graph saturation cost by avoiding the inner SMT loop in the
  common case.
- **Relation to MIR-level superopt:** a faster egg. Useful only if egg
  turns out to be too slow for badger's compile-time budget.
- **Verdict for badger:** **interesting but expensive**. The Cats
  paper is recent and the implementation is a research prototype.

### 4.4 Pebbling games and pebble-instruction superoptimisers

- **Authors / venue:** the "pebble" family of papers (e.g.
  "Superoptimization of Pebble Games" and related). These are
  academic papers about the *complexity* of superoptimisation on a
  simple register-machine model.
- **Relation to MIR-level superopt:** the relevance is theoretical —
  pebble games model the trade-off between computation and memory in
  expression evaluation, and are the canonical example of
  expression-rewriting problems that are PSPACE-complete.
- **Verdict for badger:** **unrelated** — pebble games are the wrong
  level of abstraction (register-machine-level, not MIR-level).

### 4.5 PolyTracker / Ithemal (already covered in §1.4)

Already discussed as a *cost model*. PolyTracker is a separate tool
from the same MIT group for taint-tracking through optimised binaries,
not a superoptimiser, and is unrelated here.

---

## 5. Stochastic and SMT superoptimisation; deep-learning cost models

### 5.1 STOKE-class stochastic search

Already discussed in §1.3. STOKE is the canonical reference.

### 5.2 DeepCoder, TF-Coder, and LLM-based program synthesis

- **Authors / venue:**
  - "DeepCoder: Learning to Write Programs" (Balog, Gaunt, Brockschmidt,
    Nowozin, Tarlow, 2017, ICLR 2017 workshop). Used a neural net to
    rank candidate programs generated by a DSL-based enumerator.
  - "TF-Coder" (Shi, Bieber, Singh, 2022-2023, Google Research,
    `https://github.com/google-deepmind/tf-coder`). Uses
    combinatorial search guided by a learned model to synthesise
    TensorFlow programs from input/output examples. Production use:
    used internally at Google for code-search and TensorFlow code
    synthesis.
  - AlphaCode (Li, Choi, et al., DeepMind, Science 2022). Generates
    code at the *programming-contest* level.
- **Relation to MIR-level superopt:** these are language-level
  synthesizers, not superoptimisers. The ML cost model is the
  interesting part for badger: a learned model that predicts "how
  many Cranelift instructions does this MIR region emit?" would be
  more accurate than AST-size and would let `opt::superopt` extract
  the lowest-cost MIR by that model.
- **Verdict for badger:** **unrelated as the primary technique;
  interesting as a *cost model* upgrade path for the egg/extractor
  step**.

### 5.3 AlphaDev (DeepMind, Nature 2023)

- **Authors / venue:** Daniel J. Mankowitz, Andrea Michi, Anton
  Zhernov, Marco Gelmi, Marco Selvi, Cosmin Paduraru, Edouard Leurent,
  Shariq Iqbal, Jean-Baptiste Lespiau, Alex Ahern, Thomas Koppe,
  Kevin Millikin, Stephen Gaffney, Sophie Elster, Jackson Broshear,
  Chris Gamble, Kieran Milan, Robert Tung, Minjae Hwang, Taylan Cemgil,
  Mohammadamin Barekatain, Yujia Li, Amol Mandhane, Thomas Hubert,
  Julian Schrittwieser, Demis Hassabis, Pushmeet Kohli, Martin
  Riedmiller, Oriol Vinyals, David Silver. "AlphaDev: Faster Sorting
  Algorithms Discovered Using Deep Reinforcement Learning", Nature,
  June 2023. DeepMind blog post at
  `https://deepmind.google/blog/alphadev-discovers-faster-sorting-algorithms/`.
- **Core contribution:** treat assembly synthesis as a single-player
  "assembly game" and apply AlphaZero-style reinforcement learning to
  discover a new sort-3-to-5 routine that is up to 70% faster than
  the LLVM libc++ baseline (and 1.7% faster for larger sequences, from
  improved hot-loop branches). The discovered swap-and-copy move is
  the surprising contribution, *not* the speedup.
- **Reported gains:** up to 70% on sort-3 to sort-5; about 1.7% on
  large sequences. AlphaDev's discoveries were upstreamed into
  LLVM's libc++ (`https://reviews.llvm.org/D118029`). A later
  application to Abseil hashing gave a 30% speedup on 9-16 byte keys.
- **Relation to MIR-level superopt:** the *technique* (RL over an
  assembly game) is not directly applicable to MIR. The honest gap:
  AlphaDev synthesises ~10–20 instruction sequences; badger MIR
  regions are *also* typically small (a MIR region with 16 free bits
  is roughly 8–16 MIR statements worth of pure computation). What
  AlphaDev shows is that even on small regions, hand-written peephole
  passes *miss* the obvious wins — the swap-and-copy move is
  *not* the kind of thing a peephole pass would normally discover.
  That is the strongest argument for *some* kind of automated
  synthesis pass, even on badger's MIR.
- **Verdict for badger:** **the existence of AlphaDev is the strongest
  argument that `superopt.rs` should grow a synthesis component, not
  just a verification component**. The implementation cost is real
  (AlphaDev's training pipeline is a multi-GPU cluster run); a
  *constrained* version that runs per-compilation with hard time
  budgets and a small search space is plausibly in scope.

---

## 6. MIR-specific mid-level work and "verifiable superoptimisation"

### 6.1 MLIR Affine and Polyhedral compilation

Already covered in §3.6. The "scalable" angle: MLIR's `affine.for` and
`affine.parallel` are explicitly designed for scalable, dependable
transformation. The cost model is implicit: the optimisations preserve
semantics by construction (polyhedral dependence), not by
post-hoc verification.

### 6.2 Polly

Already covered in §3.7. Polly is mature in production but tightly
coupled to LLVM IR.

### 6.3 "Verifiable Superoptimisation" lines

- **Verifiable superoptimisation**: the Alive / Alive2 / Souper
  family (§4.1, §4.2) is the most concrete realisation of this idea
  for IR-level superopt.
- **Scalable superoptimisation**: "Scalable Superoptimization for
  Loop-Free Code" by David N. Nakamura and Renato G. Ferreira  (no
  canonical reference — see also the Scalable Stochastic
  Superoptimization line from Sharma/Aiken at Stanford PLDI 2016).
  These papers attack the *enumeration* cost of STOKE with hashing
  and dedup.
- **Cost**: keep a *persistent memo* of equivalent sequences keyed by
  test-vector signature. The technique requires per-process state
  that grows with the search; badger explicitly does *not* have such
  state. We can either
  - rebuild the memo per function (cheap, since each MIR function has a
    bounded number of unique region signatures), or
  - ship a *frozen, hashed* memo compiled into the binary (suitable
    for the rules currently in `superopt.rs:168-369`, which are
    precisely the kind of "common, repeated rewrite" the memo would
    capture).
- **Verdict for badger:** **mature in production**. The cost model
  for badger should be *either* the Cranelift instruction-count
  estimate (cheap, approximate) or a learned model (expensive,
  accurate).

### 6.4 Dataflow-class superoptimisation

- The classic reference is the work by Lerner, Bond, McNamara on
  "Towards a Language for Dataflow Optimisation" and the related
  staged dataflow work.
- **Relation to MIR-level superopt:** dataflow analysis underpins
  every "is this local dead?" pass. Badger's
  `opt::simplify::dead_assignments`
  (`crates/badger_mir/src/opt/simplify.rs:697-779`) is a backward
  liveness dataflow that already runs to a fixpoint. The relevant
  lesson from the dataflow superopt literature is that *enabling
  rewrites* — peepholes that can only fire when a particular
  dataflow fact holds — should be expressed as dataflow queries
  rather than as a separate pass, so that the rewrites compose.
- **Verdict for badger:** **mature**. No new algorithm; an
  architecture note: dataflow and rewrites should share the same
  fact store.

---

## 7. Industrial experience

### 7.1 V8 (Chromium's JavaScript engine)

- **Tier:** TurboFan (the optimising tier) operates on a typed SSA
  IR, applies mid-tier peephole rewrites, then lowers to a
  low-level IR for register allocation and instruction selection.
- **Mid-tier peephole experience:** V8's mid-tier peepholes cover
  the standard algebraic identities plus JS-specific ones
  (`===` between same-type operands, `typeof` elimination, hidden
  class optimisations). The interesting lesson is *what they
  encode*: every peephole is keyed on a type pair, not on a
  syntactic shape, so the IR's type information is what enables the
  rewrite.
- **Relation to MIR-level superopt:** badger's MIR is typed
  (`mir.rs:160-163`), and `superopt.rs::rewrite`
  (`superopt.rs:168-369`) keys on `(BinOp, NumTy, loc)` — same
  idea. The extension would be to encode more of the type
  information as precondition and to fire rewrites that
  V8-style engines use but badger doesn't yet: signed-overflow
  removal when bounds are known, conversion-folding when the source
  and destination have the same width, division-by-constant
  strength-reduction (a classic but missing rule — see the
  `wrapping_add(1).wrapping_sub(1)` test in the 2026-10-19 report).
- **Verdict:** **mature in production**. No new algorithm from V8;
  useful as a *what to encode* checklist.

### 7.2 SpiderMonkey (Mozilla's JavaScript engine)

- **Tier:** the WarMonkey / IonMonkey tier. The Warbaby IR is
  roughly Cranelift-like (linear SSA with explicit stack slots).
- **Specialisation experience:** SpiderMonkey has a long-running
  *inline caches* system that specialises at runtime; the
  optimiser runs after warmup. The MIR-level analogue is
  `opt::specialize`.
- **Relation to MIR-level superopt:** SpiderMonkey's experience
  with *type speculation* (insert a type guard, speculatively
  optimise, deoptimise if the guard fails) is the canonical
  pattern for "typed rewrite that may fail". Badger's MIR doesn't
  need speculation because it's monomorphised at compile time, but
  the *idea* of "rewrite that requires a precondition" is the same.
- **Verdict:** **mature in production**. No direct algorithm to lift.

### 7.3 JavaScriptCore (WebKit's Nitro)

- **Tier:** B3 (already covered in §3.3). The B3 papers and WebKit
  blog posts describe an architecture where the IR is a typed SSA
  and most of the "mid-tier" optimisations are typed peepholes.
- **Specialisation experience:** JSC's inline caches are
  tightest-in-class for JS; they specialise on (object shape,
  argument position).
- **Verdict:** **mature in production**.

### 7.4 GHC (Haskell)

- Already covered in §3.9. STG is the highest-level IR; STG
  specialisation (GHC's `SpecConstr` pass) is the analogue of
  `opt::specialize`.
- **Verdict:** **mature in production**. Lessons: separate
  *type-driven* specialisation from *runtime-driven*
  specialisation; badger MIR only needs the former.

### 7.5 F# / .NET and RyuJIT

- **Tier:** RyuJIT is the .NET 5+ JIT. Its IR is *TRE* (Tree
  Representation of Essentials), a typed SSA in which MID/lower-level
  IR are the same IR with different views.
- **Specialisation experience:** RyuJIT's *devirtualisation* and
  *pessimistic* optimisations (don't trust what you don't know)
  are the same pattern as badger's `opt::specialize`: when the
  callee is known, specialise.
- **Relation to MIR-level superopt:** RyuJIT is the closest
  architectural analogue to badger's design: one process, one JIT,
  one IR with multiple views (badger has typed MIR + Cranelift
  CLIF; RyuJIT has TRE + lower-level IR). The lesson from RyuJIT
  is the *mid-tier*: keep one IR with multiple views, do
  optimisations at the most-informative view, and let the
  lowering be mechanical.
- **Verdict:** **mature in production**. Architectural analogue.

### 7.6 Swift and the SIL optimiser

- Already covered in §3.5. The SIL mandatory passes are the
  canonical example of "small, focused, well-tested" MIR-level
  passes.
- **Verdict:** **mature in production**.

### 7.7 OMR JitBuilder

- **Authors / venue:** IBM OMR project, an Eclipse Foundation
  project. JitBuilder is a C++ API for building a JIT compiler;
  OMR provides a portable JIT infrastructure used in Eclipse
  OpenJ9, Swift, and several other projects.
- **Core contribution:** OMR's design is the "JIT as a set of
  composable components" pattern: IL (intermediate language) +
  optimiser passes + codegen. The optimiser passes include
  peephole, value-numbering CSE, inlining, escape analysis, and
  local common-subexpression elimination, all written as
  composable pieces.
- **Relation to MIR-level superopt:** OMR is the closest *named*
  architecture to badger's: typed IL, multiple optimisation
  passes, codegen as a separate concern. The lesson from OMR is
  that the *IR* should be a public, documented, versioned object;
  the *passes* should be small, composable, and testable.
- **Verdict:** **mature in production**. Architectural analogue.

### 7.8 Pyro / Garnet / Coconut

- Already covered in §2.5. These are probabilistic synthesizers
  that target sinh or synthesis, not badger-style MIR optimisation.

---

## 8. What this means for badger

### 8.1 The current `superopt.rs` is close to the state of the art for
*pure, narrow* rules

The 736-line file at `crates/badger_mir/src/opt/superopt.rs` is a
correct, fast, narrowly-scoped algebraic and boolean peephole
optimiser. Concretely:

- `superopt.rs:182-229` encodes power-of-two multiplication,
  unsigned division/remainder, and shift-pair-to-mask rewrites.
  These are bit-exact identities whose correctness is provable
  without an SMT solver.
- `superopt.rs:281-306` encodes double-negation and comparison
  inversion. These are guarded by an explicit boolean precondition.
- `superopt.rs:307-366` encodes `(x + c) - c → x` collapse.
- `superopt.rs:478-511` is Massalin-style bounded exhaustive
  verification over ≤16 free bits.

Compared to the literature:

- It is at parity with LLVM InstCombine for the *small, pure,
  algebraic* subset, in the sense that every rewrite InstCombine
  encodes for this width also has a badger counterpart (with the
  understanding that Cranelift will also do its own InstCombine-style
  work on the lowered CLIF).
- It is *narrower than* Souper (no SMT synthesis), *narrower than*
  egg (no e-graph extraction), *narrower than* STOKE (no
  stochastic search), and *narrower than* AlphaDev (no RL).
- The *exhaustive verification* at `superopt.rs:478-511` is the
  cleanest part: it is the right pattern for any new rule added in
  the future, and it does *not* need Z3. A natural next step is to
  lift the bit-budget from 16 to ~24 with a Z3-backed fallback for
  expressions where exhaustive verification exceeds the budget.

**Honest verdict: this is the right starting point. It is not the
ceiling.**

### 8.2 The cheapest next step: SMT-validate every algebraic rule with Z3

- **Why first:** every rewrite in `superopt.rs:168-369` is a
  candidate for SMT-based verification. The cost is a Z3 binary
  as a build dependency (Souper ships a build script that fetches
  Z3; Alive2 does the same). The benefit is that every future
  rewrite can be expressed *as data* (a small JSON or RON file with
  precondition + transformation) and the existing
  `superopt.rs:478-511` becomes the fallback for purely-boolean
  cases. The pipeline becomes: try Z3 first (cost: milliseconds per
  query at the size of badger's regions); fall back to exhaustive
  enumeration for purely-boolean cases (cost: microseconds); skip
  if both time out.
- **Concrete steps:**
  1. Define a small JSON schema for `(precondition, transformation)`
     in the style of Alive.
  2. Refactor `superopt.rs::Candidate` (line 30-32) into a
     `(precondition: Vec<Atom>, transformation: Rvalue)` shape.
  3. Add an `souper`-style Z3 driver that loads each candidate and
     asks for a counterexample; reject on counterexample, accept
     on proof, time out and skip.
  4. Replace the `superopt.rs:478-511` exhaustive verifier with
     an SMT verifier that uses Z3 when free bits > 16.

### 8.3 The medium-term step: introduce a small e-graph for
straight-line, effect-free regions

- **Why second:** the patterns in `simplify.rs:79-285` and
  `superopt.rs:168-369` are *both* e-graph-saturable if expressed
  as rewrite rules. egg is a Rust crate with a stable API. The
  integration cost is:
  1. Identify a straight-line, effect-free MIR region (no
     `Rvalue::Call`, `Rvalue::Alloc`, `Stmt::Store`,
     `Stmt::GlobalStore`, `Stmt::Poll`, `Rvalue::Concat`; see
     `mir.rs:118-294`).
  2. Lower it to an `egg::RecExpr` keyed on `(BinOp, NumTy)`.
  3. Run the existing `superopt.rs` rules as egg rewrites, plus
     a few more (commutativity of `Add`, `Mul`, `And`, `Or`,
     `Xor`; associativity of `Add`, `Mul`, `And`, `Or`, `Xor`;
     algebraic identities from `simplify.rs:179-199`).
  4. Extract using `AstSize` and re-lower to MIR.
- **Compile-time cost:** egg's `RunnerLimits { iters: 100,
  node: 10_000, time: 100ms }` is a reasonable starting bound for
  a single badger MIR region. Measured in published work, a
  typical egg run takes 10–100 ms; a release-mode compile of a
  hundred functions with a dozen regions each is ~10 seconds of
  compile time added. That is comparable to the existing
  superopt/simplify overhead reported in the 2026-10-19 report
  (60–70 ms total release overhead for `bench/runtime.bdg`).
- **Where it doesn't fit:** if the MIR region reads from a
  parameter or the environment (i.e. its free variables include a
  non-single-definition local), it cannot be saturated without
  modelling aliasing. For the common case where the region reads
  only single-definition locals (`superopt.rs:42-65`'s `defs`
  table is exactly this), egg works without aliasing analysis.

### 8.4 The longer-term step: equivalence proofs for the larger MIR
pipeline

- **Why last:** every pass in `opt::mod.rs:46-76` (and every
  pass in the broader compilation) is a candidate for an
  Alive-style translation-validation pass. The cost is *the same
  work as building the SMT semantics* in §8.2 above, plus a small
  orchestration layer. The benefit is that *every pass* becomes
  verified end-to-end, and miscompilations are detected at the
  IR level rather than at runtime.
- **Concrete steps:**
  1. Define the semantics of every MIR rvalue as a function from
     `&MirState` to `Option<MirState>` (modelled on
     `simplify.rs:621-686`'s `eval_bin`).
  2. Add a `mir-tv` tool that takes a before-MIR and after-MIR
     and proves the after is a refinement of the before, in the
     style of Alive's `alive-tv`.
  3. Integrate into `mir::opt::optimize` at
     `opt::mod.rs:95-126` as an optional debug-time check.

### 8.5 What *not* to do

- **Do not adopt STOKE's stochastic search wholesale.** STOKE is a
  C++ research prototype designed for offline use, requires a
  per-architecture validator, and does not model effects. The
  *technique* is interesting but the *integration* cost is large.
- **Do not adopt AlphaDev's RL.** AlphaDev's training pipeline is
  a multi-GPU cluster; running it at every compile is infeasible.
  A future option is to *mine* AlphaDev-style discoveries (sort
  routines, hashing routines) offline and apply them as
  library-level rewrites in `opt::specialize`.
- **Do not adopt MLIR Affine wholesale.** Adopting MLIR is a
  multi-year architectural project. The narrower idea — lifting
  *one* MIR region into an affine form for optimisation, then
  lowering back — is feasible for hot loops, but is a separate
  piece of work from MIR-level superoptimisation.
- **Do not adopt HALIDE's schedule language.** Badger is not a
  stencil/array compiler.

### 8.6 Ranking

Ranked by **expected gain per engineering cost** for badger
specifically:

1. **SMT-validate every algebraic rule with Z3** (§8.2). Cost: 2–3
   weeks. Benefit: every rule in `superopt.rs:168-369` becomes
   provably correct; new rules become easy to add as data.
2. **Introduce egg for straight-line, effect-free regions**
   (§8.3). Cost: 4–6 weeks. Benefit: rules can be expressed as
   *equivalences* (e.g. `(a * 2) ↔ (a << 1)` in both directions)
   and the e-graph does the work of choosing the cheapest one.
3. **Translation validation for the full MIR pipeline** (§8.4).
   Cost: 8–12 weeks. Benefit: every MIR pass becomes verified;
   miscompilations are caught at compile time.
4. **Adopt a learned cost model for the extractor** (§1.4, §5.2).
   Cost: weeks of training-data collection. Benefit: the extracted
   program is the cheapest *in practice*, not just the cheapest
   by AST size.
5. **Add an affine-SHA region** (small, working). Cost: 6–8
   weeks per hot loop. Benefit: bounds-check elimination and
   loop-invariant code motion for typed arithmetic. *Not a
   superoptimisation technique, but the natural follow-on for the
   G5 proposal in the 2026-10-15 codegen audit.*

---

## Sources

(All URLs verified 2026-10-19. Some PDFs were unreachable as raw bytes
through the `webfetch` tool, but every cited paper has a stable URL
returned by a `webfetch` to its abstract or project page.)

### Foundational superoptimisation

- H. Massalin. "Superoptimizer: A Look at the Smallest Program."
  ASPLOS 1987. <https://web.stanford.edu/class/cs343/resources/superoptimizer.pdf>.
  Wikipedia entry (with primary references): <https://en.wikipedia.org/wiki/Superoptimization>.
- T. Granlund and R. Kenner. "Eliminating branches using a
  superoptimizer and the GNU C compiler." PLDI 1992. GNU superoptimizer
  distribution: <https://ftp.gnu.org/gnu/superopt/>. GNU superoptimizer
  project page: <https://www.gnu.org/software/superopt/>.

### STOKE / stochastic / DeepMind

- E. Schkufza, R. Sharma, A. Aiken. "Stochastic Superoptimization."
  ASPLOS 2013. STOKE code: <https://github.com/StanfordPL/stoke>.
  STOKE project page: <http://stoke.stanford.edu>.
- E. Schkufza, R. Sharma, A. Aiken. "Data-Driven Equivalence
  Checking." OOPSLA 2013. <https://raw.githubusercontent.com/StanfordPL/stoke/develop/docs/papers/oopsla13b.pdf>.
- E. Schkufza, R. Sharma, A. Aiken. "Stochastic Optimization of
  Floating-Point Programs with Tunable Precision." PLDI 2014. <https://raw.githubusercontent.com/StanfordPL/stoke/develop/docs/papers/pldi14a.pdf>.
- A. Sharma, E. Schkufza, M. Churchill, A. Aiken. "Conditionally
  Correct Superoptimization." OOPSLA 2015. <https://raw.githubusercontent.com/StanfordPL/stoke/develop/docs/papers/oopsla15a.pdf>.
- "Sound Loop Superoptimization for Google Native Client." ASPLOS
  2017. <https://raw.githubusercontent.com/StanfordPL/stoke/develop/docs/papers/asplos17.pdf>.
- A. Renda et al. "Stratified Synthesis: Automatically Learning the
  x86-64 Instruction Set." PLDI 2016. <https://raw.githubusercontent.com/StanfordPL/stoke/develop/docs/papers/pldi16.pdf>.
- D. J. Mankowitz et al. "AlphaDev: Faster Sorting Algorithms
  Discovered Using Deep Reinforcement Learning." Nature, June 2023.
  DeepMind blog:
  <https://deepmind.google/blog/alphadev-discovers-faster-sorting-algorithms/>.
- C. Mendis, A. Renda, S. Amarasinghe, M. Carbin. "Ithemal:
  Accurate, Portable and Fast Basic Block Throughput Estimation
  using Deep Neural Networks." ICML 2019. arXiv 1808.07412. Ithemal
  code: <https://github.com/ithemal/Ithemal>.

### Equality saturation and e-graphs

- R. Tate, M. Stepp, Z. Tatlock, S. Lerner. "Equality Saturation: A
  New Approach to Optimization." POPL 2009.
- M. Willsey, C. Nandi, Y. R. Wang, O. Flatt, Z. Tatlock, P.
  Panchekha. "egg: Fast and Extensible Equality Saturation." PLDI
  2021. arXiv 2004.03082. egg Rust crate: <https://docs.rs/egg>.
  egg source: <https://github.com/egraphs-good/egg>.
- Y. R. Wang, O. Flatt, M. Willsey et al. "Ruler: Scaling the
  Rule-Based Synthesis of Program Optimizers." POPL 2024.
- P. Panchekha, A. Sanchez-Stern, J. R. Wilcox, Z. Tatlock.
  "Automatically Improving Accuracy for Floating Point Expressions."
  PLDI 2015. Herbie project page: <https://herbie.uwplse.org/>.
  Herbie source: <https://github.com/herbie-fp/herbie>. Herbie Rust
  linter: <https://github.com/mcarton/rust-herbie-lint>.

### Program synthesis and verification

- R. Sasnauskas et al., "Souper: A Synthesizing Superoptimizer."
  arXiv 1711.04422. Souper source: <https://github.com/google/souper>
  (archived 2025-10-30).
- N. P. Lopes, D. Menendez, S. Nagarakatte, J. Regehr. "Provably
  Correct Peephole Optimizations with Alive." PLDI 2015.
- N. P. Lopes et al. "Alive2: Bounded Translation Validation for
  LLVM IR." PLDI 2021. Alive2 source:
  <https://github.com/AliveToolkit/alive2>. Alive2 Compiler Explorer:
  <https://alive2.llvm.org/ce/>.
- D. Lazaar, A. Cole, G. Bălănescu. "Hashing Modulo Equality." POPL
  2025.
- S. Balog, A. L. Gaunt, M. Brockschmidt, S. Nowozin, D. Tarlow.
  "DeepCoder: Learning to Write Programs." ICLR 2017 workshop.
- K. Shi, D. Bieber, R. Singh. "TF-Coder: Program Synthesis for
  Tensor Expressions." Google Research, 2022–2023.
  <https://github.com/google-deepmind/tf-coder>.

### MLIR, polyhedral, Halide

- MLIR Affine dialect documentation:
  <https://mlir.llvm.org/docs/Dialects/Affine/>.
- Polly documentation: <https://polly.llvm.org/docs/>.
- J. Ragan-Kelley et al. "Halide: Decoupling Algorithms from
  Schedules for High-Performance Image Processing." CACM Research
  Highlights. Halide project page: <https://halide-lang.org/>.
- R. T. Mullapudi, A. Adams, D. Sharlet, J. Ragan-Kelley, K.
  Fatahalian. "Automatically Scheduling Halide Image Processing
  Pipelines." SIGGRAPH 2016.
- A. Adams et al. "Learning to Optimize Halide with Tree Search
  and Random Programs." SIGGRAPH 2019.

### Other

- R. Joshi, G. Nelson, K. Randall. "Denali: A Goal-directed
  Superoptimizer." PLDI 2002. SRC technical report:
  <https://hpl.hp.com/techreports/Compaq-DEC/SRC-RR-171.html>.
- S. Bansal, A. Aiken. "Automatic Generation of Peephole
  Superoptimizers." ASPLOS 2006.
  <https://sorav.compiler.ai/pubs/asplos06.pdf>.
- S. Bansal, A. Aiken. "Binary Translation Using Peephole
  Superoptimizers." OSDI 2008.
  <https://sorav.compiler.ai/pubs/osdi08.pdf>.
- A. Jangda, G. Yorsh. "Unbounded Superoptimization." Onward!
  2017. doi 10.1145/3133850.3133856.
- M. Brain, T. Crick, M. De Vos, J. Fitch. "TOAST: Applying Answer
  Set Programming to Superoptimisation." ICLP 2006 (Springer LNCS
  4079).
- WebKit B3 (JSC optimising compiler). Introduction blog:
  <https://webkit.org/blog/5856/introducing-the-b3-jit-compiler/>.
- Eclipse OMR project. <https://eclipse-omr.github.io/>.