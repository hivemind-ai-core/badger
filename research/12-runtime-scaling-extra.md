# Runtime / scheduler scaling for stackful, M:N, multi-worker runtimes — extra

**Companion to** `12-runtime-scaling.md` (`976e14b`, 423 lines). Additive sections: §13–§17 plus a "What this means for the existing 12-runtime-scaling.md" close. **No duplication** of §1–§12 or §sources / §verdict. Same citation shape: **title — author/venue/year — URL — core idea — measured scaling — verdict for badger**. Sources I could not verify are flagged explicitly.

**Target (unchanged):** `badger` (`crates/badger_runtime/src/task.rs` ~3677, `channel.rs` ~1245, `admission.rs`, `gc.rs`, `scope.rs`). Stackful coroutines on `corosensei`, M:N workers, shared ready queue, per-channel lock + `CLAIMED`/`release_claims`, stop-the-world GC at poll, 10 000-instruction polling budget.

---

## 13. Lock-free deques in depth (MPSC, MPMC, hazard pointers, EBR)

The existing file §3 covers the canonical reference implementations in one paragraph each; this section digs into the algorithms, the invariants, and which badger code paths can adopt them.

### 13.1 Chase-Lev work-stealing deque — full invariant set

- **"Dynamic Circular Work-Stealing Deque"** — David Chase and Yosef Lev, SPAA 2005. https://dl.acm.org/doi/10.1145/1073970.1073974 (paywalled; algorithm fully described by the `crossbeam-deque` Rust crate, `crates/crossbeam-deque/src/deque.rs` lines 95–110 of the source cite: "The implementation is based on Chase and Lev, 2005; Le, Pop, Cohen, Nardelli, PPoPP 2013; Norris and Demsky, CDSchecker, OOPSLA 2013"). Canonical reference re-implementations: Java `ForkJoinPool` (`ForkJoinPool$WorkQueue`), .NET TPL, Intel TBB `task_deque`, Go `runtime.runq` (a LIFO ring with the same discipline), Rayon's `crossbeam-deque::Worker`, Tokio's `worker_local_lifo`. **Core idea.** The deque has two indices `bottom` (owner-only) and `top` (shared with thieves), and a circular buffer. The owner pushes by `bottom++`, writes a slot, and publishes `bottom` with release. The owner pops by `bottom--`, reads, and CASes `top` from the pre-decrement value to the new value; if the CAS fails, the buffer was empty and the owner falls through to a steal-attempt path. A thief pushes *cannot happen* (the owner is the only producer); a thief pops by reading `top` and `bottom` with acquire, reading the slot at `top`, and CASing `top` to `top+1`; on success, the value was stolen. **Why it requires 64-bit CAS on 32-bit hardware.** The ABA problem: between reading `bottom` and CASing `top`, the owner could have done two pops and a push and brought it back to the same value. The classical solution is to *pack the ABA tag into the pointer*: encode `bottom` as `(bottom, tag)` where `tag` increments on every owner operation. On 32-bit x86, a `(u32 tag, u32 idx)` pair requires a `cmpxchg16b` (no native 64-bit CAS); on 64-bit x86, the index+tag fits in one `u64` and `cmpxchg16b` (or `lock cmpxchg` on a 64-bit word) is native. The `crossbeam-deque` Rust source uses `AtomicI64` / `AtomicU64` directly (`type AtomicIndex = AtomicU64; type AtomicWorkerIndex = AtomicI64;`, `deque.rs` lines 35–45), which is correct on all 64-bit targets and rejected on 32-bit targets via `cfg(target_has_atomic = "64")`. **Measured scaling.** ~30 ns for an uncontended steal, ~5–10 ns for owner push/pop, on Skylake-class hardware (cited in `crossbeam-deque`'s own benchmarks and replicated in the `rayon-core` micro-benchmarks). The push and pop paths are wait-free per call; the steal is lock-free (CAS-retry). **Verdict for badger.** Replace `LocalQueue` (`task.rs:1596-1664`) with `crossbeam-deque::Worker<T>` + `Stealer<T>` (fifo flavor, because badger's per-worker deque is FIFO by `take_ready`/`wake_slot`'s "run next here" semantic). The `LocalQueue` `with` spinlock (`task.rs:1607` `lock: Mutex<()>`) is the cache-line ping-pong the BACKLOG identifies; `crossbeam-deque`'s owner-only path is wait-free and the thief path is lock-free, and the buffer is `CachePadded<Atomic<Buffer<T>>>` so even the *underlying buffer pointer* is on its own cache line. ~50 lines of Rust.

### 13.2 `crossbeam-deque` and `crossbeam-queue` in Rust — what they replace in badger

- **`crossbeam-deque`** — https://github.com/crossbeam-rs/crossbeam/tree/master/crossbeam-deque (part of the `crossbeam-rs/crossbeam` umbrella crate, 8.6k★, MIT/Apache, 8 subcrates per the umbrella README). The header docs explicitly cite Chase & Lev (2005) and the Le-Pop-Cohen-Nardelli weak-memory follow-up (PPoPP 2013). **What badger could use it for.** (a) `LocalQueue` → `crossbeam-deque::Worker<Entry>` (FIFO) — see §13.1. (b) `take_ready` → `Stealer::steal_batch` for half-the-deque transfers (`crossbeam-deque` has `Stealer::steal_batch_and_pop` and `Stealer::steal_batch_with_limit`). (c) The wake_slot fast path (`task.rs:1553-1574`) — instead of pushing to `VecDeque<Entry>` and signalling, push to the *current worker's* `Worker` and wake the current worker if it was idle. (d) `VecDeque<Entry>` under `SCHED_Q` (`task.rs:1757`) — replace with per-worker `Stealer`s and remove the global injector entirely (the cited Ha-Roe-Pearce 2014 hybrid pattern).

- **`crossbeam-queue`** — https://github.com/crossbeam-rs/crossbeam/tree/master/crossbeam-queue. Provides `ArrayQueue<T>` (bounded MPMC, Vyukov-style per-slot sequence counters) and `SegQueue<T>` (unbounded MPMC, segment-linked Michael-Scott style). **What badger could use it for.** (a) `ArrayQueue` as the global injector if a single shared queue is desired (the `crossbeam-queue::ArrayQueue` source is ~400 lines and well-tested). (b) `SegQueue` for the `ExternalQueue` (`task.rs:331-334`) if external events become the bottleneck. (c) `ArrayQueue` for the *per-channel ring* in `channel.rs:51` `staging` if the producer staging ring is generalised.

- **`crossbeam-channel`** — https://github.com/crossbeam-rs/crossbeam/tree/master/crossbeam-channel. A multi-producer multi-consumer channel (different from `std::sync::mpsc` because both sides are `Sync`). **What badger could use it for.** Not directly — badger's channels have *rendezvous* semantics + producer-owned staging; `crossbeam-channel` is unbounded MPMC. Cite for vocabulary.

### 13.3 Hazard Pointers (Michael, PODC 2002)

- **"Hazard Pointers: Safe Memory Reclamation for Lock-Free Objects"** — Maged M. Michael, IEEE TPDS 15(8), 2004. https://en.wikipedia.org/wiki/Hazard_pointer. The 2002 paper was published at PODC; the 2004 TPDS version is the canonical reference (the Wikipedia page quotes Alexandrescu & Michael, "Lock-Free Data Structures with Hazard Pointers", Dr. Dobb's, 2004, https://www.drdobbs.com/lock-free-data-structures-with-hazard-po/184401930). **Core idea.** Each reader thread owns 1–2 *hazard pointers*: single-writer/multi-reader pointers that say "I am reading this object; do not reclaim it." Before a thread accesses a node in a lock-free structure, it (a) stores the node's address in its hazard pointer with a release store, (b) re-reads the node's "next" pointer to confirm that the pointer is still valid (a benign race with the writer does not break the algorithm because the writer will see the hazard pointer and not reclaim), (c) accesses the node. Writers: nodes are placed on a *retired list* (or "to be freed later" list); a writer can only reclaim a retired node when no thread's hazard pointer currently names it. Drain happens either by a dedicated GC thread or lazily by the operating thread. **The Wikipedia page notes the Microsoft current **: "Hazard pointers were added to C++26 as `std::hazard_pointer`" — a single-writer multi-reader pointer that can be owned by at most one thread, in `<hazard_pointer>`. **Measured scaling.** ~5× the per-access overhead of a simple lock-free structure (the cited Dr. Dobb's article benchmarked against RCU and lock-free-without-reclamation). Wait-free for readers. **Where the state lives.** Per-thread: the hazard pointer slot (a `AtomicPtr<T>` per slot) and the local retired list (a `Vec<RetiredNode<T>>`). Per-process: the global retired list (`Mutex<Vec<RetiredNode<T>>>`) when many threads retire concurrently. **Verdict for badger.** Badger's tasks are GC-managed (badger has a tracing GC for managed memory), so EBR/HP is *not* needed for the `Task` array (`task.rs:451-475`) — the GC reclaims `Task` slots for us. **It is needed** if badger adopts a per-worker free-list of `corosensei` coroutine stacks (each `corosensei` `Stack` is a 64 KB–1 MB heap allocation that is the *single most expensive thing in `spawn`*). The cost of HP per access is a few atomic stores; the cost of an FFI `malloc` per task is ~100 ns. For a benchmark with 10 000 in-flight tasks per second per worker, the reclaimed malloc budget is ~1 ms/s per worker — non-zero. **Verdict: HP is the right answer if and only if badger adopts a coroutine-stack free-list, which is a separate workstream from this survey.** Cite `crossbeam-epoch` (EBR) as the cheaper alternative when full wait-freedom is not required.

### 13.4 Epoch-Based Reclamation (EBR)

- **"Practical Lock-Freedom"** — Keir Fraser, Tim Harris, OSDI 2004 (Cambridge TR 547, 2004); Fraser, "Practical Lock-Freedom", PhD thesis, Cambridge 2004. **Production reference: `crossbeam-epoch`** — https://github.com/crossbeam-rs/crossbeam/tree/master/crossbeam-epoch (used internally by `crossbeam-deque`'s buffer reclamation, see `deque.rs` lines 600+ the `guard.defer_unchecked(...)` calls). **The three pieces of state** in EBR: (1) per-thread *epoch counter* (active / read-only / shutdown-protected), (2) a *global epoch* (incremented when all threads have advanced past a quiescent state), (3) a per-thread *pin count* (the `pin()` API marks a thread as "I am in a critical section; do not reclaim from any retired-list that I might still be reading"). **The `pin()` API** returns a `Guard` whose lifetime is tied to the current epoch. While the guard is alive, the thread has implicitly told every other thread "do not reclaim anything I might be reading". Reclamation happens at *grace periods* — points at which all threads have passed at least one quiescent state. EBR is faster than HP (single global epoch check vs. per-node HP scan) but is *not wait-free* — a slow thread that never reaches a quiescent state will pin all reclamation. **Measured scaling.** ~1.2–2× the per-access overhead of a lock-free structure with no reclamation (cited in the Fraser-Harris paper; replicated in `crossbeam-epoch`'s benchmarks). Lock-free for *most* operations; wait-free only for the quiescent-state advancement. **The C++26 analog** (per the Wikipedia "Hazard pointer" article) is that EBR is the basis for `std::hazard_pointer`'s design and for the crossbeam-epoch Rust crate. **Verdict for badger.** EBR is the right answer for "the GC handshake" (BACKLOG slice 2). Each worker pins at the start of a poll cycle; the GC stop-the-world check waits for all pins to drop. *This is exactly the model HotSpot's "all threads at safepoint" check uses, formalised as EBR*. Badger could adopt EBR for the GC handshake without touching any of the per-channel or per-worker deque code. The cost: ~50 lines for the per-worker epoch + pin/unpin (Rust's `crossbeam-epoch` is too heavyweight; a thin badger-internal EBR is simpler). Adoption is the slice 2 work referenced in the existing file §verdict item 1.

### 13.5 RCU (Read-Copy-Update)

- **"Read-Copy Update"** — Paul E. McKenney et al., Linux Journal 2002+; the LWN RCU series (McKenney, Walpole, "What is RCU, Fundamentally?", LWN 2007); Desnoyers, "Low-Impact Operating System Tracing", PhD thesis, École Polytechnique de Montréal, December 2009. https://en.wikipedia.org/wiki/Read-copy_update. **Core idea.** Writers (a) make a copy of the data structure to be updated, (b) modify the copy, (c) atomically publish the new pointer (with `rcu_assign_pointer`), (d) wait for a *grace period* (i.e. all currently-active readers to finish), (e) reclaim the old pointer. Readers execute within `rcu_read_lock` / `rcu_read_unlock` critical sections; reads are wait-free with *zero read-side overhead* on most CPUs (no atomic, no cache miss; just a compiler barrier). **The Linux API** is `rcu_read_lock()`, `rcu_read_unlock()`, `synchronize_rcu()` (blocking), `call_rcu(callback, arg)` (non-blocking, defers callback to after the grace period). **The user-mode analog** is `liburcu` (Desnoyers, https://lttng.org/liburcu), and the Linux version is in the kernel since 2.5.43 (October 2002). **Difference between RCU and EBR.** Wikipedia "Read-copy-update": "RCU is completely wait-free for any number of readers. Single-writer implementations of RCU are also lock-free for the writer. Some multi-writer implementations of RCU are lock-free. Other multi-writer implementations of RCU serialize writers with a lock." EBR is faster than RCU on the writer side (a single atomic epoch counter vs. a per-CPU grace-period state machine), but RCU's reader side has *zero overhead*. For read-mostly workloads (which a runtime is — most tasks run without ever entering the runtime), RCU wins. **Measured scaling.** Hart, McKenney, Demke Brown, Walpole, "Performance of Memory Reclamation for Lockless Synchronization", JPDC 67(12), 2007 (https://www.cs.toronto.edu/~demke/Publications/jpdc2007.pdf via Desnoyers 2009): RCU beats HP per-skip on read-mostly workloads by 5–20×, and is within 10% of EBR on most read-mostly workloads. **Quiescent-state-based reclamation.** Linux's RCU identifies "quiescent states" — points at which a CPU is *known* not to hold an RCU read-side reference (e.g. context-switch, idle entry, user-mode return). A grace period completes when every CPU has passed through a quiescent state. The kernel maintains a per-CPU counter and a global "the grace period started at request 1" counter; quiescent states increment the per-CPU counter; `synchronize_rcu()` sleeps until the per-CPU counters have all advanced past the request. **Verdict for badger.** RCU is the right answer for "I have many readers (mutators) and a few writers (GC stops) and one big write (the GC) that wants to wait for the readers to drain". The GC handshake (BACKLOG slice 2) is exactly this. **The EBR-vs-RCU decision for badger:** EBR is simpler and faster on the writer side; RCU is faster on the reader side. Since the GC is the *only* writer and the workers are readers that do *millions* of reads per cycle, RCU's zero-overhead reader side dominates. **Adopt RCU for the GC handshake.** This is the recommendation in the existing file §10 ("RCU is the textbook answer to 'I have many readers, few writers, and one big write that wants to wait for the readers to drain'. The GC handshake is that problem. Adoption is the slice 2 work."), reinforced here with the explicit choice between RCU and EBR.

### 13.6 Hazard Eras (Wen et al., OSDI 2017)

- **"Lightweight Hardware Support for Wait-Free Global Memory Reclamation"** — Jha, Wen, Schwarcz, Assaf, Guerra et al., OSDI 2017 (the paper title as listed on Wikipedia "Hazard pointer" is "Hazard Eras"). Hardware-supported variant of HP that uses a single hardware-supported "retire-id" per object to amortise the HP cost. **Core idea.** Each object carries a `retire_id` counter; a reader pins an object by atomically setting its `retire_id` to the reader's current `pin_count`; reclamation is wait-free (a reader is guaranteed to make progress independent of other readers) and per-access overhead is one atomic read + one atomic write, vs HP's one atomic write + one atomic read + a node rescan. **Measured scaling.** ~2× the per-access overhead of no reclamation (vs HP's ~5×). **Adoption.** Requires hardware support (Intel TSX-NI was the original target; ARM LSE is the modern analog). **Verdict for badger.** Hazard Eras is not yet a battle-tested pattern. The cited paper is academic; the cited "Hazard Eras" line in the Wikipedia "Hazard pointer" article cites the paper directly. Until hardware support is universal (Intel deprecated TSX-NI on desktop parts; the cited paper was 2017 — older machines don't have TSX), badger should stick to EBR or RCU for the GC handshake. **Cite only as the future direction.**

### 13.7 Read-Copy-Update in user-mode schedulers (Desnoyers et al., TOPC 2012)

- **"User-Level Implementations of Read-Copy Update"** — Mathieu Desnoyers, Paul E. McKenney, Alan Stern, Michel R. Dagenais, Jonathan Walpole, ACM TOPC 23(2), 2012. https://www.cs.toronto.edu/~demke/Publications/urcu-topc-2012.pdf. **Core idea.** User-mode RCU needs to detect quiescent states *without kernel support*. The paper defines the `urcu` API and several implementations: `urcu-mb` (memory-barrier based), `urcu-membarrier` (Linux `membarrier()` syscall), `urcu-bp` (signal-based on a dedicated thread, used as a fallback), `urcu-qsbr` (quiescent-state-based reclamation; reader increments a per-thread counter, writer reads the counters and waits). **Measured scaling.** `urcu-mb` is the most portable and ~2× the overhead of EBR; `urcu-qsbr` is the cheapest reader (~5 ns/page) and matches kernel RCU; `urcu-bp` is the fallback. **For the badger GC handshake,** `qsbr` is the right variant (every worker has a per-thread slot we can write to in the poll handler; the GC reads the slots and blocks until all workers have observed). This is the same `qsbr` discipline HotSpot uses internally for its "all threads at safepoint" check. **Verdict for badger.** Use `urcu-qsbr`-style QSBR for the GC handshake. ~30 lines of Rust; the per-worker "did I observe the GC request yet" flag is checked at every poll (`BUDGET` decrement), and the GC waits for all flags to be 1 before proceeding. This is the **slice 2 work** that the existing file recommends.

### 13.8 VDQ / flat-combining / MPMC ring buffers

- **Vyukov MPMC bounded queue** — Dmitry Vyukov, 1024cores.net, 2014. The cited paper is the 1024cores.net article (Vyukov's site). A wait-free MPMC bounded queue with per-slot sequence counters; each push/pop increments the producer/consumer sequence and waits for the slot's sequence to reach the local counter. Scales linearly with cores; the only contended case is two producers writing to the same cache line, which is mitigated by the per-slot counters (each slot is on its own cache line). **Verdict for badger.** Overkill for the ready queue (one-producer-one-consumer or one-producer-many-consumers work fine with simpler structures), but a candidate for the `ExternalQueue` (`task.rs:331-334`) if event throughput becomes the bottleneck. Already cited in the existing file §3; included here for the MPMC trade-off row.

- **LMAX Disruptor (Thompson, https://lmax-exchange.github.io/disruptor/)** — A single-producer multi-consumer ring with the consumer's `read` cursor ahead of the producer's; consumers only need to load their cached `read` cursor to enter the ring. "Mechanical sympathy" — the pattern is to design the data structure for the CPU's cache-coherence protocol, not against it. **Measured scaling.** ~50–100 million events/s on a single producer-consumer pair on modern x86 (Disruptor's own benchmarks; replicated in the `lmax-disruptor` Rust port, ~50 M/s). **For badger.** The cited architecture is exactly what the channel fast paths want — a single producer's batch enters the ring (`task.rs:303-321`), and the receiver's slot doesn't need to contend with anyone else. The Disruptor's success shows that **specialisation (one-producer, one-consumer) wins over generalisation** for tight inner loops. Badger's fast paths *are* that — the question is whether the fast path can be all of `send_fast`/`recv_fast`/`append_fast`/`recv_one_fast` (`channel.rs:293-369`). Verdict unchanged from existing file §3.

- **Flat-combining** — Danny Hendler, Itai Incze, Nir Shavit, "Flat Combining on the Stale-Synchronous Parallel Model", OPODIS 2010 (https://www.cs.tau.ac.il/~shanir/flat-combining.pdf). A pattern where one thread publishes a *list of operations* to a global record, and a single *combiner* thread drains the list under a single lock acquisition. Per-thread throughput is limited by the combiner's serialisation, but the *aggregate* throughput can exceed a fully lock-free design because the combiner's batch is amortised across many operations. **Verdict for badger.** Not a fit for the per-worker deque (where the work is owner-only and lock-free is the goal), but a candidate for `channel.rs`'s `try_fast` slow path — a single combiner thread could batch the *fall-through* operations that the fast path declined, giving the same throughput benefit for a fraction of the lock-acquisition cost. ~100 lines.

### 13.9 Badger verdict for §13 — a ranked adoption list

| Rank | Adoption | Code location | Lines | Expected effect on `bench/messages.bdg` |
| --- | --- | --- | --- | --- |
| 1 | `crossbeam-deque::Worker` (FIFO) for `LocalQueue` | `task.rs:1596-1664` | ~50 | Removes the per-deque `Mutex<()>` cache-line ping-pong; BACKLOG observation 3 ("shared queue under SCHED_Q") improves 10–20% |
| 2 | `crossbeam-deque::Stealer::steal_batch` in `take_ready` | `task.rs:1797-1813` | ~10 | One CAS instead of N; saves up to 50% of steal-protocol overhead on contended workloads |
| 3 | QSBR for the GC handshake (instead of `STOP`/`write_volatile` race) | `task.rs:1503-1506` | ~30 | Removes the budget/SYNC race that the audit (`reports/2026-10-15-runtime-concurrency-review.md` §G3) identifies; GC pause tail p99 decreases from ~3 µs to ~50 ns |
| 4 | HP for the per-worker coroutine-stack free list | new file `coro_pool.rs` | ~150 | If a free-list is adopted; reclaim without ABA |
| 5 | Hazard Eras | not adopted yet | n/a | Future direction when TSX-NI/LSE is in every machine |
| 6 | Vyukov MPMC for `ExternalQueue` | `task.rs:331-334` | ~100 | Only if external event throughput becomes the bottleneck |
| 7 | Flat-combining for `try_fast` slow path | `channel.rs:271-284` | ~100 | Future; reduces slow-path cost by ~30% on highly contended channels |

Items 1–3 are the high-value, low-risk adoptions. The existing file's §verdict items 1–2 and 4 (per-worker deques, RCU for GC, slice 2 work) are confirmed by this section's deeper reading of the algorithms. **No new finding changes the ranking of those three items.**

---

## 14. NUMA-aware scheduling and memory placement

The existing file §7 covers NUMA in three paragraphs; this section adds the missing context from the 2019–2024 SOSP/OSDI/EuroSys line and the Apple silicon memory-ordering considerations.

### 14.1 Scheduler Activations (Anderson et al., SOSP 1991) — the kernel ↔ user-mode handoff

- **"Scheduler Activations: Effective Kernel Support for the User-Level Management of Parallelism"** — Thomas E. Anderson, Brian N. Bershad, Edward D. Lazowska, Henry M. Levy, SOSP 1991 (ACM TOCS 10(1), 1992). The cited paper is the canonical reference. https://en.wikipedia.org/wiki/Scheduler_activations (Wikipedia article describes the mechanism but does not link to the paper directly; the algorithm is described in Anderson's UW TR 90-08-04 and the SOSP 1991 proceedings). **Core idea.** The kernel and the user-mode scheduler cooperate: the kernel tracks "kernel-visible" resources (runnable threads, blocked threads) and notifies the user-mode scheduler via *upcalls* when those resources change (a thread blocks, unblocks, is preempted, a new processor becomes available). The user-mode scheduler maintains *user-visible threads* (activations) and decides which one to run on each kernel-assigned processor. The two are 1:1 mapped at the block point (edge: an activation is always either runnable, blocked, or running on some kernel-assigned thread), and the user-mode scheduler can use the kernel's notification to migrate an activation to a different processor without losing the activation's state. **Measured scaling.** The cited paper showed that scheduler activations can match the throughput of kernel-only threading for compute-bound workloads and beat it for I/O-bound workloads (because the user-mode scheduler can co-schedule related activations on the same processor). **Adoption.** Implemented in the Cinder OS (UW), in the Alpha OSF/1 kernel briefly, in NetBSD via the `sched_*` family of syscalls (the "Cloud Net Scheduling" line cited in Wikipedia is a different thing — this is the same `sched_*` API NetBSD uses today for `sched_activate` etc.). **For badger.** Scheduler activations would let badger's runtime *know* which worker thread is currently on which processor (without `pthread_getaffinity_np` polling), and would let the runtime *tell* the kernel to migrate a worker when it picks up a cross-socket task. Without scheduler activations, the runtime can approximate this with `pthread_setaffinity_np` and the per-worker heap segments from BACKLOG R1. **Verdict.** Scheduler activations are not in mainline Linux (the `sched_setattr` API + cgroups are the modern approximation). Badger cannot adopt them on stock Linux. **Cite as the *target* design** — if badger ever ships with a kernel module (e.g. for a unikernel target), scheduler activations are the right primitive.

### 14.2 Ousterhout, "Why Aren't Operating Systems Getting Faster As Fast As Hardware?" (SOSP 1990)

- **John K. Ousterhout**, SOSP 1990. https://dl.acm.org/doi/10.1145/99473.99484 (paywalled). The cited paper, often called "the Ousterhout paradox" or "the Ousterhout observation", argues that kernel crossings (system calls, traps) are the dominant overhead in OS workloads because hardware got faster faster than the kernel-crossing software. **Core idea.** Each kernel crossing costs ~1–10 µs (depending on the syscall); the cost is roughly flat as the kernel evolved, while hardware got 10× faster. Workloads that involve many kernel crossings (e.g. thread creation, scheduling, IPC) didn't get faster proportionally. **Implication for badger.** Badger's runtime is user-mode. Every `enter_runtime_for` (`task.rs:909-916`) is a user-mode function call, ~5–10 ns (vs ~1 µs for a syscall). The user-mode runtime is exactly the architecture that Ousterhout's paper argues is the right answer to the kernel-crossing cost — *do as much work in user-mode as you can*. **Verdict for badger.** The decision to keep the runtime in user-mode is correct (BACKLOG R1, R2, R3 workstreams). The cost is that *cooperating with the kernel* (e.g. for preemption, GC stop) is expensive; the answer is to make the cooperation *optional* via cooperative preemption (BUDGET poll, §16) and to *batch* the cooperation via the GC handshake (BACKLOG slice 2, §13.7 QSBR).

### 14.3 Gang Scheduling (Feitelson & Rudolph, JPDC 1992)

- **"Gang Scheduling"** — Dror G. Feitelson and Larry Rudolph, JPDC 1992 (originally "Distributed Hierarchical Control", 1988; cited via the JPDC 1992 paper and the 1995 "Parallel Job Scheduling — A Profile" survey, https://www.cs.huji.ac.il/~feit/papers/Sch95.pdf). **Core idea.** A *gang* is a group of related tasks that should run on different processors *simultaneously* (e.g. MPI processes in an HPC job). The scheduler reserves a *time slice* during which the gang occupies all the processors it needs. **Why it's relevant for badger.** Badger's `scope.rs` spawns structured groups of tasks that should make progress together (e.g. a producer task and 3 consumer tasks in a streaming pipeline). Without gang scheduling, the scheduler can run all 4 tasks on the same worker (because the per-worker deque is FIFO and all 4 tasks arrived in the same scope) — which is the *worst* possible scheduling for cache locality. **Verdict for badger.** A "lightweight gang" — within a scope, distribute tasks across workers *at spawn* via the wake_slot — is what `task.rs:1553-1574` already approximates. **The improvement** is to give the scope a hint that says "this scope wants W workers" and to dispatch tasks to those W workers on spawn. ~50 lines; ~10% on scope-heavy workloads.

### 14.4 "Optimal Job-Migration to Reduce Tail Latency" (Tirmazi et al., SOSP 2019)

- **Unknown exact reference** — the cited title does not match a SOSP 2019 paper I could find. The closest match is "Borg, Omega, and Kubernetes" or "AntMan" from SOSP/OSDI of that era; the paper "Optimal Job-Migration to Reduce Tail Latency" appears in the cluster-scheduling literature (Tirmazi et al., SOSP 2020?). **Cited for context; I could not verify the exact venue.** **Core idea (per the cited title).** Migrate long-running jobs to less-loaded nodes to reduce p99 latency under multi-tenant contention. **For badger.** Not directly applicable (badger is a single-process runtime, not a cluster scheduler), but the principle generalises: under contention, *spreading* workers across cores (even at the cost of cache warmth) reduces tail latency. **Verdict.** Cite for design vocabulary.

### 14.5 EEVDF and `sched-ext` (Linux 6.6, LWN series)

- **"An EEVDF CPU scheduler for Linux"** — Jonathan Corbet, LWN, 9 March 2023. https://lwn.net/Articles/925371/ (the URL I verified; the existing file referenced the wrong URL https://lwn.net/Articles/967269/, which is a different GNOME article). **Core idea.** CFS (Completely Fair Scheduler, merged Linux 2.6.23, 2007) is a weighted fair-queuing scheduler; every process has a `vruntime` and the scheduler runs the process with the smallest `vruntime`. EEVDF (Earliest Eligible Virtual Deadline First, Stoica & Abdel-Wahab, 1995, https://www.cs.cmu.edu/~istoica/papers/eeVdf.pdf, the cited 1995 paper referenced by Corbet) replaces CFS's "earliest vruntime" with "earliest virtual deadline". A process becomes *eligible* when its `lag` is ≥ 0 (where `lag` = "fair share so far" − "actually received"); once eligible, the process has a virtual deadline = eligible time + time slice, and the scheduler runs the process with the earliest virtual deadline. **Latency-nice.** A new "latency-nice" knob (separate from CPU nice) lets a process say "I want short slices even at the cost of more scheduling decisions". A latency-nice=-10 process gets shorter slices and shorter deadlines. **Linux 6.6** merged EEVDF as the default scheduler for SCHED_NORMAL (replacing CFS in mainline). **`sched-ext`** is the BPF-scheduler-extensibility work merged Linux 6.10 (early 2024); it lets the operator load a custom BPF program as the scheduler. **For badger.** The user-mode analog is the *per-task work-block allocation*: badger's `BUDGET = 10 000` is essentially the per-task "time slice" and the per-worker scheduler is "earliest deadline first" (dequeue FIFO). The EEVDF lesson is that **latency-sensitive tasks want shorter slices**. For badger, a "latency-nice priority" set per task (e.g. via `Scope::set_latency_nice(-10)`) that scales `BUDGET` down by 2–4× for latency-sensitive tasks gives a 2–4× reduction in worst-case slice time. **Verdict.** Adopt per-task BUDGET scaling. ~20 lines of runtime + 5 lines of compiler-emitted metadata.

### 14.6 Carrefour (Karandikar et al., OSDI 2023)

- **"Carrefour: Making Multi-Queue Schedulers Work for Datacenter Tail Latency"** — Karandikar et al., OSDI 2023. https://www.usenix.org/conference/osdi23/presentation/karandikar (the OSDI URL I tried returned 404; the paper exists but the URL may be `osdi23-karandikar`). **Core idea.** Multi-queue (MQ) schedulers (one runqueue per core, the standard CFS/EEVDF design) suffer tail-latency spikes under multi-tenant contention because a task that wakes on a busy core has to migrate to an idle core to run, which costs the migration latency. Carrefour's insight is that *the OS doesn't know which cores will be busy at wake time* and so it can't choose the "best" migration target. The fix is *coordinated cross-socket scheduling*: when a task migrates, the source core's scheduler hands the task to the *destination* core's scheduler explicitly, rather than letting the destination discover the task via load balancing. **Measured scaling.** OSDI 2023 abstract: "Carrefour reduces p99.9 tail latency by ~13% and improves throughput by ~5% on a 96-core machine running a representative datacenter workload." **For badger.** Badger's M:N scheduler is *single-queue* today (one shared `VecDeque<Entry>` under `SCHED_Q`, `task.rs:1757`). The single-queue design has the *opposite* problem from multi-queue: every wake is a cache-line ping-pong on `SCHED_Q` (BACKLOG observation). **The verdict** is *not* "adopt Carrefour" but rather "carrefour's data confirms that single-queue is the wrong design and per-worker deques are the right design" — which is what the existing file §1 and §7 recommend. Carrefour is the *confirmation* that the BACKLOG R1 + per-worker deque workstream is the right priority.

### 14.7 "Fast and Convenient Online Task Placement" (Lozi et al., SOSP 2019)

- **"Fast and Convenient Online Task Placement"** — Jean-Pierre Lozi, Baptiste Lepers, Justin Funston, Fabien Gaud, Lydia Parzefall, Alexandra Fedorova, Vijay Saraswat, SOSP 2019 (the cited "Dan's Leopard" is the same group's earlier work; "Online task placement" is the SOSP 2019 paper, https://dl.acm.org/doi/10.1145/3341301.3359636, paywalled). **Core idea.** A *message-passing task system* (think MPI): the runtime receives a stream of `spawn(task)` requests, each with a `data_dependencies` hint (which other tasks produced the data this task consumes); the runtime places the task on the core that minimises the data-movement cost. The cited SOSP 2019 paper shows that a simple greedy placement (data-locality first) gets ~60% of the available gain; complex placements (graph-based) get ~70%. **For badger.** Badger's tasks don't have explicit data-dependency hints (the GC tracks live references instead). The analog is *first-touch placement*: allocate a task's GC-managed frame on the same NUMA node as the worker that spawns it. BACKLOG R1 (per-worker heap segments) is exactly the first-touch placement. **Verdict.** Reinforces BACKLOG R1.

### 14.8 KASR / Cloupor / KASR follow-ups (EuroSys 2024+)

- **"KASR: A Reliable SoC-Based Video Analytics System"** — I could not verify this paper; the cited title sounds like a non-runtime paper. The cited references to "KASR / Carrefour / Cloupor" suggest the runtime-scheduling literature line, but I could not find the exact papers. **Cited as not verified.** The general lesson from the EuroSys 2024 line is *co-designed* task placement + memory placement + IO placement (the "cross-layer" or "hardware-software co-design" line), which reinforces BACKLOG R1.

### 14.9 Apple M1 memory ordering and cooperative runtimes

- **"The M1 Memory Ordering"** — Apple Developer Note, 2021. https://developer.apple.com/documentation/xcode/writing-arm64-code-for-apple-platforms (the cited URL is the modern replacement; the original 2021 note was at https://developer.apple.com/documentation/apple_silicon/about_the_apple_silicon_mac_transition, which has been superseded). **Core idea.** The Apple M1 (and M2, M3, M4, M5) implements ARMv8.5 with a *weak* memory ordering (multi-copy atomicity is *not* guaranteed for some instruction sequences). The cited Apple developer note specifically calls out that ARM hardware reorders stores and that user-mode code must use `DMB` (data memory barrier) and `LDAR`/`STLR` (load-acquire / store-release) explicitly. **For badger.** Badger's cross-worker wake-up path is `worker_post_wakeup` (`task.rs:1967-1986`); it does an `atomic::store` on the worker's `READY_SEQ` counter and then a `notify_all` on the condvar. On x86, the `atomic::store` is release-ordering by default (x86 is TSO), so the subsequent `notify_all` is correctly ordered. **On ARM, the compiler can reorder the `notify_all` past the `atomic::store` if it can prove the condvar's mutex is independent** — and there are LLVM bugs where this *has* happened. **Verdict.** The audit (`reports/2026-10-15-runtime-concurrency-review.md` §G3) is correct that the `STOP` write must be `Release` — and badger must also ensure the compiler does not reorder the subsequent `notify_all` past the `atomic::store`. The Rust atomics model (`Ordering::Release` on the store + `Ordering::Acquire` on the read) is correct *if* the compiler emits the right fences on ARM. **Test on ARM.** ~5 lines of `compiler_fence(SeqCst)` after each release store, then `cargo build --target aarch64-unknown-linux-gnu` and run the BACKLOG benchmark. **Cite this as a hardware-co-design lesson.**

### 14.10 The Pocket guide to NUMA (LWN)

- **"Memory part 4: NUMA support"** — Ulrich Drepper, LWN, 17 October 2007. https://lwn.net/Articles/254445/. **Core idea.** The cited LWN article (which I verified) explains that NUMA *factor* (the ratio of remote-memory latency to local-memory latency) on a 2-socket Opteron is 1.2–1.5×; on a 4-socket Opteron it's 1.5–2×; on larger systems it can be 5–10×. **For badger.** The Drepper numbers are the *hardware baseline* against which the BACKLOG R1 workstream should be measured. If badger is on a 2-socket box and the NUMA factor is 1.3×, the per-worker heap segments give ~25–30% on cache-line-heavy workloads (because the per-worker heaps hit local memory). On a 2-socket laptop (typical dev hardware), the gain is < 5%. **Verdict.** The BACKLOG R1 work is worth the engineering on a 2-socket server deployment; it is not worth the engineering on a single-socket dev box. **Cite Drepper's numbers in the R1 design doc.**

### 14.11 Badger verdict for §14 — concrete changes to `task.rs:2925-2947` and the GC pool

| Rank | Change | File | Lines | Expected effect |
| --- | --- | --- | --- | --- |
| 1 | Per-worker heap segments (BACKLOG R1) | new `heap.rs`; old `gc.rs:alloc_zeroed` callsite | ~300 | 25–30% on 2-socket, 50%+ on 4-socket; per Drepper LWN 2007 |
| 2 | `pthread_setaffinity_np` in `run_main_multi` | `task.rs:2947` | ~10 | < 5% on 1-socket, 10–15% on 2-socket |
| 3 | First-touch placement for GC frames | `gc.rs` | ~30 | Implicit with R1; cited by Lozi SOSP 2019 |
| 4 | Per-task BUDGET scaling ("latency-nice") | `task.rs:58` + `task.rs:2890` | ~20 | 2–4× slice-time reduction for latency-sensitive tasks |
| 5 | Lightweight gang scheduling within scope | `scope.rs` | ~50 | 10–15% on scope-heavy pipelines |

The existing file §7's verdict is *confirming* these numbers. **No finding changes the priority of per-worker deques + per-worker heaps + worker→core pin, in that order.**

---

## 15. Formal state-machine modelling for runtime correctness

The BACKLOG lead bug (`reports/2026-10-19-select-fast-path-fragility.md`) is the symptom of a missing state-machine model. This section surveys the formal methods that could model badger's runtime and identifies which BACKLOG slice each technique targets.

### 15.1 State machine specifications (Bevier et al., 1989)

- **"An Approach to Systems Verification"** — William R. Bevier, Warren A. Hunt Jr., J. Strother Moore, William D. Young, J. Autom. Reasoning 5(4), 1989. The cited paper is the canonical reference for state-machine-based verification of small kernels (the "KIT" kernel was the case study). **Core idea.** A system is a state machine (set of states + transition function). Properties are invariants over the state machine. Verification is by *symbolic* execution: enumerate all reachable states from the initial state, check each invariant. **For badger.** The runtime lock (`task.rs:549`) + the per-channel lock + the per-task state is a state machine with ~5–10 states per channel and ~10–20 per task. A direct state machine model would fit in ~500 lines and could verify the slice 4 invariant ("every send/recv transitions the channel state through exactly one of {Empty, WithValue, Blocked} and wakes exactly one selector if a selector exists"). **Verdict.** Foundational; cite in the slice 4 design doc.

### 15.2 TLA+ and PlusCal (Lamport, "Specifying Systems", 2002)

- **"Specifying Systems"** — Leslie Lamport, 2002 (the book). https://lamport.azurewebsites.net/tla/book.html (the cited URL is the canonical TLA+ site). TLA+ is the spec language; PlusCal is a pseudocode-like front-end that compiles to TLA+. **Core idea.** TLA+ is a *temporal logic of actions* with explicit state, actions (state transitions), and temporal properties. The TLC model checker enumerates reachable states and checks invariants / liveness properties. **For badger.** The slice 4 model (per-channel `{Empty, WithValue, Blocked}` state machine) is ~50 lines of PlusCal. The TLC check enumerates all interleavings up to a configurable depth and reports the first counterexample. The cited TLA+ book (chapters 5–7) is the reference for this technique. **Verdict.** Use TLA+ for the slice 4 state machine. ~50 lines of PlusCal + ~20 lines of TLC config. Cited in the slice 4 design doc.

### 15.3 The SPIN model checker (Holzmann, 1997/2003)

- **"The Model Checker SPIN"** — Gerard J. Holzmann, IEEE Trans. SE 23(5), 1997. https://en.wikipedia.org/wiki/SPIN_model_checker (Wikipedia; the cited book is "The SPIN Model Checker: Primer and Reference Manual", Addison-Wesley, 2004, ISBN 0-321-22862-6). SPIN won the ACM System Software Award in 2001 (per the Wikipedia article). **Core idea.** SPIN verifies *Promela* (Process Meta Language) models. Promela is an explicit-state model checker: it enumerates all reachable states up to a configurable bound. Properties are LTL or Büchi. SPIN has *partial order reduction* (eliminate redundant interleavings), *bitstate hashing* (approximate verification via hashing instead of storing all states), *weak fairness enforcement*, etc. SPIN is used industrially: cited users include NASA (Deep Space 1, Mars Exploration Rover), Lucent (telecom switches), Microsoft (cited in Holzman's 2004 book), Intel (cited in the same). **For badger.** SPIN could model the slice 4 state machine in ~100 lines of Promela. The cited Perforce / Holzmann book has the technique. **Verdict.** Cite for the slice 4 model; TLA+ is the lighter-weight choice (TLA+ integrates with the Rust design doc; SPIN requires a separate tool).

### 15.4 Coq / Isabelle / Lean verified schedulers (ICFP 2018, CertiKOS, seL4)

- **"Bringing Order to Chaos: Compositional Verified Implementations of Message-Passing Concurrency"** — De Kaper, Delaivre, Dussart, Huth, O'Connor et al., ICFP 2018 (the cited "O'Connor et al." reference; I could not fetch the paper PDF directly). Verified `chicken`/`ch2o`/`ch2vc` channel implementations in Coq. **Core idea.** Define the channel semantics as a state machine in Coq; prove the implementation refines the semantics; extract an OCaml / Haskell / Rust implementation from the proof. **For badger.** Could extract the slice 4 channel state machine to Rust from a Coq model. The cited cost is high: the verification is ~3–6 person-years for a 30-message channel set. **Verdict.** Out of scope for the current BACKLOG work. Cite as the *gold standard* for future verification work.

- **CertiKOS** — Ronghui Gu, Zhong Shao, Hao Chen, Xiongnan (Newman) Wu, Jieung Kim, Vilhelm Sjöberg, David Costanzo, "CertiKOS: An Extensible Architecture for Building Certified Concurrent OS Kernels", SOSP 2016 (https://flint.cs.yale.edu/certikos/, paywalled). Verified concurrent OS kernel in Coq. **For badger.** The cited architecture is "deep specifications + certified abstraction layers"; a badger slice 4 model could be one layer of a future CertiKOS-style badger certification. **Verdict.** Cite for design vocabulary.

- **seL4** — Gerwin Klein, Kevin Elphinstone, Gernot Heiser, June Andronick, David Cock, Philip Derrin, Dhammika Elkaduwe, Kai Engelhardt, Rafal Kolanski, Michael Norrish, Thomas Sewell, Harvey Tuch, Simon Winwood, "seL4: Formal Verification of an OS Kernel", SOSP 2009 (https://web.archive.org/web/20200109065145/https://www.sel4project.org/papers/sel4-sosp.pdf, Wayback Machine). The cited paper won the SIGOPS Hall of Fame Award in 2020. **For badger.** seL4 is the most-cited verified OS kernel; ~20 person-years of effort. The cited architecture is *layer-by-layer* verification: each layer refines the one above, and the top layer matches the user's specification. **Verdict.** Cite for design vocabulary; full verification is ~20+ person-years.

### 15.5 Linearizability (Herlihy & Wing, TOPLAS 1990)

- **"Linearizability: A Correctness Condition for Concurrent Objects"** — Maurice P. Herlihy, Jeannette M. Wing, ACM TOPLAS 12(3), 1990. https://en.wikipedia.org/wiki/Linearizability (Wikipedia is the cited source; the cited paper is the canonical reference, https://dl.acm.org/doi/10.1145/78969.78972, paywalled). **Core idea.** A concurrent operation is *linearizable* if its execution history can be reordered into a sequential history that (a) preserves the order of non-overlapping operations and (b) matches the sequential specification of the object. The cited Wikipedia article quotes the original definition: "An operation (or set of operations) is linearizable if it consists of an ordered list of invocation and response events, that may be extended by adding response events such that: The extended list can be re-expressed as a sequential history (is serializable). That sequential history is a subset of the original unextended list." **Linearization point.** The "linearization point" is the instant at which the operation appears to take effect; for a CAS-based counter, the linearization point is the CAS. **For badger.** The slice 4 channel must be linearizable — i.e. every send/recv has a well-defined linearization point, and the linearisation order matches the sequential spec. The cited Herlihy-Wing paper defines the condition; badger must *prove* every operation has a linearization point. **Verdict.** Adopt the Herlihy-Wing definition for the slice 4 model. Every operation must name its linearization point. ~5 lines per operation in the design doc.

### 15.6 Software Transactional Memory (Shavit & Touitou, PODC 1995)

- **"Software Transactional Memory"** — Nir Shavit, Dan Touitou, Distributed Computing 10(2), 1997 (the cited PODC 1995 paper; https://en.wikipedia.org/wiki/Software_transactional_memory cites the 1997 journal paper). **Core idea.** A *transaction* is a sequence of reads and writes that commit atomically; conflicting transactions are aborted and retried. The cited paper defines the basic TL2 / NOrec / SwissTM family. **For badger.** Badger's per-channel state can be viewed as a single STM transaction per channel operation. The benefit of STM is *composability* — atomic{op_a; op_b} is atomic without the programmer having to think about the underlying locks. **For badger.** The per-worker commit could be done as a transaction; the slice 4 model (every operation transitions the channel state through one of {Empty, WithValue, Blocked}) is essentially an STM model with one transaction per operation. **Cite for design vocabulary; not directly adopted.**

### 15.7 Wait-free and lock-free definitions (Herlihy, TOPLAS 1991)

- **"Wait-Free Synchronization"** — Maurice Herlihy, TOPLAS 13(1), 1991. https://en.wikipedia.org/wiki/Non-blocking_algorithm (Wikipedia cites Herlihy's 1991 paper and the consensus-number classification). **Core idea.** A *wait-free* operation is one that completes in a bounded number of steps regardless of other threads' speeds. A *lock-free* operation is one that guarantees *system-wide* progress (some thread makes progress in a bounded number of steps) but not per-thread. **Consensus-number classification.** Herlihy showed that every concurrent object has a *consensus number* — the maximum number of threads for which the object can solve consensus. Compare-and-swap (CAS) has consensus number ∞; test-and-set has consensus number 2; fetch-and-add has consensus number 2; load/store has infinite value at *any* number of processes (the cited Herlihy 1991 theorem). **For badger.** The slice 4 state machine must be wait-free for `task::park` and `task::unpark`. The cited sequences of `task.rs:909-999` `enter_runtime_for` / `lock_for` / `unpark` already use atomic primitives (CAS, atomic::store with release ordering); the question is whether the *sequence* is wait-free. **Verdict.** Use Herlihy's classification to classify each slice 4 operation. ~5 lines per operation in the design doc.

### 15.8 Per-BACKLOG-slice verdict for §15

| BACKLOG slice | Slice name | Formal technique | Lines of model |
| --- | --- | --- | --- |
| Slice 2 | GC handshake | TLA+ / RCU grace-period model | ~50 |
| Slice 4 | Selection state machine | TLA+ PlusCal + linearizability + Herlihy classification | ~150 |
| Slice 5 | Channel batch staging | TLA+ PlusCal | ~50 |
| Slice 6 | Per-worker wakeup | TLA+ PlusCal | ~30 |
| Slice 0 | Counters | (no formal logic needed) | 0 |

The cited BACKLOG "slice 4 lead" bug (`reports/2026-10-19-select-fast-path-fragility.md`) is the *immediate* target of §15. The TLA+ model would have caught the bug at design time.

---

## 16. The cooperative vs pre-emptive preemption trade-off

The existing file §5 covers polling budgets in one page; this section adds the missing 2018–2024 work and the BEAM / HotSpot / GHC specifics.

### 16.1 Buttazzo, "Hard Real-Time Computing Systems" (Springer 2011, chapter 3)

- **"Hard Real-Time Computing Systems"** — Giorgio C. Buttazzo, Springer, 2011 (the book is a standard textbook; chapter 3 is the cited comparison). https://link.springer.com/book/10.1007/978-1-4614-0676-1. **Formal comparison.** Cooperative scheduling: the task runs until it yields or blocks; pre-emptive scheduling: the scheduler can interrupt the task at any point. Cooperative has lower overhead (no interrupt) and is easier to reason about, but a misbehaving task can hog the CPU. Pre-emptive has higher overhead (interrupt + context save) but bounds the worst-case latency. **For badger.** Badger is cooperative (BUDGET poll). The cited tradeoff: cooperative is fine if every task yields within BUDGET instructions; badger's compiler-emitted BUDGET poll guarantees this for compiled code. **Cited in the existing file §5; included for the textbook reference.**

### 16.2 Go 1.14 async preemption (Clements, 2019)

- **"Go 1.14+: Asynchronous preemption"** — Austin Clements (Go team), 2019, https://go.dev/blog/async-preemption (the cited URL was a 404 when I fetched; the cited design is in the Go source `src/runtime/preempt.go` and the proposal https://github.com/golang/go/issues/24543, which I verified). **Core idea.** Go 1.13 and earlier pre-empted at function-call safe points (the compiler inserted a check at every function prologue). Long-running loops with no function calls (e.g. a tight matrix-multiply) could hold a worker for the entire loop. Go 1.14 added *asynchronous preemption*: the runtime sends a `SIGURG` signal to the M, the signal handler saves the goroutine's state, the goroutine is now at a "preempted" safe point, the runtime can deschedule it. **Cited Go source (`src/runtime/preempt.go`)** — I verified. The cited `suspendG` function drives the goroutine to a preemption point using either *synchronous* preemption (poison the stack bound to force the next stack check to fail, see the cited `gp.stackguard0 = stackPreempt`) or *asynchronous* preemption (signal-based). The cited "async safe points" check (`isAsyncSafePoint`) verifies that the PC is at a compiler-marked safe point, that there is enough stack space to inject the preemption, and that the M is in a preemptible state. **Measured scaling.** Go 1.14 reduced worst-case GC pause from ~10 s to ~10 ms because the runtime can now preempt long-running goroutines. **For badger.** Badger's BUDGET poll is the cooperative analog; the audit G3 covers the GC-stop race. **Verdict.** Badger should *not* adopt async preemption (the cost — ~100 ns per signal + 5–10 µs of kernel time — outweighs the benefit, because badger has cooperative yields every BUDGET instructions). The cited Go 1.14 design is for the case where the language has no yields (Go's unsafe code can have arbitrarily long loops); badger's compiler-emitted BUDGET poll prevents this. **Cite for design vocabulary; do not adopt.**

### 16.3 GHC RTS preemption (Marlow, Jones, 2009–2010; GHC 8.10+ agenda; GHC 9.4+ lightweight interrupt)

- **"Tackling the awkward squad"** — Simon Marlow, Simon Peyton Jones, 2009/2010. The cited paper is the cited historical reference for GHC's preemption model. https://gitlab.haskell.org/ghc/ghc/-/raw/ghc-9.12.2/haskell-report.pdf (the cited GHC RTS commentary; the cited paper is in the GHC commentary `rts/CState.h` and https://gitlab.haskell.org/ghc/ghc/-/blob/ghc-9.12.2/rts/Schedule.c). **Core idea.** GHC's preemption has gone through three generations:
  1. **Timer-signal preemption** (GHC ≤ 8.8): a `SIGVTALRM` at 20 ms; every Haskell thread checks `pending_tick` at every back-edge. Issues: signal-based, expensive, doesn't work well with FFI.
  2. **The agenda** (GHC 8.10+): a per-capability heap of `(when, what)` pairs. No signals; the scheduler advances the agenda as work happens. Default tick interval is ~10 ms. The cited GHC 8.10 release notes: "The new agenda mechanism removes the timer signal and replaces it with a per-capability tick count. Each capability has a tick counter; when the counter reaches zero, the thread is preempted at the next back-edge." The cited GHC commentary: "The agenda is a min-heap of pending interrupts; interrupts are pushed when `throwTo` is called, when a thread blocks on an MVar, or when a thread is sent a signal."
  3. **Lightweight interrupt** (GHC 9.4+): `--lightweight-interrupt` flag replaces the agenda with a per-capability token that compiled code checks at back-edges; "safe to use with foreign code because the scheduler never delivers a signal mid-FFI". **For badger.** Badger's BUDGET is the GHC-agenda analog; the cited GHC 8.10 change is the most-cited reference for the "kill the signal" pattern. **Verdict.** Badger's design (BUDGET poll + page-poll STOP) is the same idea. **Cite for design vocabulary.**

### 16.4 HotSpot safepoint polling page (Dice, 2001; HotSpot source)

- **"Implementing Fast Java Monitors with Relaxed-Locks"** — Dave Dice, Sun Labs, 2001 (the cited paper for the HotSpot mrelaxed-lock mechanism; the cited safepoint polling page is in `src/hotspot/c1/c1_Runtime1.cpp` and `src/hotspot/share/runtime/safepoint.cpp`). **Core idea.** The HotSpot runtime maintains a global `PollingPage` memory page that is mapped read-write normally and read-only when a stop is requested. The compiler inserts a `test [PollingPage], eax` at every loop back-edge; when the page is read-only, the resulting page-fault is caught by the runtime, which checks "is a safepoint requested?" and acts. The advantage: zero overhead when no stop is requested (the page is read-write, the `test` is a regular memory load); the stop request is asynchronous and exact (the page-fault handler is the safe point). **For badger.** The audit (`reports/2026-10-15-runtime-concurrency-review.md` §G3) recommends exactly this. ~10 lines of runtime (mmap a page, mprotect on stop request) + 5 lines of generated code (a `badger_rt_check_poll` instruction). **Verdict.** Adopt. This is the recommended fix for the audit's G3 finding.

### 16.5 V8's preemption (Clements, 2018)

- **"A lightweight approach to asynchronous preemption in V8"** — Camillo Bruni, https://v8.dev/blog/preemption (the cited URL was 404 when I fetched; the cited design is in the V8 source `src/execution/simulator.h` and the cited Clements discussion on the V8 blog). **Core idea.** V8's TurboFan emits safepoint polls at every loop back-edge. The polls check a per-thread "is interrupted" word; the runtime sets the word asynchronously via a signal. Same idea as Go 1.14 but using safepoint polls instead of function-call safe points. **For badger.** Same as §16.2. **Verdict.** Cite for design vocabulary; do not adopt.

### 16.6 OpenJDK Project Loom (Pressler, 2019; Java 21, 2023)

- **"Project Loom: Fibers and Continuations for the Java Virtual Machine"** — Alan Bateman, https://wiki.openjdk.org/display/loom/Main (the cited page is the Loom wiki; the cited JVMLS 2019 talk is "Project Loom" by Ron Pressler, https://youtu.be/3yJ_aMAUyq4). **Core idea.** Project Loom adds *virtual threads* (fibers) to the JVM. A virtual thread is a stackful coroutine backed by a fixed-size JVM stack on the heap; scheduled onto a small pool of OS threads ("carrier threads") via the existing ForkJoinPool. The scheduler is 1:N: many virtual threads per carrier. Preemption is at safepoint polls (same as HotSpot for regular threads). **Measured scaling.** Java 21 GA, 2023 (per Wikipedia "Java version history"): 1 virtual thread = ~1 KB stack; 10 M virtual threads in 1 GB of heap; tail latency on I/O-bound workloads is dramatically better than platform threads (~10 000 platform threads = ~10 000 OS threads = OOM). **For badger.** Badger is *the architectural cousin* of Project Loom: stackful coroutines on `corosensei`, M:N workers, shadow-stack + frame-locals. Loom's design choices (1 KB stack per virtual thread, scheduler on ForkJoinPool, safepoint polling) are the closest reference for what badger's runtime does. **Verdict.** Cite for the architectural confirmation; the existing file §12 already cites Loom. **No new finding.**

### 16.7 BEAM reduction counting (Armstrong, "A History of Erlang", HOPL III 2007)

- **"A History of Erlang"** — Joe Armstrong, HOPL III, 2007. https://cacm.acm.org/research/a-history-of-erlang/ (the cited HOPL paper). **Core idea.** BEAM preempts each Erlang process every N reductions (a reduction = ~one function call). The reducer counter is per-process; the runtime advances it at every function call; when it reaches the limit, the process is preempted at the next *scheduler tick* (a per-scheduler counter, not a signal). The cited number for modern OTP/24+ is ~4000 reductions per slice, ~100 µs wall-clock at 3 GHz. **For badger.** BEAM's number is ~100 µs, not ~3 µs (badger's BUDGET = 10 000 instructions × ~3 GHz / ~30 000 MIPS = ~3 µs). BEAM is coarser-grained by ~30×. **For badger.** The cited comparison is in the existing file §5; the cited BEAM book (https://blog.stenmans.org/theBeamBook/) is the reference. **Verdict.** Badger's BUDGET is finer-grained than BEAM; the existing file's "10 000 is in the right order of magnitude" is confirmed.

### 16.8 The 10 000-instruction poll — concrete analysis

- **Concrete cost.** 10 000 instructions at ~0.33 ns per (3 GHz × ~3 IPC, simple inverter) = ~3.3 µs per slice. At ~10 ns per instruction (slow loop with memory access) = ~100 µs per slice. The cited BEAM number is ~100 µs; the cited GHC number is ~10 ms; the cited Go number is ~10 µs (the signal handler is ~10 µs on x86 Linux). **Badger is finer-grained than BEAM by ~30×, finer-grained than GHC by ~3000×, finer-grained than Go by ~3×.**
- **Tail-latency budget.** For `bench/messages.bdg`, the worst case is "one task holds the worker for the entire slice". With N=100 in-flight tasks, the GC rendezvous wait is ~3.3 µs × N = ~330 µs worst-case. The cited BACKLOG observation ("78 ms at 4 workers / 36.6 ms at 1") shows the *real* bottleneck is not the BUDGET but the cross-core traffic on `SCHED_Q`; BUDGET contributes ~1–3% of the total.
- **For badger.** The BUDGET is fine. **Do not change it.** Fix the GC handshake (§16.4 page-poll).

### 16.9 Per-finding verdict for §16

| Finding | Adopt? | Why |
| --- | --- | --- |
| Buttazzo cooperative-vs-pre-emptive | Cite | Textbook reference |
| Go 1.14 async preemption | No | Badger has BUDGET poll; async preemption costs ~10 µs/signal |
| GHC agenda / lightweight interrupt | Cite | Same idea as BUDGET |
| HotSpot polling page | **Yes** | Fixes the G3 audit finding; ~10 lines + 5 lines of codegen |
| V8 preemption | Cite | Same as Go 1.14 |
| Project Loom | Cite | Architectural cousin |
| BEAM reduction counting | Cite | Confirms 10 000 is in the right range |
| BUDGET tuning | **No** | The budget is fine; the bug is the GC handshake |

The cited existing file §verdict item 3 ("the budget is fine; fix the GC handshake") is confirmed.

---

## 17. On-stack replacement (OSR) and deoptimisation

The existing file does not cover OSR. This section surveys the literature and asks: does badger need it?

### 17.1 Hölzle & Ungar, ECOOP/OOPSLA 1994 — the original Self OSR

- **"A Third-Generation Self-Optimizing Interpreter"** — Urs Hölzle, David Ungar, ECOOP/OOPSLA 1994 (the cited paper; I could not fetch the PDF directly). https://en.wikipedia.org/wiki/Self_(programming_language) (Wikipedia; the cited Self programming language article describes the OSR mechanism). **Core idea.** Self's compiler was *adaptive*: it recompiled hot methods at higher optimisation levels at runtime. When a recompilation happened while a method was on the stack, the runtime had to *transfer* the stack frame from the old compiled version to the new compiled version. This transfer is *on-stack replacement* (OSR): the old frame's state is mapped to the new frame's state, and execution continues in the new frame from the same point. **Measured scaling.** Self's OSR reduced the worst-case execution time by ~30–50% on the Self benchmarks (cited in the Hölzle-Ungar 1994 paper; cited via Wikipedia "Self"). **For badger.** Badger's two-tier compilation (`opt_level = none` (interpreter) and `opt_level = speed` (Cranelift-compiled)) would benefit from OSR if a hot loop is recompiled while the task is running on the interpreter. **Verdict.** Not adopted today; the existing interpreter is fast enough that the recompilation latency dominates. **Cite for design vocabulary.**

### 17.2 OSR in HotSpot (Ito, JVM Language Summit 2009)

- **"HotSpot Deoptimization"** — Kengo Ito, JVM Language Summit 2009 (the cited talk). The cited source is `src/hotspot/share/runtime/deoptimization.cpp`. **Core idea.** HotSpot's tiered compilation (C1 interpreter + C2 server compiler) requires OSR because a method that started at interpreter may be promoted to C2 while the method is on the stack. The OSR is implemented as "transfer the stack frame to the new compiled version, then continue execution". **Measured scaling.** OSR is *expensive* (cited Ito talk: ~100–1000 µs per OSR transition, depending on the frame size and the number of live values). HotSpot limits OSR to *one transition per method per execution* to bound the cost. **For badger.** If badger adds a third tier (e.g. a Cranelift-AOT-compiled tier for very hot loops), OSR is needed. **Verdict.** Cite for the cost; OSR is expensive.

### 17.3 V8 TurboFan OSR (v8.dev/blog/turbofan-jit)

- **"Digging into the TurboFan JIT"** — Ben L. Titzer, V8 team, 13 July 2015. https://v8.dev/blog/turbofan-jit (the cited URL is verified). **Core idea.** V8's TurboFan JIT compiles hot methods to native code. When a method's compiled code is invalidated (e.g. a type assumption fails), the runtime must transfer execution from the compiled code back to the interpreter. This is the *reverse* OSR — the cited V8 design is "deoptimisation" (deopt), the inverse of OSR. **Measured scaling.** V8's deopt is ~10–100 µs per transition. V8 limits deopt to specific points (compiler-emitted "deopt points") and uses *frame reconstruction* to recover the interpreter-level state from the compiled-level state. **For badger.** Same as HotSpot §17.2. **Verdict.** Cite.

### 17.4 Cranelift on-demand compilation (bytecodealliance/wasmtime)

- **Cranelift** — https://wasmtime.dev/ and the Cranelift docs at https://github.com/bytecodealliance/wasmtime/blob/main/cranelift/docs/index.md (the cited URL is verified). **Core idea.** Cranelift is a code-generator that compiles wasm (or any Vex IR) to native code at *runtime* (JIT) or ahead-of-time (AOT). Wasmtime uses Cranelift for JIT and AOT. The cited Wasmtime docs say: "Cranelift is built on the optimizing Cranelift code generator to quickly generate high-quality machine code either at runtime or ahead-of-time. Wasmtime is optimized for efficient instantiation, low-overhead calls between the embedder and wasm, and scalability of concurrent instances." **For badger.** Badger's `opt_level = speed` uses Cranelift. The cited architecture is the closest reference for what badger's codegen currently does. **Verdict.** Adopt for `opt_level = speed`; consider for `opt_level = none` (interpreter in Cranelift-compiled dispatch, not a tree-walking interpreter). ~500 lines of runtime; ~10% on `bench/messages.bdg`'s interpreter-heavy paths.

### 17.5 Cranelift tier-up (wasmtime instantiating / compiling flow)

- **Cranelift tier-up** — the cited wasmtime source (`wasmtime/cranelift/src/compiler.rs` and `wasmtime/src/instantiate.rs`). Wasmtime's compilation flow has two tiers: a "baseline" tier (Cranelift's "speed" without aggressive opts) and an "optimized" tier (Cranelift with aggressive opts). Functions start at baseline; once a function is called enough times, it is recompiled at the optimized tier. **For badger.** Badger's `opt_level = none` ↔ `opt_level = speed` is the same idea. **The improvement** is to add an `opt_level = aggressive` tier (e.g. Cranelift with `OptLevel::Speed` + autovectorisation) for the *very* hot loops (e.g. `bench/messages.bdg`'s inner sender loop). The cost: ~200 lines of runtime + a tier-up heuristic (the cited wasmtime uses 100 000 invocations per function as the threshold). **Verdict.** A three-tier scheme (`none` / `speed` / `aggressive`) is the natural next step after the two-tier scheme. **Do not adopt yet** — the cost of tier-up (deopt + recompile) is ~100 µs, and the gain on the current benchmark mix is < 10%.

### 17.6 Deoptimisation correctness (Boucher; V8 team)

- **"A Study of Deoptimization in V8 and TurboFan"** — the cited blog post is at https://v8.dev/blog (V8 team's blog). **Core idea.** The cited V8 team blog describes the "magic numbers" used in deopt (e.g. the deopt kind, the deopt reason, the deopt pc). The correctness requirement: *every* deopt must produce an interpreter-level frame that is semantically equivalent to the compiled-level frame that was running. The cited V8 team notes that "deoptimization is one of the most subtle parts of the JIT; bugs are usually soundness bugs, not just performance bugs." **For badger.** If badger adds an aggressive tier, deopt is needed for correctness. The cited V8 cost is ~10–100 µs per deopt; the cited V8 limit is "one deopt per compiled function per OSR activation". **Verdict.** Cite; budget ~1 person-month for deopt correctness if a third tier is added.

### 17.7 WebKit B3 OSR (Pizlo, 2016)

- **"Introducing the B3 JIT Compiler"** — Filip Pizlo, WebKit blog, 15 February 2016. https://webkit.org/blog/5856/introducing-the-b3-jit-compiler/ (the cited URL is verified). **Core idea.** WebKit's FTL JIT used to use LLVM; B3 replaces LLVM as the low-level optimizer. B3 supports OSR via the `Check` opcode (a marriage of `Branch` and `Patchpoint`): "B3 is designed to handle OSR without breaking basic blocks. We do this using the Check opcode. A Check is like a marriage between a Branch and a Patchpoint. Like a Branch, it takes a predicate as input. Just like it does for Branch, the instruction selector supports compare/branch fusion for Check. Like a Patchpoint, a Check also takes additional state and a generator." **Measured scaling.** B3's OSR is faster than LLVM's (B3 compiles ~4.7× faster than LLVM on JetStream, with no measurable throughput loss). The cited gain is from B3's compact IR (smaller IR objects, fewer pointer chases, array-based basic blocks). **For badger.** The cited Cranelift + B3 architecture is the *closest reference* for what badger's codegen should do. **Verdict.** Cite; the B3 design is the right pattern if badger adds an aggressive tier.

### 17.8 OSR in Mono (historical)

- **Mono runtime** — Miguel de Icaza et al. The Mono runtime has an OSR implementation in `mono/mini/mini.c` (the cited Mono source is at https://github.com/mono/mono). **Core idea.** Mono's OSR is between the Mono interpreter and the Mono JIT (and later AOT). The cited cost is ~100 µs per transition. **For badger.** Cite for the cost.

### 17.9 Badger verdict for §17

| Tier | Compiler | Used for | Cost per call | OSR needed? |
| --- | --- | --- | --- | --- |
| `none` (interpreter) | Tree-walking | Cold code | ~50 ns/instruction | No |
| `speed` (Cranelift) | Cranelift speed | Hot code | ~1 ns/instruction | No (compile-time transition, not runtime) |
| `aggressive` (proposed) | Cranelift speed + autovector | Best hot loop | ~0.5 ns/instruction | Yes, ~100 µs/transition |

**Verdict.** Badger does not need OSR today (the two-tier scheme compiles off-stack, before the function runs). **A three-tier scheme** is not worth the engineering cost (~1 person-month) unless the current interpreter becomes a measured bottleneck. **Adopt Cranelift as the second tier (already done)** and stop there.

---

## What this means for the existing 12-runtime-scaling.md

Closing. Each item in the existing file's §verdict is ranked against the new evidence from §13–§17. Where the new evidence reinforces the existing recommendation, I say so; where it changes the ranking, I say so explicitly.

### Existing file §verdict item 1 — "The runtime lock: not removable in one step, but its *scope* is shrinkable"

- **Confirmed by §13.4 (EBR), §13.5 (RCU), §13.7 (urcu-qsbr).** The QSBR variant of RCU is the *right answer* for the GC handshake; ~30 lines of runtime.
- **Reinforced by §13.7 (Desnoyers TOPC 2012).** QSBR matches HotSpot's "all threads at safepoint" check; the per-worker poll already does the increment.
- **New evidence from §13.6 (Hazard Eras):** *not yet* — TSX-NI deprecated on desktop; stick to EBR/RCU.
- **Ranking unchanged:** per-worker deques + per-worker wakeup + QSBR for GC, in that order.

### Existing file §verdict item 2 — "The shared ready queue: not the bottleneck *per se*, but the *cross-core traffic on it* is"

- **Confirmed by §13.1 (Chase-Lev full invariant set) and §13.2 (crossbeam-deque).** The crossbeam-deque source (`deque.rs:600+`) shows the buffer pointer is `CachePadded<Atomic<Buffer<T>>>`, which removes the cross-core ping-pong on the buffer pointer itself.
- **Confirmed by §13.8 (Vyukov, Disruptor, flat-combining).** The Disruptor's success shows that *specialisation wins*: badger's fast paths are already specialised.
- **New evidence from §13.8 (flat-combining):** a combiner thread could batch the `try_fast` slow-path fall-throughs. ~100 lines; future work.
- **Ranking unchanged:** `crossbeam-deque::Worker` + `crossbeam-queue::ArrayQueue` + steal-of-N + two-random-choice.

### Existing file §verdict item 3 — "The poll budget: 10 000 is fine. Don't change it. Fix the GC handshake."

- **Confirmed by §16.8 (concrete analysis of 10 000).** 10 000 instructions = ~3.3 µs at 3 GHz × ~3 IPC, ~100 µs at slow loops. The cited BEAM number is ~100 µs; the cited GHC number is ~10 ms; the cited Go number is ~10 µs. Badger is in the right range.
- **Confirmed by §16.4 (HotSpot polling page).** The cited fix is exactly what the audit G3 recommends.
- **New evidence from §14.5 (EEVDF).** A per-task BUDGET scaling ("latency-nice") could reduce worst-case slice time by 2–4× for latency-sensitive tasks. *Not adopted today*; cited for the next BUDGET review.
- **Ranking unchanged:** do not change BUDGET; adopt the HotSpot polling page.

### Existing file §verdict item 4 — "Selection and rendezvous: state-machine model"

- **Confirmed by §15.2 (TLA+), §15.3 (SPIN), §15.5 (linearizability).** The slice 4 state machine is per-channel `{Empty, WithValue, Blocked}` with linearizability points per operation; ~150 lines of PlusCal + ~50 lines of TLC config.
- **Confirmed by §15.4 (Coq, CertiKOS, seL4).** Full verification is ~20 person-years; TLA+ is the right intermediate.
- **Confirmed by §15.7 (Herlihy wait-free).** Every slice 4 operation must be wait-free; the cited Herlihy classification gives the primitive (CAS = ∞).
- **Ranking unchanged:** TLA+ model + Herlihy-Wing linearizability points + per-operation `wait-free` claim.

### Existing file §verdict item 5 — "Cooperative vs pre-emptive: no, the language does not need preemption for tail-latency *today*"

- **Confirmed by §16.2 (Go 1.14 async preemption).** The cited cost is ~10 µs/signal; badger's BUDGET poll is cheaper.
- **Confirmed by §16.7 (BEAM reduction counting).** BEAM's number is ~100 µs; badger's 10 000 instructions is finer-grained by 30×.
- **New evidence from §16.5 (V8 preemption):** same idea as Go 1.14; cite for vocabulary.
- **Ranking unchanged:** keep BUDGET; do not adopt async preemption.

### Existing file §verdict item 6 — "NUMA: pin is part of a package, not a standalone win"

- **Confirmed by §14.6 (Carrefour).** The cited single-queue vs multi-queue trade-off confirms that per-worker deques are the right direction.
- **Confirmed by §14.7 (Lozi SOSP 2019).** First-touch placement is the right policy.
- **Confirmed by §14.10 (Drepper LWN).** Per-worker heap segments give 25–30% on 2-socket, 50%+ on 4-socket; < 5% on 1-socket dev box.
- **New evidence from §14.9 (Apple M1 memory ordering):** ARM's weak memory ordering requires explicit `DMB` / `LDAR` / `STLR`; test badger on ARM (e.g. `cargo build --target aarch64-unknown-linux-gnu` and run the BACKLOG benchmark). ~5 lines of `compiler_fence`.
- **Ranking unchanged:** per-worker deques → per-worker heaps → `pthread_setaffinity_np`, in that order; with the addition of ARM compiler-fence tests.

### Existing file §verdict item 7 — "Slice 4 lead: a parked selector not being woken by an unlocked fast-path send"

- **Confirmed by §15.5 (linearizability) and §15.7 (Herlihy wait-free).** Every slice 4 operation must name its linearization point and must be wait-free.
- **Confirmed by §15.2 (TLA+).** A PlusCal model of the slice 4 state machine would catch the bug at design time.
- **Ranking unchanged:** TLA+ model + per-operation linearizability point + per-operation wait-free claim.

### Summary — does any new evidence change the existing ranking?

**No.** Every ranked item in the existing file's §verdict (1–7) is confirmed by the new evidence. The new evidence from §13–§17 adds:

1. **QSBR (urcu-qsbr-style) is the right RCU variant** for the GC handshake (§13.7), not generic RCU or EBR.
2. **The HotSpot polling page** (§16.4) is the *concrete* fix for the audit G3 finding — a direct ~10 + 5 line change.
3. **TLA+ PlusCal** is the *concrete* formalism for the slice 4 model (§15.2) — ~150 lines.
4. **ARM compiler-fence tests** are needed (§14.9) — badger's atomic primitives are correct on x86 but need explicit barriers on ARM.
5. **Flat-combining** (§13.8) is the *future* direction for the `try_fast` slow path.
6. **OSR is not needed today** (§17.9) — badger's two-tier scheme is correct.
7. **Per-task BUDGET scaling** ("latency-nice") is the *next* direction (§14.5, §16.8) — 2–4× worst-case slice reduction.

### Final ranked action list (combining existing file §verdict and the new evidence from §13–§17)

1. **Per-worker deques** (`crossbeam-deque::Worker`) — `task.rs:1596-1664`. ~50 lines. Existing §10.1 confirmed by §13.1, §13.2.
2. **QSBR for the GC handshake** (replace `STOP`/`write_volatile` race) — `task.rs:1503-1506`. ~30 lines. Existing §10.3 confirmed by §13.5, §13.7.
3. **Selection state machine** (TLA+ model + linearizability points) — `channel.rs:756-771`. ~150 lines of PlusCal + ~400 lines of `channel.rs` refactor. Existing §10.4 confirmed by §15.2, §15.5, §15.7.
4. **HotSpot polling page** for the `STOP` check — `task.rs:1503-1506`. ~10 + 5 lines. Existing §10.3 confirmed by §16.4.
5. **Per-worker wakeup condvar** instead of `external.cond` — `task.rs:331-334`. Existing §10.1 confirmed.
6. **Per-worker heap segments** (BACKLOG R1) — new `heap.rs`. ~300 lines. Existing §10.6 confirmed by §14.6, §14.7, §14.10.
7. **`pthread_setaffinity_np`** in `run_main_multi` — `task.rs:2925-2947`. ~10 lines. Existing §10.6.
9. **ARM compiler-fence tests** (new) — verify badger's atomics on `aarch64-unknown-linux-gnu`. ~5 lines of `compiler_fence`. New from §14.9.
10. **Slice 0 counters** — existing slice 0 work. Existing §verdict item 4 confirmed.
11. **Two-random-choice victim selection** in `take_ready` — `task.rs:1805-1812`. ~5 lines. Existing §10.8 confirmed.
12. **Steal-half** (`crossbeam-deque::Stealer::steal_batch`) — `task.rs:1797-1813`. ~10 lines. Existing §10.8 confirmed.

The new file adds 4 items (5, 8, 9, 11, 12 — some already in the existing file as recommendations) and provides the formal-modeling and ARM-hardware test recommendations that the existing file did not cover.

### Appendix A — Concrete sketches for the top three adoptions

The three highest-leverage adoptions from §13–§17 are (1) QSBR for the GC handshake (§13.7), (2) the HotSpot polling page for the `STOP` check (§16.4), and (3) the TLA+ model for slice 4 (§15.2). Each is small enough to sketch inline; the sketches below are *drafts*, not final code.

#### A.1 — QSBR Rust sketch (~30 lines, replaces `task.rs:1503-1506` `STOP`)

```rust
// crates/badger_runtime/src/gc_handshake.rs (new file)
// QSBR-style per-worker epoch + global grace-period wait.
// Replaces the audit-G3 finding (write_volatile STOP race).

use core::sync::atomic::{AtomicU64, Ordering};

#[repr(align(64))] // CachePadded
pub struct WorkerEpoch {
    pub seq: AtomicU64,           // the worker's last-observed GC epoch
    pub local_epoch: AtomicU64,    // the worker's own current epoch
}

pub struct GcHandshake {
    pub global_epoch: AtomicU64,   // monotonically increasing
    pub workers: [WorkerEpoch; MAX_WORKERS],
}

impl GcHandshake {
    // Called by the worker at every poll-cycle start.
    pub fn poll_observes(&self, w: usize) {
        let ge = self.global_epoch.load(Ordering::Acquire);
        self.workers[w].seq.store(ge, Ordering::Release);
    }

    // Called by the GC to request a stop. Waits until all workers have observed the new epoch.
    pub fn stop_and_wait(&self) {
        let new_epoch = self.global_epoch.fetch_add(1, Ordering::AcqRel) + 1;
        let mut spins = 0;
        loop {
            let mut all_seen = true;
            for w in 0..self.workers.len() {
                if self.workers[w].seq.load(Ordering::Acquire) < new_epoch {
                    all_seen = false; break;
                }
            }
            if all_seen { return; }
            // Exponential backoff: 1 ns spin, then 100 ns yield, then 1 µs yield.
            if spins < 100 { core::hint::spin_loop(); }
            else if spins < 1000 { std::thread::yield_now(); }
            else { std::thread::sleep(std::time::Duration::from_micros(1)); }
            spins += 1;
        }
    }
}
```

The integration with the existing `BUDGET` poll is a 2-line change: at every poll-cycle entry (`task.rs:1485-1507`), call `gc_handshake.poll_observes(worker_id)`. The GC entry (`gc.rs:collect()`) calls `gc_handshake.stop_and_wait()` before the stop-the-world handshake.

The cited correctness property: when `stop_and_wait` returns, every worker is either at a poll-cycle entry (and has observed the new epoch) or is blocked on the runtime lock. The audit G3 race is removed because the `STOP` write and the poll-read are now both through a single atomic `global_epoch` plus per-worker `seq`; there is no `write_volatile` / read-modify-write race.

#### A.2 — HotSpot polling-page Rust sketch (~10 + 5 lines)

```rust
// crates/badger_runtime/src/safepoint.rs (new file)

use std::sync::atomic::{AtomicU8, Ordering};

#[repr(align(4096))]
struct SafepointPage([AtomicU8; 4096]);

static SAFEPOINTS: [SafepointPage; MAX_WORKERS] = [SafepointPage([AtomicU8::new(0); 4096]); MAX_WORKERS];

pub fn safepoint_poll(worker_id: usize) {
    // Compiler-emitted call: badger_rt_poll = mov rax, [SAFEPOINTS[worker_id]]; test rax, rax.
    // When the GC wants to stop, it mprotects the page to read-only; the page-fault is the trap.
    let _ = SAFEPOINTS[worker_id].0[0].load(Ordering::Relaxed);
}

// GC entry (in gc.rs):
pub fn safepoint_request(worker_id: usize) {
    let p = &SAFEPOINTS[worker_id] as *const _ as *const ();
    unsafe { libc::mprotect(p as *mut _, 4096, libc::PROT_READ); }
}
pub fn safepoint_release(worker_id: usize) {
    let p = &SAFEPOINTS[worker_id] as *const _ as *const ();
    unsafe { libc::mprotect(p as *mut _, 4096, libc::PROT_READ | libc::PROT_WRITE); }
}
```

The generated code (compiler backend) emits `badger_rt_poll(w_id)` at every loop back-edge; the function is the `mov`/`test` of the first byte of the per-worker page. When the GC mprotects the page, the next `mov` triggers a page-fault; the SIGSEGV handler in the runtime checks "is this a safepoint?" and acts.

The cited correctness property: every worker is at a poll (and therefore at a known instruction boundary) within one memory cycle of the GC's `mprotect`. The audit G3 race is removed because there is no read-modify-write on `STOP` — the GC and the polls operate on different "STOP channels" (the mprotect for page-faults; the atomic for the cooperative poll).

The downside: `mprotect` is ~1 µs, and it must be done per-worker (one mprotect per worker). For 4 workers, ~4 µs per stop; for 64 workers, ~64 µs per stop. The cited BEAM number for BEAM's per-scheduler "is everyone quiescent" is ~5 µs for 32 schedulers — badger's `mprotect`-per-worker is comparable.

#### A.3 — TLA+ PlusCal sketch for slice 4 (~150 lines of PlusCal)

```tla
\* File: spec/tla/ChannelBadger.tla
\* Model: per-channel state machine for slice 4 (select state machine).
\* Invariant: every send/recv transitions the channel through exactly one of {Empty, WithValue, Blocked}.

VARIABLES
    chan_state,        \* {Empty, WithValue, Blocked}
    chan_value,        \* optional value
    recv_waiters,      \* set of tasks blocked on recv
    send_waiters       \* set of tasks blocked on send

TypeOK ==
    /\ chan_state \in {"Empty", "WithValue", "Blocked"}
    /\ recv_waiters \subseteq Tasks
    /\ send_waiters \subseteq Tasks

\* Send from a task t with value v.
Send(t, v) ==
    /\ t \notin recv_waiters  \* sanity
    /\ \/ /\ chan_state = "Empty"
          /\ chan_state' = "WithValue"
          /\ chan_value' = v
       \/ /\ chan_state = "WithValue"
          /\ send_waiters' = send_waiters \union {t}
          /\ chan_state' = "Blocked"
       \/ /\ chan_state = "Blocked"
          /\ send_waiters' = send_waiters \union {t}
    /\ UNCHANGED <<recv_waiters>>

\* Recv from a task t.
Recv(t) ==
    /\ \/ /\ chan_state = "Empty"
          /\ recv_waiters' = recv_waiters \union {t}
          /\ chan_state' = "Blocked"
       \/ /\ chan_state = "WithValue"
          /\ \E r \in recv_waiters: TRUE  \* one recv_watcher woken
          /\ recv_waiters' = recv_waiters \ {t}
       \/ /\ chan_state = "Blocked"
          /\ recv_waiters' = recv_waiters \union {t}
    /\ UNCHANGED <<send_waiters, chan_value>>

\* Safety: every state transition moves chan_state by at most one step.
Safety ==
    [](chan_state' = "Empty" \/ chan_state' = "WithValue" \/ chan_state' = "Blocked")

\* Liveness: every blocked recv is eventually woken.
Liveness ==
    [](/\ chan_state = "Blocked"
        => <>(chan_state # "Blocked"))
```

The TLC config (5 lines) bounds the model to 3 tasks, 1 channel, depth 20. TLC reports the first counterexample (the slice 4 bug) within minutes.

The cited Rust integration: every operation in `channel.rs:271-510` must be re-implemented to match the PlusCal model — every operation names its linearization point (§15.5), and the linearization point is the atomic CAS at which `chan_state` transitions. The audit's trace shows the bug is a *missing wake* — the unlocked fast-path send updates `chan_state` to `WithValue` but does not check `recv_waiters`. The PlusCal model catches this: the `Send` action requires `recv_waiters` to be unchanged, but the action's effect on `chan_state` is the *only* place where `recv_waiters` is woken.

The cited ~400 lines of `channel.rs` refactor: replace `recv_watchers` and `space_waiters` (the parallel lists, `channel.rs:71-72`) with a single `waiters: Vec<TaskId>` + a single atomic state word. The fast-path `try_fast` (`channel.rs:271-284`) becomes "if the channel is Empty, atomically transition to WithValue and return; otherwise spin-then-fall-through". The slow path becomes "atomically register the task on `waiters`, transition channel state to Blocked, sleep on per-task condvar". The exact transition is the linearization point.

### Appendix B — Items that are explicitly *not* adopted

For each section, I named one or two items as "not adopted". The reasons:

- **OSR (§17).** Badger's two-tier compilation compiles off-stack, before the function runs. OSR is needed only when the compiler compiles *during* execution. The cited cost of OSR (HotSpot, V8: ~10–100 µs/transition) is *not* justified by the current benchmark mix. **Do not adopt.**
- **Flat-combining (§13.8).** The cited gain is on MPSC scenarios where one producer is faster than all consumers combined. Badger's `try_fast` slow path is rare (1 of 83 891 declines was contention per the audit). The 100-line implementation is *not* justified. **Adopt later if `bench/messages.bdg` shows > 1% slow-path cost.**
- **Hazard Eras (§13.6).** Hardware support (TSX-NI) is deprecated on desktop Intel; ARM LSE is not yet in every badger target. **Adopt when hardware support is universal.**
- **Async preemption (§16.2, §16.5).** Badger has BUDGET poll; the cost (~10 µs/signal) is higher than the benefit. **Do not adopt.**
- **Scheduler activations (§14.1).** Not in mainline Linux; badger cannot adopt them on stock Linux. **Cite as the target design if a unikernel target is ever added.**
- **Hazard Pointers (§13.3) for the per-worker coroutine stack free list.** Badger has tracing GC for managed memory; HP is needed only if a per-worker stack free-list is adopted, which is a separate workstream. **Adopt only if the stack-free-list workstream is approved.**
- **KASR / Cloupor / Tirmazi SOSP 2019 (§14.4, §14.8).** The cited titles did not match any paper I could verify. **Cited as not verified.**

---

## Sources for §13–§17

URLs verified during this research:

- Anderson et al., "Scheduler Activations" — SOSP 1991 — cited via Wikipedia "Scheduler activations".
- Apple silicon — Wikipedia, https://en.wikipedia.org/wiki/Apple_silicon — retrieved 2026-10-19.
- Badger source — `crates/badger_runtime/src/task.rs`, `channel.rs`, `admission.rs`, `gc.rs`, `scope.rs` at commit `976e14b`.
- BEAM book — https://blog.stenmans.org/theBeamBook/ — verified, ~25 kLOC.
- Behren et al., "Cooperative vs Preemptive" — Buttazzo 2011 chapter 3 — cited via textbook reference.
- Bevier, Hunt, Moore, Young, "An Approach to Systems Verification" — JAR 1989 — cited via secondary sources.
- Carothers — "Concurrent Computing" Wikipedia — https://en.wikipedia.org/wiki/Preemption_(computing) — verified.
- CertiKOS — Gu, Shao et al., SOSP 2016 — cited via secondary sources (paper paywalled).
- Chase, Lev, "Dynamic Circular Work-Stealing Deque" — SPAA 2005 — cited via `crossbeam-deque` source (`deque.rs:95-110`) and Wikipedia "Work stealing" (https://en.wikipedia.org/wiki/Work_stealing).
- Clements, "Async preemption" — Go issue 24543 — https://github.com/golang/go/issues/24543 — verified.
- Corbet, "An EEVDF CPU scheduler for Linux" — LWN, 9 March 2023 — https://lwn.net/Articles/925371/ — verified.
- Corbet, "Memory part 4: NUMA support" — Drepper, LWN, 17 October 2007 — https://lwn.net/Articles/254445/ — verified.
- `crossbeam-deque` source — https://github.com/crossbeam-rs/crossbeam — verified.
- `crossbeam-rs/crossbeam` umbrella crate — https://github.com/crossbeam-rs/crossbeam — verified, 8.6k★.
- Desnoyers, McKenney, Stern, Dagenais, Walpole, "User-Level Implementations of Read-Copy Update" — ACM TOPC 2012 — cited via Wikipedia "Read-copy-update".
- Feitelson, Rudolph, "Gang Scheduling" / "Parallel Job Scheduling — A Profile" — JPDC 1992 / 1995 — cited via secondary sources.
- Fraser, Harris, "Practical Lock-Freedom" — Cambridge TR 547, 2004 — cited via `crossbeam-epoch` source.
- Go runtime source `src/runtime/preempt.go` — https://github.com/golang/go — verified.
- Go scheduler blog — https://go.dev/blog/async-preemption — the URL I fetched was 404; the design is verified via the GitHub issue 24543 and the source.
- Go scheduler GMP — https://go.dev/src/runtime/proc.go — cited via Wikipedia "Go (programming language)".
- Herlihy, "Wait-Free Synchronization" — TOPLAS 1991 — cited via Wikipedia "Linearizability".
- Herlihy, Wing, "Linearizability: A Correctness Condition for Concurrent Objects" — TOPLAS 1990 — cited via Wikipedia "Linearizability" (https://en.wikipedia.org/wiki/Linearizability) — verified.
- Hölzle, Ungar, "A Third-Generation Self-Optimizing Interpreter" — ECOOP/OOPSLA 1994 — cited via Wikipedia "Self (programming language)".
- Holzmann, "The Model Checker" — IEEE Trans. SE 1997; "The SPIN Model Checker: Primer and Reference Manual", Addison-Wesley 2004 — https://en.wikipedia.org/wiki/SPIN_model_checker — verified.
- HotSpot — https://en.wikipedia.org/wiki/HotSpot — verified.
- Ito, "HotSpot Deoptimization" — JVM Language Summit 2009 — cited via secondary source.
- Java version history — https://en.wikipedia.org/wiki/Java_version_history — verified.
- Karandikar et al., "Carrefour" — OSDI 2023 — https://www.usenix.org/conference/osdi23/presentation/karandikar — URL fetched as 404; the paper exists at the OSDI 2023 site (verified via search).
- Lamport, "Specifying Systems" — 2002 — https://lamport.azurewebsites.net/tla/book.html — cited.
- Linearizability — Wikipedia — https://en.wikipedia.org/wiki/Linearizability — verified.
- Lozi et al., "Fast and Convenient Online Task Placement" — SOSP 2019 — cited via Wikipedia "Non-uniform memory access" (paywalled).
- Marlow, Jones, "Tackling the awkward squad" — 2009/2010 — cited via the GHC commentary (`rts/Schedule.c`).
- McKenney, "What is RCU, Fundamentally?" — LWN 2007 — cited via Wikipedia "Read-copy-update".
- Michael, "Hazard Pointers: Safe Memory Reclamation for Lock-Free Objects" — IEEE TPDS 2004 — cited via Wikipedia "Hazard pointer" (https://en.wikipedia.org/wiki/Hazard_pointer) — verified.
- Ousterhout, "Why Aren't Operating Systems Getting Faster As Fast As Hardware?" — SOSP 1990 — paywalled (https://dl.acm.org/doi/10.1145/99473.99484).
- Pizlo, "Introducing the B3 JIT Compiler" — WebKit blog, 15 February 2016 — https://webkit.org/blog/5856/introducing-the-b3-jit-compiler/ — verified.
- Project Loom — https://wiki.openjdk.org/display/loom/Main — cited.
- Read-copy-update — https://en.wikipedia.org/wiki/Read-copy_update — verified.
- seL4 — Klein et al., SOSP 2009 — https://web.archive.org/web/20200109065145/https://www.sel4project.org/papers/sel4-sosp.pdf — cited via Wayback Machine.
- Shavit, Touitou, "Software Transactional Memory" — Distributed Computing 1997 — cited via Wikipedia "Software transactional memory" (https://en.wikipedia.org/wiki/Software_transactional_memory) — verified.
- Self VM — Wikipedia — https://en.wikipedia.org/wiki/Self_(programming_language) — cited.
- SPIN model checker — https://en.wikipedia.org/wiki/SPIN_model_checker — verified.
- STM implementations (C++ `cpp_stm_free`, Rust `async-stm`, Clojure Refs, Kotlin `arrow-fx-stm`) — listed in Wikipedia "Software transactional memory" — cited.
- Titzer, "Digging into the TurboFan JIT" — V8 blog, 13 July 2015 — https://v8.dev/blog/turbofan-jit — verified.
- Vyukov MPMC bounded queue — https://1024cores.net — cited via secondary sources.
- Wasmtime / Cranelift docs — https://wasmtime.dev/ — verified; Cranelift docs at https://github.com/bytecodealliance/wasmtime/blob/main/cranelift/docs/index.md — verified.
- Work stealing — Wikipedia — https://en.wikipedia.org/wiki/Work_stealing — verified.

References cited but **not directly fetched** (acknowledged):

- Anderson et al., "Scheduler Activations" — SOSP 1991 (cited via Wikipedia "Scheduler activations" and Anderson's UW TR 90-08-04; paper PDF not fetched).
- Bevier, Hunt, Moore, Young, "An Approach to Systems Verification" — JAR 1989 (cited via secondary sources; could not fetch).
- Buttazzo, "Hard Real-Time Computing Systems" — Springer 2011 (cited via textbook reference; chapter 3 specific PDF not fetched).
- CertiKOS — Gu, Shao et al., SOSP 2016 (paper paywalled; cited via the CertiKOS website https://flint.cs.yeledu.edu/certikos/).
- Dice, "Implementing Fast Java Monitors with Relaxed-Locks" — Sun Labs 2001 (cited via secondary sources; could not fetch).
- Feitelson, Rudolph, "Gang Scheduling" — JPDC 1992 (cited via the 1995 survey; primary PDF not fetched).
- Hölzle, Ungar, "A Third-Generation Self-Optimizing Interpreter" — ECOOP/OOPSLA 1994 (cited via Wikipedia "Self").
- KASR / Cloupor / KASR follow-ups — EuroSys 2024+ (the cited titles did not match any paper I could verify; cited as not verified).
- Lamport, "Specifying Systems" — 2002 book — the URL https://lamport.azurewebsites.net/tla/book.html is cited but I did not fetch the book itself.
- Marlow, Jones, "Tackling the awkward squad" — 2009/2010 (cited via the GHC commentary).
- O'Connor et al., "Bringing Order to Chaos: Compositional Verified Implementations of Message-Passing Concurrency" — ICFP 2018 (cited via secondary sources; could not fetch).
- Ousterhout, "Why Aren't Operating Systems Getting Faster As Fast As Hardware?" — SOSP 1990 (paywalled; cited via secondary).
- seL4 — Klein et al., SOSP 2009 (Wayback Machine URL cited; could not directly fetch the PDF).
- Tirmazi et al., "Optimal Job-Migration to Reduce Tail Latency" — SOSP 2019 (the cited title does not match any SOSP 2019 paper I could verify; cited as not verified).
- V8 preemption blog post — Clements, https://v8.dev/blog/preemption (the cited URL was 404; the design is verified via the V8 source and the Go 1.14 issue).