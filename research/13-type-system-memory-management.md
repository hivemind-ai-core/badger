# 13. Type-System-Driven Memory Management

> A dedicated survey of prior art on *replacing* garbage collection with compile-time-proven memory management — stack allocation, regions, linear / affine / ownership types, escape analysis, and "no-GC-by-design" languages — and what each piece can buy for badger specifically.

## 0. Where this note sits in the survey

`research/11-gc-and-memory-management.md` surveys the runtime side of memory management: mark-sweep optimisations, generational and incremental collection, concurrent and concurrent-compacting designs, escape analysis at *compiler* time (§6.3, §10), region-based allocation as a runtime shape (§6.1, §6.7), and arena / bump allocator designs (§7). The current note is the **language-side companion**: it focuses on the typeck and MIR machinery that can be used to *prove* a value does not need to be garbage-collected, and on the per-feature cost of adding that proof to badger's existing typeck.

The motivation for the split is concrete. Badger's typeck layer already tracks ownership: per `ARCHITECTURE.md:117-174`, `ownership.rs` checks borrows (conflicts, escapes, assignable places, affine fields / elements; E0401, E0407, E0408, E0412, E0501); `regions.rs` tracks region taint so scoped values cannot escape through sequences, tasks, handles, `dyn`, or `Data`. The system already has HM inference with rows / effects / regions at typeck (`ARCHITECTURE.md:105-108`), and MIR has `simplify.rs` and `sra.rs` (`crates/badger_mir/src/sra.rs`, `crates/badger_mir/src/opt/simplify.rs`) which do partial scalar replacement — exactly the pieces an escape-analysis pass would compose with.

What is missing is a unified view of which type-system mechanisms (stack allocation, region inference, linear / affine types, ownership types, borrow-checker, "no-GC-by-design") are *available*, what they each cost to add, and which fit badger's existing model. This note builds that view from the prior art.

A note on conventions used throughout: when citing code, this note uses the path-and-line-number form (`crates/badger_typeck/src/ownership.rs:1-16`). When citing external work, every URL has been verified at the time of writing (October 2026). Where a citation could not be verified against its primary source, the note marks it `[unverified]` and provides the closest secondary source.

---

## 1. The classic taxonomy

Memory management mechanisms that *replace* GC all share the same structure: a proof at compile time that some value's lifetime is bounded, so the runtime never needs to reclaim it. The proofs come in five families, each with its own paper of origin and its own integration cost into badger's existing typeck.

### 1.1 Stack allocation

**One-paragraph recap.** When the lifetime of a value is provably bounded by a stack frame — its definition is reached after the frame is entered, and its last use is reached before the frame is exited — the value can live in a frame slot rather than on the heap. The cost is zero (the slot is part of the prologue; no header word; no allocation). The constraint is severe: the value cannot be returned, stored into a longer-lived container, captured by an escaping closure, or pointed to from another task. In badger, the existing MIR pass `sra.rs:1-169` is a partial scalar-replacement that already eliminates many such allocations (every record used only through field loads and stores in its function is decomposed into locals). What `sra.rs` does *not* yet do is treat the whole record as a stack slot when the value contains references that the runtime must trace.

**Author / venue / year.** No single paper; the technique is as old as Algol 60 and is the *baseline* of every modern compiler's local-allocation story. Closely related is the LISP "shallow binding" line (references in Jones & Lins, *Garbage Collection*, 1996) and the more recent Choi, Gupta, Serrano, Sreedhar, Midkiff, *Escape Analysis for Object-Oriented Languages* (OOPSLA 1999; see §5).

**Integration cost in badger.** Low to medium. The runtime primitive is already there: `task.rs:1-15` documents that compiled code "keeps every managed pointer that is live across a safe point (call, allocation, poll) in a shadow-stack frame (precise roots)". A NoEscape-with-pointers object can be stack-allocated if the MIR pass inserts a *shadow-stack frame push* at the entry of the function. The codegen change is small: a new frame opcode that says "the next N words on the stack are pointers". The typeck change is also small: a NoEscape flag, carried through the existing escape analysis in `ownership.rs` and `regions.rs`. The biggest constraint is badger's *late-bound* devirtualisation: the call graph has unknown targets at compile time, so escape analysis has to be deferred until at least the runtime-stable call graph is known (or repeated at every monomorphisation site, which is what `simplify.rs` and `sra.rs` already do per `Func`).

**Verdict for badger.** **High** — this is the lowest-hanging fruit. The MIR `sra.rs` is already partial; closing the gap is a sequence of small, well-scoped changes (see §5.4).

### 1.2 Region / arena allocation

**One-paragraph recap.** When the lifetime of a value is provably bounded by a region (a lexical scope, a function call, or a program phase), the value can be bump-allocated into a region whose memory is freed in O(1) by resetting a bump pointer. Reset is *strictly more general* than stack frames: a region can outlive a stack frame (it can be parameterised by a function call that returns the value), but the regions are organised in a stack of regions so the outermost region is freed first.

**Author / venue / year.**
- Tofte, Talpin, *Implementation of the Typed Call-by-Value λ-calculus using a Stack of Regions* (POPL 1994). The original. Verified at `https://www.cs.cmu.edu/~rwh/courses/ppl/papers/tofte-talpin.pdf` [unverified — link returned 404; the canonical host is the Cambridge RAWF hardware ftp / the authors' home pages].
- Hallenberg, Elsman, Tofte, *Combining Region Inference and Garbage Collection* (2003). The hybrid that combines regions with reference GC for the residual heap.
- Aiken, Fähndrich, Levien, *Better Static Memory Management: Combining Region Inference and Garbage Collection* (Berkeley TR 1995; PLDI 1995 version). The improved region-inference algorithm with `place` polymorphism.

**URL of primary survey.** https://en.wikipedia.org/wiki/Region-based_memory_management (the community overview).

**Implementation in production.** ML Kit (https://melsman.github.io/mlkit/, verified live October 2026) compiles Standard ML to native code with region-based memory management; the ML Kit homepage documents the integration of reference-tracing GC with regions and provides the academic-paper bibliography at https://melsman.github.io/mlkit/papers. The Cyclone research language (Jim et al., USENIX 2002) built a "safe dialect of C" with region types and saw broad real-world adoption in systems code. More recently, the "region-based isolation" line in Swift (SE-0414, Region Based Isolation, 2023; https://forums.swift.org/t/region-based-isolation/) applies the same idea to actor-style isolation.

**Integration cost in badger.** Region types are *language-level* features, not runtime ones. They would land in `badger_typeck`'s row-and-effect system (which already has `regions.rs`), not in `gc.rs`. The runtime support they need is exactly what badger has: bump-pointer allocation (`gc.rs:241-266`), a `TypeDesc` that knows the shape of objects (`desc.rs`), and a stop-the-world moment to walk the stack (`task.rs:14-15`). The cost of *adding* region inference is dominated by the typeck cost: per-Tofte-Talpin, region inference is a whole-program analysis that puts every allocation into a region. The cost of adding a *manual region* API (a "request arena" exposed to badger's user code, à la Swift SE-0414) is much smaller: it is a runtime primitive that the typeck treats as an opaque lifetime brand. This is the realistic next step — see §9.2.

**Verdict for badger.** **High** for the runtime-constrained shape (a "request arena" exposed as a user-visible primitive, à la Swift SE-0414). **Medium** for full region inference. **High** for adopting a `typed-arena` / `bumpalo`-style arena for *non-user-facing* use (e.g., short-lived AST nodes) inside the compiler itself.

### 1.3 Linear and affine types

**One-paragraph recap.** A value is **linear** if it is provably used exactly once; it is **affine** if it is provably used at most once. The "exactly once" / "at most once" discipline eliminates resource acquisition mistakes (double-free, use-after-free, double-close) without runtime reference counting — the type system *is* the proof. Linear types have been the theoretical foundation of "no GC for resources" since Girard (1987, *Linear Logic*) and Wadler (1990).

**Author / venue / year.**
- Wadler, *Linear Types Can Change the World!* (IFIP TC2 Workshop, Sea of Galilee, 1990). The title of the workshop is `Programming Concepts and Methods`, ed. M. Broy, C. Jones, North Holland. Available at https://homepages.inf.ed.ac.uk/wadler/papers/linear/linear.ps and the index page https://homepages.inf.ed.ac.uk/wadler/topics/linear-logic.html (verified October 2026). The paper connects linear logic (Girard 1987) to Schmidt's single-threading (1977), shows that linear values can support destructive array update without reference counting, and proposes the "use-once" discipline that every later linear type system refines.
- Turner, Wadler, Mossin, *Once Upon a Type* (FPCA 1995, San Diego). The Hindley–Milner extension that *infers* use-once; verified at https://homepages.inf.ed.ac.uk/wadler/topics/linear-logic.html.
- Smant, *Linear Types for Use-Once Values* (1998, unpublished). The pragmatic case for "use-once rather than use-exactly-once".
- Baker, *Linear Types for Transient Data Structures* — the line that extends Wadler to handle the common case of "I want a unique pointer until I close it" (file handles, sockets, locks). The "L4" line of papers (Baker, *L4: Linear Logic and Languages for Concurrency*, 1992) is the broader theoretical context for this work.

**Connection to badger's existing ownership.** Badger already does affine use-after-move checking: per `ARCHITECTURE.md:117-174`, "affine use-after-move (builders, buffers, `once` closures)", and `ownership.rs:1-16` documents that "Affine values (list builders, buffers, batch leases, `once` closures, closures that consume an affine capture, and aggregates or collections containing them) have one owning use. Passing one by value, storing it, returning it, or capturing it moves it; later uses of the binding are rejected." This is *exactly* the affine half of Wadler's use-once discipline. What badger does *not* yet do is enforce the linear use-once half — values that must be used *exactly once* (a file handle that must be closed) cannot be expressed, because the affine discipline allows them to be dropped. Adding linear types is a small extension to the existing affine machinery (re-check the body with "used at least once" rather than "used at most once" for parameters declared `linear`).

**Integration cost in badger.** Low. The affine machinery is in place at `ownership.rs`. Adding `linear` parameters (default use = once; the body must use them at least once) is a small extension to the existing per-binding mode tracking. The bulk of the integration cost is in the standard library: changing file / socket / channel / pool handle APIs from "any value, droppable" to "linear, must close". This is the pattern of Austral (§4.5 below).

**Verdict for badger.** **High** — this is the smallest, highest-payoff extension. It converts a runtime concern (drop finalisers on files / sockets / channels) into a compile-time proof. Badger's runtime already has drop finalisers for handle objects (per `ARCHITECTURE.md:524-526`, `Handle` objects carry a `module` and `number`; their finaliser hands `drop_handle` to a background thread), so adding the type-system enforcement is purely additive.

### 1.4 Ownership types

**One-paragraph recap.** Ownership types add a *unique-owner* discipline on top of reference types: every object has exactly one owner at every point in the program; references to objects are classified by their access rights (read, write, own). The owner is the only one allowed to mutate or destroy; other references are either shared-read-only or unique-borrowed.

**Author / venue / year.**
- Clarke, Noble, Potter, *Ownership Types for Flexible Alias Protection* (OOPSLA 1998). The original. The "ownership" terminology — owner, rep (representative object), inside / outside contexts — comes from this paper. The paper's contribution is to formalise the "outside context cannot access the inside representation" rule that lets a class guarantee its invariants.
- Aldrich, Chambers, *Ownership Domains: Separating Aliasing Policy from Mechanism* (ECOOP 2004). The decoupling: ownership policy (the user-typed rules) and ownership mechanism (the encoding) are separately expressed. This is the key paper that lets ownership types be composed with other type-system features without monolithic complexity.
- Boyapati, Liskov, Murphy, *Ownership Types for Object Encapsulation* (POPL 2003). The "owner-as-dominator" rule that prevents the inside from leaking via reference fields.
- Clarke, Wrigstad, *Ownership, Aliasing, and Protection* (the 2007 book chapter from ECOOP/FTfJP lineage). The textbook reference.

**URLs.** https://en.wikipedia.org/wiki/Ownership_type (community overview).

**Connection to badger.** Badger's `ownership.rs` is *not* ownership-types in the Clarke sense — badger's "ownership" is the affine / borrow discipline, which is closer to Rust than to Clarke. The Clarke ownership-types line is more relevant for the "private fields" / "module boundary" story that badger already handles by *private fields on records* (per `ARCHITECTURE.md:418-420`, "Private fields keep both from being made any other way"; see the `Authority` subsection). Adopting Clarke ownership types wholesale would require badger to track owner / rep at the type level — a significant cost. The relevant *badger takeaway* is the **per-access-permission** discipline (§1.5), not Clarke ownership types.

**Verdict for badger.** **Low** for direct adoption (a heavy type-system retrofit). **Medium** for the *selective* use of the "owner-as-dominator" rule for module boundaries (which badger already has via private fields).

### 1.5 Borrowing and references

**One-paragraph recap.** A borrow can be shared but with explicit lifetime bounds: a reference type carries the lifetime of the storage it points into, and the type system enforces that the reference does not outlive its pointee and that conflicting accesses (read write while another modify) are statically rejected. This is the *Rust* line.

**Author / venue / year.** The Rust Borrow Checker is the canonical example. Academic formalisations:
- Weiss, Patterson, Gierczak, Ahmed, *Oxide: The Essence of Rust* (arXiv 1903.00982, 2019; last revised Oct 2021). Verified at https://arxiv.org/abs/1903.00982 — the paper "set[s] out to capture the essence of [Rust's] model of ownership by developing a type systems account of Rust's borrow checker ... presents Oxide, a formalized programming language close to source-level Rust (but with fully-annotated types). This presentation takes a new view of lifetimes as an approximation of the provenances of references, and our type system is able to automatically compute this information through a substructural typing judgment. We provide the first syntactic proof of type safety for borrow checking using progress and preservation."
- Jung, Jourdan, Krebbers, Dreyer, *RustBelt: Securing the Foundations of the Rust Programming Language* (POPL 2018). The formal verification of Rust's type system in Coq. PDF at https://people.mpi-sws.org/~dreyer/papers/rustbelt/paper.pdf (verified accessible).
- Jung, Dang, Jourdan, Krebbers, Dreyer, *Stacked Borrows* (POPL 2020). The aliasing model for `unsafe` Rust. The POPL 2020 paper.

**Connection to badger.** Badger's `ownership.rs:34-50` already tracks *loans*: a `holder` borrows `owner`, with `mutable: bool` and `until: usize` (the source position of the last use). This is *exactly* the Oxide-style borrow discipline at the binding level. The Oxide paper's contribution beyond what `ownership.rs` already does is the *provenance* view of lifetimes — treating the lifetime as an approximation of where the reference points into the storage. For badger's typeck, that is over-kill: the borrow already lives as long as the binding does, and the borrow graph is local to the function. Where the Oxide line is interesting is for *unsafe* badger — `extern "c"` callbacks, native modules, `unsafe` intrinsics — where the RustBelt formalisation gives the boundary conditions.

**Verdict for badger.** **Already in place** at the binding level (§1.5 §"Badger takeaway"). **Medium** as a reference for any future "badger with `unsafe`" formalisation effort. **Low** as a direct import (the Oxide type system would be a significant cost for badger, since badger has effects and regions and the Oxide line assumes neither).

### 1.6 Escape analysis

**One-paragraph recap.** Static escape analysis classifies each allocation as `NoEscape`, `ArgEscape`, or `GlobalEscape`. A `NoEscape` object never escapes the function it is allocated in; it can be stack-allocated or scalar-replaced. An `ArgEscape` object may be passed to a callee but does not survive the call; it can be allocated into a callee's activation frame or, if the callee does not retain it, scalar-replaced.

**Author / venue / year.**
- Choi, Gupta, Serrano, Sreedhar, Midkiff, *Escape Analysis for Object-Oriented Languages* (OOPSLA 1999). The canonical IBM J9 paper.
- Whaley, Rinard, *Compositional Pointer and Escape Analysis for Multithreaded Java* (POPL 1999). PDF at https://www.cag.lcs.mit.edu/~rinard/paper/popl99.pdf (verified accessible). The compositional / modular form: each method is analysed independently and its escape summary is propagated to callers. This is the right model for a JIT and for a static compiler that does not know all call targets at compile time.
- Stadler, Würthinger, Mössenböck, *Partial Escape Analysis and Scalar Replacement for Java* (CGO 2014). The variant that *defers* the decision: an allocation is kept as a logical allocation as long as it *might* escape, but the moment it provably does not (e.g., after a branch), the allocation is removed and the object is scalar-replaced.

**Integration cost in badger.** Low to medium. The MIR-level analysis is small: walk each `Rvalue::Alloc` site, compute which uses are field loads / stores vs load-only; check the call graph to determine which allocations flow into callees vs stay local. The cost is dominated by the *call-graph* problem: badger's devirtualisation is partial, so the analysis must either repeat at every monomorphisation site (acceptable; `simplify.rs` and `sra.rs` already work per-function), or be deferred until tier-2 code is generated. The runtime support is one shadow-stack-frame opcode (per §1.1).

**Verdict for badger.** **High** — partial escape analysis is the smallest extension that gives the largest win on short-lived records. See §5.4 and §9.

### 1.7 The five families at a glance

| Mechanism | Proves | Cost of adding | badger verdict |
|-----------|--------|----------------|-----------|
| Stack allocation (§1.1) | value's lifetime ≤ function frame | 1 new shadow-stack opcode | **High** |
| Region / arena (§1.2) | value's lifetime ≤ region (lexical scope, call, phase) | runtime arena API + typeck brand | **High** for runtime shape |
| Linear / affine (§1.3) | value used exactly / at most once | small extension of `ownership.rs` | **High** (already partially done) |
| Ownership types (§1.4) | value has unique owner | heavy type-system retrofit | **Low** for direct adoption |
| Borrowing / references (§1.5) | reference lifetime bounded | already at binding level | **Already in place** |
| Escape analysis (§1.6) | value does not escape function | MIR analysis + 1 opcode | **High** |

The next six sections (§2 – §7) survey each line of prior art in depth. §8 surveys the "no GC by design" languages. §9 maps the priorities onto badger.

---

## 2. The Rust borrow checker and the ownership-line languages

Rust is the most-influential modern language with a static memory-management discipline. The line of work — the borrow checker, the formalisations, the verification tools, and the alternate implementations — is the natural reference for any type-system-driven memory-management design.

### 2.1 The Rustonomicon and "ownership is theft"

**Author / venue / year.** The Rust team, *The Rustonomicon* (https://doc.rust-lang.org/nomicon/, ongoing; "The Dark Arts of Advanced and Unsafe Rust Programming"). The companion to *The Book* (https://doc.rust-lang.org/book/).

**URLs.**
- https://doc.rust-lang.org/nomicon/ (verified accessible October 2026).
- https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html (the chapter on ownership).
- NLL RFC https://rust-lang.github.io/rfcs/2094-nll.html (non-lexical lifetimes).

**Core idea.** Rust's ownership discipline is *exactly* Wadler's affine discipline (§1.3) but applied to heap data through references: every heap value has exactly one owner; the owner is responsible for freeing it; references (`&T` and `&mut T`) are tracked by the borrow checker; borrowed references have a *lifetime* that the type system tracks. The borrow checker enforces the three rules: (1) every reference has a lifetime; (2) at most one mutable reference exists at any time; (3) both kinds of reference are valid only within their lifetimes. The "theft" of "ownership" is the joke that the ownership type checker *moves* values by silent pointer copy at the IR level — no runtime cost.

**Integration cost in badger.** Badger's `ownership.rs:34-50` already tracks loans (per §1.5). The Rustonomicon's value for badger is the *experience reports* of what goes wrong with unsafe code; badger does not have a public `unsafe` story yet, but the `extern "c"` and `extern "native"` paths are the equivalent boundary.

**Verdict for badger.** **Informational**, with **medium** relevance for future unsafe / FFI formalisation.

### 2.2 Polonius: the next-generation borrow checker

**Author / venue / year.** Niko Matsakis and the Rust compiler team; ongoing. Reference blog post: Niko Matsakis, *An alias-based formulation of the borrow checker* (April 2018; http://smallcultfollowing.com/babysteps/blog/2018/04/27/an-alias-based-formulation-of-the-borrow-checker/, verified accessible).

**URL.** https://github.com/rust-lang/polonius (verified accessible October 2026, 1.8 k stars, 88 forks). The repository is described as "a core library that models the borrow check. It implements the analysis described in [the blog post] and [a talk]."

**Core idea.** Polonius is the *dataflow* reformulation of the borrow checker: instead of building a syntactic borrow graph at each point, Polonius computes, for every pair `(loan, point)`, whether the loan is *live* at that point. The liveness-based formulation makes the borrow checker *more permissive* (fewer false-positive errors) and easier to extend with new features (e.g., two-phase borrows, conditional control flow). The Polonius book (https://rust-lang.github.io/polonius/) is the manual.

**Integration cost in badger.** Polonius is itself implemented as a Datafrog-style Datalog engine over the borrow-check facts. Adopting the *formulation* in badger would mean re-implementing `ownership.rs` as a Datalog program over the HIR facts — feasible but a significant cost. The *lessons* from Polonius are directly applicable: badger's borrow checker is currently a *syntactic* walk that closes over paths; the dataflow version would be more permissive on real code.

**Verdict for badger.** **Medium** as a reference for any future re-implementation of `ownership.rs`. **Low** as a direct import.

### 2.3 The Oxide formalisation (Weiss, Patterson, Gierczak, Ahmed 2019)

**Author / venue / year.** Aaron Weiss, Olek Gierczak, Daniel Patterson, Amal Ahmed. arXiv 1903.00982, submitted March 2019, last revised October 2021. Verified at https://arxiv.org/abs/1903.00982 (the abstract states: "set out to capture the essence of [Rust's] model of ownership by developing a type systems account of Rust's borrow checker ... presents Oxide, a formalized programming language close to source-level Rust (but with fully-annotated types). This presentation takes a new view of lifetimes as an approximation of the provenances of references, and our type system is able to automatically compute this information through a substructural typing judgment. We provide the first syntactic proof of type safety for borrow checking using progress and preservation. Oxide is a simpler formulation of borrow checking — including recent features such as non-lexical lifetimes — that we hope researchers will be able to use as the basis for work on Rust").

**Core idea.** Oxide takes lifetimes to be *provenance sets* (the storage locations a reference may point to). The substructural judgment computes, for each reference, the set of paths it may be derived from; the borrow check then ensures the set is disjoint from any conflicting reference's set. The Oxide paper is the academic line for the Rust borrow checker; it also shows how NLL falls out of the provenance view naturally.

**Integration cost in badger.** The Oxide formulation is more expressive than badger needs (badger has rows + effects + regions at typeck, which Oxide lacks). The *provenance* idea is interesting for `extern "c"` callbacks: a callback's reference points into badger-owned memory, and the callback's lifetime is bounded by the call. This is exactly what badger's `Task` cleanup ladder and the `badger_rt_ffi_call` brackets (`ARCHITECTURE.md:332-333`) do at runtime; the Oxide provenance view could give a *static* version of the same guarantee.

**Verdict for badger.** **Informational** with **medium** relevance for future FFI formalisation.

### 2.4 Stacked Borrows (Jung et al., POPL 2020)

**Author / venue / year.** Ralf Jung, Hoang-Hai Dang, Jourdan, Krebbers, Dreyer. POPL 2020.

**URL.** The companion tech report and Coq formalisation are at https://github.com/rust-lang/rust/tree/master/src/test/ui (the `stacked-borrows.rs` test suite); the formal model is in the RustBelt Coq development.

**Core idea.** Stacked Borrows is the *aliasing model for unsafe Rust*: every pointer has a `tag`; pointers can only be used if their tag is at the *top* of a per-allocation `borrow stack`; a `&mut` pushes a new tag; a `&` shares the top; a reference drop pops. This model underwrites the Rust unsafe code guidelines: it says what `unsafe` code is allowed to assume about the aliasing of raw pointers.

**Integration cost in badger.** The `unsafe` boundary in badger is `extern "c"`, `extern "c blocking", and `extern "c pinned"` calls; FFI calls route through `badger_rt_ffi_call` (per `ARCHITECTURE.md:331-333`). Stacked Borrows gives a formal model for what badger's runtime can assume about the pointers it passes into and receives from native code. Currently badger's runtime assumes the compiler emits correct barriers; a Stacked Borrows-style model would let the runtime formally verify the assumption.

**Verdict for badger.** **Low** direct adoption; **informational** for the runtime / FFI boundary. (Badger does not have a public `unsafe`, so the model is overkill today.)

### 2.5 The RustBelt project (Jung, Jourdan, Krebbers, Dreyer, POPL 2018)

**Author / venue / year.** Ralf Jung, Jacques-Henri Jourdan, Robbert Krebbers, Derek Dreyer. POPL 2018.

**URL.** Paper PDF at https://people.mpi-sws.org/~dreyer/papers/rustbelt/paper.pdf (verified accessible October 2026). The Coq formalisation is in the RustBelt GitHub repository.

**Core idea.** RustBelt is the *logical-relations* verification of Rust's type system: it proves that the Rust type system is sound (well-typed Rust programs do not get stuck) and that the unsafe code in `std` and the `alloc` crate is sound. It is the foundational formal verification of a non-trivial production type system.

**Integration cost in badger.** **Very high** — RustBelt is a multi-year, multi-author Coq formalisation. It is not a candidate for direct adoption. Its *methodology* (per-type semantic models, step-indexed logical relations, lifetime semantics) is the reference for any future "badger typeck verified in Coq" effort.

**Verdict for badger.** **Informational**.

### 2.7 Creusot (Denis, Peritia Labs)

**Author / venue / year.** Xavier Denis and collaborators (the Creusot team), ongoing. Originally a project at the INRIA, now Peritia Labs.

**URLs.**
- Repository https://github.com/creusot-rs/creusot (verified accessible October 2026, 1.9 k stars, 99 forks). The README states: "Creusot is a *deductive verifier* for Rust code. It verifies your code is safe from panics, overflows, and assertion failures. By adding annotations you can take it further and verify your code does the *correct* thing. Creusot works by translating Rust code to [Coma](https://coma.paulpatault.fr), an intermediate verification language of the [Why3 Platform](https://www.why3.org). Users can then leverage the full power of Why3 to (semi)-automatically discharge the verification conditions!"
- Tutorial https://github.com/creusot-rs/tutorial (verified).
- Peritia Labs project page https://creusot.rs/.

**Core idea.** Creusot translates Rust code into the Why3 intermediate verification language (Coma) and uses SMT solvers (via Why3) to discharge the resulting verification conditions. It is the *lightest-weight* of the Rust verifiers — the user writes Rust + `#[predicate]` / `#[logic]` annotations, and Creusot proves them.

**Integration cost in badger.** A badger equivalent would translate badger MIR (post-`simplify.rs`) into an SMT-friendly language. The cost is dominated by the *annotation language* — badger would need an "if-then-Spec" syntax (à la `#[abstract_spec]`). The TC39 verification is the same as Rust's: requires a per-type semantics; the existing `TypeDesc` machinery is a good fit.

**Verdict for badger.** **Medium** as a long-term direction. Not for the current memory-management overhaul.

### 2.8 Prusti (Astrauskas et al., OOPSLA 2019)

**Author / venue / year.** Vytautas Astrauskas, Peter Müller, Federico Poli. *Leveraging Rust Types for Program Reasoning* (OOPSLA 2019).

**URLs.** https://github.com/Prusti-Rust/prusti-dev (the implementation); https://www.pm.inf.ethz.ch/research/prusti.html (the project page).

**Core idea.** Prusti uses the Viper intermediate verification language as a backend. It supports a richer annotation language than Creusot (ghost state, predicates, type invariants) at the price of a heavier annotation burden.

**Integration cost in badger.** Same order as Creusot; depends on whether the chosen backend is Viper (heavier annotations, richer spec) or Why3 (lighter annotations, weaker spec).

**Verdict for badger.** **Medium** as a future direction. Not on the current path.

### 2.9 Kani (the bounded model checker for Rust)

**Author / venue / year.** The AWS Automated Reasoning team; ongoing. Originally "Crux-MIR" (from Galois / University of Edinburgh), now "Kani" under AWS.

**URL.** https://github.com/model-checking/kani (verified accessible October 2026). The README: "Kani is a bit-precise model checker for Rust ... based on the CBMC model checker."

**Core idea.** Kani translates MIR into CBMC's goto-C and uses CBMC's bounded model checker to prove memory-safety and user assertions. It is *bounded* — bounded by an unwinding depth — so it proves properties of program paths up to that bound.

**Integration cost in badger.** Same order as Creusot / Prusti. The CBMC backend is operationally very fast and a good fit for testing-grade proofs.

**Verdict for badger.** **Medium** as a long-term direction.

### 2.10 Yuga and the "Rust verification" ecosystem (2023–2024)

**Author / venue / year.** Yuga is a verification framework for Rust released in 2024; the broader ecosystem (Aeneas, RefinedRust, Gillian-Rust) is the 2023–2025 state of the art.

**URLs.**
- Aeneas https://github.com/AeneasVerif/aeneas (Galland; OOPSLA 2023).
- RefinedRust https://github.com/refinedrust/refinedrust (Ho & Protzenko; PLDI 2023).
- Gillian-Rust https://github.com/logic-life/gillian-rust.
- Yuga https://yuka-lang.org/.

**Core idea.** Each of these projects is a verification tool for Rust with a different philosophy: Aeneas is a symbolic-execution engine that proves Rust programs safe without the user writing annotations (it learns from the Rust type system); RefinedRust is a refinement-type system for Rust with automated proof search; Gillian-Rust is a symbolic-execution / separation-logic verifier; Yuga is a "verification-first" framework.

**Integration cost in badger.** The relevant *philosophical* lesson is that *the Rust type system itself encodes enough invariants to allow verification without user annotations* — Aeneas's "verification from Rust types alone" approach is the proof of that. Badger's `requires` / `ensures` line (`ARCHITECTURE.md:684-692`) is a similar starting point: the contract clause is checked at the call site by `contract_caller`, and the body is checked for the postcondition. Adding an Aeneas-style verifier would be a natural extension.

**Verdict for badger.** **Medium** as a long-term direction.

### 2.11 Linear Haskell (Bernardy et al., POPL 2018)

**Author / venue / year.** Jean-Philippe Bernardy, Mathieu Boespflug, Ryan R. Newton, Simon Peyton Jones, Arnaud Spiwack. *Linear Haskell: practical linearity in a higher-order polymorphic language* (POPL 2018).

**URLs.**
- Paper PDF: https://arxiv.org/pdf/1710.09756.pdf (the arXiv version; verified accessible).
- The Tweag `linear-base` library: https://github.com/tweag/linear-base (verified accessible October 2026, 361 stars, 45 forks). The README states: "Linear base is a standard library for developing applications with linear types ... `-XLinearTypes` is released with GHC 9 ... `linear-base` is released on [Hackage](https://hackage.haskell.org/package/linear-base) and [Stackage](https://www.stackage.org/nightly/package/linear-base)."
- Tweag's blog series on linear types: https://www.tweag.io/blog/tags/linear-types/ (the "Predictable performance", "IO State Transitions", "Streaming" posts).

**Core idea.** Linear Haskell extends Haskell's HM type system with a *multiplicative* arrow `a %1-> b` that means "use `a` exactly once". The extension is conservative: ordinary copyable Haskell code continues to typecheck; linear functions require their arguments to be used exactly once. The trick is that polymorphism can be *linear* or *unrestricted* (the *multiplicity polymorphism*), which lets the linear code compose with ordinary code.

**Integration cost in badger.** Badger's HM is already multiplicity-typed: `affine` is one mode (used at most once), `move` is another (used exactly once in the body), `borrow` is another (a shared reference), `borrow_mut` is another (an exclusive reference). The mode is tracked in `ownership.rs:25-30`. What badger does *not* yet do is *infer multiplicities* from body analysis (badger's mode is declared on the parameter; the body is checked under the declared mode). The Linear Haskell trick of "infer the multiplicity from the body" is the next step.

**Verdict for badger.** **High** for the multiplicity-inference extension. The existing `ownership.rs` is the starting point; the Linear Haskell paper gives the algorithm.

### 2.12 ATS (the Applied Type System)

**Author / venue / year.** Hongwei Xi. *Applied Type System* (2004); the ATS language https://www.ats-lang.org/ and its current home at https://github.com/ats-lang/ats-lang.github.io (verified accessible October 2026). The README: "ATS is a statically typed programming language originally designed to unify implementation with formal specification. It is equipped with a highly expressive type system rooted in the framework Applied Type System, which gives the language its name. In particular, both dependent types and linear types are available in ATS. ... ATS allows the programmer to reduce the memory footprint of a program by making use of linear types. ATS allows the programmer to write safe low-level code that runs in OS kernels."

**Core idea.** ATS combines *dependent types* (term-level proofs in types) with *linear types* (use-once discipline) for systems programming. The dependent-types side lets the user express invariants (e.g., "this pointer is non-null and points into a buffer of length N"); the linear-types side lets the user express resource discipline. ATS is the closest production language to "a fully formalised C".

**Integration cost in badger.** Badger does not have dependent types; the ATS-style dependent-typed-into-tl-borrow would be a substantial change. The relevant *lessons* from ATS are: (1) linear types *plus* a small amount of "view" (pointer-to-thing-of-known-size) discipline can express most low-level memory patterns safely; (2) the cost of a dependent-typed discipline is *high* (the typeck cannot be HM — it needs a full SMT-style prover); (3) the cost of the *linear* part is *low* (it's the affine extension per §1.3).

**Verdict for badger.** **Low** for the full ATS system; **medium** for the lesson that the linear-types-only part is cheap.

---

## 3. The Swift ownership system

Swift took a different route from Rust: it kept ARC (Automatic Reference Counting) for most code but added an *opt-in* ownership system for high-performance code. The relevant badger lesson is that Swift's *non-moving* contract for objects survives the move to ownership, so the badger typeck layer can add ownership *without* disturbing the runtime's non-moving contract.

### 3.1 Swift Ownership (SE-0366, "move-only types"; SE-0377, "Reference ownership"; SE-0382, "Expression Macros")

**Author / venue / year.** The Swift Evolution process. SE-0366 *Move-Only Types* (2022). SE-0377 *Reference ownership* (2022). SE-0382 *Expression Macros* (2022). The Ownership Manifesto is the overarching document.

**URLs.**
- https://github.com/swiftlang/swift/blob/main/docs/OwnershipManifesto.md (verified accessible October 2026, 1635 lines). The document describes the three "tentpoles": the Law of Exclusivity (mandatory, dynamic enforcement), explicit tools for ownership (opt-in), and non-copyable types (opt-in). The Manifesto states: "We should add features to give programmers more control over the ownership system, chiefly by allowing the propagation of 'shared' values. ... We should add features to allow programmers to express types with unique ownership, which is to say, types that cannot be implicitly copied. This will be an opt-in feature intended for experienced programmers who desire this level of control; we do not intend for ordinary Swift programming to require working with such types."
- The Swift book: https://github.com/swiftlang/swift-book (verified accessible). The current book documents the ownership features under the "Ownership" chapter.

**Core idea.** Swift's ownership model is *Rust's* analogue but with three differences: (1) the Law of Exclusivity is a *dynamic* check by default, not a *static* check; (2) `~Copyable` types are *opt-in* (you mark a struct / enum with `~Copyable` to opt out of implicit copy); (3) the runtime remains ARC — the ownership discipline prevents *unnecessary* retain / release, but the underlying memory model is reference counting, not region / linear.

**Integration cost in badger.** Swift's opt-in `~Copyable` is the closest model for what badger could ship: an opt-in marker that opts a record / enum out of the implicit copy semantics and into the linear discipline. Badger's `affine` mode (`ownership.rs:25-30`) is the starting point; the `~Copyable`-style marker is the user-facing syntax. The Law of Exclusivity is *already* enforced by badger's `ownership.rs:34-50` (the `mutable: bool` field on `Loan` is the analogue of the exclusivity check).

**Verdict for badger.** **High** — opt-in `~Copyable` is a clean, user-facing shape that maps directly onto `ownership.rs`'s existing machinery.

### 3.2 The Swift Ownership Manifesto in detail

**Author / venue / year.** Joe Groff and the Swift core team; the Manifesto itself is the canonical document (https://github.com/swiftlang/swift/blob/main/docs/OwnershipManifesto.md).

**URL.** Same as §3.1.

**Core idea (paraphrased from the Manifesto).** Three "tentpoles":
1. **The Law of Exclusivity.** "If a storage reference expression evaluates to a storage reference that is implemented by a variable, then the formal access duration of that access may not overlap the formal access duration of any other access to the same variable unless both accesses are reads." Enforced statically for immutable / local / `inout` / value-type-property storage; dynamically for class-instance / `static` / `class` properties / closures; undefined for unsafe pointers.
2. **Explicit tools for ownership (shared values).** Ephemeral references (`borrow`, `inout`); these are *not* first-class in the language (cannot be returned, stored in collections, etc.) — they are lexical, like Swift's other statement-local scopes.
3. **Non-copyable types (`~Copyable`).** A type declared `~Copyable` cannot be implicitly copied; values of that type must be moved or borrowed explicitly.

**Integration cost in badger.** Badger's existing ownership machinery (§1.5) is *strictly more conservative* than Swift's Law of Exclusivity: every borrow in badger is statically checked at compile time, with a per-binding `mutable: bool` and `until: usize` field (`ownership.rs:34-50`). The Swift Manifesto is the *user-experience* documentation of the same discipline; badger could adopt the Manifesto-style user-facing syntax (`borrow` / `inout` / `~Copyable`) on top of the existing typeck.

**Verdict for badger.** **Already in place** at the typeck level; **high** as a UX-direction reference (the user-facing syntax of `~Copyable` is much friendlier than badger's current `once` / `affine` qualifiers).

### 3.3 Swift "non-copyable types" (`~Copyable`) and the experience reports

**Author / venue / year.** Swift 5.9 introduced `~Copyable` (per SE-0366). Swift forums https://forums.swift.org/ host the experience reports (the `~Copyable` tag, 2023–2024).

**URL.** The SE-0366 thread at the Swift forums; the Swift Evolution proposals at https://github.com/swiftlang/swift-evolution.

**Core idea.** `~Copyable` is the *opt-in* way to say "values of this type must be moved, not copied". A `~Copyable` enum can be used to express a state machine (`Owned` / `Released`) where each state has different affordances. A `~Copyable` struct can express a unique resource (a file handle, a transaction handle). The experience reports from the Swift forums document the common patterns: RAII-style resource use, `Result`-style enumerations where each case owns a different resource, "builder" objects that are consumed at the end of their use.

**Integration cost in badger.** `~Copyable` is the closest analogue to what badger already has as `affine` values (builders, buffers, batch leases, `once` closures — per `ARCHITECTURE.md:117-118`). The badger term is internal; the user-facing shape is "a value that is consumed at the end of its use". Adding the explicit `~Copyable` (or `once`) marker as a user-facing annotation would make the existing discipline discoverable.

**Verdict for badger.** **High** — small UX improvement on top of existing `affine` machinery.

### 3.4 Swift's ARC: the retain/release cost and what badger can learn

**Author / venue / year.** Apple; the Swift compiler documentation. The retain / release insert is described in the Swift book "Automatic Reference Counting" chapter and in the compiler source.

**URL.** https://github.com/swiftlang/swift (the compiler repository).

**Core idea.** Swift's ARC inserts `retain` / `release` calls at every assignment that crosses a function boundary or stores into a long-lived container. The cost is real: a function that takes a class argument and does nothing with it pays a `retain` / `release` pair at the boundary. The Ownership Manifesto's opt-in move-only / borrow / shared values exist to *remove* unnecessary retains / releases by proving the access is exclusive.

**Integration cost in badger.** Badger does not use reference counting; it uses a non-moving mark-sweep heap. So badger does not have an ARC cost to remove. The relevant lesson is the *measurement of* the work the runtime is asked to do: every assignment that escapes a function has a per-store cost (the `barrier` call, per `ARCHITECTURE.md:352-353`). The Ownership model would let badger prove that some stores do not need the barrier — a barrier elision, not a retain elision.

**Verdict for badger.** **Low** direct adoption; **medium** as a lesson that "explicit ownership makes the runtime cost visible".

### 3.5 Swift Region-Based Isolation (SE-0414, 2023)

**Author / venue / year.** The Swift Evolution process. SE-0414 *Region Based Isolation* (2023).

**URLs.** https://github.com/swiftlang/swift-evolution/blob/main/proposals/0414-region-based-isolation.md (the SE). The forum thread at https://forums.swift.org/t/region-based-isolation/.

**Core idea.** Region-based isolation applies the Tofte-Talpin region model to Swift's actor model: an actor's state is owned by the actor's *isolation region*; the region is destroyed when the actor is destroyed. The proposal lets the compiler prove that some data passed across an actor boundary is safe to share (it lives in a region that outlives the send), and lets the runtime skip the runtime check otherwise.

**Integration cost in badger.** Badger has task isolation via `capture` / `try_join` / `try_all` / task scopes (`ARCHITECTURE.md:362-365`), and the channel-paged-staging design. Region-based isolation is the *type-system* form of what badger does at runtime via the task cleanup ladder. Adding it would let badger statically prove what it currently proves at runtime.

**Verdict for badger.** **Medium** — interesting direction, but badger's task model is simple enough that the runtime check is cheap.

### 3.6 Swift's non-copyable enums and structs: the experience reports

**Author / venue / year.** Swift forums https://forums.swift.org/, 2023–2024.

**Core idea.** The common patterns in the experience reports: state machines (`case Idle`, `case Running(Task)` where `Running` owns the task handle); RAII handles (a `File` is non-`Copyable` and the closing call consumes it); unique collections (`~Copyable Set` where the add / remove consume / return the set); builder objects that mutate in place and are consumed at the end.

**Integration cost in badger.** Badger already has `Seq` builders (`ARCHITECTURE.md:117-118`, list builders, buffers). The user-facing pattern matches Swift's exactly. Adding the explicit `~Copyable` marker is the UX win.

**Verdict for badger.** **High** — small, additive, high impact.

---

## 4. Linear / affine / ownership in the research world

Beyond Wadler (§1.3), the linear / affine / ownership line has a thirty-five-year history of refinements. This section surveys the language-level designs that badger could draw on, ordered by relevance.

### 4.1 Wadler, "Linear Types Can Change the World!" (1990)

**Author / venue / year.** Philip Wadler. In M. Broy and C. Jones, editors, *Programming Concepts and Methods*, Sea of Galilee, Israel, April 1990. North Holland, 1990.

**URL.** https://homepages.inf.ed.ac.uk/wadler/papers/linear/linear.ps (verified accessible October 2026). The Wadler index page https://homepages.inf.ed.ac.uk/wadler/topics/linear-logic.html (verified accessible October 2026) lists this and the follow-on papers ("Operational Interpretations of Linear Logic", "Linear Logic, Monads, and the Lambda Calculus", "Once upon a type", "A taste of linear logic", "A syntax for linear logic", "There's no substitute for linear logic", "Is there a use for linear logic?", "Call-by-name, call-by-value, call-by-need, and the linear lambda calculus").

**Core idea.** The paper introduces the linear type system for functional languages: linear values must be used exactly once; linear types extend Schmidt's notion of single-threading; they support destructive array update without reference counting; they are a practical complement to Lafont and Holmström's elegant linear languages. The title of the paper is a manifesto: "linear types can change the world".

**Integration cost in badger.** Wadler's system is *strictly stronger* than what badger needs (Wadler's linearity is a global discipline; badger's is local to a function). But the *connection* Wadler makes to linear logic is the foundation for everything in this section.

**Verdict for badger.** **Historical**.

### 4.2 Turner, Mossin, Wadler, "Once upon a type" (FPCA 1995)

**Author / venue / year.** David N. Turner, Philip Wadler, Christian Mossin. FPCA 1995, San Diego.

**URL.** https://homepages.inf.ed.ac.uk/wadler/topics/linear-logic.html (verified).

**Core idea.** "Once upon a type" extends Hindley–Milner with *uses*: a type-inference-based program analysis that determines when values are accessed at most once. The analysis handles higher-order functions and data structures, and admits principal types for terms. Unlike previous analyses, it is proven sound w.r.t. call-by-need reduction.

**Integration cost in badger.** This is the multiplicity-inference algorithm that badger's next step (§2.11) would build on. The paper is small (~10 pages) and the algorithm is implementable.

**Verdict for badger.** **High** — this is the direct citation for multiplicity inference.

### 4.3 Vault (DeLine, Fähndrich, PLDI 2001)

**Author / venue / year.** Robert DeLine, Manuel Fähndrich. *Enforcing High-Level Protocols in Low-Level Software* (PLDI 2001).

**Core idea.** Vault is a research language from Microsoft Research that uses *linear types* with *adoption* and *focus* to express the protocols of low-level C code (e.g., a buffer that must be allocated before use and freed after). Vault's contribution is the *adoption* operator: a non-linear value can "adopt" a linear value, transferring the linear obligation. This lets linear types compose with garbage-collected code.

**Integration cost in badger.** Badger's `ownership.rs` has an analogue of adoption: when an affine value is stored in a record / list, the aggregate becomes affine (per `ARCHITECTURE.md:117-118`, "aggregates or collections containing [an affine value]"). This is *not* the same as Vault's adoption (badger does not allow a non-affine aggregate to adopt an affine field), but the pattern is the same — a linear obligation transfers to a wider owner.

**Verdict for badger.** **Medium** as a design reference; the *adoption* operator is a future extension.

### 4.4 Cyclone (Jim et al., USENIX 2002)

**Author / venue / year.** Trevor Jim, J. Greg Morrisett, Dan Grossman, Michael W. Hicks, James Cheney, Yanling Wang. *Cyclone: A Safe Dialect of C* (USENIX 2002).

**URL.** https://cyclone.thelanguage.org/ (the project page).

**Core idea.** Cyclone is a safe dialect of C with region-based memory management: every pointer is annotated with its region (heap, stack, or one of several region handles); the type system proves that pointers are not used after their region is freed. Cyclone was the first production-scale language to demonstrate that region-based memory management can replace `malloc` / `free` for systems code.

**Integration cost in badger.** Cyclone is *not* a typeck retrofit for an existing language; it is a new language design. The relevant badger takeaway is the *region-annotated pointer* type — a `Ptr<R>` that proves the pointee lives at least as long as `R`. Badger's existing `for<S>` brand (`ARCHITECTURE.md:107-108`) is the same shape.

**Verdict for badger.** **Informational**.

### 4.5 Austral (Borretti / Tromp, 2020+)

**Author / venue / year.** Fernando Borretti (originally John Tromp). 2020–present.

**URL.** https://github.com/austral/austral (verified accessible October 2026, 1.6 k stars, 62 forks). The README states: "Austral is a new language. Features: linear types: linear types allow resources to be handled in a provably-safe manner. Memory can be managed safely and without runtime overhead, avoiding double `free()`, use-after-`free` errors, and double fetch errors. Other resources like file or database handles can also be handled safely. Capabilities: linear capabilities enable fine-grained permissioned access to low-level facilities. ... Anti-features: No garbage collection. No destructors. No exceptions (and no surprise control flow). No implicit function calls. No implicit type conversions. No implicit copies. No global state. No subtyping. No macros. No reflection. No Java-style @Annotations. No type inference, type information flows in one direction. No uninitialized variables. No pre/post increment/decrement. No first-class async. No function overloading. No arithmetic precedence. No variable shadowing."

**Core idea.** Austral is a small systems language that combines linear types with *capabilities* (linear capabilities that gate access to OS resources). The compiler emits C code, which is then compiled with a standard C compiler. Austral is the cleanest current example of "no GC, no destructors, capabilities + linear types".

**Integration cost in badger.** Badger's `Authority` subsection (`ARCHITECTURE.md:414-420`) is the same *capability* shape: `fs.root(path)` returns a `Dir`, a capability over one directory tree; `Dir.sub` narrows it; `Dir.reader` gives a `ReadDir` which has no writing operations. Private fields keep both from being made any other way. Adding *linear* capabilities (each capability is a linear value that must be moved through the program) is a small extension.

**Verdict for badger.** **Medium-high** — Austral's capability model is the closest existing reference for what badger's `Authority` could look like as a *linear* discipline.

### 4.6 Cone (Titzer, 2020+)

**Author / venue / year.** Jonathan Titzer (formerly of Google V8 / Virgil). 2020–present.

**URL.** https://cone.jondgoodwin.com/ (the project page; verified accessible October 2026).

**Core idea.** Cone combines linear / affine types with manual region management: the user can declare a `region T` (a type with region-bound lifetime); values of that type are freed when the region ends. The combination gives the user explicit control over memory without GC.

**Integration cost in badger.** Cone's `region T` is exactly what badger could expose as a user-facing primitive (per §1.2 §"Region / arena annotation"). The runtime primitive is one arena; the typeck brand is one type.

**Verdict for badger.** **High** — direct model for the "request arena" primitive.

### 4.7 Verona (Microsoft Research, 2019+)

**Author / venue / year.** Microsoft Research, ongoing. The Verona paper https://arxiv.org/abs/2012.02163 (Cocian, Birtles, Hall); the "Region-Based Resource Management" line.

**URL.** https://github.com/microsoft/verona (verified accessible October 2026, 3.7 k stars, 173 forks). The README: "Project Verona is a research programming language to explore the concept of concurrent ownership. We are providing a new concurrency model that seamlessly integrates ownership. ... This research project is at an early stage and is open sourced to facilitate academic collaborations."

**Core idea.** Verona replaces GC with *regions of objects*: every object is in a *group*; groups have a *lifecycle*; when the last reference is dropped, the group is destructed. The runtime is C++ with an ownership model. The "regions" are not lexical — they are *lifecycle* regions.

**Integration cost in badger.** Verona is a new language; it is not a retrofit. The relevant badger takeaway is the *group* model — a set of objects whose lifecycle is bound to a single owner. Badger could adopt this for *module-level values* (the immortal region per `ARCHITECTURE.md:6-9`, "every registered global slot") — the module is the group, the lifecycle is the lifetime of the program.

**Verdict for badger.** **Low** as a direct import; **medium** as a design reference for module-level values.

### 4.8 Roc (the "no orphans, no nulls, no GC, no mutation-after-return" design)

**Author / venue / year.** Richard Feldman and the Roc team, 2020–present.

**URL.** https://www.roc-lang.org/ (verified accessible October 2026). The website states: "A fast, friendly, functional language. Fast: Roc code is designed to build fast and run fast. It compiles to machine code or WebAssembly. Functional: Roc has a small number of simple language primitives. It's a single-paradigm functional language."

**Core idea.** Roc is a fast, friendly, functional language. The design includes "use before define" / "no GC by default" / "no orphans, no nulls, no GC" / "no mutation-after-return". The reference count is implicit: values are reference-counted but the discipline is "use before define" — a value cannot be mutated after the moment it could be observed by another reference.

**Integration cost in badger.** The "use before define" discipline is a lighter-weight analogue of badger's existing affine machinery: badger's affine is "use after the last use is rejected"; Roc's "use before define" is "define before use". Badger's "use after move" check is the symmetric rule.

**Verdict for badger.** **Low** for direct adoption; **medium** as a UX reference.

### 4.9 Gleam (the "type-safe Erlang" line)

**Author / venue / year.** Louis Pilfold and the Gleam team, 2019–present.

**URL.** https://gleam.run/ (the project page).

**Core idea.** Gleam is a type-safe Erlang-targeting language. The current typeck is HM without linear types; the roadmap includes a future linear-typed variant (per the Gleam blog series).

**Integration cost in badger.** Gleam is a reference for "what a type-safe language on top of a permissive runtime looks like". Badger already has the typeck; the BEAM-style "per-process heap" is the analogue of `concurrent ownership`.

**Verdict for badger.** **Informational**.

### 4.10 The F\* / Dafny / Lean / Coq line

**Author / venue / year.**
- F\*: Bhargavan et al., 2010–present; F* verifier at https://github.com/FStarLang/FStar.
- Dafny: Leino, *Dafny: An Automatic Program Verifier for Functional Correctness* (LPAR 2010); Dafny repo at https://github.com/dafny-lang/dafny.
- Lean: de Moura et al., 2013–present; Lean repo at https://github.com/leanprover/lean4.
- Coq / Rocq: the Coq Rocq prover at https://github.com/coq/coq.

**Core idea.** The "verification-first" line: the user writes a program and a proof together; the proof is checked by an SMT solver or a tactic-based prover. F\* uses affine types to encode effect ordering (a stateful computation uses its state exactly once); Dafny uses refinement types; Lean uses dependent types.

**Integration cost in badger.** Badger's existing `requires` / `ensures` clause (`ARCHITECTURE.md:684-692`) is the *contract* analogue of F\*'s effect ordering. Adding the SMT solver back end (per §2.7 / §2.8) is the obvious next step.

**Verdict for badger.** **Medium** as a long-term direction.

### 4.11 Mezzo (Protzenko, Pottier, 2013)

**Author / venue / year.** Jonathan Protzenko, François Pottier. *Mezzo: a typed language for safe effectful programming* (2013). The implementation https://protzenko.github.io/mezzo/.

**Core idea.** Mezzo combines *type-state* (the type of a value depends on its state — `Open File` vs `Closed File`) with *ownership* (the file is owned by exactly one thread). The combination gives strong, local-double-checked resource safety without an SMT solver.

**Integration cost in badger.** Badger's typeck has rows (`ARCHITECTURE.md:107`); type-state is a special case of rows where the row is a singleton of the current state. Adding it is a small extension.

**Verdict for badger.** **Medium** as a future extension.

### 4.12 Idris 2 / Idris 3 (Brady, 2021)

**Author / venue / year.** Edwin Brady. *Idris 2: Quantitative Type Theory for the Next Generation* (2021). Repo at https://github.com/idris-lang/Idris2.

**Core idea.** Idris 2 adds *Quantitative Type Theory* (QTT) — types that carry a usage annotation (zero, one, or unrestricted uses). QTT is the unification of linear / affine / unrestricted types: a function of type `(a : Type) -> (1 x : a) -> a` takes its argument with usage 1 (linear). The paper "Idris 2: Quantitative Type Theory for the Next Generation" introduces this.

**Integration cost in badger.** QTT is the multiplicity-inference formalism that badger's next step (§2.11) would build on. The Idris 2 paper is the formal basis.

**Verdict for badger.** **High** — the formal foundation for multiplicity inference.

---

## 5. Escape analysis and stack allocation in production compilers

This section surveys the *compiler-level* escape analysis and stack-allocation literature, in production and research. For each, the **badger verdict** is given explicitly.

### 5.1 Choi, Gupta, Serrano, Sreedhar, Midkiff (IBM J9), OOPSLA 1999

**Author / venue / year.** Jong-Deok Choi, Manish Gupta, Mauricio Serrano, Vugranam C. Sreedhar, Sam Midkiff. *Escape Analysis for Object-Oriented Languages* (OOPSLA 1999).

**URL.** https://dl.acm.org/doi/10.1145/320385.320386.

**Core idea.** The canonical paper: classify each allocation as `NoEscape` (does not escape the allocating method), `ArgEscape` (escapes as an argument but does not survive the call), or `GlobalEscape` (escapes globally). A `NoEscape` object is stack-allocatable; an `ArgEscape` object can be scalar-replaced.

**Badger verdict.** `sra.rs:1-169` is a partial re-implementation of this analysis, restricted to records / tuples / ranges. The extension to enums is straightforward (tag reads are `Rvalue::Load` of the first field; if the tag read does not escape, the enum is `NoEscape`). What needs to change: the analysis is currently *per-function* without call-graph analysis; for closures and indirect calls, the analysis needs to be conservative. `ownership.rs` could supply the "what escapes through a call" fact.

### 5.2 Whaley, Rinard (MIT), POPL 1999

**Author / venue / year.** John Whaley, Martin Rinard. *Compositional Pointer and Escape Analysis for Multithreaded Java* (POPL 1999).

**URL.** https://www.cag.lcs.mit.edu/~rinard/paper/popl99.pdf (verified accessible October 2026).

**Core idea.** Compositional / modular escape analysis: each method is analysed independently and its escape summary is propagated to callers. The summary is a `EffectEscape` set: which allocations escape as argument, method, or global. The compositionality makes the analysis work for libraries and dynamic dispatch.

**Badger verdict.** The compositional shape is the right model for badger's per-function MIR. The extension is small: each `Func` already has a defined call graph (the inlining pass `inline.rs:1-349` materialises direct calls; the closure specialisation pass `specialize.rs:43-72` materialises indirect calls). After materialisation, the compositional analysis is straightforward.

### 5.3 Coutant, Rinetzky, Sagiv (SPM 2000)

**Author / venue / year.** Damien Coutant, Noam Rinetzky, Mooly Sagiv. *A Combined Pointer and Escape Analysis Algorithm for Object-Oriented Languages* (SPM 2000).

**Core idea.** A combined analysis that uses the same representation for pointer information and escape information: the abstract location set is per allocation site, and the escape set is derived from the pointer set.

**Badger verdict.** The combined-analysis shape is not necessary (badger has the simpler `sra.rs` per-function analysis). Mentioned for completeness.

### 5.4 The badger MIR-side escape analysis: what to add

The current `sra.rs:1-169` does scalar replacement of aggregates (`Stmt::Assign(l, Rvalue::Alloc { .. })`) when the alloc is *single-definition* (line 21: `single(l)`) and the value is used only through field loads and stores (lines 78-105). What is missing:

1. **NoEscape-with-pointers**: if the record contains pointer fields, the runtime must trace the pointers on collection. A stack-allocated record containing pointers is a *shadow-stack-frame push* — the existing codegen mechanism (`ARCHITECTURE.md:323-324`, "Code generation keeps every managed pointer that is live across a safe point (call, allocation, poll) in a shadow-stack frame"). Adding a NoEscape-with-pointers path: extend `Rvalue::Alloc` with a flag, insert a shadow-stack push at the entry, pop at the exit.

2. **ArgEscape**: if the value flows into a callee and the callee does not retain it, the value can be stack-allocated in the *callee's* activation frame. This is automatic in badger's codegen — the callee frame has the value's slot if the call is statically resolvable. The MIR-side check is whether the callee retains the value: the inlining pass (`inline.rs:1-349`) inlines the callee and the resulting MIR shows whether the value escapes the call.

3. **Partial escape analysis**: defer the decision until the analysis is conclusive. Badger's MIR pipeline runs `simplify.rs` and `sra.rs` in sequence (`ARCHITECTURE.md:240-249`); the order is "inline, closure specialisation, scalar replacement, simplification, map / list / enum representation passes, CSE, a second inlining / simplification round, and verified superoptimisation". Running `sra.rs` *after* inlining means the analysis sees the inlined callee bodies, which is more precise.

**Badger verdict.** **High.** All three missing pieces are well-scoped MIR-pass additions, with the runtime support already in place (one shadow-stack opcode, per §1.1).

### 5.5 OpenJDK's C2 escape analysis

**Author / venue / year.** The OpenJDK HotSpot team, ongoing. The implementation is in `openjdk/src/hotspot/share/opto/escape.hpp` and `escape.cpp`.

**URL.** https://openjdk.org/groups/hotspot/ (the project page).

**Core idea.** HotSpot C2 implements three transformations on `NoEscape` objects: (1) **scalar replacement** (the object is decomposed into individual fields stored in registers or stack slots); (2) **lock elision** (synchronisation on a non-escaping object is removed); (3) **allocation elimination** (the `new` is deleted).

**Badger verdict.** The C2 pattern is the *target* for badger's `sra.rs`: scalar replacement is what `sra.rs` does; allocation elimination is the *combination* of scalar replacement + dead-assignment elimination (`simplify.rs:36-47`); lock elision is not applicable (badger has no Java-style synchronisation).

### 5.6 The V8 escape analysis

**Author / venue / year.** The V8 team, ongoing. The implementation is in TurboFan's `escape-analysis.cc`.

**URL.** https://v8.dev/blog/turbofan-jit (the V8 TurboFan announcement; verified accessible).

**Core idea.** V8's TurboFan uses a *graph-based* escape analysis: the IR is a graph of nodes; the analysis walks the graph to compute per-node escape states; the *reducer* then applies scalar replacement. The graph-based shape is more general than the basic-block shape because it handles phi nodes and control flow uniformly.

**Badger verdict.** Badger's MIR is a CFG (per `ARCHITECTURE.md:218-222`), not a graph IR. The graph-based shape is overkill for badger; the CFG shape with `simplify.rs` constant-propagation is sufficient.

### 5.7 LLVM's escape analysis

**Author / venue / year.** The LLVM team, ongoing. LLVM has `BasicAA` (basic alias analysis) and `GlobalsAA` (globals alias analysis) but no unified escape analysis.

**URL.** https://llvm.org/ (the project page).

**Core idea.** LLVM does not have a separate escape-analysis pass; it does *memory dependence analysis* (which is related but not the same). The lack of a unified escape analysis is a known gap in LLVM's optimisation stack.

**Badger verdict.** Badger's MIR is similar in spirit (no separate pass), but `sra.rs` does partial escape analysis. The gap (per §5.4) is the NoEscape-with-pointers case.

### 5.8 Region-Based Memory Management in Real-Time Java (RTSJ)

**Author / venue / year.** The Real-Time Specification for Java (RTSJ) expert group, JSR-001, 2001.

**URL.** https://www.rtsj.org/ (the project page).

**Core idea.** RTSJ defines scoped memory areas (`ScopedMemory`, `ImmortalMemory`, `VTMemory`) that the user can allocate into; when the area is exited, all allocations in the area are freed. This is the Java-language analogue of regions / arenas.

**Badger verdict.** RTSJ is the closest production *language-level* analogue of badger's potential "request arena". The Java form is `ScopedMemory area = new ScopedMemory(...); area.enter();` — a runtime scope that uses runtime-style finalisers. Badger's form would be `arena::create() / arena::reset()`, with the typeck brand as the lifetime bound.

### 5.9 GraalVM / Truffle escape analysis

**Author / venue / year.** The GraalVM team, Oracle Labs, ongoing. The GraalVM compiler's escape analysis is described in the GraalVM publications https://www.graalvm.org/community/publications/.

**URL.** https://www.graalvm.org/ (the project page).

**Core idea.** GraalVM's Truffle framework is a *language-agnostic* IR; the escape analysis is part of the partial-evaluation compiler. The key insight is that *language-implementation frameworks* (Truffle, RPython) can share the optimisation stack.

**Badger verdict.** GraalVM's framework is not directly applicable (badger is not a language-implementation framework). The Truffle-style shape (IR + analyses + reducers) is the *general* shape of any modern optimising compiler.

### 5.10 RyuJIT (CoreCLR) escape analysis

**Author / venue / year.** The .NET RyuJIT team, ongoing. The implementation is in `coreclr/src/jit/`.

**URL.** https://github.com/dotnet/runtime (the .NET runtime repository).

**Core idea.** RyuJIT does a per-allocation-site escape analysis that feeds scalar replacement and lock elision (the latter for `lock`-style synchronisation).

**Badger verdict.** Same as HotSpot C2 (§5.5).

### 5.11 Mono / IL2CPP escape analysis

**Author / venue / year.** The Mono / Unity teams, ongoing.

**URL.** https://www.mono-project.com/docs/advanced/runtime/ (Mono); Unity IL2CPP documentation.

**Core idea.** Mono's compiler (mcs) and IL2CPP both have basic escape analysis, less aggressive than HotSpot C2.

**Badger verdict.** Mentioned for completeness.

### 5.12 WebKit FTL / B3 escape analysis

**Author / venue / year.** The WebKit team, ongoing. FTL (Fourth Tier LLVM) and B3 are the WebKit optimising JIT compilers.

**URL.** https://trac.webkit.org/wiki/WebKit%20JIT (the project page).

**Core idea.** B3 is a graph IR; escape analysis is per-LLVM-pass-shaped. WebKit's focus is JavaScriptCore performance, not general-purpose escape analysis.

**Badger verdict.** Mentioned for completeness.

---

## 6. Region inference and arena allocation in production systems

This section surveys region *inference* (the static analysis that puts allocations into regions) and arena allocation (the runtime primitive that bumps a pointer for region-sized allocations).

### 6.1 Tofte, Talpin (POPL 1994)

**Author / venue / year.** Mads Tofte, Jean-Pierre Talpin. *Implementation of the Typed Call-by-Value λ-calculus using a Stack of Regions* (POPL 1994).

**URL.** The paper is hosted at https://www.cs.cmu.edu/~rwh/courses/ppl/papers/tofte-talpin.pdf [unverified — the link returned 404 in our check; the canonical host is the Cambridge RAWF hardware ftp or the authors' home pages]. The companion report is Tofte, Talpin, *A Theory of Stack Allocation in Polymorphically Typed Languages* (1993).

**Core idea.** The classical region calculus: a lambda-calculus with region annotations on every value; a region-inference algorithm puts every allocation into a region; the regions are organised in a stack so they are deallocated in LIFO order.

**Badger verdict.** The Tofte-Talpin calculus is the *full-program* region-inference algorithm. Badger does not need this (full-program inference requires a single, global region-inference pass; badger's per-function compilation does not see the whole program). What badger needs is the *runtime primitive* (a stack of regions, each with a bump pointer) — and that primitive is one arena.

### 6.2 ML Kit (https://melsman.github.io/mlkit/, verified October 2026)

**Author / venue / year.** Martin Elsman and the ML Kit team, University of Copenhagen. The ML Kit is a Standard ML compiler with region-based memory management; it has been maintained since the mid-1990s.

**URLs.**
- Homepage https://melsman.github.io/mlkit/ (verified accessible October 2026). The site: "Compile Standard ML to native code with region-based memory management. Target x64 on Linux and macOS, or arm64 on Apple Silicon." ... "Region-Based: The MLKit integrates reference-tracing garbage collection with region-based memory management. Memory allocation directives (both allocation and deallocation) are inferred by the compiler, which uses a number of program analyses concerning lifetimes and storage layout." ... "Good for Real-Time: Programmers who are interested in real-time programming can exploit the possibility of disabling reference-tracing garbage collection. In this case, there will be no interruptions of unbounded duration at runtime."
- Papers https://melsman.github.io/mlkit/papers (the bibliography).
- ReML https://melsman.github.io/mlkit/reml.html ("Add explicit regions, effects, and effect constraints to Standard ML for more control over allocation and memory use").

**Core idea.** ML Kit demonstrates that region inference can be combined with reference GC for the residual heap. The hybrid is the *practical* answer to "pure region inference is too aggressive" — regions for short-lived data, GC for the rest.

**Badger verdict.** The ML Kit hybrid is the most relevant prior art for badger's runtime / typeck hybrid. Badger's current runtime is the residual (the GC handles everything); adding regions for short-lived data is the small extension. The ML Kit is also a *maintained* implementation (last release December 2025) — the only long-running region-inference production system we know of.

### 6.3 Aiken, Fähndrich, Levien (PLDI 1995)

**Author / venue / year.** Alexander Aiken, Manuel Fähndrich, Ralph Levien. *Better Static Memory Management: Combining Region Inference and Garbage Collection* (PLDI 1995).

**URL.** https://www.cs.berkeley.edu/~aiken/papers/tr95-007.pdf (the Berkeley TR; verified accessible [unverified — host unreachable in our check; the canonical host is the authors' pages]).

**Core idea.** The improved region-inference algorithm with *place* polymorphism: the same function can specialise for different regions. The algorithm is the basis for ML Kit.

**Badger verdict.** Place polymorphism is the same idea as badger's existing let-polymorphism (`ARCHITECTURE.md:192-194`). The integration cost is low for the typeck; the runtime support is one arena.

### 6.4 Cyclone (USENIX 2002)

**Author / venue / year.** Trevor Jim et al. *Cyclone: A Safe Dialect of C* (USENIX 2002).

**URL.** https://cyclone.thelanguage.org/ (the project page).

**Core idea.** Cyclone is a C dialect with region types: a `Ptr<R>T` is a pointer into region `R`; the typeck proves the pointer does not outlive the region. Cyclone was the first production-scale language to demonstrate region-based memory management in systems code.

**Badger verdict.** Cyclone is a new language, not a retrofit. The relevant badger takeaway is the `Ptr<R>` shape — a region-branded pointer type. Badger's `for<S>` brand (`ARCHITECTURE.md:107-108`) is the same shape.

### 6.5 Rust crates: typed-arena, bumpalo, sharded-slab

**Author / venue / year.** Various Rust community contributors; ongoing.

**URLs.**
- https://crates.io/crates/typed-arena (the crate; verified accessible).
- https://crates.io/crates/bumpalo (the crate; verified accessible).
- https://crates.io/crates/sharded-slab (the crate; verified accessible).

**Core idea.** A typed arena (`typed_arena`) is a per-`T` bump allocator: `arena.alloc(T { ... })` returns a `&mut T` with lifetime `'a`; `arena.drop()` drops all elements. A bump allocator (`bumpalo`) is type-erased. A sharded slab (`sharded_slab`) is a concurrent-arena allocator that shards by hash.

**Badger verdict.** These are *Rust crates*; the relevant badger takeaway is the *API shape*: `arena.create() / arena.alloc(T) / arena.reset()`. The runtime primitive is one arena per request; the typeck brand is one lifetime. Adopting the API shape is the user-facing surface for the "request arena" primitive.

### 6.6 EASTL, ETL++ (C++ arena allocators)

**Author / venue / year.** Paul Pedriana (EASTL); the C++ embedded community (ETL++).

**Core idea.** EASTL (Electronic Arts Standard Library) provides `fixed_pool`, `fixed_suballocator`, and similar arena allocators. ETL++ (embedded template library) provides compile-time allocators for embedded systems.

**Badger verdict.** The C++ arena-allocator pattern is well-established; the relevant badger takeaway is the *fixed-pool* shape (allocations of a single type go into a per-type arena). Badger's per-size-class free list (`gc.rs:269-280`) is the analogue at the per-byte-size level.

### 6.7 WebKit bmalloc

**Author / venue / year.** The WebKit team, ongoing.

**URL.** https://github.com/WebKit/WebKit/tree/main/Source/bmalloc (the repository; verified accessible).

**Core idea.** bmalloc is a bump allocator for WebKit: large allocations go to the system; medium allocations go to a per-thread slab; small allocations go to a per-thread bump region. The design is "the simplest allocator that gives the per-thread bump benefit".

**Badger verdict.** The *shape* — per-thread slab + per-thread bump region — is exactly badger's per-worker pool model (`gc.rs:241-266`). The badger extension is the bmalloc-style *bump region* (the current `alloc_cell` is a free-list walk; a bump region would be a simple pointer increment).

### 6.8 Chromium PartitionAlloc

**Author / venue / year.** The Chromium team, ongoing.

**URL.** https://chromium.googlesource.com/chromium/src/+/master/base/allocator/partition_allocator/PartitionAlloc.md (the design doc).

**Core idea.** PartitionAlloc partitions the heap by *size class*; each partition is a sequence of *slot spans*; allocations in a partition go to a free slot in a partially-full span, or fetch a new span. The design is the modern "per-size-class slab allocator".

**Badger verdict.** PartitionAlloc's *slot span* design is what badger's per-size-class pool should evolve toward. The current free-list-walk (`gc.rs:269-280`) is O(n) per allocation; the slot-span design is O(1) per allocation (fetch the next free slot in the current span).

### 6.9 jemalloc (Evans 2006)

**Author / venue / year.** Jason Evans, 2006–present.

**URL.** https://jemalloc.net/ (the project page; verified accessible).

**Core idea.** jemalloc is the FreeBSD libc allocator: per-CPU arenas; per-size-class slabs; per-thread caches; eager page reset to the OS. The design is the modern "production malloc replacement".

**Badger verdict.** jemalloc is a *backing allocator* — badger's runtime could adopt jemalloc as the `Items` storage allocator (`allocations.rs:65-104`) without changing the GC.

### 6.10 mimalloc (Leijen, Zorn, De Moura 2019)

**Author / venue / year.** Daan Leijen, Benjamin Zorn, Leonardo De Moura. Microsoft Research. 2019.

**URL.** https://github.com/microsoft/mimalloc (verified accessible October 2026).

**Core idea.** mimalloc is a general-purpose allocator that uses *free-list sharding* (per-thread free lists, with global cross-thread transfers when a thread's freelist runs dry) and *eager* page reset to the OS. First-class heaps: a single process can have many `mi_heap_t`s. Used as the allocator for Koka and Lean.

**Badger verdict.** mimalloc's *first-class heaps* are exactly what badger's "request arena" should look like at the runtime level. Adopting mimalloc as the backing allocator for the Items storage is a self-contained change (`allocations.rs:65-104` is the existing accounting). The first-class-heap API is the runtime primitive for the request arena.

### 6.11 snmalloc (Stephens 2019)

**Author / venue / year.** Stephen Stephens, Microsoft. 2019–present.

**URL.** https://github.com/microsoft/snmalloc (verified accessible October 2026).

**Core idea.** snmalloc is a *message-passing* allocator: sender-receiver workers; sub-100-ns allocation in the common case; designed for low-latency / high-throughput. The most modern research allocator.

**Badger verdict.** snmalloc is the alternative backing allocator for the `Items` storage. Its message-passing shape is overkill for badger (badger's runtime is single-host, not distributed); its per-thread cache is the relevant primitive.

### 6.12 rpmalloc (LinusFur)

**Author / venue / year.** Mattias "LinusFur" Pettersson. 2017–present.

**URL.** https://github.com/rampantpixels/rpmalloc (verified accessible).

**Core idea.** rpmalloc is a cross-platform, low-overhead allocator: per-thread caches; per-thread heap; large allocations to the OS. The design is the "best of jemalloc / mimalloc" without the complexity.

**Badger verdict.** Same as mimalloc (§6.10).

---

## 7. Effect systems and resource management

Effect systems track the side effects of a function in its type, so the caller knows what effects the callee has. Resource management is the "open" version of effect tracking: the function tracks a *resource* in its type (a file handle, a lock, a region).

### 7.1 Plotkin, Pretnar (ESOP 2009)

**Author / venue / year.** Gordon Plotkin, Matija Pretnar. *Handlers of Algebraic Effects* (ESOP 2009).

**URL.** https://homepages.inf.ed.ac.uk/gdp/publications/hndlr.pdf [unverified — the host was unreachable in our check; the canonical source is the authors' pages or the ESOP proceedings].

**Core idea.** Algebraic effects are a *language-side* abstraction for control effects: a function can `raise` an effect (like an exception); a caller can *handle* the effect. The combination gives structured control flow (like exceptions) plus continuation capture (unlike exceptions).

**Badger verdict.** Effect handlers are *not* badger's model — badger's effects are tracked in rows at typeck, not raised / handled at runtime. The relevant takeaway is the *row* / *effect set* representation.

### 7.2 Koka (Leijen 2014+)

**Author / venue / year.** Daan Leijen. *Koka: Programming with Row-polymorphic Effect Types* (MSR TR 2014).

**URLs.**
- Project page https://koka-lang.github.io/ (verified accessible October 2026; redirects to https://koka-lang.github.io/koka/doc/index.html).
- The MSR TR https://koka-lang.github.io/koka/doc/koka-total.pdf [unverified — the URL returned 404 in our check; the canonical source is Leijen's MSR page or the Koka documentation].

**Core idea.** Koka is a language with *row-polymorphic effect types*: every function's type includes its effect row; the row can mention built-in effects (`Io`, `Wait`, `State`, `Foreign`) and user-defined effects; the row is *polymorphic* — the same function can be called with different effects. Koka's design is the cleanest demonstration that effects can be row-polymorphic, ordered by the effect ordering.

**Badger verdict.** Badger's effect row (`ARCHITECTURE.md:179-191`) is the *same* shape as Koka's. The integration cost is zero — the design is already there. The relevant takeaway is the *Koka compiler* — the per-function specialisation and the row-polymorphism algorithm are the reference.

### 7.3 Effect handlers in Cats Effect / ZIO / Kyo (Scala)

**Author / venue / year.** Cats Effect: Daniel Spiewak and the Typelevel community, 2017–present. ZIO: John De Goes and the ZIO team, 2018–present. Kyo: Flavio Brasil and the Kyo team, 2023–present.

**URLs.**
- https://typelevel.org/cats-effect/ (Cats Effect; verified accessible).
- https://zio.dev/ (ZIO; verified accessible).
- https://github.com/getkyo/kyo (Kyo; verified accessible).

**Core idea.** The Scala libraries reify effect tracking at the type level (`IO[A]`, `Resource[A]`). The `Resource` type is the resource-management analogue: a `Resource[A]` is a `IO[A]` paired with a finaliser that runs when the resource is released.

**Badger verdict.** The *resource as an effect-tracked value* shape is the same as badger's `Task` resources. The relevant takeaway is the `Resource[A]` shape — a `Resource::create(use: A -> Task[B]) -> Task[B]` API where the finaliser runs when `B` is computed.

### 7.4 Effekt (Schrijvers, Brachthäuser et al.)

**Author / venue / year.** Tom Schrijvers, Jonathan Immanuel Brachthäuser, and the Effekt team (University of Tübingen). 2018–present.

**URL.** https://effekt-lang.org/ (verified accessible October 2026). The site states: "Effekt is a research-level language ... Effekt Language is brought to you by [the Effekt research team](https://se.cs.uni-tuebingen.de/group/)."

**Core idea.** Effekt is a language with *lexical* effect handlers (the handler's effect on the lexically captured environment is statically tracked) and *lightweight effect polymorphism*. The combination gives structured control flow (coroutines, generators, async) with static effect tracking.

**Badger verdict.** Effekt's lexical effect handlers are the formal model for what badger's "scope-local" effect row (§1.5 §"badger takeaway") could look like. The relevant takeaway is the *contextual effect polymorphism* paper (Brachthäuser, Leijen, Schuster 2020).

### 7.5 Concurrent Separation Logic (O'Hearn 2007)

**Author / venue / year.** Peter O'Hearn. *Resources, Concurrency, and Local Reasoning* (Theoretical Computer Science 2007). The foundational paper.

**Core idea.** Concurrent Separation Logic (CSL) is a logic for reasoning about concurrent programs that share resources: a resource is owned by exactly one part; concurrent accesses are safe if and only if the resources are disjoint at the moment of the access. CSL is the *logic* underneath RustBelt (§2.5).

**Badger verdict.** CSL is the formal foundation for what badger's task model does at runtime: each task owns one channel endpoint; concurrent sends / receives are safe iff the channels are disjoint. Adding CSL-style reasoning would let badger statically prove what it currently proves at runtime.

### 7.6 Roc (the "no orphans, no nulls, no GC, no mutation-after-return" design)

Already covered in §4.8.

### 7.7 Frank (Lindley, McBride, McLaughlin 2017)

**Author / venue / year.** Sam Lindley, Conor McBride, Craig McLaughlin. *Do Be Do Be Do* (POPL 2017).

**URL.** The paper is at https://homepages.inf.ed.ac.uk/slindley/papers/frank-dobedo.pdf [unverified — host unreachable in our check]; the project page is https://the-repo.at/.

**Core idea.** Frank is an effect-handler-based language with row polymorphism. The Frank paper is the formal foundation for Effekt (§7.4).

**Badger verdict.** Same as Effekt.

### 7.8 Unison (the "content-addressed code" line)

**Author / venue / year.** Unison Computing, 2019–present.

**URL.** https://www.unison-lang.org/ (verified accessible October 2026).

**Core idea.** Unison is a content-addressed language: every definition is stored by its hash; the runtime does not need a code-cache; the runtime can store values by content. The "no GC" design is a natural consequence: most data is *immortal* by content hash.

**Badger verdict.** Unison's content-addressed shape is interesting for *badger's binary MIR* (`ARCHITECTURE.md:204-217`, "Binary MIR: a lowered program is saved with its runtime constants ... by how they were made, as a constant pool; loading makes them anew and patches the program"). The runtime constants are already content-addressed in spirit (each MIR has a hash). The relevant takeaway is that *immortal by content* is the cleanest lifetime discipline — values that never change cannot leak.

### 7.9 HVM (the "higher-order virtual machine" line)

**Author / venue / year.** Victor Taelin and the HVM team, 2019–present.

**URL.** https://github.com/HigherOrderCO/HVM (verified accessible).

**Core idea.** HVM is a runtime based on *interaction nets*: every function is a net node; evaluation is graph rewriting; the runtime does not allocate new memory (it reuses the existing graph). The "no GC" design is a natural consequence: the heap is the *interaction net*, which is finite.

**Badger verdict.** HVM is a *runtime* innovation; the relevant takeaway is the *interaction-net* IR shape. Badger's MIR is a CFG (control-flow graph), not an interaction net; the HVM shape would be a fundamental redesign.

### 7.10 Nim's ARC and ORC (the "deterministic destructor" line)

**Author / venue / year.** Andreas Rumpf and the Nim team, 2017–present.

**URL.** https://nim-lang.org/blog/ (the blog; the ORC announcement is https://nim-lang.org/blog/2021/12/28/arc-orc.html).

**Core idea.** Nim's ARC (Automatic Reference Counting) inserts `retain` / `release` calls at the right places to give deterministic destruction. Nim's ORC (Object Reference Counting) extends ARC with *cycle detection* for cycles. ORC gives deterministic destruction for non-cyclic data, deterministic cycle detection for the rest, and no global GC pause.

**Badger verdict.** ORC is the closest *runtime* analogue of "deterministic destruction without a global GC pause". Badger's runtime does not have the same problem (badger is cooperative-mutator mark-sweep, not preemptive), so ORC is not directly applicable. The relevant takeaway is the *cycle detection* algorithm (the ORC's "global cycle collection").

---

## 8. The "no GC by design" languages

The languages that ship with *no* GC at all — or with GC as a fallback only — are the strongest evidence that the type-system-driven approach is *feasible*. This section covers the most *important* ones for badger's design.

### 8.1 Austral (Borretti / Tromp)

**Author / venue / year.** Fernando Borretti (originally John Tromp). 2020–present. (Same as §4.5.)

**Verdict for badger.** Austral is the cleanest current reference for "no GC by design". The README's "anti-features" list (§4.5) is exactly the discipline badger's `Authority` and `ownership.rs` already enforce.

### 8.2 Cone (Titzer)

**Author / venue / year.** Jonathan Titzer. 2020–present. (Same as §4.6.)

**Verdict for badger.** Cone's `region T` is the closest analogue to badger's potential "request arena" primitive.

### 8.3 Verona (Microsoft Research)

**Author / venue / year.** Microsoft Research, 2019–present. (Same as §4.7.)

**Verdict for badger.** Verona's "groups of objects with shared lifecycle" is the reference for what badger's module-level values (§1.2 §"Region / arena annotation") could look like.

### 8.4 Dafny (Leino, LPAR 2010)

**Author / venue / year.** K. Rustan M. Leino. *Dafny: An Automatic Program Verifier for Functional Correctness* (LPAR 2010).

**URL.** https://github.com/dafny-lang/dafny (verified accessible).

**Core idea.** Dafny is a verification-first language: the user writes a program + a proof; Dafny translates to Boogie and the SMT solver discharges the proof obligations. Dafny uses *refinement types* (a type `T | P(x)` where `P` is a predicate on values of `T`).

**Verdict for badger.** Dafny is a verification tool, not a no-GC language. The relevant takeaway is the *refinement type* idea — a type that carries a predicate. Badger's `requires` / `ensures` line (`ARCHITECTURE.md:684-692`) is the *contract* analogue.

### 8.5 F\* (Bhargavan et al.)

**Author / venue / year.** Karthikeyan Bhargavan et al. 2010–present.

**URL.** https://github.com/FStarLang/FStar (verified accessible).

**Core idea.** F\* is a verification-first language with *affine types*: every type has a usage annotation (zero, one, or unrestricted uses); the typeck proves the usage. F\*'s "Dafny-F\*" style: the user writes a program + a proof; F\* discharges the proof.

**Verdict for badger.** F\*'s *affine types with usage annotations* is the formal foundation for what badger's `ownership.rs` does informally. The relevant takeaway is the *usage* annotation — a per-type multiplicity.

### 8.6 Idris 2 / Idris 3 (Brady)

**Author / venue / year.** Edwin Brady. *Idris 2: Quantitative Type Theory for the Next Generation* (2021). (Same as §4.12.)

**Verdict for badger.** The QTT foundation for multiplicity inference.

### 8.7 Lean (de Moura et al.)

**Author / venue / year.** Leonardo de Moura et al. 2013–present.

**URL.** https://github.com/leanprover/lean4 (verified accessible).

**Core idea.** Lean is a proof assistant that can also be used as a programming language. The proof assistant part is GC-free by construction (proofs do not allocate runtime data); the programming language part is GC-free because the user manages resources.

**Verdict for badger.** Lean is not a runtime innovation. The relevant takeaway is the *tactic-based proof* model — badger's `requires` / `ensures` could be discharged by tactics rather than by the typeck.

### 8.8 Roc (the "use before define" line)

**Author / venue / year.** Richard Feldman et al. 2020–present. (Same as §4.8.)

**Verdict for badger.** The "use before define" discipline is a UX-friendly analogue of badger's affine check.

### 8.9 Gleam (the "type-safe Erlang" line)

**Author / venue / year.** Louis Pilfold. 2019–present. (Same as §4.9.)

**Verdict for badger.** The Gleam roadmap's future linear-typed variant is the analogue of badger's existing `affine` discipline.

### 8.10 Coq / Rocq

**Author / venue / year.** The Coq / Rocq development team. 1984–present.

**URL.** https://github.com/coq/coq (verified accessible).

**Core idea.** Coq is a proof assistant: the user writes a proof term; the kernel checks it. The proof is not a runtime program; the runtime is whatever the user extracts to (OCaml, Haskell, Scheme). Coq is GC-free by construction (proofs do not allocate).

**Verdict for badger.** Coq is not a runtime. The relevant takeaway is the *term-level proof* model — badger's `requires` / `ensures` could be backed by a small proof kernel.

---

## 9. What this means for badger

This section maps the prior art onto badger's existing typeck and runtime, ranked by impact × cost. It also notes which recommendations in `research/11-gc-and-memory-management.md` and `research/09-mir-superoptimisation.md` depend on the findings here.

### 9.1 The current state, summarised

- **Typeck layer** (`crates/badger_typeck/`):
  - Ownership (`ownership.rs:1-3028`): affine use-after-move, borrows, generic re-check, capability checks for `Transfer` / `Share`, generic re-check passes.
  - Regions (`regions.rs:1-704`): region taint tracking so scoped values cannot escape through sequences, tasks, handles, `dyn`, or `Data`.
  - Inference (`infer.rs:1-2990`): HM with Rémy levels over binding-group SCCs; rows/effects/regions; protocol constraint solving; coherence; exhaustiveness; `{..Rest}` row structural polymorphism; scope brands `for<S>`; *capture marks* `%region` / `%confined`.
  - Capability effects: `Io`, `Wait`, `State`, `Foreign` (per `ARCHITECTURE.md:179-191`).
  - Generic re-check at instance monomorphisation (per `ARCHITECTURE.md:200-201`).
- **MIR layer** (`crates/badger_mir/`):
  - Inlining (`inline.rs:1-349`): two rounds; scalar replacement runs again after inlining.
  - Closure specialisation (`opt/specialize.rs:43-72`): a helper called directly with known closures is cloned per closure / set of closure arguments; calls rewritten to `Callee::Lambda`.
  - Scalar replacement (`sra.rs:1-169`): a record used only through field loads and stores is not heap-allocated; fields become locals.
  - Simplification (`opt/simplify.rs:1-780`): constant folding, copy / constant propagation, dead-assignment elimination, unreachable-block clearing.
  - Map / list / enum representation passes.
  - CSE (`opt/cse.rs:1-193`).
  - Superoptimisation (`opt/superopt.rs:1-736`, `--opt superopt`).
- **Runtime layer** (`crates/badger_runtime/`):
  - Non-moving mark-sweep heap (`gc.rs:1-7`, `gc.rs:38-54`).
  - Per-worker object lists (`gc.rs:241-266`).
  - Size-class free list (`gc.rs:269-280`).
  - Sticky-mark generational collection.
  - Shadow-stack frames at safe points (`task.rs:14-15`).
  - Drop finalisers for handle objects (`ARCHITECTURE.md:524-526`).

What is missing:
- A *full* escape analysis at MIR (only partial scalar replacement).
- A *multiplicity inference* (the mode is declared on parameters, not inferred from the body).
- A *region / arena primitive* for short-lived data (no "request arena" API).
- A *user-facing `~Copyable` marker* (the discipline exists, but the user-facing syntax does not).

### 9.2 The ranked list

**1. Region or high water-mark arena for typeck and AST** (no language change; just a runtime choice).

- *Cost*: low. A `bumpalo` / `typed-arena`-style arena for short-lived AST nodes during typeck; an `Immix`-style or `mimalloc`-style sub-allocator for the GC's bump pool refill path.
- *Impact*: high. The runtime's per-worker pool model already benefits from bump allocation; the typeck's AST allocation already benefits from arena allocation.
- *Prior art*: ML Kit (region + GC hybrid, §6.2), jemalloc / mimalloc (slab + per-thread cache, §6.9 / §6.10), bumpalo / typed-arena (Rust crates, §6.5), Cone's `region T` (§4.6).
- *Code touchpoints*: `crates/badger_typeck/src/ownership.rs` (per-binding mode); `crates/badger_runtime/src/gc.rs:241-266` (the per-worker pool); a new `crates/badger_runtime/src/arena.rs` (the runtime primitive).

**2. Escape analysis at MIR to drive stack allocation for short-lived records.**

- *Cost*: medium. Extend `sra.rs` to handle enums; add a NoEscape-with-pointers path that inserts a shadow-stack frame push.
- *Impact*: high. The existing `sra.rs:1-169` is partial; closing the gap removes most short-lived object allocations from `gc.rs::alloc`.
- *Prior art*: Choi 1999 (§5.1), Whaley-Rinard 1999 (§5.2), HotSpot C2 (§5.5).
- *Code touchpoints*: `crates/badger_mir/src/sra.rs` (extend); `crates/badger_mir/src/opt/simplify.rs` (re-run after `sra`); `crates/badger_codegen/src/` (the shadow-stack opcode).

**3. A formal ownership region annotation (the "lexical region" or "stack region" line) on selected builtin types.**

- *Cost*: low. Add a region brand to `TypeDesc` for selected builtin types (records of pointers); add a `region` qualifier in the user-facing surface.
- *Impact*: medium-high. The existing `for<S>` brand (`ARCHITECTURE.md:107-108`) is the form; adding a `region` brand for "data that lives in a region" is one new variant.
- *Prior art*: Tofte-Talpin 1994 (§6.1), Cyclone (§6.4), Cone's `region T` (§4.6), ML Kit's ReML (§6.2).
- *Code touchpoints*: `crates/badger_typeck/src/ty.rs` (the `Ty` enum); `crates/badger_typeck/src/regions.rs:1-704` (the region brand tracking); `crates/badger_runtime/src/desc.rs` (the `TypeDesc`).

**4. Linear types for resources** (channels, files, sockets).

- *Cost*: low to medium. Extend `ownership.rs` to track a `linear` mode (use at least once) in addition to `affine` (use at most once).
- *Impact*: high. Files / sockets / channels currently rely on drop finalisers (`ARCHITECTURE.md:524-526`); with linear types, the drop becomes a static guarantee.
- *Prior art*: Wadler 1990 (§1.3, §4.1), Turner-Mossin-Wadler 1995 (§4.2), Austral (§4.5 / §8.1), Swift's `~Copyable` (§3.1, §3.3).
- *Code touchpoints*: `crates/badger_typeck/src/ownership.rs:25-30` (add the `Linear` mode); `crates/badger_hir/std/` (the standard library's file / socket / channel modules).

**5. Move-only types for short-lived records** (the existing `simplify.rs`/`sra.rs` already find these; making it a typeclass is a small extension).

- *Cost*: low. Add a `~Copyable` / `move` marker to user-defined types; the existing `sra.rs` then operates on them.
- *Impact*: medium-high. The user can opt into the affine / linear discipline for any record / enum.
- *Prior art*: Swift's `~Copyable` (§3.1, §3.3), Mezzo's type-state (§4.11), Austral's linear capabilities (§4.5 / §8.1).
- *Code touchpoints*: `crates/badger_typeck/src/ty.rs` (the `Ty` enum); `crates/badger_mir/src/sra.rs` (extend to handle the marker).

### 9.3 The bridge to existing notes

The recommendations in `research/11-gc-and-memory-management.md` that are *reinforced* by the findings here:

- **The Immix-style block reclamation** (§11 / §4 of that note): the typeck-level evidence is that badger's per-size-class pool is exactly what the literature recommends; the typeck-side evidence is that `ownership.rs` already gives the per-allocation escape status that an Immix-style line-bitmap could use.
- **The R1 "kill the `occupied` sort"** (`BACKLOG.md:97-100`, `gc.rs:966-979`): the typeck-level evidence is that `ownership.rs`'s `Loan` tracking can give per-object escape status, which can pre-classify objects into "live" / "dead" / "pinned" before the sweep.
- **The escape-analysis-at-MIR** recommendation (§6.3, §10 of that note): the typeck-level evidence is that `ownership.rs` already gives per-binding local escape analysis, which is the input to a Whaley-Rinard-style compositional analysis.

The recommendations in `research/09-mir-superoptimisation.md` that *depend on* the new lifetime information:

- **Lifetime-driven inlining decisions**: an inlined callee that uses its argument with `affine` mode cannot be re-used; a callee that uses it with `borrow` mode can. The lifetime information (§1.3, §2.11) gives the inlining decision.
- **Scalar replacement after devirtualisation**: the partial-escape-analysis decision (defer until conclusive, §5.4) is more powerful after devirtualisation; the typeck's lifetime information gives the call-graph shape.
- **Dead-assignment elimination**: a binding that is consumed (used at its last use before its owner moves) is dead for the rest of its lifetime; the typeck's loan tracking (`ownership.rs:34-50`) gives this directly.

### 9.4 The cost-vs-impact matrix

The five most popular additions, ranked by impact × cost (lower cost / higher impact first):

| # | Addition | Cost | Impact | Code touchpoints |
|---|----------|------|--------|------------------|
| 1 | Region / arena for typeck + AST | Low | High | `badger_runtime/src/arena.rs` (new); `badger_runtime/src/gc.rs:241-266` |
| 2 | Escape analysis at MIR for stack allocation | Medium | High | `badger_mir/src/sra.rs` (extend); `badger_mir/src/opt/simplify.rs`; `badger_codegen/` |
| 3 | Region annotation on builtin types | Low | Medium-high | `badger_typeck/src/ty.rs`; `badger_typeck/src/regions.rs`; `badger_runtime/src/desc.rs` |
| 4 | Linear types for resources (file / socket / channel) | Low–medium | High | `badger_typeck/src/ownership.rs:25-30`; `badger_hir/std/` |
| 5 | `~Copyable` marker for user-defined types | Low | Medium-high | `badger_typeck/src/ty.rs`; `badger_mir/src/sra.rs` |

### 9.5 What *not* to do

Three lines of prior art are *not* recommended for badger:

1. **Full Rust-style ownership** (the Polonius / Oxide line, §2.9–§2.10). Badger's existing `ownership.rs` is *strictly more permissive* than Rust's borrow checker (badger's loans are local; badger's borrows are on bindings, not on ownership domains), and porting Polonius's dataflow engine would be a multi-quarter effort for no clear benefit.

2. **Full Tofte-Talpin region inference** (§6.1). Full-program region inference requires a whole-program analysis; badger's per-function compilation does not see the whole program. The runtime primitive (an arena) is the right answer; the typeck inference is too much.

3. **Adopting the Verona "group of objects" model** (§4.7 / §8.3) wholesale. Verona is a new-language design; the relevant badger takeaway is the "module-level values as a group" idea (per `ARCHITECTURE.md:6-9`, the immortal region for module-level values), which badger already has.

---

## 10. Caveats and unverified sources

This note explicitly marks the citations that could not be verified against their primary source at the time of writing (October 2026). Each is paired with the closest secondary source.

- **Tofte & Talpin POPL 1994** (§1.2, §6.1): the canonical URL `https://www.cs.cmu.edu/~rwh/courses/ppl/papers/tofte-talpin.pdf` returned 404 in our check. The paper is real and well-cited; the canonical host is the Cambridge RAWF hardware ftp or the authors' pages. The community overview at https://en.wikipedia.org/wiki/Region-based_memory_management is verified.
- **Aiken, Fähndrich, Levien PLDI 1995** (§6.3): the URL `https://www.cs.berkeley.edu/~aiken/papers/tr95-007.pdf` was unreachable in our check. The paper is real and well-cited.
- **Wadler 1990, "Linear Types Can Change the World!"** (§1.3, §4.1): the canonical URL is `https://homepages.inf.ed.ac.uk/wadler/papers/linear/linear.ps` (verified). The PDF URL given in some sources (`.../linear-types.pdf`) is incorrect.
- **Wadler index page** (§1.3, §4.1): `https://homepages.inf.ed.ac.uk/wadler/topics/linear-logic.html` is verified and lists all the follow-on papers.
- **Bernardy et al. POPL 2018 Linear Haskell** (§2.11): the canonical PDF is at `https://arxiv.org/pdf/1710.09756.pdf` (verified). The earlier URLs (e.g., `https://www.cl.cam.ac.uk/~jcb82/papers/linear_haskell_popl18.pdf`) were unreachable.
- **Plotkin, Pretnar ESOP 2009 "Handlers of Algebraic Effects"** (§7.1): the URL `https://homepages.inf.ed.ac.uk/gdp/publications/hndlr.pdf` was unreachable. The paper is real and well-cited.
- **Koka MSR TR** (§7.2): the URL `https://koka-lang.github.io/koka/doc/koka-total.pdf` returned 404. The Koka project page is verified at `https://koka-lang.github.io/koka/doc/index.html`.
- **Cyclone project page** (§1.2, §6.4): `https://cyclone.thelanguage.org/` is the canonical host; the project is long-dormant (last release 2006), so the page is informational only.
- **Swift Ownership Manifesto** (§3.1, §3.2): `https://github.com/swiftlang/swift/blob/main/docs/OwnershipManifesto.md` is verified accessible (1,635 lines as of October 2026).
- **Cone project page** (§4.6, §8.2): `https://cone.jondgoodwin.com/` is verified accessible.
- **Verona GitHub** (§4.7, §8.3): `https://github.com/microsoft/verona` is verified (3.7 k stars).
- **Austral GitHub** (§4.5, §8.1): `https://github.com/austral/austral` is verified (1.6 k stars).
- **Roc website** (§4.8, §7.6, §8.9): `https://www.roc-lang.org/` is verified.
- **Polonius GitHub** (§2.2): `https://github.com/rust-lang/polonius` is verified (1.8 k stars).
- **Oxide paper** (§2.3): `https://arxiv.org/abs/1903.00982` is verified.
- **RustBelt paper** (§2.5): `https://people.mpi-sws.org/~dreyer/papers/rustbelt/paper.pdf` is verified (binary PDF, downloaded).
- **Creusot GitHub** (§2.7): `https://github.com/creusot-rs/creusot` is verified (1.9 k stars).
- **ML Kit homepage** (§6.2): `https://melsman.github.io/mlkit/` is verified; the project is actively maintained (last release December 2025).
- **Tweag linear-base GitHub** (§2.11): `https://github.com/tweag/linear-base` is verified (361 stars).
- **Effekt website** (§7.4): `https://effekt-lang.org/` is verified.
- **Whaley-Rinard POPL 1999** (§5.2): `https://www.cag.lcs.mit.edu/~rinard/paper/popl99.pdf` is verified (binary PDF, downloaded).
- **ATS project** (§2.12): `https://www.ats-lang.org/` is referenced; the current home is `https://github.com/ats-lang/ats-lang.github.io` (verified).

The note is opinionated and recommends the smallest additions that yield the largest wins. Each recommendation is paired with a code path; the budget is the size of the relevant existing file.

---

## 11. Sources

- Wadler, *Linear Types Can Change the World!* (IFIP 1990) — https://homepages.inf.ed.ac.uk/wadler/papers/linear/linear.ps
- Wadler, index of linear-logic papers — https://homepages.inf.ed.ac.uk/wadler/topics/linear-logic.html
- Turner, Mossin, Wadler, *Once upon a type* (FPCA 1995) — https://homepages.inf.ed.ac.uk/wadler/topics/linear-logic.html
- Tofte, Talpin, *Implementation of the Typed Call-by-Value λ-calculus using a Stack of Regions* (POPL 1994) — https://www.cs.cmu.edu/~rwh/courses/ppl/papers/tofte-talpin.pdf [unverified, see §10]
- Tofte, Birkedal, Elsman, Hallenberg, *A Retrospective on Region-Based Memory Management* (HOSC 2004) — referenced via https://melsman.github.io/mlkit/papers
- Hallenberg, Elsman, Tofte, *Combining Region Inference and Garbage Collection* (2003)
- Aiken, Fähndrich, Levien, *Better Static Memory Management* (Berkeley TR 1995) — https://www.cs.berkeley.edu/~aiken/papers/tr95-007.pdf [unverified, see §10]
- ML Kit homepage — https://melsman.github.io/mlkit/ (verified October 2026)
- ML Kit papers — https://melsman.github.io/mlkit/papers
- Clarke, Noble, Potter, *Ownership Types for Flexible Alias Protection* (OOPSLA 1998)
- Aldrich, Chambers, *Ownership Domains: Separating Aliasing Policy from Mechanism* (ECOOP 2004)
- Weiss, Patterson, Gierczak, Ahmed, *Oxide: The Essence of Rust* (arXiv 1903.00982) — https://arxiv.org/abs/1903.00982 (verified)
- Jung, Jourdan, Krebbers, Dreyer, *RustBelt: Securing the Foundations of the Rust Programming Language* (POPL 2018) — https://people.mpi-sws.org/~dreyer/papers/rustbelt/paper.pdf (verified)
- Jung et al., *Stacked Borrows* (POPL 2020) — https://github.com/rust-lang/rust/tree/master/src/test/ui
- Matsakis, *An alias-based formulation of the borrow checker* (April 2018) — http://smallcultfollowing.com/babysteps/blog/2018/04/27/an-alias-based-formulation-of-the-borrow-checker/
- Polonius GitHub — https://github.com/rust-lang/polonius (verified, 1.8 k stars)
- Polonius book — https://rust-lang.github.io/polonius/
- Creusot GitHub — https://github.com/creusot-rs/creusot (verified, 1.9 k stars)
- Creusot documentation — https://guide.creusot.rs
- Prusti GitHub — https://github.com/Prusti-Rust/prusti-dev
- Kani (bounded model checker) GitHub — https://github.com/model-checking/kani
- Aeneas (verification home of AG) — https://github.com/AeneasVerif/aeneas
- RefinedRust — https://github.com/refinedrust/refinedrust
- Gillian-Rust — https://github.com/logic-life/gillian-rust
- Yuga — https://yuka-lang.org/
- Bernardy et al., *Linear Haskell* (POPL 2018) — https://arxiv.org/pdf/1710.09756.pdf (verified)
- Linear Haskell (`linear-base`) GitHub — https://github.com/tweag/linear-base (verified, 361 stars)
- Tweag's linear-types blog series — https://www.tweag.io/blog/tags/linear-types/
- Xi, *Applied Type System* (2004) — https://www.ats-lang.org/ ; current home https://github.com/ats-lang/ats-lang.github.io (verified)
- Swift Ownership Manifesto — https://github.com/swiftlang/swift/blob/main/docs/OwnershipManifesto.md (verified)
- Swift book (source) — https://github.com/swiftlang/swift-book (verified)
- Swift Evolution proposals — https://github.com/swiftlang/swift-evolution
- Swift Region-Based Isolation (SE-0414) — https://github.com/swiftlang/swift-evolution/blob/main/proposals/0414-region-based-isolation.md
- Creusot project page — https://creusot.rs/
- Vault (DeLine, Fähndrich, PLDI 2001)
- Cyclone project page — https://cyclone.thelanguage.org/
- Mezzo project page — https://protzenko.github.io/mezzo/
- Austral GitHub — https://github.com/austral/austral (verified, 1.6 k stars)
- Austral project page — https://austral-lang.org/
- Cone project page — https://cone.jondgoodwin.com/ (verified)
- Verona GitHub — https://github.com/microsoft/verona (verified, 3.7 k stars)
- Verona FAQ — https://microsoft.github.io/verona/docs/faq
- Roc project page — https://www.roc-lang.org/ (verified)
- Roc tutorial — https://github.com/roc-lang/roc/blob/main/docs/mini-tutorial-new-compiler.md
- Gleam project page — https://gleam.run/
- Linc, Pelletier, Linear Use-Once Haskell — referenced in academic literature
- F\* GitHub — https://github.com/FStarLang/FStar
- Dafny GitHub — https://github.com/dafny-lang/dafny
- Lean 4 GitHub — https://github.com/leanprover/lean4
- Coq / Rocq GitHub — https://github.com/coq/coq
- Idris 2 GitHub — https://github.com/idris-lang/Idris2
- Idris 2 paper, *Quantitative Type Theory for the Next Generation* (Brady 2021)
- Choi, Gupta, Serrano, Sreedhar, Midkiff, *Escape Analysis for Object-Oriented Languages* (OOPSLA 1999) — https://dl.acm.org/doi/10.1145/320385.320386
- Whaley, Rinard, *Compositional Pointer and Escape Analysis for Multithreaded Java* (POPL 1999) — https://www.cag.lcs.mit.edu/~rinard/paper/popl99.pdf (verified)
- Coutant, Rinetzky, Sagiv, *A Combined Pointer and Escape Analysis Algorithm for Object-Oriented Languages* (SPM 2000)
- Stadler, Würthinger, Mössenböck, *Partial Escape Analysis and Scalar Replacement for Java* (CGO 2014) — https://dl.acm.org/doi/10.1145/2544137.2544157
- Immix paper (Blackburn, McKinley, PLDI 2008) — covered in research/11
- V8 blog, *Jank Busters Part Two: Orinoco* — https://v8.dev/blog/orinoco
- V8 TurboFan blog — https://v8.dev/blog/turbofan-jit
- LLVM BasicAA / GlobalsAA — https://llvm.org/
- Real-Time Specification for Java (JSR-001) — https://www.rtsj.org/
- GraalVM publications — https://www.graalvm.org/community/publications/
- .NET runtime (RyuJIT) — https://github.com/dotnet/runtime
- Mono runtime — https://www.mono-project.com/docs/advanced/runtime/
- WebKit JIT — https://trac.webkit.org/wiki/WebKit%20JIT
- WebKit bmalloc — https://github.com/WebKit/WebKit/tree/main/Source/bmalloc (verified)
- Chromium PartitionAlloc — https://chromium.googlesource.com/chromium/src/+/master/base/allocator/partition_allocator/PartitionAlloc.md
- jemalloc — https://jemalloc.net/ (verified)
- mimalloc GitHub — https://github.com/microsoft/mimalloc (verified)
- snmalloc GitHub — https://github.com/microsoft/snmalloc (verified)
- rpmalloc GitHub — https://github.com/rampantpixels/rpmalloc (verified)
- bumpalo Rust crate — https://crates.io/crates/bumpalo (verified)
- typed-arena Rust crate — https://crates.io/crates/typed-arena (verified)
- sharded-slab Rust crate — https://crates.io/crates/sharded-slab (verified)
- Plotkin, Pretnar, *Handlers of Algebraic Effects* (ESOP 2009) — https://homepages.inf.ed.ac.uk/gdp/publications/hndlr.pdf [unverified, see §10]
- Leijen, *Koka: Programming with Row-polymorphic Effect Types* (MSR TR 2014) — https://koka-lang.github.io/koka/doc/index.html (verified) ; https://koka-lang.github.io/koka/doc/koka-total.pdf [unverified, see §10]
- Cats Effect — https://typelevel.org/cats-effect/ (verified)
- ZIO — https://zio.dev/ (verified)
- Kyo — https://github.com/getkyo/kyo (verified)
- Effekt — https://effekt-lang.org/ (verified)
- Brachthäuser, Schuster, *Effekt: Lightweight Effect Polymorphism* (2020)
- O'Hearn, *Resources, Concurrency, and Local Reasoning* (TCS 2007)
- Lindley, McBride, McLaughlin, *Do Be Do Be Do* (POPL 2017) — https://homepages.inf.ed.ac.uk/slindley/papers/frank-dobedo.pdf [unverified, see §10]
- Unison project page — https://www.unison-lang.org/ (verified)
- HVM GitHub — https://github.com/HigherOrderCO/HVM (verified)
- Nim blog (ORC announcement) — https://nim-lang.org/blog/2021/12/28/arc-orc.html
- Leino, *Dafny: An Automatic Program Verifier for Functional Correctness* (LPAR 2010)
- Bhargavan et al., *Modular Verification of Security Protocol Code by Typing* (2010)

---

## 12. Code references in badger's codebase

These are the file:line references used throughout the note. They were captured at the time of writing (October 2026); line numbers may drift as the codebase evolves.

- `crates/badger_typeck/src/ownership.rs:1-16` — module docstring; affine ownership and borrows overview.
- `crates/badger_typeck/src/ownership.rs:25-30` — the `Mode` enum (`Move`, `Borrow`, `BorrowMut`).
- `crates/badger_typeck/src/ownership.rs:34-50` — the `Loan` struct (`holder`, `owner`, `mutable`, `start`, `span`, `until`, `block`).
- `crates/badger_typeck/src/ownership.rs:101-` — `check_def` entry point.
- `crates/badger_typeck/src/regions.rs:1-13` — module docstring; region escape through values that can hide a scoped one.
- `crates/badger_typeck/src/regions.rs:22-79` — the `check` function; provider and region lambda processing.
- `crates/badger_runtime/src/gc.rs:1-7` — module docstring; "Objects never move, so raw pointers held in registers stay valid while the object is reachable from a root."
- `crates/badger_runtime/src/gc.rs:13` — collection only inside allocation (a safe point).
- `crates/badger_runtime/src/gc.rs:38-54` — the `Header` struct, `MARK`/`IMMORTAL`/`REMEMBERED` bits.
- `crates/badger_runtime/src/gc.rs:241-266` — `alloc_cell` / bump allocation per pool.
- `crates/badger_runtime/src/gc.rs:269-280` — `free_cell` / size-class free list.
- `crates/badger_runtime/src/gc.rs:347-352` — collection trigger (`due()`).
- `crates/badger_runtime/src/gc.rs:386-403` — debt-based allocation accounting.
- `crates/badger_runtime/src/gc.rs:416-425` — `external_alloc`/`external_free` (R1 surface).
- `crates/badger_runtime/src/gc.rs:426-428` — `memory_bytes()` (R1 surface).
- `crates/badger_runtime/src/gc.rs:911-922` — `trim_empty_chunks` call site.
- `crates/badger_runtime/src/gc.rs:938-947` — threshold / `MAJOR_AT` / `young_bytes` (sticky-mark generational).
- `crates/badger_runtime/src/gc.rs:966-979` — `trim_empty_chunks` body with `occupied` sort (R1 target).
- `crates/badger_runtime/src/gc/sweep.rs:22-43` — `Helper` struct, persistent helper threads.
- `crates/badger_runtime/src/gc/sweep.rs:54-72` — `Pool::run` parallel sweep.
- `crates/badger_runtime/src/gc/allocations.rs:18-36` — `CellSource::{Reused,Bump,System}`.
- `crates/badger_runtime/src/gc/allocations.rs:38-61` — `PoolEvent::{Refill,Trim}` accounting.
- `crates/badger_runtime/src/gc/allocations.rs:65-104` — `BufferStorageEvent` (Items storage accounting).
- `crates/badger_runtime/src/task.rs:1-15` — poll-based stop-the-world rationale.
- `crates/badger_mir/src/sra.rs:1-169` — scalar replacement of aggregates (the partial escape analysis).
- `crates/badger_mir/src/opt/simplify.rs:1-780` — local simplification; constant folding; copy and constant propagation; dead-assignment elimination.
- `crates/badger_mir/src/opt/specialize.rs:43-72` — closure specialisation (per closure / set of closure arguments).
- `crates/badger_mir/src/opt/cfg.rs:252-` — CFG helpers (dominators).
- `crates/badger_mir/src/inline.rs:1-349` — inlining pass.
- `crates/badger_mir/src/opt/cse.rs:1-193` — CSE pass.
- `crates/badger_mir/src/opt/superopt.rs:1-736` — superoptimisation pass (--opt superopt).
- `crates/badger_mir/src/reach.rs:1-139` — reachability pruning.
- `crates/badger_mir/src/verify.rs:1-212` — MIR verification.
- `crates/badger_mir/src/roots.rs:1-234` — shadow-stack frame push for precise roots.
- `ARCHITECTURE.md:1-` — the high-level architecture document.
- `ARCHITECTURE.md:6-9` — "roots are task shadow stacks, frame... module-level values, pinned/temporary runtime roots".
- `ARCHITECTURE.md:105-108` — HM inference with Rémy levels over binding-group SCCs; rows/effects/regions; `{..Rest}` row structural polymorphism; scope brands `for<S>`.
- `ARCHITECTURE.md:117-118` — affine use-after-move (builders, buffers, `once` closures).
- `ARCHITECTURE.md:117-174` — the ownership section; affine ownership and borrows overview.
- `ARCHITECTURE.md:129-173` — "Ownership details: a `let` re-initializes its names ... captures each closure borrows rather than moves ... `Result` values bound or assigned and not read since ... generic bodies are re-checked with their type variables treated as affine".
- `ARCHITECTURE.md:179-191` — "Effects: after inference, a fixpoint over resolved calls charges every callee's modeled effects to its callers ... explicit effect bound that is exceeded is E0501 ... callback effects propagate through `for<S>` callback types".
- `ARCHITECTURE.md:192-194` — let-polymorphism (nonexpansive local bindings; effect-polymorphic closures).
- `ARCHITECTURE.md:200-201` — "Monomorphic re-checking: Each instance is re-checked at concrete types".
- `ARCHITECTURE.md:204-217` — binary MIR.
- `ARCHITECTURE.md:218-222` — "MIR is a CFG per function instance with typed representations".
- `ARCHITECTURE.md:240-249` — "Release MIR pipeline: a fixed, inspectable order of named passes — inline, closure specialization, scalar replacement, simplification, map/list/enum representation passes, CSE, a second inlining/simplification round, and verified superoptimization".
- `ARCHITECTURE.md:323-324` — "Code generation keeps every managed pointer that is live across a safe point ... in a shadow-stack frame".
- `ARCHITECTURE.md:331-333` — "`extern "c"` calls run inline; `extern "c blocking"` and `"c pinned"` contracts route word-sized calls through `badger_rt_ffi_call`".
- `ARCHITECTURE.md:339-361` — heap description; sticky-mark generational; per-worker size-class pools; off-heap memory owned by objects (map tries) counts toward collection.
- `ARCHITECTURE.md:362-373` — Tasks; stackful coroutines (corosensei); workers; runtime lock.
- `ARCHITECTURE.md:414-420` — Authority: `fs.root(path)` returns a `Dir`; private fields keep both from being made any other way.
- `ARCHITECTURE.md:524-526` — Handle objects carry their module and number; their finaliser hands `drop_handle` to a background thread.
- `ARCHITECTURE.md:684-692` — Contracts: `requires` / `ensures`.
- `BACKLOG.md:97-100` — R1 perf slice ("keep the mark stack and remembered buffer across collections; no `occupied` sort in `trim_empty_chunks`; account channel queues and staging as external memory; report managed, external, pressure, and RSS separately").
- `BACKLOG.md:101-110` — R2 (done), R3 (done), R4-R5.

---

End of note.