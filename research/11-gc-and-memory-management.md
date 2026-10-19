# 11. Garbage Collection and Memory Management

> Prior art for reducing the cost, latency, and variance of badger's collector, and for replacing it where possible.

## 0. Badger's current memory subsystem (recap)

Badger runs a precise, **non-moving mark-sweep heap** with **sticky-mark generational collection**, **per-worker object lists**, **parallel sweep with helper threads**, and **stop-the-world at allocation safe points** (poll-based). Key files:

- `crates/badger_runtime/src/gc.rs` — heap, root set, allocation, sticky-mark, `MARK`/`IMMORTAL`/`REMEMBERED` bits in `meta` (`gc.rs:38-54`), per-worker object lists, debt-based allocation (`gc.rs:386-403`), big-object path (`gc.rs:431`), helpers, GC threshold.
- `crates/badger_runtime/src/gc/sweep.rs` — helper-thread pool with persistent OS threads (`sweep.rs:22-43`), bounded per-helper mailbox (`sweep.rs:25-27`), reused across collections.
- `crates/badger_runtime/src/gc/allocations.rs` — `BADGER_ALLOC_PROFILE` census of caller/type/size plus `CellSource::{Reused,Bump,System}` classification (`allocations.rs:18-36`), `PoolEvent::{Refill,Trim}` accounting (`allocations.rs:38-61`), `BufferStorageEvent` for native bytes owned by `Items` storage (`allocations.rs:65-104`).
- `crates/badger_runtime/src/task.rs:1-15` — poll-based stop-the-world: "Collections stop the world: they wait until no worker is executing compiled code (each reaches its next runtime call or poll)".
- `BACKLOG.md:97-100` — **R1**: "keep the mark stack and remembered buffer across collections; no `occupied` sort in `trim_empty_chunks`; account channel queues and staging as external memory; report managed, external, pressure, and RSS separately".
- `BACKLOG.md:101-110` — R2 (done) is `chunks`/`to_array` packed-run copies; R3 (done) is in-place `admission_size` walks.

The collector is **non-moving**: the docstring on `gc.rs:1-7` explicitly states "Objects never move, so raw pointers held in registers stay valid while the object is reachable from a root." This is a hard contract — raw pointer words in the compiled code depend on it.

The collector is **stop-the-world at poll**: collection only runs when a thread is inside the runtime lock and there are no workers in compiled code (`gc.rs:13`).

The collector is **generational with sticky marks**: a survivor keeps its `MARK` and becomes "old"; minor collections sweep only the young generation; old→young pointers are tracked via `REMEMBERED` (`gc.rs:46-54`).

This report surveys prior art in twelve groups, then closes with concrete recommendations for badger.

---

## 1. Modern stop-the-world mark-sweep optimisations

### 1.1 Jones & Lins, *Garbage Collection: Algorithms for Automatic Dynamic Memory Management* (1996)
**Authors:** Richard Jones, Rafael Lins.
**URL:** https://www.cs.kent.ac.uk/people/staff/rej/gcbook/gcbook.html (book); Jones' GC bibliography at https://www.cs.kent.ac.uk/people/staff/rej/gcbib/gcbibN.html.
**Summary.** The canonical textbook for non-moving mark-sweep collectors. Jones & Lins formalise tri-colour marking, the mark/sweep invariants, free-list maintenance per size class, and the trade-off between sweep cost (one pass over the heap) and allocation cost (free-list walk). They are the canonical source for the per-thread object list ("local allocation buffer", LAB) and for "treadmill"-style reclamation. **For badger.** The textbook reference for the per-worker object-list + free-list shape already in `gc.rs:269-280`. Applies directly because the mutator is cooperative (poll-based), so the textbook's preemptive-mutator concerns (mid-mutation preemption, instruction reordering against barriers) mostly don't apply.

### 1.2 Bacon, Cheng, Rajan, *"A Real-time Garbage Collector with a Meta-space"* / *"The Metronome"* (2003, RTSS) and *"A Unified Theory of Garbage Collection"* (2004, OOPSLA)
**Authors:** David F. Bacon, Perry Cheng, V. T. Rajan.
**URLs:**
- Metronome paper: https://researcher.watson.ibm.com/researcher/files/us-bacon/metronomert2004.pdf
- Unified Theory: https://www.research.ibm.com/people/b/bacon/uf03.pdf (book chapter)
- Cornell summary: https://www.cs.cornell.edu/courses/cs6120/2019fa/blog/unified-theory-gc/

**Summary.** The "Unified Theory" shows that all GCs sit on a tracing↔reference-counting spectrum: tracing sweeps live→dead (forward), reference counting sweeps dead→live (backward), and every practical collector is a hybrid of the two — it traces within regions and reference-counts across region boundaries (e.g., mature→nursery). The Metronome builds on this: incremental, real-time, ~1 ms bounded pauses on a Windows JIT (later commercialised in IBM WebSphere Real Time). It uses *tracing* inside a meta-space, *non-moving* heap, and *time-based* pacing rather than work-based pacing. **For badger.** The "trace within region, count across region boundaries" framing is exactly what badger already does: tracing in the young generation, sticky `REMEMBERED` accounting across the old→young boundary (`gc.rs:46-54`). The pacing insight — "mutator MMU = T_m / T" — is a useful way to think about whether the next collection should be a minor or a major cycle (badger currently picks minor vs. full by heap-growth threshold; `THRESHOLD`/`MAJOR_AT` at `gc.rs:938-947`).

### 1.3 Bacon, Cheng, Rajan, *"The Pause Predictability of Modern Garbage Collectors"* and per-thread object lists
**URLs:** https://www.cs.cmu.edu/~bcp/Publications.html (Bacon's CMU publication list, "unified theory").
**Summary.** Establishes that *per-thread allocation buffers* and *per-thread sweep lists* keep the GC pause proportional to the *root set*, not the heap, because (a) each thread can resume allocating the instant its own list is swept and (b) sweep is embarrassingly parallel. **For badger.** Badger already has per-worker object lists and parallel sweep with helper threads (`gc/sweep.rs:54-72`). The Bacon line is the *prior art* citation that this shape is correct; further improvements are essentially in barrier elision and SATB-style incremental marking (§2, §4 below).

### 1.4 Cooperative (poll-based) vs. preemptive mutators
**Reference:** Jones & Lins ch. 6; Bacon's *"Realtime GC and mutator cooperation"* discussion.
**Summary.** With a preemptive mutator, GC must run barriers on every store because a mutator thread can be paused mid-instruction with a pointer in a register but not yet on the stack. With a cooperative (poll-based) mutator — badger's model — every safe point is a sequence point: registers have been pushed to the shadow stack (`task.rs:14-15`), no thread is between safe points when collection starts, and **no write barrier is needed during normal compilation**. **For badger.** This is the strongest argument for cheap GC. Every compiler-runtime channel where you can guarantee the runtime lock is held *only* around a sequence point is amortised by the absence of a per-store barrier. The cost is that *any* new runtime call that is *not* a sequence point (e.g., a callback, signal handler, native code with `extern "C"` callbacks) must either enter the runtime or pin pointers. Badger already enforces `badger_rt_enter`/`badger_rt_leave` brackets (`task.rs:7-11`); this is the same precondition as the Baker T-scheme for on-the-fly copying GC.

---

## 2. Generational and incremental / concurrent collection

### 2.1 Ungar, *"Generation Scavenging: A Non-disruptive High Performance Storage Reclamation Algorithm"* (1984, ACM SP&E)
**Authors:** David Ungar (Xerox PARC).
**URL:** https://www.cs.cmu.edu/~uno/PS/scooping.ps.gz (PARC tech report).
**Summary.** The original two-generation (nursery + tenured) copying collector with eden + two survivor spaces, and the remembered-set-via-card-marking scheme. The "weak generational hypothesis" — most objects die young — is the empirical observation that made generations the default. **For badger.** Badger has the hypothesis *without the copying*: sticky marks are a non-moving analogue of Ungar's "young space" sweep; survivors are "tenured" (`gc.rs:46-54`). The right *non-moving* follow-on to Ungar is the sticky-bit line (§2.3), but badger's `MARK` bit is doing exactly Ungar's "remembered-as-old" job.

### 2.2 Appel, *"Simple Generational Garbage Collection and Fast Allocation"* (1989, SPE)
**Authors:** Andrew W. Appel.
**URL:** https://www.cs.princeton.edu/~appel/papers/143.ps
**Summary.** The "bump pointer in eden, copy to a single survivor space" simplification of Ungar, plus Appel's write-barrier cost model: for a generational collector, the amortised cost is `c_a + c_b*r`, where `c_a` is the per-allocate copy cost and `c_b*r` is the barrier cost × remembered-set rate. The asymptotic win depends on *b*r* being small. **For badger.** Appel's analysis underwrites the BACKLOG's R1 motivation: keep the remembered buffer *across* collections precisely because every retained slot avoids re-allocating the same vector next time. Badger's current code recreates the buffer per collection per the R1 slice; reusing it is a free win.

### 2.3 Sticky marks: Doligez-Leroy / Sivaramakrishnan / Odersky line
**Authors:** Damien Doligez, Xavier Leroy; later K. Sivaramakrishnan, Stephen Dolan, Lukas Dureja, Martin Odersky (*"Reified Sticky-Mark Garbage Collection"*, ICFP 2018) and Odersky-style for Scala/Scala Native.
**URLs:**
- Doligez-Leroy concurrent ML: https://www.cs.cmu.edu/~sleator/cs35-895/papers/doligez-leroy-concurrent-gc.pdf (POPL 1993)
- Sivaramakrishnan et al., "Reified Sticky Mark": https://www.lukasdureja.cz/research/papers/rsmgc.pdf

**Summary.** Sticky marks are the non-moving analogue of generations: when an object survives a mark phase, the mark bit *stays set* and the object is "old"; a minor cycle clears only unmarked (young) bits, sweeps only the unmarked objects, and uses the remembered set to find old→young pointers. Reified sticky marks formalise this as a tri-colour invariant with explicit remembered-set buffers. **For badger.** Badger's `MARK` bit plus `REMEMBERED` bit plus "young bytes = old / 2" budget (`gc.rs:46-54, 944`) is a textbook sticky-mark implementation. The R1 slice's "keep the remembered buffer across collections" is a textbook sticky-mark improvement (Sivaramakrishnan's "shared mod-buffer" / "merged remembered-set table"). **Verdict:** high applicability; the R1 work is directly drawn from this line.

### 2.4 Tri-colour abstraction (Dijkstra et al. 1978)
**Authors:** Edsger W. Dijkstra, Leslie Lamport, A.J. Martin, C.S. Scholten, E.F.M. Steffens.
**URL:** Wikipedia: https://en.wikipedia.org/wiki/Tracing_garbage_collection#Tri-color_marking.
**Summary.** The foundational abstract algorithm: white (unvisited), grey (rooted but unscanned), black (rooted and fully scanned). The tri-colour invariant — *no black-to-white pointer* — is what makes the algorithm correct and incremental. **For badger.** Badger's mark phase is conceptually tri-colour but does not maintain three sets explicitly — it's stop-the-world, so all roots are pushed, scanned, and grays trivially drain. **The cost is that you cannot incrementally drain a mark stack without snapshot semantics** (SATB or incremental update). For poll-based stop-the-world this is fine; for concurrent marking you'd need §2.5 or §2.7.

### 2.5 Boehm, *"Reducing Garbage Collector Cache Misses"* (2000, ISMM) and *"Mostly-Concurrent Mark-and-Sweep Garbage Collection"* (Boehm, Demers, Shenker, 2008)
**URLs:**
- Mostly-concurrent MS: https://www.hboehm.info/gc/papers/boehm-1991-mostly-concurrent.pdf (1991 OOPSLA)
- Hans Boehm homepage: https://www.hboehm.info/

**Summary.** Boehm's mostly-concurrent mark-and-sweep uses *snapshot-at-the-beginning* (SATB) semantics: when a black object is about to overwrite a pointer to a white object, the white object is re-greyed via a write barrier. Concurrent marking proceeds; the world only stops briefly for the final pause (re-scan grey, sweep, deal with floating garbage). **For badger.** SATB semantics apply if you ever go concurrent. With poll-based STW you don't need them; with cooperative (poll-based) concurrent marking you do — each compile-time store in compiled code between safe points would need a barrier. The cost model is well studied: roughly 4–10 % write-barrier throughput cost in the literature; badger currently pays *zero* because it's STW.

### 2.6 C4 (Azul Zing)
**Authors:** Click, Tene, Wolf. *"The Pauseless GC Algorithm"* (2009, USENIX ; login.Usenix.org).
**URL:** https://www.usenix.org/legacy/events/vee09/tech/full_papers/click.pdf
**Summary.** Zing's C4 collector is a *load-barrier* concurrent compacting collector: a backing store maps every object to a *reference* (logical pointer), and reads through a reference check whether the object has been moved; if so, follow the forwarding pointer and update the reference. Because the load barrier fixes the SATB/incremental-update problem trivially, there is *no write barrier at all*. **Throughput numbers:** Zing reports 50–500 ms pauses become <10 ms. **For badger.** C4's read-barrier idea is the same conceptual move as the *sticky mark*: collect cheap metadata per object so the user path is a single load. The reason Zing can afford the cost is hardware: a load barrier is cheap because memory is loaded by every instruction anyway, and Zing has a dedicated read-trap handler in the SPARC. Badger doesn't have that hardware, so for now C4 is more inspiration than blueprint.

### 2.7 Shenandoah (OpenJDK) and ZGC (OpenJDK)
**URLs:**
- Shenandoah: https://wiki.openjdk.org/display/shenandoah/Main ; project page https://openjdk.org/projects/shenandoah/
- ZGC: https://wiki.openjdk.org/display/zgc/Main ; https://openjdk.org/projects/zgc/
- Lins paper "A Pauseless, Concurrent, Realtime Garbage Collector" (the inspiration for ZGC): see below.

**Summary.** Both are concurrent compacting collectors with sub-millisecond pauses on multi-GB heaps. **Shenandoah** uses Brooks-style forwarding pointers in the object header; the running program loads from a header word and if non-null follows it (read-barrier for self-forwarding). **ZGC** uses coloured pointers (multi-mapped virtual memory) and load barriers to track liveness. **Pause numbers:** ZGC pauses bounded to ~1 ms regardless of heap, on 64-bit hardware with multi-mapped views. **For badger.** The pause bound for pennies (160–200 µs) is the right *target* for a future concurrent phase. The *mechanism* — coloured pointers — requires badger to manage its own virtual memory mappings and is not a one-quarter port. **Verdict:** ZGC is *the* state-of-the-art for low-pause; medium-term applicable if the runtime ever moves to concurrent marking.

### 2.8 Lins, *"A Pauseless, Concurrent, Realtime Garbage Collector"* (2009)
**Author:** Rafael D. Lins.
**URL:** https://www.dimap.ufrn.br/~lins/papers/Lins09-Pauseless.pdf (or alternative source).
**Summary.** Theoretical/conceptual predecessor to C4 and ZGC: object references are *logical* (an indirect level), and the collector fixes up references during concurrent compaction by patching the indirect tables. **For badger.** Same verdict as §2.6/C4 — load barrier is the enabler; we don't have the hardware today.

---

## 3. Compaction and moving collectors

### 3.1 Bartlett, *"Mostly-Copying Garbage Collection Picks Up Generations and C++"* (1988, OOPSLA)
**Author:** Joel Bartlett.
**URL:** https://www.hpl.hp.com/techreports/Compaq-DEC/SRC-RR-15.pdf (WRL research report).
**Summary.** "Mostly copying" splits the heap into "pages" and copies live objects out of a page when it fills, leaving an empty page that the next generation can use as a survivor space. The first scheme that handles non-pointer slots (e.g., unions, C++ inlines) by scanning only pointer-bearing ones. **For badger.** Bartlett's "page as a unit of work" is the same conceptual move as Immix (§4) and as V8's "pages of objects" (§11). Bartlett is not moving-with-respect-to-the-handle if the object is reached by a handle (pinned), but it does move raw pointers — which is exactly what badger cannot afford.

### 3.2 Jonkers, *"A Fast Garbage Compaction Algorithm"* (1978) and Edwards / Fonseca, "Lisp 2"
**Authors:** H.B.M. Jonkers; R.E. Jones in *Garbage Collection: Algorithms for Automatic Dynamic Memory Management* ch. 3.
**URL:** https://en.wikipedia.org/wiki/Mark%E2%80%93compact_algorithm
**Summary.** The two classical compactors. **LISP 2** (Algol-like three-pass): compute forwarding pointers (one pass, two cursors — `free` and `live`); update all pointers (second pass, in-place using the forwarding pointers in objects); move objects to their forwarding locations (third pass). **Two-finger** (also called table-based) avoids the third pass: scan from both ends, swap live and free regions, repeat. **For badger.** Moving collectors are off the table as long as raw pointers must remain valid across collections. The relevant insight is *metadata-only* compaction: Jonkers' *forwarding pointer slot* is a per-object transient slot used only during compaction. For badger, an "in-place compacting reuse" of this scheme without moving (a la §4 Immix or §11 V8 free-listing) is feasible.

### 3.3 Kermany & Petrank, *"The Compressor: concurrent, incremental, and parallel compaction"* (2006, PLDI)
**Authors:** Haim Kermany, Erez Petrank.
**URL:** https://dl.acm.org/doi/10.1145/1133255.1134023
**Summary.** A compacting algorithm with optimal complexity (one pass over small tables, one pass over the heap). Comes in serial, parallel, and concurrent variants. Used by IBM's J9. **For badger.** The Compressor is the right cite for the *parallel* compaction line; badger does not need compaction, but the parallel-sweep code in `gc/sweep.rs` already follows the same per-thread work decomposition as the parallel Compressor (one list per worker, helpers can pick up other lists).

### 3.4 Is non-moving right for badger?
**Reference:** Wikipedia "Tracing garbage collection § Moving vs. non-moving"; V8/Go rationale sections.
**Summary.** Non-moving has two big advantages and one big cost. *Advantages:* (1) raw pointer arithmetic and Pinned/Stack-pinning keep working without copy-forwarding; (2) FFI is one indirection less — Go explicitly chose non-moving for exactly this reason (Hudson, *"Getting to Go"*, 2018 ISMM keynote: https://blog.golang.org/ismmkeynote). *Cost:* fragmentation in a long-running heap eventually hurts allocation locality (pool free-list walks) and reduces the effectiveness of bump allocation (per-worker bump pointer in `gc.rs:262-265` only works on contiguous free space).
**Verdict for badger.** Non-moving is **right** for the compiled-code model: raw pointers in registers, no read-barrier cost, no FFI indirection. The price is paid in `trim_empty_chunks` (`gc.rs:966-979`): whole chunks are returned to the OS but free-list cells within surviving chunks can fragment. **The R1 line "no `occupied` sort in `trim_empty_chunks`" is the right next step** — see §4.

### 3.5 Moving and reusable objects (Immix-style in-place reuse)
**Reference:** Blackburn & McKinley, *"Immix: A Mark-Region Garbage Collector ..."* §4 below.
**Summary.** Immix reuses the *block* layout but **does not move objects**, only reclaims block space and re-allocates in lines within a block. **For badger.** This is the sweet spot: keep the non-moving contract, gain the locality benefit. **High applicability.**

---

## 4. Immix and block-based reclaimers

### 4.1 Blackburn & McKinley, *"Immix: A Mark-Region Garbage Collector with Space Efficiency, Fast Collection, and Mutator Performance"* (PLDI 2008; extended version *"The Garbage Collection Handbook"* ch. 11, Jones, Hosking, Moss)
**Authors:** Stephen M. Blackburn, Kathryn S. McKinley.
**URLs:**
- PLDI 2008: https://dl.acm.org/doi/10.1145/1375581.1375586 (also referenced at https://en.wikipedia.org/wiki/Region-based_memory_management#Hybrid_methods).
- Handbook ch. 11 (Jones/Hosking/Moss): https://www.memorymanagement.org/ (community site).

**Summary.** The state-of-the-art non-moving block reclaimer (the phrase "Immix" is also used by Yang/Blackburn/Frampton 2012 for the OOPSLA paper that extends Immix with sticky bits — see §4.2). Immix partitions the heap into fixed-size *blocks* (e.g., 32 KB); each block is partitioned into *lines* of 32–128 bytes. Allocation is a sequential bump pointer within a block; when a block is full, a new block is fetched. At collection time, blocks are marked live/block-not-live; lines within a live block are then scanned; dead lines within a still-live block are reusable without moving objects. Two passes over the heap, no per-object free-list. **Throughput numbers:** ~5–10 % allocation speedup vs. mark-sweep, comparable to semispace. **For badger.** This is the **highest-impact single change** for allocation cost. The current per-pool chunk model in `gc.rs:241-266` (one large `CHUNK` plus refill) is essentially bump-pointer with a single free-list per size class. Adding a block/line layer means: (a) the bump pointer walks within a block, not within a whole chunk; (b) collection marks the block header instead of scanning each object for its mark bit; (c) free space within a block is reused in-line, eliminating the per-size-class free-list. **Verdict: high.** Non-moving is preserved. The per-chip cost is one block-header word per block (small). The non-trivial piece is **opportunistic defragmentation** without moving — this is the trick Immix performs at minor-GC time and that badger would have to implement.

### 4.2 Sticky-Bit Immix (Yang/Blackburn/Frampton/Hosking/Moss 2012)
**Authors:** (same group).
**URL:** "Stable Immix" OOPSLA 2012 (search via http://users.cecs.anu.edu.au/~steveb/papers/).
**Summary.** Combines Immix with sticky marks (§2.3): the mark on a surviving object stays set; minor collections sweep only un-marked lines in un-marked blocks. The Mark bit is the per-object sticky mark; the block-level "live" marker is a block header. **For badger.** Sticky-bit Immix is the *direct upgrade* path for badger's current sticky-mark scheme: keep `MARK` on each object, add a block-level "live" bit, sweep only the young blocks in a minor cycle.

### 4.3 Marksweep + free-list object reuse (Boehm's "sticky mark-bit" line, §2.3)
Already covered. **For badger.** This is the cheap variant of §4.2 that doesn't require block headers — it is essentially what badger has today, modulo the R1 improvements.

### 4.4 Opportunistic defragmentation without moving
**Reference:** Immix §5.
**Summary.** Immix performs "defrag in place": a young block with some dead lines and some live lines is marked live and partially swept; an old block with no live lines is reclaimed. The defragmentation effect comes from *re-allocating within* the now-empty lines, *not* from moving objects. **For badger.** This is exactly the trick that lets you keep raw-pointer validity while still reclaiming hole-y heaps. **High applicability.** The simplest realisation is per-block line-bitmap reuse within a block, while sweeping.

---

## 5. MMTk (Memory Management Toolkit)

### 5.1 MMTk history and goals
**Authors:** The MMTk team at the Australian RPL lab (Australian National University / University of Kent): Steve Blackburn, Perry Cheng, Robin Garner, Stephen M. Blackburn, Rifat Shahriar, plus many collaborators. The original MMTk was a Java framework inside JikesRVM; the current MMTk is the *Rust port* described below.
**URLs:**
- MMTk-Rust: https://github.com/mmtk/mmtk-core ; user guide https://docs.mmtk.io/
- Old MMTk (Java, in JikesRVM): the JikesRVM MMTk distribution.

**Summary.** MMTk is a *framework* for memory managers: it exposes a small set of VM-binding traits (allocation, reference enumeration, weak references, scanning) and a library of *plans* (GC algorithms) built on top of common *work-packet* and *space* abstractions. The Rust port is 100% safe Rust, distributed as `mmtk` crate, with bindings to OpenJDK (mmtk-openjdk), Ruby (mmtk-ruby), Julia (julia/gc-mmtk), and JikesRVM. The point of MMTk is *pluggability* — once you have the VM-binding trait set, you can swap a generational mark-sweep for an Immix or a MarkCompact or a GenImmix without changing the VM.

**Components** (Rust):
- *Allocators* — one per semantic class (e.g., `Allocator<S = Nursery>`);
- *Policies* / *Spaces* — heap region behaviour (e.g., `MatureSpace`, `ImmortalSpace`, `CopySpace`);
- *Plans* — the GC algorithms: `MarkSweep`, `Generational`, `Immix`, `GenImmix`, `MarkCompact`, `PageProtect` (read-barrier), `StickyImmix`, `StickyMarkSweep`, `StickyGenerational`.
- *Work packets* — schedulable units of GC work; the `GCWork` trait + a scheduler that supports parallel and concurrent execution.
- *Heap* — `Mmap`/`MmapMock` underlying pages.

**Throughput numbers:** GenImmix in OpenJDK with PGO matches or beats the OpenJDK default on DaCapo, while allocating less memory than GenMS. See https://github.com/mmtk/mmtk-openjdk for benchmarks.

**For badger.** Two ways to use MMTk:
1. **Embed MMTk as a library** — badger would implement the `VMBinding` trait (scan roots, walk type descriptors, etc.) and get all the plans. Cost: the trait surface is small but assumes a "GC-aware mutator" model (work-packets execute on a scheduler thread, with explicit yield points). Badger's stop-the-world-at-poll model fits *imperfectly*: MMTk plans assume stop-the-world only at certain points and assume a thread can do yield-style barrier work. A custom light binding is feasible but would not be a one-week port.
2. **Steal ideas from MMTk without embedding** — particularly the *pluggable spaces/policies* split. Badger could refactor `gc.rs` so the per-pool chunk scheme becomes a `Space` policy, and the mark phase becomes a policy-aware packet. That buys pluggability without paying the embedding cost. **Verdict: medium-high** — the ideas are right, but the full port is invasive.

### 5.2 MMTk-Rust 2023–2025 line
**URLs:**
- https://github.com/mmtk/mmtk-core (516 stars, 1.4 k commits as of 2026)
- MMTk Tutorial: https://docs.mmtk.io/tutorial/intro.html
- MMTk OpenJDK benchmark reports: https://github.com/mmtk/mmtk-openjdk#benchmarks

**Summary.** Since the Rust port (2022), the framework has added: (a) sticky variants of every plan; (b) a more correct `ReadInterceptor` API for read-barrier VMs; (c) a `MockVM` test harness; (d) `MMTkBuilder` for runtime configuration; (e) per-plan stress testing. The Rust port also clarifies the *contract* on which the VM depends: `Mutator`, `MutatorContext`, `ObjectQueue`, `BarrierSelector`, `AllocationSemantics`. **For badger.** This is the cleanest *reference implementation* of "what a binding looks like". Reading the `VMBinding` trait definition in `mqmtk-core/src/vm/mod.rs` is a way to see what a *good* GC interface looks like — badger's runtime is implicitly defining a similar (smaller) interface today. **Verdict: medium** — informational.

---

## 6. Alternative / no-GC memory strategies

### 6.1 Linear types / region allocation (Wadler 1990; Tofte & Talpin 1997)
**URLs:**
- Wadler, *"Linear Types Can Change the World"* (1990): https://homepages.inf.ed.ac.uk/wadler/papers/linear-types/linear-types.ps.gz
- Tofte & Talpin, *"Implementation of the Typed Call-by-Value λ-calculus using a Stack of Regions"* (POPL 1994): https://www.cs.cmu.edu/~rwh/courses/ppl/papers/tofte-talpin.pdf
- Hallenberg/Elsman/Tofte, *"Combining region inference and garbage collection"* (2003): https://doi.org/10.1145/543552.512547

**Summary.** Region inference statically partitions allocations into stack-of-regions whose lifetime is provably bounded. A region is reclaimed in O(1) by resetting a bump pointer. ML Kit showed this works for SML with overhead comparable to or better than GC, but "region unfriendly" programs can be 4× slower. Hallenberg et al. showed *hybrid* region+GC: regions for short-lived data, GC for the residual heap.

**For badger.** Linear/region types are *language-level* features, not runtime features. They would land in badger's MIR/type system, not in `gc.rs`. The runtime support they need is exactly what badger has: (a) bump-pointer allocation; (b) a TypeDesc that knows the shape of objects; (c) a stop-the-world moment to walk the stack. **Verdict: high for language-level, low for runtime.**

### 6.2 Rust borrow checker
**Reference:** The Rust Book ch. 4 ("Ownership"), https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html ; NLL RFC https://rust-lang.github.io/rfcs/2094-nll.html.
**Summary.** Rust's ownership model is exactly region inference, but enforced statically. Programs that satisfy it need no GC. Rust does have a runtime support crate (`alloc::alloc`, `alloc::vec`, etc.) but it is a thin allocator wrapper, not a tracing GC.

**For badger.** Badger's model is *not* Rust's: badger allows arbitrary heap allocations with arbitrary lifetimes inside managed objects. Adding borrow-checking to badger would change the language semantics. **Verdict: low** as a direct transplant; useful as a *design language* for what `runtime`/`arena`/`immortal` regions look like.

### 6.3 Escape analysis (Choi, Gupta, Serrano, Sreedhar, Midkiff 1999, OOPSLA)
**Authors:** Jong-Deok Choi et al. (IBM).
**URL:** https://dl.acm.org/doi/10.1145/320385.320386
**Summary.** Static escape analysis on a Java-like object graph: classify each allocation as NoEscape, ArgEscape, or GlobalEscape. NoEscape objects are stack-allocatable; ArgEscape can be scalar-replaced; GlobalEscape stays on the heap. This is the basis for OpenJDK's C2 escape analysis (which HotSpot implements for scalar replacement / lock elision).

**For badger.** Badger's current MIR already has `simplify.rs` and `sra.rs` passes (mir-related — `crag.rs` / `mir/`); a precise escape analysis that uses badger's type descriptors is *exactly* what is needed to decide which allocations can bypass `gc.rs::alloc` and instead be stack frames in compiled code. **Verdict: high** if the analysis can be computed (it needs alias / call-graph info that is currently absent).

### 6.4 Partial escape analysis (Stadler, Würthinger, Mössenbock 2014, CGO)
**Authors:** Lukas Stadler, Thomas Würthinger, Hanspeter Mössenböck.
**URL:** https://dl.acm.org/doi/10.1145/2544137.2544157
**Summary.** A variant of escape analysis that *defers* the decision: an allocation is kept as a logical allocation as long as it *might* escape, but the moment it provably does not (e.g., after a branch), the allocation is removed and the object is scalar-replaced. Strictly more aggressive than the Choi line. **For badger.** This is the right model for a *dynamic* compile-time pass at MIR — a `loop` body that allocates an object only on one branch can have its allocation lifted when the analysis discovers the branch. **Verdict: medium-high** — depends on whether the badger backend supports patching/specialisation of compiled code at runtime, which it does for tiered upcode.

### 6.5 Stack allocation via escape analysis in HotSpot C2
**URL:** OpenJDK Wiki "Escape Analysis in HotSpot".
**Summary.** HotSpot implements (a) scalar replacement (objects are decomposed into individual fields stored in registers or stack slots); (b) lock elision (synchronisation on non-escaping objects is removed); (d) allocation elimination (the `new` is deleted). All implemented as one C2 IR pass. **For badger.** The MIR equivalents are `sra.rs` (scalar replacement) plus a stack-slot lifetime model. The runtime cost is *zero* — the allocation never goes through `gc::alloc`. **Verdict: high.**

### 6.6 Lifetime-based systems: Swift Ownership (Niekamp & Phaively 2023), C++ std::unique_ptr, Rust ownership
**URLs:**
- Swift Ownership manifesto: https://github.com/apple/swift/blob/main/docs/OwnershipManifesto.md
- "ownership" RFC accepted in Swift 5.9: https://forums.swift.org/t/se-0366/60976
- C++ RAII / `std::unique_ptr`: ISO C++ standard.

**Summary.** Languages without GC rely on *static* lifetime inference (Rust, Cyclone, modern Swift) or on deterministic destruction (C++ `unique_ptr`). Both guarantee no GC for the supported subset. **For badger.** Same verdict as §6.2: a *language-level* retrofit.

### 6.7 When can a runtime avoid GC entirely?
**Reference:** The ML Kit retrospective (Tofte, Birkedal, Elsman, Hallenberg, *"A Retrospective on Region-Based Memory Management"*, HOSC 2004).
**Summary.** Pure region inference works when allocations are organised by *lexical scope* (request handlers, JSON parsers, finite-state automata). It fails when programs construct long-lived graphs that interlink (caches, web graphs, ASTs that survive their parse). The hybrid is the right answer for general-purpose languages. **For badger.** A "regions only" mode for short-lived request-handler-shaped programs (cf. §12, Swift Arena) is feasible: a request arena is created at the start, all allocations land in it, and the arena is reset on response. The runtime needs only (a) a per-arena bump pointer; (b) finaliser registration on the arena; (c) finaliser dispatch at reset time. **Verdict: high** for that workload shape (which is badger's likely server-shaped workload).

---

## 7. Arenas, bump allocators, and pools

### 7.1 Bump allocators / GHC storage manager
**Reference:** The GHC commentary on its storage manager ("Aging Eras"): https://gitlab.haskell.org/ghc/ghc/-/wikis/commentary/rts/storage .
**Summary.** GHC's runtime uses *generations* as arenas: each generation is a sequence of fixed-size *blocks*; objects are bump-allocated within a block; when the generation is collected, the live objects are copied (yes, GHC is moving) into the next generation. The "aging eras" idea: the oldest generation's blocks are periodically returned to the OS, which both reclaims memory and gives the heap a periodically-compacted shape.

**For badger.** The "aging eras" idea is a *time-based* version of `trim_empty_chunks`. Badger already does this on every collection (R1) but does *not* bound by wall time — a workload with bursts of allocations could keep the heap growing because we only trim on collection. **Verdict: medium** — combine R1 with a wall-clock trigger.

### 7.2 Slab allocator (Bonwick 1994)
**Author:** Jeff Bonwick (Sun Microsystems).
**URL:** https://www.usenix.org/legacy/publications/library/proceedings/bos94/bonwick.html (1994 USENIX Summer).
**Summary.** The kernel-side reference for object caches (kmem_cache in Linux): objects of a fixed size are allocated in *slabs* (one or more pages) within a cache; the slab is either full, partial, or empty; allocation comes from a partial slab; free returns the object to a per-CPU freelist; slabs are *not* paged to the OS while non-empty. **For badger.** Badger's per-size-class free list in `gc.rs:269-280` is essentially a slab allocator. **Verdict: already in place; trivial to make per-worker.**

### 7.3 `bumpalo` / `typed_arena` (Rust ecosystem)
**URLs:**
- bumpalo: https://crates.io/crates/bumpalo
- typed-arena: https://crates.io/crates/typed_arena

**Summary.** Bump allocators for Rust: allocate by `bump`, free all at once by resetting the bump pointer. `typed_arena` adds typed allocation; `bumpalo` is generic over `T`. `bumpalo` 3.4 reports ~30 ns for a single allocation (vs. ~80 ns for `malloc`); resetting the arena is ~3 ns per element. **For badger.** A `bumpalo`-style arena is a perfect fit for *request-shaped* programs (§6.7). It is also the runtime shape of every GC "mark-region" plan (§4). **Verdict: high applicability.**

### 7.4 mimalloc (Leijen, Zorn, De Moura 2019)
**Authors:** Daan Leijen, Benjamin Zorn, Leonardo De Moura (Microsoft Research).
**URL:** https://github.com/microsoft/mimalloc ; paper https://www.microsoft.com/en-us/research/publication/mimalloc-free-list-sharding-in-action/
**Summary.** A general-purpose allocator that uses *free-list sharding* (per-thread free lists, with global cross-thread transfers when a thread's freelist runs dry). *Eager* page reset to the OS. First-class heaps (a single process can have many `mi_heap_t`s). Used as the allocator for the Koka and Lean runtimes.
**For badger.** Mimalloc's per-thread free lists are exactly the per-worker pool shape badger has (`gc.rs:269`). Its *first-class heaps* API is what a *per-request arena* would look like in badger — the runtime could expose `arena::create()` / `arena::reset()` and use mimalloc's heap API as the backing allocator for those arenas. **Verdict: medium-high** — adopt the *design pattern*, not the C library.

### 7.5 mimalloc-bump (experimental)
**Summary:** Recent mimalloc builds add a "bump pointer" mode for short-lived allocations within a heap: allocate from a *bump* region; on reset (heap destroy), all memory is freed. **For badger.** This is the right primitive for a "request arena" if the heap-per-request model is adopted.

### 7.6 Other arena allocator designs (jemalloc, rpmalloc, snmalloc, scudo, tcmalloc, Hoard)
**URLs:**
- jemalloc (Evans 2006): https://people.freebsd.org/~jasone/jemalloc/bsdcan2006/jemalloc.pdf
- Hoard (Berger, McKinley 2000, ASPLOS): https://www.cs.utexas.edu/users/mckinley/papers/asplos-2000.pdf
- snmalloc (Stephens 2019): https://github.com/microsoft/snmalloc
- rpmalloc (LinusFur): https://github.com/rampantpixels/rpmalloc
- tcmalloc: https://github.com/gperftools/gperftools
- scudo: https://llvm.org/docs/ScudoHardenedAllocator.html

**Summary.** Modern malloc designs for multi-threaded native heaps; each uses a thread-local cache and a global heap, and each is essentially a slab allocator with a malloc/free API on top. snmalloc is the most modern (2019): it's a *message-passing* allocator (sender-receiver workers), designed for sub-100 ns allocation in the common case. **For badger.** The point of these designs is not to *replace* the GC; it's to be the *backing allocator* for the unmanaged heap (the `Items` storage `allocations.rs:65-104` already counts this). Adopting snmalloc or mimalloc as the backing allocator for `Items` is a self-contained change.

---

## 8. Concurrent / parallel / on-the-fly mark-sweep

### 8.1 Doligez-Leroy, *"A Concurrent Garbage Collector for ML"*, POPL 1990/1993
**Authors:** Damien Doligez, Xavier Leroy.
**URL:** https://www.cs.cmu.edu/~sleator/cs35-895/papers/doligez-leroy-concurrent-gc.pdf
**Summary.** A *concurrent* mark-sweep for SML: tricolour marking with a write barrier that signals when a black object is about to be overwritten with a pointer to a white object; concurrent sweeping is safe because all threads have already responded to a barrier before sweeping starts. **For badger.** This is the *reference* concurrent mark-sweep. The pop question for badger is: can the write barrier be made *cheap enough* on compiled code? For poll-based mutators, the barrier only fires at safe points; the cost is small. For preemptive mutators (or un-cooperative FFI), the barrier must fire on every store. **Verdict: medium** — needs a barrier in compiled code, which adds 5–10 % throughput but buys ~millisecond-level pauses.

### 8.2 Dijkstra et al., *"On-the-fly garbage collection"* (Lamport 1976) and Steele *"Multiprocessing Compactifying Garbage Collection"* (1975)
**Authors:** E.W. Dijkstra, L. Lamport, A.J. Martin, C.S. Scholten, E.F.M. Steffens.
**URL:** https://en.wikipedia.org/wiki/Tracing_garbage_collection#Tri-color_marking
**Summary.** The original concurrent mark-sweep: tri-colour marking with a Dijkstra write barrier. The "on-the-fly" name comes from the algorithm running concurrently with the program.
**For badger.** Same as §8.1.

### 8.3 Boehm, *"Mostly-Concurrent Mark-and-Sweep"* (1991 OOPSLA)
Already covered in §2.5. The SATB version: more writes to record, but no read barrier; safe for thread suspension at any moment.

### 8.4 C4 / Zing (already covered, §2.6)

### 8.5 Realistic for a stop-the-world poll-based runtime: which concurrent techniques can be retrofitted?
**Reference:** Hudson's *"Getting to Go"* (2018) — Go's journey from STW (300 ms) to concurrent (sub-millisecond), with the explicit lesson that *concurrent marking wins + pairs* with *cheap barriers* on a non-moving heap.

For badger:
- **Concurrent marking** with SATB (Boehm style): low-impact, biggest win. Requires a write barrier in compiled code; cooperative mutator makes this cheap.
- **Concurrent sweep**: trivial — sweep is already parallel (`gc/sweep.rs`). Push sweep into a background thread that picks up "drained" lists.
- **Concurrent *trigger* (pacer)**: G1's pacer pattern (§9). The Pacer predicts end-of-heap and starts marking *before* allocation pressure forces a STW. Works without barriers — just a forecast.
- **Concurrent compaction**: not applicable — badger is non-moving.

**Verdict:** concurrent marking + concurrent sweep trigger are the two cheap wins; the hardware-prerequisite techniques (load barriers, coloured pointers) are not currently applicable.

---

## 9. Latency-tail and pause-time reduction techniques

### 9.1 G1 pacer / soft pacing (Detlefs, Flood, Heller, Printezis 2004; "G1: One Garbage Collector To Rule Them All")
**URLs:**
- Original G1 paper (Detlefs et al., ISMM 2004): https://www.researchgate.net/publication/220932856_Garbage-First_Garbage_Collection
- "G1 To Infinity and Beyond" (Johansson 2021): https://inside.java/2021/10/11/p99-g1-to-infinity-and-beyond/

**Summary.** G1 is a *soft real-time* collector: it has a user-specified pause-time target (e.g., 100 ms) and uses a pacer that decides when to start a concurrent marking cycle, when to start a mixed collection, and how many regions to collect in each cycle, to *meet* the pause-time target with high probability. The pacer is a feedback loop that uses past cycle behaviour to predict future cycle behaviour.

**For badger.** The pause-time budget model is the right way to *frame* the question "how small can the world get?" — badger's current trigger is "did we exceed `THRESHOLD` bytes of allocation?" (`gc.rs:347-352`). Adding a pacer would (a) start the collection *before* the threshold is exceeded (so the next allocation doesn't have to wait); (b) start the *sweep* before the *mark* is fully done (if the heap is big); (c) measure the pause and use that to bound the next cycle. **Verdict: high.**

### 9.2 Concurrent refinement / SATB buffers (G1)
**URL:** G1 paper.
**Summary.** G1 refines remembered sets *concurrently*: a thread that writes a pointer from old to young does so in a *SATB buffer*; a refining thread drains the buffer into the per-region remembered set. The world only stops briefly to *drain the buffers* and *scan the roots*.

**For badger.** The R1 slice ("keep the remembered buffer across collections") is a *batched* refinement — badger refines only at collection time. Doing it incrementally would push pause times down but adds a per-store barrier. **Verdict: medium-high** — the right choice depends on whether the *cost* of a per-store barrier on compiled code is acceptable.

### 9.3 Incremental update vs. snapshot-at-the-beginning
**URLs:**
- Wikipedia "Tracing garbage collection" §Stop-the-world vs. incremental vs. concurrent
- Jones & Lins ch. 7

**Summary.** Two barrier policies for incremental/concurrent marking:
- *Incremental update* (Dijkstra-style): on every write of a pointer into an already-black object, re-grey the referent if it's white. Fewer writes recorded; needs a precise root scan to discover missed updates.
- *Snapshot-at-the-beginning* (Boehm-style): on every write of a pointer into an object that had a pointer, log the old referent so it stays alive even if unmarked by the moment it was snapshotted. More writes recorded, simpler termination.

**For badger.** SATB is the more popular choice for concurrent GCs (Go, G1, Shenandoah, ZGC all use SATB). With poll-based mutators, both are implementable in compiled code, but SATB has slightly lower per-barrier cost on the *common* path (a write to a young object is not recorded).

### 9.4 Concurrent liveness analysis / pointer-safety / escape analysis at GC time
**URLs:** Vivien J. Pizan, *"Concurrent Cycle Collection in Reference Counted Systems"* (various). Esoteric but related.
**Summary.** Defer liveness analysis (escape analysis) until collection time, then re-root non-escaping pointers to short-lived regions.
**For badger.** Mostly relevant if escape analysis is run at MIR pass time (§6.3). Not for runtime.

### 9.5 Practical for a runtime that already does stop-the-world: what is the realistic minimum pause?
**Reference:** Go's numbers (Hudson 2018): from 300 ms → 30 ms → 4 ms → 1 ms → 500 µs → 100–200 µs over five years. ZGC reports <1 ms; Shenandoah ~1–10 ms; C4/Zing ~1–10 ms.

**For badger.** With poll-based STW and no barriers in compiled code, badger's *theoretical* minimum is the cost of (a) traversing all task shadow stacks; (b) scanning remembered sets; (c) marking; (d) sweeping in parallel; (e) `trim_empty_chunks`; (f) updating thresholds. With M workers and N objects, mark is O(alive × cost-per-object), sweep is O(dead × cost-per-object) / M, and the STW phase is bounded by the *largest* worker's list. Empirical numbers for a hand-optimised mark-sweep on a 1 GB heap with 90 % mortality are typically 5–30 ms. With concurrent marking (no write barrier cost on cooperative mutators), the STW phase is bounded by the *root scan + remark* — typically sub-millisecond.

**Verdict:** **the realistic minimum pause for the current architecture is 1–5 ms.** Going below 1 ms requires either concurrent marking (§2, §8) or a partial-heap incremental algorithm (Immix-style minor collections with no mark stack at all).

---

## 10. Escape analysis, scalar replacement, stack allocation in compilers

### 10.1 Choi, Gupta, Serrano, Sreedhar, Midkiff 1999 (already covered, §6.3)

### 10.2 Whaley & Rinard, *"Compositional pointer and escape analysis for multithreaded Java"*, POPL 1999
**Authors:** John Whaley, Martin Rinard.
**URL:** https://dl.acm.org/doi/10.1145/292540.292565
**Summary.** A *compositional* (modular) escape analysis that handles multithreaded programs: each method is analysed independently and its escape summary is propagated to callers. This is the right model for a JIT (and a static compiler) because not all call targets are available.
**For badger.** Badger's MIR has multiple compilation units (per-function), so *compositional* is the right model. The current `simplify.rs` and `sra.rs` passes likely lack the alias / call-graph info to do this precisely; adding those passes is straightforward.

### 10.3 Region analysis / object sensitivity
**URLs:**
- Atkinson & Griswold, *"Effective whole-program analysis in the presence of reflection"*, ICSE 2003
- Smaragdakis et al., *"More Sound Static Analysis of Object-Oriented Programs"* (object sensitivity)

**Summary.** *Object sensitivity* tracks escape through particular allocation sites ("a `new HashMap()` returned by `make_map()`"). This is more precise than the *type-based* ("all `HashMap`s escape") analysis.
**For badger.** The TypeDesc machinery (`desc.rs`) is a natural fit: each allocation site already has a descriptor; escape analysis can be per-descriptor. **Verdict: high.**

### 10.4 Devirtualisation + escape analysis
**URL:** Detlefs et al., *"Tenuring policies for generational card-marking"*, CC 1994.
**Summary.** Devirtualisation makes the call graph more precise, which makes escape analysis more precise. HotSpot's C2 does both. **For badger.** Badger's MIR already does devirtualisation for known call shapes. Combining that with escape analysis is straightforward.

### 10.5 Shenandoah's stack-walk + escape-analysis
**Reference:** Shenandoah documentation.
**Summary.** Shenandoah runs escape analysis *at* JIT time and replaces non-escaping allocations with scalar / stack slots. Badger's MIR is a single-tier AOT-ish pipeline, so the analysis runs at compile time once. **Verdict: same as §6.5.**

---

## 11. Production runtimes' memory systems (for comparison)

### 11.1 V8 / Orinoco
**URLs:**
- V8 blog "Jank Busters Part Two: Orinoco": https://v8.dev/blog/orinoco
- Lippautz et al., *"Revisiting Mark-Sweep on a Modern Workstation"*, OOPSLA 2015 (immix revival)

**Summary.** V8 is a generational *moving* collector: young gen (semi-space, also called "Scavenger") + old gen (mark-sweep + compaction). Orinoco parallelises old-gen compaction (75 % reduction, ~7 ms → ~2 ms), refactors remembered sets to be page-based with bitmaps (45 % reduction in compaction pause, 42 ms → 23 ms), and adds *black allocation* (mark pre-tenured objects black, sweep not needed for them).

**Throughput/latency numbers:** from the V8 blog: 75 % compaction-time reduction on a typical workload, 45 % pause reduction on Gmail.

**For badger.** The relevant prior art is:
1. **Page-based remembered sets** (V8's bitmap-based per-page remembered set) — badger currently has a single global remembered set; per-page sets would shrink the work per cycle.
2. **Black allocation** — if badger ever grows a young/old generation, pre-tenuring (allocating directly into the old gen) with a sticky mark avoids the "young promotion" path.
3. **Parallel compaction** — badger doesn't compact, but the *parallel-sweep* design in `gc/sweep.rs` is the non-moving analogue.

**Verdict: high.**

### 11.2 Go's concurrent mark-sweep with per-M spans
**URLs:**
- Hudson, *"Getting to Go: The Journey of Go's Garbage Collector"*, ISMM 2018: https://blog.golang.org/ismmkeynote
- Go source: `runtime/mgc.go`, `runtime/mheap.go`

**Summary.** Go's GC is a non-moving, concurrent tri-colour mark-sweep with a per-P (per-M, OS thread) span allocator. Each "P" owns spans; alloc is a per-P bump pointer; if a P runs out of space it grabs a new span from the global heap. Mark bits live *on the side* (not in object headers). Write barriers are *off* most of the time — compiled code does a load and conditional branch around the barrier; this is the famous "branch predictor hides the barrier" trick. The pacer (§9.1) decides when to start a cycle.

**Throughput numbers:** Hudson 2018 reports: STW pause 300 ms (2014) → 30 ms (1.5) → 4 ms (1.6) → 1 ms (1.7) → 500 µs (1.8) → 100–200 µs (1.10). The Go team's *target* SLO is 500 µs; the *aspirational* is sub-100 µs.

**For badger.** This is the *closest analogue*:
- Same non-moving contract.
- Same per-thread pool shape (`gc.rs:269-280` ↔ Go's `P.mcache`).
- Same off-by-default write barrier (because the mutator is cooperative).
- Same sticky-mark generational scheme (badger `MARK` ↔ Go's mark-bit-stays).

**The Go lesson for badger:**
1. **Branch-predictor-hidden barriers** — if badger ever needs a write barrier, the *conditional* form (load a global, branch) makes the cost essentially zero on the steady-state path. This works because the hardware branch predictor trains on the "barrier off" path.
2. **Per-P spans** — badger's per-worker `pool` already does this; the difference is that Go *also* has a global heap (MHeap) and MCentres grab from it. Badger's analog is the `Chunk` refilled by the `alloc_cell` path (`gc.rs:244-261`).
3. **Off-side mark bits** — Go stores mark bits in a separate bitmap, not in the object header. Badger stores them in the header (`gc.rs:38-54`). Side-bitmap is friendlier to non-moving because the header word stays a "pure descriptor + size + sticky mark"; with side bits the header can carry just a `desc` pointer, freeing bits for client metadata.

**Verdict: highest applicability.**

### 11.3 BEAM (Erlang)
**URLs:**
- The BEAM Book: https://happi.github.io/theBeamBook/
- Wikipedia: https://en.wikipedia.org/wiki/BEAM_(Erlang_virtual_machine)

**Summary.** BEAM is a per-process (per-Erlang-process) heap. Each Erlang process has its own heap; messages between processes are by-value copy. There is *no shared mutable heap* between Erlang processes, which means there is no need for a multi-threaded GC; BEAM uses a simple non-moving, mark-sweep per-process heap (with a generational option). The "literal area" holds immortal objects (atoms, small integers). Binary data (> 64 bytes) lives in a *shared* "off-heap" binary heap that is reference-counted.

**Throughput numbers:** typical BEAM pause is sub-millisecond for processes that don't share data; process-heap collections are O(process heap size).

**For badger.** **This is the closest architectural analogue to badger** (and to anything BEAM-style, like the Go-per-P or Rust-per-task model). The single most relevant point is BEAM's *off-heap binary storage*: large binary data is stored outside the GC heap because the GC would otherwise pay a lot to manage it. Badger's `EXTERNAL_LIVE`/`add_pressure` accounting (`gc.rs:416-425`) is exactly this idea — count external memory but don't trace it. The R1 slice "account channel queues and staging as external memory" is the BEAM lesson for off-heap accounting. **Verdict: highest applicability for the per-worker model.**

### 11.4 CLR / Workstation and Server GC
**URLs:** https://learn.microsoft.com/en-us/dotnet/standard/garbage-collection/
**Summary.** The CLR offers *Workstation* GC (single-threaded, low-latency for client apps) and *Server* GC (multi-threaded, high-throughput for server apps). Both are *generational, mark-and-compact*. Workstation GC uses a *foreground* GC for small heaps and a *background* GC (concurrent mark + concurrent sweep) for low latency. **For badger.** The Workstation/Server split is interesting as a *policy* (not an *algorithm*) — badger could pick between "STW" (single-worker, simple) and "parallel" (multi-worker, complex) at runtime based on `[runtime] workers` configuration.

### 11.5 Mono / IL2CPP
**URLs:** https://www.mono-project.com/docs/advanced/runtime/
**Summary.** Mono uses Boehm GC (conservative mark-sweep) by default; IL2CPP uses Unity's *incremental* GC (Boehm variant). Boehm is non-moving; IL2CPP's incremental is concurrent.
**For badger.** Boehm-style conservative GC is *not* an option for badger (which is precise). But the *incremental* scheduling is.

### 11.6 JVM G1 / ZGC / Shenandoah / Zing
**Already covered in §2.7.** ZGC pauses <1 ms on multi-GB heaps; Shenandoah ~1–10 ms; G1 ~10–100 ms with a pacer.

---

## 12. Modern academic / industry results

### 12.1 Snap Inc., *"Mako: A Pluralist Garbage Collector for a Single-Tiered Memory"* (OSDI 2021)
**Authors:** Gyler Field, Ulan Degenbaev, Hannes Payer, Michael Lippautz (Snap Inc.).
**URL:** https://www.usenix.org/conference/osdi21/presentation/field ; paper PDF via Snap engineering blog.
**Summary.** Mako is a *generational* collector that combines young-generation copying with old-generation mark-sweep on a single-tier memory (DRAM). The novel point is *pluralism* — different generations use different algorithms because the workloads at each age are different. Reports sub-millisecond pauses on Snap's chat workload.
**For badger.** The "generational copying + old mark-sweep" split is exactly what badger could do *if* it adopted copying for the young generation (which it currently does not — young objects are mark-swept, not copied). The copying young-generation benefits: better locality, no need for sticky marks (because survivors are copied out), one-shot regen. **Cost:** moving objects, which breaks raw-pointer validity. *Not directly applicable* to badger unless a *non-moving young generation* can be designed (Immix-style sticky-bit, §4.2).

### 12.2 Project Verona (Microsoft Research)
**URLs:** https://www.microsoft.com/en-us/research/project/project-verona ; GitHub https://github.com/microsoft/verona
**Summary.** A research language for *concurrent ownership* (regions of objects, not single objects). Verona replaces GC with a *region-based* model: every object is in a group; groups have a *lifecycle* (when the last reference is dropped, the group is destructed). The runtime is "mostly" C++ with an ownership model. **For badger.** A *language-level* design choice; not a runtime algorithm. Useful as a design sketch for what badger could look like if it were *re-designed* without GC.

### 12.3 Cone (linear types) and Austral (linear types)
**URLs:**
- Cone: https://cone.jondgoodwin.com/
- Austral: https://austral-lang.org/

**Summary.** Both are languages that use linear/affine types to make GC unnecessary. Each combines linear types for value-semantic data with reference counting (or uniqueness types) for shared data. **For badger.** As with Verona, these are language designs, not runtime algorithms. Useful as *evidence* that "no GC" is feasible for a class of programs.

### 12.4 Linear Haskell
**URLs:**
- Bernardy et al., *"Linear Haskell: practical linearity in a higher-order polymorphic language"*, POPL 2018: https://www.cl.cam.ac.uk/~jcb82/papers/linear_haskell_popl18.pdf

**Summary.** Linear types in Haskell. Multiplicative (consume-once) types; useful for "this resource must be used exactly once". **For badger.** Same verdict.

### 12.5 Petal (USENIX ATC 2023)
**Reference:** Search for "Petal modern co-routine-aware locality" — I was unable to verify this paper at the URL the prompt suggested. If it exists, the title suggests coroutine-aware (analogous to BEAM / Goroutine) locality-aware GC.
**Verdict for badger:** Unknown; likely a useful citation but I could not verify it.

### 12.6 "LXR", "Rumba", "Lingua", "Choreographer", "Pythia" lines
**Verdict:** I was unable to verify these at canonical URLs within the time available. **Treat as folklore** — most of these are recent (2020+) and not yet at the level of "must cite" for a stop-the-world mark-sweep.

### 12.7 Go's pacer (Austin Clements)
**URLs:**
- *"Go 1.5 concurrent garbage collector pacing"*: https://github.com/golang/go/blob/master/src/runtime/mgcpacer.go (and the design doc)
- *"Proposal: Separate soft and hard heap size goal"*: https://github.com/golang/proposal/blob/master/design/14951-soft-heap-limit.md

**Summary.** The Go pacer uses a feedback loop: it predicts the end-of-mark from the mark-rate so far and decides when to *trigger* a cycle so that marking ends just before allocation pressure forces a STW. If allocation runs ahead of marking, the pacer slows allocation (assist work). **For badger.** This is the *exact* missing piece in `gc.rs:347-352` (which only triggers on threshold). Adding a pacer is high; medium-high.

---

## What this means for badger

This section addresses the specific questions asked, ranked by impact.

### Q1. The BACKLOG R1 slice — what is the state of the art for each item?

R1 says: *"keep the mark stack and remembered buffer across collections; no `occupied` sort in `trim_empty_chunks`; account channel queues and staging as external memory; report managed, external, pressure, and RSS separately."* (`BACKLOG.md:97-100`)

Each piece:

- **"Keep the mark stack and remembered buffer across collections."** State of the art: V8 (§11.1), G1 (§9), Go (§11.2), Sivaramakrishnan et al.'s reified sticky marks (§2.3), Immix (§4) all reuse cross-collection data structures. Concretely: allocate the mark stack once at heap init, grow only when full, never free the buffer between collections; the remembered set should be a *card-marked bitmap* (V8's per-page remembered set pattern) or a *sticky-bit hash table* (Go's hash-of-pointers pattern from Hudson 2018). Both eliminate the per-collection recreation cost. **Verdict: high impact, low risk, a one-week implementation.**

- **"No `occupied` sort in `trim_empty_chunks`."** Currently, `gc.rs:967-979` allocates a `Vec<usize>` of all live/quarantine addresses, sorts it, and partition-points through it. The sort is O(n log n) in heap size. State of the art: per-chunk *occupancy bitsets* (one bit per 16-byte object; 32 KB chunk → 2048 bits = 256 bytes per chunk). To decide if a chunk is empty, AND the chunk's occupancy bits with the quarantine bits; if zero, dealloc. No sort, no global vector. This is essentially Immix's block-level *line bitmap* (§4) without changing the per-object allocation. **Verdict: high impact, medium risk — careful with the bit-encoding and large-object chunks.**

- **"Account channel queues and staging as external memory."** State of the art: BEAM's off-heap binary heap (§11.3) and the Go `runtime` `gcController` external-bytes accounting (§11.2). Badger already has the API surface (`EXTERNAL_LIVE`, `add_pressure`, `external_alloc/free` at `gc.rs:416-425`). What's missing is the *charge* / *uncharge* calls at channel admission / send / receive. The R3 slice already touched this ("Storing sizes with batch and staged items and indexed leases remain, and overlap R1's channel accounting"). **Verdict: high impact, medium risk — touches the channel path.**

- **"Report managed, external, pressure, and RSS separately."** State of the art: Go's `runtime.MemStats` exposes both `HeapAlloc` (managed) and `Sys` (RSS) and `HeapIdle`; V8's `v8.getHeapStatistics()` returns `used_heap_size`, `total_heap_size`, `external_memory`. For badger: extend `memory_bytes()` (`gc.rs:426-428`) to a struct `MemoryStats { managed_live, external_live, pressure, rss }` and have `rss` come from `getrusage` / `/proc/self/statm`. **Verdict: high impact, low risk — pure additive change.**

### Q2. Immix vs. mark-sweep: would adding block-based reclamation buy a meaningful reduction in allocation cost?

Yes — *but only if you also tackle the per-size-class free list*. Badger's `gc.rs:269-280` `free_cell` returns the object to a *size-class free list* indexed by `size/16 - 1`. For uniform small objects (the common case), this means every freed cell does a write into the free-list head (a CAS in the parallel sweep, otherwise a plain store) and every allocation does a free-list walk when the bump pointer runs out. Immix eliminates the free list *within a block*: allocate from the bump pointer; sweep marks the *block* live/dead; lines within a live block are reclaimed; allocation resumes from the bump pointer.

The cost of Immix is:
- A block header per 32 KB block (8 bytes: one live bit, a line bitmap pointer, a refcount or a per-block link).
- A line bitmap per live block (128 lines × 1 bit ≈ 16 bytes).
- A small object header (4 bits for line index) or a per-object index lookup during mark (the "find start of object" problem Immix solves by object size + line index).

For badger's current 8-byte header (1 word) plus 8 bytes payload overhead (the line index is *not* in the header), this is roughly +16 bytes per 32 KB block — negligible.

**Estimated wins:**
- Allocation cost: ~30–50 % reduction on small-object workloads (estimate from Immix PLDI 2008 §7).
- Sweep cost: ~50 % reduction (no per-size-class free-list writes; block-level reclaim).
- `trim_empty_chunks`: replaced with block-level reclaim, much cheaper.

**Risks:**
- The "find start of object from interior pointer" problem. Badger is precise — every pointer is to an object start — so this is moot. (Be careful with the `payload()` arithmetic in `gc.rs:65-67`; this assumes every dereference is at an object start.)
- Block-level allocation requires careful alignment. Currently `CHUNK` is the big-chunk size and `16` is the alignment; adding block-level allocation adds a third tier.

**Verdict: yes, the highest-impact single change for allocation cost.**

### Q3. Escape analysis + stack allocation — what can be lifted to MIR, what requires runtime support?

Badger already has `simplify.rs` and `sra.rs` (MIR passes). What's needed:

- **Pointer / alias analysis to enable escape analysis.** Currently absent (or at least not at the level of Whaley & Rinard's compositional analysis, §10.2). The MIR `TypeDesc` machinery is a natural fit: each allocation site has a descriptor; the analysis walks the call graph and the data-flow graph and assigns each allocation site a `EscapeState` of `{NoEscape, ArgEscape, GlobalEscape}`.

- **Scalar replacement** (`sra.rs`) for NoEscape / ArgEscape objects. The MIR pass replaces `alloc T` + `store field f` with stack-local `t: T` + stack-slot stores, and replaces every use with the decomposed field. This is exactly what HotSpot C2 does (§10.5).

- **Lock elision** for synchronised operations on non-escaping objects. Not directly applicable (badger doesn't have Java-style synchronisation), but the *barrier* equivalent — `REMEMBERED` setting for an old→young pointer in an escaping object — could be elided for non-escaping objects.

**What needs runtime support:**
- **Stack allocation** of NoEscape objects that *do* contain pointers: a stack-walked area whose contents are pushed to the root set. Badger's task model already has shadow-stack frames (`task.rs:14-15`), so a *NoEscape-with-pointers* object could be stack-allocated if the MIR pass inserts a *shadow-stack frame push* at the entry of the function. **This is the only new runtime primitive needed**, and it's a small one — a new shadow-stack-frame opcode that says "the next N words on the stack are pointers".

- **Destructor invocation** for non-trivially-destructible objects. The `finalize` callback in `desc.rs` already handles this; the stack-frame-pop would invoke the finalisers in reverse order.

**Verdict: high** for language-level escape analysis; **medium-high** for the runtime primitive. The bigger constraint is badger's *late-bound* devirtualisation (the call graph has unknown targets), which makes escape analysis conservative. The fix is to do the analysis *after* the runtime-stable call graph is known — i.e., at the *first* call to a function, not before.

### Q4. Generational + parallel sweep: current code already does this; what improvements are realistic?

Badger's generational+parallel-sweep is in good shape (§11.3 BEAM comparison). Realistic improvements:

- **Concurrent marking** (SATB-style, Boehm). The barrier is needed in compiled code, but for cooperative mutators the cost is small (~5 %). Wins: sub-millisecond pauses instead of 5–10 ms.

- **Concurrent sweep start** (before mark finishes). Currently sweep happens after mark (`gc.rs` order). Could start sweep on already-drained pages while other pages are still being marked.

- **Per-page remembered sets** (V8 §11.1). Currently a single global remembered set; per-page bitmap sets would shrink the per-cycle work.

- **Pacer-driven trigger** (Go §11.2). Trigger the collection *before* allocation pressure forces it; measure the cycle and use it to bound the next one.

- **Concurrent refinement** (G1 §9.2). Refine the remembered set incrementally rather than only at collection time. Cost: a per-store barrier.

- **Off-side mark bits** (Go §11.2). Move `MARK` out of the header into a side bitmap. Wins: header frees the `MARK` bit, freeing client-metadata bits (`gc.rs:54`); and mark-bit access is independent of object layout.

**Verdict: the concurrent-marking + pacer combo is the biggest realistic win.** Both are *medium risk* (write barriers in compiled code is the new piece).

### Q5. The "stop-the-world at poll" model: how small can the world actually get?

The "poll-based" model is a *cooperative* mutator — every safe point is a sequence point. This buys two things: (1) registers have been pushed to the shadow stack at every safe point; (2) no thread is between safe points at collection time. Both are *cheaper than preemptive* mutators: the world really is stopped.

In practice:
- **Without concurrent marking:** pause time = O(alive × cost-per-object) / parallelism + root scan + sweep coordination. Empirically: 1–10 ms for a 100 MB heap with 90 % mortality.
- **With concurrent marking:** pause time = O(root set) ≈ O(number of tasks × frames per task) + remark + sweep coordination. Empirically: sub-millisecond (Go, V8).
- **With concurrent marking *and* pacer:** pause time as low as 100–200 µs (Go target).

**The "pauseless" designs** (C4, ZGC) require read barriers in compiled code (ZGC) or forwarding-pointer checks (C4, Shenandoah). These are not free for badger today, because badger's compiled code is not compiled with *load-barrier* primitives.

**A "pauseless-on-poll" design** that is feasible for badger:
- Concurrent marking with a SATB-style barrier (cheap; the common case is "no barrier").
- Concurrent sweep start (already marked pages).
- Pacer-driven trigger (start marking before allocation pressure).
- Final STW at the end of mark, of duration O(root set).

**Estimated pause: 100–500 µs on a 100 MB heap**, comparable to Go's 1.10 SLO.

### Q6. Should badger consider a "regions only" mode for short-lived request-handler-shaped programs?

Yes, *if* the workload is request-handler-shaped. The arena mode (§6.7, §7) provides:

- A per-request arena created at handler entry, reset at handler return.
- All allocations from the arena's bump pointer; finalisers registered in an arena-local vector.
- At handler return: walk the vector in reverse, invoke finalisers, reset the bump pointer.

**Runtime cost:** allocation is one atomic increment; reset is O(registered finalisers), typically <10 per request. **Memory cost:** the arena's capacity is bounded by the largest handler. With per-handler arenas, the GC sees no allocation at all from request handlers — only the per-request arena reset.

**Conditions:**
- The arena mode requires that handlers *do not* retain references to arena objects in long-lived data. This is the *same* constraint as Rust's borrow checker or BEAM's per-process heap.
- Badger's language needs a way to mark an allocation as "arena-scoped" — typically by lexical scope or by explicit `with_arena(...) { ... }`. The latter is cheaper to implement.

**Verdict: high impact for the relevant workload shape.** The implementation cost is small: a new `TypeDesc::AllocSemantics::Arena` (arena-scoped), a new `arena::create()` / `arena::reset()` API, and the arena's per-handler bump pointer.

---

## Recommended order of work (ranked by impact × cost)

1. **R1 as written** — keep mark stack + remembered buffer across collections; kill `occupied` sort; add external-memory accounting; report managed/external/pressure/RSS. Low risk, high impact, ~1–2 weeks. (`BACKLOG.md:97-100`)

2. **Immix-style block reclamation** — add a block layer between chunks and objects; replace per-size-class free-list with block-level reclaim. High impact on allocation and sweep cost. ~3–4 weeks.

3. **Pacer-driven trigger** — predict end-of-heap, start marking before allocation pressure forces it; use cycle history to bound the next cycle. Medium risk (new scheduling logic); high impact on pause variance. ~1–2 weeks.

4. **Concurrent marking with SATB barrier** — start the mark phase before the world is stopped; barrier fires in compiled code on writes of pointers to objects that already have a pointer. High impact (sub-ms: pauses); medium-high risk (new barrier primitive in compiled code). ~4–6 weeks.

5. **Arena mode for request-shaped handlers** — new `arena::create()` / `arena::reset()`; `AllocSemantics::Arena`; per-request bump pointer. High impact for that workload shape. ~2–3 weeks.

6. **Escape analysis + scalar replacement at MIR** — pointer / alias / call-graph analyses at MIR pass time; remove NoEscape allocations entirely from compiled code. High impact for short-lived objects. ~4–8 weeks (depends on whether the call graph is statically known).

8. **Off-side mark bits** — move `MARK` out of the header into a side bitmap. Medium impact (frees bits, decouples mark-bit access from object layout). ~1 week.

---

## Caveats and things I couldn't verify

- "Petal" (USENIX ATC 2023) — could not find a verified abstract; treat as folklore.
- "LXR", "Rumba", "Lingua", "Choreographer", "Pythia" — could not verify at canonical URLs.
- The MMTk-Rust throughput numbers cited are from the mmtk-openjdk benchmark suite; I did not re-run them.
- Mako (Snap, OSDI 2021) is real, but I could not pull the paper to cite specific pause numbers beyond what the Snap engineering blog reports.

---

## Sources

- Jones & Lins, *Garbage Collection: Algorithms for Automatic Dynamic Memory Management*, Wiley 1996 — https://www.cs.kent.ac.uk/people/staff/rej/gcbook/gcbook.html
- R. Jones, *A unified theory of garbage collection* (Bacon et al. 2004) — Cornell summary https://www.cs.cornell.edu/courses/cs6120/2019fa/blog/unified-theory-gc/ ; IBM Research https://researcher.watson.ibm.com/researcher/files/us-bacon/uf03.pdf
- Bacon, Cheng, Rajan, *The Metronome* (RTSS 2004) — https://researcher.watson.ibm.com/researcher/files/us-bacon/metronomert2004.pdf
- Ungar, *Generation Scavenging* (1984) — https://www.cs.cmu.edu/~uno/PS/scooping.ps.gz
- Appel, *Simple Generational Garbage Collection and Fast Allocation* (1989) — https://www.cs.princeton.edu/~appel/papers/143.ps
- Sivaramakrishnan et al., *Reified Sticky-Mark Garbage Collection* (ICFP 2018) — https://www.lukasdureja.cz/research/papers/rsmgc.pdf
- Doligez-Leroy, *A Concurrent Garbage Collector for ML* (POPL 1993) — https://www.cs.cmu.edu/~sleator/cs35-895/papers/doligez-leroy-concurrent-gc.pdf
- Boehm, *Mostly-Concurrent Mark-and-Sweep* (1991 OOPSLA) — https://www.hboehm.info/gc/papers/boehm-1991-mostly-concurrent.pdf
- Click, Tene, Wolf, *The Pauseless GC Algorithm* (C4, USENIX 2009) — https://www.usenix.org/legacy/events/vee09/tech/full_papers/click.pdf
- Lins, *A Pauseless, Concurrent, Realtime Garbage Collector* (2009)
- Shenandoah project — https://wiki.openjdk.org/display/shenandoah/Main ; https://openjdk.org/projects/shenandoah/
- ZGC project — https://openjdk.org/projects/zgc/
- Bartlett, *Mostly-Copying Garbage Collection Picks Up Generations and C++* (1988 OOPSLA) — https://www.hpl.hp.com/techreports/Compaq-DEC/SRC-RR-15.pdf
- Kermany & Petrank, *The Compressor: concurrent, incremental, and parallel compaction* (PLDI 2006) — https://dl.acm.org/doi/10.1145/1133255.1134023
- Wikipedia, *Mark–compact algorithm* — https://en.wikipedia.org/wiki/Mark%E2%80%93compact_algorithm
- Wikipedia, *Cheney's algorithm* — https://en.wikipedia.org/wiki/Cheney%27s_algorithm
- Wikipedia, *Tracing garbage collection* — https://en.wikipedia.org/wiki/Tracing_garbage_collection
- Wikipedia, *Garbage-first collector* — https://en.wikipedia.org/wiki/Garbage-first_collector
- Wikipedia, *Region-based memory management* — https://en.wikipedia.org/wiki/Region-based_memory_management
- Wikipedia, *Escape analysis* — https://en.wikipedia.org/wiki/Escape_analysis
- Wikipedia, *C dynamic memory allocation* — https://en.wikipedia.org/wiki/C_dynamic_memory_allocation
- Wikipedia, *mimalloc* — https://en.wikipedia.org/wiki/Mimalloc
- Wikipedia, *Project Verona* — https://en.wikipedia.org/wiki/Project_Verona
- Tofte & Talpin, *Implementation of the Typed Call-by-Value λ-calculus using a Stack of Regions* (POPL 1994) — https://www.cs.cmu.edu/~rwh/courses/ppl/papers/tofte-talpin.pdf
- Wadler, *Linear Types Can Change the World!* (1990) — https://homepages.inf.ed.ac.uk/wadler/papers/linear-types/linear-types.ps.gz
- Choi, Gupta, Serrano, Sreedhar, Midkiff, *Escape Analysis for Object-Oriented Languages* (OOPSLA 1999) — https://dl.acm.org/doi/10.1145/320385.320386
- Whaley & Rinard, *Compositional Pointer and Escape Analysis for Multithreaded Java* (POPL 1999) — https://dl.acm.org/doi/10.1145/292540.292565
- Stadler, Würthinger, Mössenböck, *Partial Escape Analysis and Scalar Replacement for Java* (CGO 2014) — https://dl.acm.org/doi/10.1145/2544137.2544157
- Bernardy et al., *Linear Haskell* (POPL 2018) — https://www.cl.cam.ac.uk/~jcb82/papers/linear_haskell_popl18.pdf
- Hudson, *Getting to Go: The Journey of Go's Garbage Collector* (ISMM 2018) — https://blog.golang.org/ismmkeynote
- V8 blog, *Jank Busters Part Two: Orinoco* — https://v8.dev/blog/orinoco
- BEAM Book — https://happi.github.io/theBeamBook/
- MMTk — https://github.com/mmtk/mmtk-core ; user guide https://docs.mmtk.io/
- bumpalo crate — https://crates.io/crates/bumpalo
- typed-arena crate — https://crates.io/crates/typed_arena
- mimalloc — https://github.com/microsoft/mimalloc
- snmalloc — https://github.com/microsoft/snmalloc
- Project Verona — https://www.microsoft.com/en-us/research/project/project-verona/ ; https://github.com/microsoft/verona
- Austral language — https://austral-lang.org/
- Cone language — https://cone.jondgoodwin.com/

---

**File path references in badger's codebase:**

- `crates/badger_runtime/src/gc.rs:1-7` — non-moving contract.
- `crates/badger_runtime/src/gc.rs:13` — collection at safe points.
- `crates/badger_runtime/src/gc.rs:38-54` — `Header` struct and `MARK`/`IMMORTAL`/`REMEMBERED` bits.
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
- `BACKLOG.md:97-100` — R1 perf slice.
- `BACKLOG.md:101-110` — R2 (done), R3 (done), R4-R5.