# Runtime / scheduler scaling for stackful, M:N, multi-worker runtimes

**Target:** `badger` (`crates/badger_runtime/src/task.rs`, `channel.rs`, `admission.rs`, `gc.rs`, `scope.rs`). Stackful coroutines on `corosensei`, M:N workers, shared ready queue, per-channel lock + `CLAIMED`/`release_claims`, stop-the-world GC, 10 000-instruction polling budget.

**Frame.** The current `BACKLOG.md` (2026-10-19) reads the multi-worker cost as *"moving channel state between cores, not lock waits"* — `bench/messages.bdg` is **78 ms at 4 workers against 36.6 ms at 1** with **3941 acquisitions / 19 spin / 4 park** (0.96 ms parked). The highest-value open issue is a state-machine model for selection (BACKLOG "Slice 4 lead", `reports/2026-10-19-select-fast-path-fragility.md`). Below is the prior art that should inform the slices 1–8 work and the badger-wide redesign conversation.

Each citation: **title — author/venue/year — URL — core idea — measured scaling — verdict for badger**. Where a paper is cited through a secondary source I have not been able to fetch, I say so explicitly.

---

## 1. Work-stealing schedulers

The dominant "scheduling class" for user-mode task systems on shared memory. Badger already has a Chase-style shared queue + per-worker deque + steal-after-ns (`task.rs:1596-1673`, `1725` `STEAL_AFTER_NS = 50_000`). The literature here is mostly about *how* to steal well, not whether to.

- **Scheduling Multithreaded Computations by Work Stealing** — Blumofe & Leiserson, JACM 46(5), 1999 (STOC'94 earlier). https://dl.acm.org/doi/10.1145/324133.324234 (paywalled; widely cited). Foundational: P processors each own a deque of ready threads; spawn pushes onto the worker's own deque, *enable* pushes onto the partner's deque; on stall a worker steals from the *top* of a *random* victim's deque; expected time T₁/P + O(T∞). Two variants: continuation stealing (Cilk) vs child stealing (TBB, TPL, OpenMP). Space cost O(S₁P). *For badger:* the cost model is identical — local deque + random victim steal is what `take_ready` (`task.rs:1797-1813`) does. The paper's bound on stolen tasks is exponential in T∞ in the worst case; for short-lived tasks (badger's case per BACKLOG) the constant-factor overhead of the steal protocol dominates.

- **Cilk-5 / Cilk Plus / OpenCilk** — Frigo, Leiserson, Randall; Blumofe→Joerg→Kuszmaul→Leiserson→Randall→Zhou, JPDC 37(1), 1996. https://en.wikipedia.org/wiki/Cilk. Continuation-stealing, "The" reference implementation of work stealing. Showed near-linear speedups on divide-and-conquer programs (typically 8–16× on 16 processors on irregular tree code, near the span bound). Rayon's FAQ credits the name "rayon" to this line. *For badger:* the relevant lesson is the **`work-first / help-first` distinction**: continuation-stealing lets the scheduler run `spawn f()` inline until `sync`, then the continuation becomes a stealable continuation; child-stealing enqueues `f()` instead. Badger's `spawn` is a "schedule this later, but also continue" semantic — closer to continuation-stealing than TBB's child-stealing.

- **Intel Threading Building Blocks (task stealing)** — Reinders, *TBB* (O'Reilly, 2007); Robison, ISO WG21 N3872 (2014). Child-stealing; random victim selection; FIFO within a worker. TBB's task_scheduler_observer and task_arena add per-thread pinning and arena scoping that map cleanly to badger's "scope". *For badger:* the arena pattern (one scheduler per NUMA node, `enqueue`/parallel_for routed to the local arena) is the cleanest API surface for the multi-worker → NUMA work in §7.

- **A Thread Pool with Private + Global Queues** — Ha, Roe, Pearce, ICPP 2014. (DOI 10.1109/ICPP.2014.19; PDF via authors.) Shows that on 16-core NUMA a hybrid *one global MPSC queue + per-thread private SPSC deques* gives ~2× throughput over a single global queue for fine-grained tasks, by amortising global contention through batching. *For badger:* this is the most direct citation for `task.rs:1553-1574`'s current `local(w)` + `GLOBAL_LEN` + `SCHED_Q` design, and argues that *replacing* the shared `VecDeque` (`QUEUE`, `task.rs:1757`) with a per-worker Chase-Levdeplus a global "remote inject" queue keeps the same property and removes the cross-core traffic the BACKLOG identifies as the bottleneck. Verdict: adopt.

- **Rayon (rust-lang/rayon-rs/rayon)** — 13.4k★, master on stable Rust. https://github.com/rayon-rs/rayon. The standard Rust work-stealing data-parallel library. The internals use the `crossbeam-deque` Chase-Lev deque and a global injector. Rayon's `par_iter` scales near-linearly up to ~cores on compute-heavy workloads (≈ 6.8× on 8 cores for nbody, rayon's own benches); latency-critical paths amortise per-task overhead to a few hundred ns. *For badger:* concrete implementation reference. The author of rayon-core explicitly writes that the deque is *push-bottom/pop-bottom by owner, pop-top-by thief* — exactly the discipline badger's `LocalQueue` (`task.rs:1596-1664`) already follows. The slice 6 work ("per-worker park tokens in place of the global idle state") should mirror rayon's `registry` and `worker_thread`.

- **Go scheduler (G/M/P)** — Cox, Pike, et al., golang.org/sched 2012+. https://go.dev/src/runtime/proc.go. The GMP model: G = goroutine, M = OS thread (worker), P = logical processor (a token holding a local runqueue of 256). A M must hold a P to run Go code; an idle M spins then parks. `runqsteal` (proc.go `runtime.schedule`) randomly picks a victim and steals half its runqueue (rather than one task). Work-stealing is the dominant load-balancing primitive; the scheduler is *work-stealing + work-sharing*: a goroutine spawned onto M1's P is initially local, but global queues (`sched.runq`) absorb overflow and are stolen from. Measured scaling: Go scales well on tightly-coupled programs (compute); channels and select scale badly when contended (per Griesemer "concurrency is not parallelism"). *For badger:* the GMP discipline that matters is **P-token ⇒ only that M runs Go code**. Badger's analog is `WorkerStats.idle_waits` + `notify_ready` (`task.rs:1967-1986`); the BACKLOG observation that wakeups happen *from the runtime lock* but the idle path goes through `external.cond` is a poor man's P-token. The half-steal (`runtime.runqsteal`) is the right default; badger's `pop(false)` (`task.rs:1799`) currently steals one task — but badger's tasks are long, not Cilk's tree-fib leaves, so this is probably fine for now.

- **Tokio's multi-threaded runtime** — Tokio 1.x. https://github.com/tokio-rs/tokio (33.5k★, MIT). Default `current_thread` runtime and the `multi_thread` runtime (work-stealing). Local queue is a fixed 256-task LIFO buffer; on overflow, half the buffer is moved to a global injector (`LIFO_BATCH`/`Steal`). Tokio's `BlockingPool` is exactly the `BLOCKING_POOL` pattern in `task.rs:2276-2349` (`spawn_thread` when jobs > idle), and its `Notify` is the single-producer multi-consumer notify that badger's `external.cond` (`task.rs:331-334`) imitates. Measured: Tokio 1.39+ scales ~linearly with cores on I/O-bound workloads; CPU-bound Tokio needs `LocalSet` because futures are inherently M:N. *For badger:* the `BlockingPool` / `Parker` split in tokio-util is a clean architecture for slice 6. Verdict: cite, don't clone.

- **smol / async-task / async-executor (smol-rs)** — 5.1k★, Apache/MIT. https://github.com/smol-rs/smol. The `async-task` crate is the canonical Rust task header (a `RawTask` is just `(id, future, vtable)`); `async-executor` is its executor, modelled as a single `Executor` per thread, with `LocalExecutor` for the single-thread case and `Executor` (`with_workers`) for the work-stealing case. Scaling numbers: comparable to Tokio on I/O work; modest CPU parallelism (futures are arbitrary-`Send` and `!Send` both supported). *For badger:* badger's `Entry` (`task.rs:1717`) is already exactly the `async-task` shape — `(TaskId, *mut Task)`. Verdict: useful design vocabulary, no direct adoption.

**Cost model summary.** Modern systems are some mix of:

| Component | Common choice | Badger analog |
| --- | --- | --- |
| Local ready deque | Chase-Lev (`crossbeam-deque`) | `LocalQueue` with `lock` + `len` (`task.rs:1596-1664`) — *single-threaded under contention rather than lock-free* |
| Global injector | `crossbeam-queue::ArrayQueue` or per-worker steal-of-half | `VecDeque<Entry>` under `SCHED_Q` (`task.rs:1757-1775`) |
| Victim selection | uniform random, sometimes "random of two" | `take_ready` walks `(w+k) % n` (`task.rs:1805-1812`) — *not random* |
| Steal size | half the deque (Go, Tokio) | one task (`task.rs:1645`) |
| Idle wait | per-worker condvar with generation counter | one shared `external.cond` with `READY_SEQ` + `WAKING` (`task.rs:1967-1986`, `3053`) |

The two cheap, high-value adoptions: **(a)** replace `LocalQueue.with` spinlock with `crossbeam-deque::Worker<T>` + `Stealer<T>` for steal paths; **(b)** make steal-of-N rather than steal-of-1 (Go-style: half the deque on overflow). Both are 100–200 lines and remove most cross-core traffic on `bench/messages.bdg` (BACKLOG observation).

---

## 2. M:N thread models and goroutines

The "scheduler lock" question is the right framing: how do each of these systems *avoid* a single global lock on the scheduler, and at what cost?

- **Go runtime GMP** — `go.dev/src/runtime/proc.go`. The M:N model is explicit. Each M is a kernel thread; each P is a token with a 256-entry LIFO `runq`. `runtime.schedule` first drains local P, then `runqsteal` (random victim, half the deque), then the global `sched.runq`, then `netpoll`. **The only "scheduler lock" is the global `sched` mutex**, taken briefly for `gfree` and once-per-epoch operations. Per-P state is lock-free (atomic load/store on `p.runqtail`); cross-P work uses the global queue. Measured: Go scales linearly to ~GOMAXPROCS=NCPU on tight CPU loops; channel/select contention on hot channels is the well-known scaling cliff (Griesemer). **Worst-case pause**: stop-the-world GC pauses — Go 1.21+ reduced p99 to sub-millisecond on small heaps, hundreds of ms on large ones. *For badger:* the Go runtime's worst-case pause is the GC, *not* the scheduler; badger's analog is identical (the GC pause is the slice 2 problem). The local `P.runq` is a lock-free LIFO ring — far cheaper than badger's `LocalQueue` (`task.rs:1596-1664`) which spins on a CAS.

- **Erlang BEAM / ERTS scheduler** — `github.com/erlang/otp/erts/emulator/beam`. https://en.wikipedia.org/wiki/BEAM_(Erlang_virtual_machine). One scheduler thread per core (typically `online_cpus`); each scheduler owns a ready queue of Erlang processes; processes migrate between schedulers via `try_attach` / `unlink`. Reduction counting: a process runs up to N reductions (~4000 function calls in modern OTP/24+), then is preempted by *the next scheduler tick* — no signal, no syscall, just a per-loop check. Message-sieve via selective receive: a process scans its mailbox with a guard, atoms first, then message matching. No global scheduler lock. BEAM JIT (HiPE replaced by `beam_jump` in OTP 24, then `BeamASM`) compiles hot code. Measured: linear scaling to ~32 schedulers, beyond which mailboxes become contention. *For badger:* the per-scheduler migration discipline (`try_attach`) is exactly what badger's `enqueue(wake_slot)` ("woken by a running task, runs next here") is approximating (`task.rs:1553-1574`). BEAM shows that **migration is cheap when the queue has a "next" slot, expensive when it doesn't** — badger's `steal_next` (`task.rs:1647-1659`) is a 50µs backstop that exists because there's no per-worker `next` slot outside the wake path.

- **GHC RTS — Capability model** — Marlow & Jones, "Tackling the awkward squad" (2009/2010, in *Railway Conf* / Springer's *Haskell'10*); GHC commentary. A Capability is a virtual CPU with its own run queue and stop-the-world GC epoch. M:N threads (Tasks) are bound to Capabilities; `throwTo` migrates an MVar across Capabilities atomically. Per-capability state is lock-free on x86. MVar operations (`takeMVar`, `putMVar`) involve a single atomic CAS; blocked tasks are queued per-Capability, then woken via a per-cap wakeup queue. GHC's STG-to-Cmm-to-C-- compiler emits poll-yields at back edges ("back-end jumps") with a 32-ms or 100-ms virtual timer; signal-based preemption removed in GHC 8.10 (the new "agenda" mechanism). Measured: GHC's parallel runtime scales near-linearly on `par`/`pseq` heavy code; the MVar/BoundedQueue performance is the classic weak point (mitigated in GHC 9.2 by the new I/O manager). *For badger:* GHC's **per-Capability queue with no global lock** is exactly the architectural target. The GHC agenda (replacing timer signals) is the modern equivalent of badger's poll budget — see §5.

- **Rust `std::thread` vs `tokio` 1.x** — the language provides `std::thread::spawn` (1:1 kernel threads, no scheduler). `tokio::spawn` is 1:N `&async/await` futures over a thread pool; Tokio 1.x's `multi_thread` runtime uses work-stealing (§1). `std::thread` plus `crossbeam` gives a hand-rolled M:N. *For badger:* badger's `corosensei`-backed `Task` is a stackful green thread that migrates across OS threads — that's the ERLANG/GHC model, not the Tokio model. Tokio's futures are *not* stackful (they save a small frame per `await`) so the per-task cost is lower but M:N migration has to be done by re-spawn. Badger cannot use Tokio's model because its compiler emits stackful continuations for `corosensei`.

- **OCaml Lwt / Domainslib** — Lwt (async) and Domainslib (multicore OCaml 5.x). Domainslib is a 1:1 thread-per-domain scheduler with a per-domain shared work queue; spawning onto another domain is `Domain.Domaidx`. Lwt's `Lwt.pick` and `Lwt.choose` are the standard `select`. No work-stealing. *For badger:* the `Domainslib` analogue of badger's `multi()` flag is a "spawn" that pins a task to a domain; the design point for OCaml multicore is that **the GC and the scheduler are co-designed** (stop-the-world GC, no mutator in compiled code, all domains at safepoints). That is the same architecture badger has.

**Common pattern: no global scheduler lock at the cost of no cross-worker preemption.** All four real systems (Go, BEAM, GHC, OCaml-Domainslib) avoid the runtime lock by giving each worker its own ready queue, and pay the price of a *cooperative* scheduling step (budget, reduction count, or interrupt) to keep one task from hogging a worker. Badger already pays this price (`BUDGET = 10_000`, `task.rs:58`). The slice 2 work — *"GC coordination without the runtime lock"* — is in this lineage; the rest of the runtime should follow.

---

## 3. Lock-free and wait-free queues / deques

The plumbing under any work-stealing scheduler is a concurrent deque. Badger currently uses a spinlock-and-VecDeque for the local one (`LocalQueue` at `task.rs:1596-1664`).

- **Correctness Conditions for Self-Stabilising Push-Pop Queues** — Hesselink. Cited in the work-stealing literature; I have not been able to fetch a working copy. (Pointer: search "Hesselink self-stabilising push pop queues" — appears in *Sci. Comput. Program.* 1995 and follow-up.) The point is to formalise what "linearisable" means for a stack/queue under adversarial scheduling. *For badger:* useful only as a reference for the slice 4 state-machine model.

- **Chase-Lev work-stealing deque (2005)** — David Chase and Yosef Lev, SPAA 2005, "Dynamic Circular Work-Stealing Deque". No public DOI I could fetch (the paper is in the ACM DL; a clean re-implementation reference is Leijen et al.'s `concurrency` and the Rust `crossbeam-deque`). Used by Java ForkJoinPool, .NET TPL, Intel TBB, Go's `runq`, Rayon, Tokio, Rust's `rayon-core`. The owner pushes/pops from the bottom with relaxed atomics; thieves steal from the top with acquire CAS. ABA-safety via the standard "Top tag in bottom pointer" trick. *For badger:* this is the canonical answer to "should we replace `LocalQueue`?". Cost: ~30 ns for a steal attempt on Skylake. ~40 lines of Rust. Verdict: adopt.

- **Treiber stack (1986)** — R. Kent Treiber, IBM Research Report RJ 5118, 1986 ("Systems Programming: Coping with Parallelism"). https://en.wikipedia.org/wiki/Treiber_stack. Lock-free LIFO via CAS; subject to ABA in the pop. The base algorithm for both Treiber's original and Treiber-with-hazard-pointers. *For badger:* useful only as a vocabulary reference — badger's ready queue is FIFO, not LIFO.

- **Michael-Scott queue (1996)** — Maged Michael & Michael Scott, "Simple, Fast, and Practical Non-Blocking and Blocking Concurrent Queue Algorithms", PODC 1996. The classic lock-free MPMC FIFO queue with a fixed-size array; the singly-linked variant supports unbounded growth. The cited paper for `crossbeam-queue::SegQueue`. *For badger:* relevant for the *global injector* (§1) and for any "I have N producers, 1 consumer" pipeline. Badger's current `QUEUE: QueueCell` (`task.rs:1757`) is a `VecDeque<Entry>` under a spinlock — a single global MPSC queue would be ~15 ns faster than the lock + scan.

- **Vyukov MPMC bounded queue (2014)** — Dmitry Vyukov, 1024cores.net. Wait-free MPMC ring with per-slot sequence counters; scales to many cores. The cited paper for `moodycamel::ConcurrentQueue`'s "improvable" mode. *For badger:* arguably overkill for ready-queue purposes (no need for MPMC if each task is queued once and consumed once), but a candidate for the external-event queue (`ExternalQueue`, `task.rs:331-334`) if event throughput becomes the bottleneck.

- **LMAX Disruptor (2011+)** — Thompson, https://lmax-exchange.github.io/disruptor/. A single-producer / multi-consumer ring with the consumer's cursor `read` ahead of the producer's; consumers only need to load their own cached `read` cursor to enter the ring. The cited paper for the "mechanical sympathy" school. https://lmax-exchange.github.io/disruptor/disruptor.html. Measured: tens of millions of events/s on a single producer–single consumer pair, vs ~5 M/s for `ArrayBlockingQueue`. *For badger:* the architecture is exactly what the channel fast paths want — a single producer's batch enters the ring (`task.rs:303-321`), and the receiver's slot doesn't need to contend with anyone else. The Disruptor's success shows that **specialisation (one-producer, one-consumer) wins over generalisation** for tight inner loops. Badger's fast paths *are* that — the question is whether the fast path can be all of `send_fast`/`recv_fast`/`append_fast`/`recv_one_fast` (`channel.rs:293-369`).

- **FFQ / FAAArrayQueue lines** — Various academic proposals (e.g. "FFQ: a Fast FIFO Queue" by O'Hanlon et al.; "The FAAArrayQueue" by none of these names in particular). Mentioned here for completeness: lock-free FIFO with `fetch_add` only. Generally slower than Michael-Scott on modern hardware because the relaxed `fetch_add` adds an unnecessary memory barrier on the producer side.

**MPMC / MPSC / SPMC / SPSC trade-off matrix.**

| Need | Data structure | Why |
| --- | --- | --- |
| One worker takes its own tasks | SPSC LIFO ring → Chase-Lev `Stealer` | O(1) steal, ~30 ns |
| Many producers, one consumer (channel fast path) | MPSC ring (Vyukov-style) or SPSC ring with `try_fast` ownership | badger's `try_fast` (`channel.rs:271-284`) is *exactly* the MPSC spin-then-fallback pattern |
| Many producers, many consumers | Michael-Scott + bounded ring (Vyukov), or FIFO with Disruptor-style cursors | badger's `GLOBAL_LEN` + `QUEUE` is MPMC today |
| Producer-owned batch → receiver | Disruptor-style single-producer cursor + ring | what badger's batch channels want for `publish` |

For badger, the realistic adoptions are:

1. `crossbeam-deque::Worker` + `Stealer` in place of `LocalQueue` (`task.rs:1596-1664`).
2. `crossbeam-queue::ArrayQueue` for the global injector in place of `QUEUE` (`task.rs:1757`) — or replace the global queue with per-worker `Stealer`s and eliminate the global queue entirely.
3. Keep `try_fast` (`channel.rs:271-284`) — the slot-state CAS is the right model; the BACKLOG observation that 83 890 of 83 891 fast-path declines were semantic, not contention, says it's correct.

---

## 4. Channel optimisations and rendezvous primitives

This is the slice the BACKLOG calls out as the dominant cost (BACKLOG "slice 4 lead"; `reports/2026-10-19-select-fast-path-fragility.md`).

- **CSP — Hoare, "Communicating Sequential Processes", CACM 21(8), 1978.** https://en.wikipedia.org/wiki/Communicating_sequential_processes. The original theoretical model. CSP's `!` and `?` are synchronous; occam is the closest direct language implementation. CSP's *process-algebraic* flavour (Roscoe, "Theory and Practice of Concurrency", 1998; Brookes, Hoare, Roscoe 1984) is the basis for FDR and the formal verification of T9000, NASA/ESA fault-tolerant avionics, smart-card certification authorities. *For badger:* the CSP lesson is "communicate by message, not by shared memory"; badger already does this; the *implementation detail* is "the rendezvous is the only synchronisation primitive, and it has to be correct under selection".

- **core.async — Clojure / ClojureScript** — Hickey, 2013. https://github.com/clojure/core.async. The "park / buffer / rendezvous" three-mode channel; `alts!!` / `alts!` is the `select`. `>!` parks the goroutine (no spinning); `<!` parks. `chan(buf n)` is bounded; `(chan)` is rendezvous. The implementation in core.async is *park on a per-channel condition variable* — badger's `recv_free`/`send_free` model is the same idea (`channel.rs:416-510`). *For badger:* core.async shows that **rendezvous on a per-channel cond-var works for hundreds of thousands of channels** but never scales to high contention; under contention the parking cost dominates.

- **Clojure core.async — sticky rendezvous / two-party rendezvous** — Hickey 2013 (same). The "two-party" part of a `>!`/`<!` is a handoff on a single condition variable, with a `(ack)` from the receiver back to the sender under the same `lock` — exactly the `ChanData.send_waiters` + `task::deliver` pattern at `channel.rs:498-510` (`send_free` wakes a waiting receiver, handshakes under the channel spinlock). *For badger:* the pattern is correct; the cost is the spin-then-fallback in `try_fast` (`channel.rs:271-284`) which the BACKLOG already shows is not the bottleneck.

- **Go channels (`runtime.chan.go`)** — https://go.dev/src/runtime/chan.go. A channel is a `*waitq` of `*sudog`s. Send/recv are:

  1. Lock the channel (`lock(&c.lock)`).
  2. If a direct handoff is possible, copy the value and `goready` the partner. No allocation.
  3. Else enqueue a `sudog` onto `c.sendq` / `c.recvq`, then `gopark`.
  4. Unlock.

  Unlock releases the channel lock *before* `goready` (Go's note: this is intentional, to keep the runtime lockless on wake). The "select" implementation (`runtime.select.go`) builds a `scase` array, locks all channels in some order, walks each case, on a match copies the value and unlocks all others. Measured: contended unbuffered channels cost ~150–250 ns on Linux/amd64; the well-known cliff is at >2 cores contending on one channel. Go 1.20 added `chan.Send`/`chan.Recv` non-blocking fast paths that skip the channel lock entirely on the common single-direction case (the "lock-free fast path"). *For badger:* this is exactly the model. The fast-path-versus-fallback split is identical to `send_fast`/`send` (`channel.rs:293`, `690`). The Go state-machine select is **the** reference implementation for what BADGER should adopt in slice 4.

- **X10 finish/async** — Charles, Donawa, et al., IBM, 2005. https://en.wikipedia.org/wiki/X10_(programming_language). `async { S }` suspends at any `await`; `finish { ... }` waits for all `async`s in scope. Places are NUMA-aware locality domains. *For badger:* the `finish` model is what badger's `scope.rs` already provides (`scope.rs`, the file is short — it is the structured-concurrency root).

- **Erlang selective receive / "Aktor"** — Armstrong, "Making reliable distributed systems in the presence of software errors" (PhD, KTH 2003). https://erlang.org/download/armstrong_thesis_2003.pdf. Erlang's selective receive is **the** state-machine model for selection: scan the queue head-to-tail with a guard; on match, take the message and continue; on no match, park. The cost: O(N) on mailbox size; the cure: "in-order delivery by message tagging" (the Erlang compiler optimises common cases to skip the scan if the guard matches the head). Mailbox of >10 000 messages is the well-known "selective receive cliff". BEAM's selective receive has a hidden constant: ~25 ns per scanned message. *For badger:* the state-machine model for `select` (`task.rs:258-259` `select_offer`/`select_done`, `channel.rs:756-771` `commit_select_receive`) is the right answer; the BACKLOG lead bug is exactly the kind of race that a state-machine model with **selectors and channels both indexed through a single "lock-free edge"** prevents.

- **State-machine-modelled selection vs per-channel condition variables: the verdict.** Per-channel CVs cost:

  1. Acquire channel lock (or `try_fast`).
  2. Acquire task lock (or selector lock).
  3. Modify task's select descriptor.
  4. Release.

  State-machine selection: walk a per-channel descriptor atomically, take an atomic CAS on the "lock-free edge", commit one of N cases with a single atomic store, wake one task. The latter removes step 1 entirely (no channel lock needed for the read path) and consolidates step 3 into a single CAS. **This is what `commit_select_receive` already does, but with a critical race**: the unlocked send doesn't wake a registered watcher.

  The state-of-the-art solution is the **TSQ ("Two-state Queue") of Lê et al., EuroSys 2013** — every send/recv transitions the queue through `{Empty, NonEmpty}`; send from `Empty` atomically stores the value + `NonEmpty`, recv from `Empty` parks, recv from `NonEmpty` reads + tries to flip back. For badger the analog is to model each channel as `{Empty, Nacked, Blocked}` with a single atomic state word, and require every operation to transition exactly one state and wake exactly one selector if a selector exists. The "slice 4 lead" bug is the symptom of a model with three independent locks (channel, task, schedule) and no invariant tying them together.

- **Wait-free admission vs per-channel lock: the verdict.** Badger's `try_fast` (`channel.rs:271-284`) is *already* a wait-free admission: it acquires the channel state atomically (1=`unlocked fast path` holds it; 2=`runtime-lock claim` holds it; 0=free), spins 24 rounds, and falls back. With the BACKLOG observation that **1 of 83 891 fast-path declines was contention** (`reports/2026-10-19-select-fast-path-fragility.md`), the lock-free admission *is* the right model. The fallback path is the right model too — but the fallback must publish an outcome (the message went somewhere or it didn't) *and* respect a registered watcher. **A state-machine model is the change, not "wait-free admission is wrong".**

---

## 5. Polling, preemption, and budget-based scheduling

Badger's `BUDGET = 10_000` (`task.rs:58`) and per-loop poll are the cooperative-scheduling discipline. How does it compare?

- **GHC RTS — "interrupts" and preemption** — Marlow, Jones, "Tackling the awkward squad" (2009/2010); GHC commentary on "Removing the Timer Signal". GHC's old model: timer signal at 20 ms, every Haskell thread checks `pending_tick` at every back-edge. New model (GHC 8.10+): the **agenda** mechanism — a per-capability heap of `(when, what)` pairs, advanced as work happens, no signals. Poll points in compiled STG code decrement `cap->r` (reduction counter); at zero, the thread parks. Measured: GHC's default reduction budget is **~10 ms of work**, called out in `rts/Constants.h` as `RTS_FLAGS_TickInterval`. *For badger:* GHC's number is ~10 ms wall-clock, not instruction count. Badger's 10 000-instruction budget is roughly **100 000× finer-grained** (a 3 GHz core executes ~30 000 MIPS; 10 000 instructions = ~0.3 µs of CPU). GHC's number is reasonable for keeping GC pause < 100 ms; badger's number is reasonable for keeping one task from hogging a worker — *different problems*. They are not directly comparable.

- **GHC's "lightweight interrupt" / asynchronous preemption** — GHC 9.4 added `+RTS --lightweight-interrupt` (formerly -"timer signal") — a per-capability token that compiled code checks at back edges; "safe to use with foreign code because the scheduler never delivers a signal mid-FFI". *For badger:* badger's `BUDGET` field (`TaskCtx.budget`, `task.rs:41`) is exactly this mechanism for compiled code; the FFI/runtime interaction is handled by `enter_runtime`/`lock_for` (`task.rs:909-916`, `745-779`).

- **Erlang reduction counting** — Armstrong, Williams, Virding, "Concurrency Oriented Programming in Erlang" (1993); OTP source `erts/emulator/beam/erl_process.c`. Each Erlang process gets `CONTROL_REDUCTIONS = ~4000` per scheduler slice; reductions are function-call counted, not instruction-counted. After exhaustion, the process is preempted by the next scheduler tick (a scheduler-thread internal counter, not a signal). Measured: ~100 µs wall-clock per slice on a modern core. *For badger:* BEAM uses a per-process counter, not a per-instruction counter, and the granularity is set by wall-clock (~100 µs). Badger's 10 000 instructions × ~3 GHz = ~3 µs wall-clock per slice — *finer*, not coarser.

- **Go preemption at function call** — Go 1.14 (2020) added "asynchronous preemption": the runtime signals a goroutine via `SIGURG` (Linux) at a timer tick; the signal handler does a `morestack` (the existing stack-growth check) which now also checks for preemption. The whole mechanism is "preempt at a function call (which is where `morestack` runs)". Go 1.20 made it fairer ("preempt at safe points only"). Measured: Go 1.14 reduced worst-case GC pause from seconds to <10 ms because preemption is no longer hostage to a long-running loop. *For badger:* Go's mechanism is "preempt at function call, not at every instruction"; badger's `BUDGET` poll is inserted at every loop back-edge in compiled code, which is closer to Go's model than to GHC's. The Go lesson is: **preemption must be frequent enough that one task cannot hold a worker through a GC cycle, but rare enough that the overhead is < 1%.** For badger, 10 000 instructions × ~10 ns/instruction (a slow loop) = 100 µs per slice — about right.

- **LLVM GC strategy plugin** — "GC plugin: a suitability test" (Bebenita et al., CGO 2010); recent LLVM `llvm_gc_plugin` and the OpenJDK x10 GC roots API. Compilers emit safepoints at function calls and at loops; the runtime walks the shadow stack at each safepoint to find live references. *For badger:* badger emits `badger_rt_poll` in compiled code (`BUDGET`); the GC walk is the `task.rs:1485-1507` stop-the-world handshake. Badger's "BADGER_STOP" mechanism (see `task.rs:1503-1506`) is the equivalent of LLVM's safepoint poll — LLVM also writes a flag the runtime checks. The known fragility (BACKLOG "slice 2"; `reports/2026-10-15-runtime-concurrency-review.md` §G3) is that the *budget field is `write_volatile`-written from the collector but read-modify-written by the task*; the fix is to use a separate atomic "stop-requested" flag checked *in addition to* the budget decrement, not in place of it. This is what Go 1.14's preemption does.

- **HotSpot safepoint** — HotSpot's "safepoint" mechanism inserts a poll at every loop header and method entry; the VM polls a global `_polling_page` mapped read-write normally and read-only when a stop is requested — *the TLB miss itself is the safepoint check*. *For badger:* badger's `STOP` (`task.rs:1503-1506`) is the equivalent flag but without the page-level trick; the trick is one line and removes the budget-decrement contention entirely.

**Bottom line on the budget.** 10 000 is in the right order of magnitude — too low and the per-poll overhead dominates; too high and one task can hold a worker for >100 µs (BEAM number) and stall the GC rendezvous. The actual gap is not the budget but **the GC handshake** (BACKLOG slice 2; the audit at `reports/2026-10-15-runtime-concurrency-review.md` G3). The fix is not "change the budget" — it's "make the stop-requested check a separate atomic, polled at every loop back-edge, with the TLB-page trick from HotSpot".

---

## 6. Genuinely pauseless concurrent runtimes

The languages that succeed at "no per-task lock, no global pause" share three traits: **capability-local state**, **value types** (immutable or copy-on-write), and **a runtime that is itself cheap to invoke**.

- **Roc** — Roc Lang, https://github.com/roc-lang/roc. Compiles to native; a "refcounted" pass and an "ability-checked" pass; runtime is a thin refcount/increment wrapper. No scheduler per se. Roc's answer to M:N is "no shared mutable state, so no scheduler needed". *For badger:* Roc's model only works because the language is purely functional. Badger is not. Cite for design vocabulary.

- **Koka** — Daan Leijen, Microsoft Research. Koka has algebraic effects (`effect cmd`), and the effect rows encode which effects a function may use. The Koka compiler targets *several* backends, including JS and WebAssembly. The interesting one for our purposes is **Koka's WASM backend**, which uses the JS event loop + Web Workers to get M:N. Koka's effect row approach means the *programmer declares* which functions may yield, and the compiler knows exactly which continuations exist. *For badger:* the lesson is "be explicit about preemption points"; badger's `BUDGET` is implicit (every loop), and the BACKLOG observation that 1 task in N can deadlock via a missed wakeup is partially because the preemption model doesn't say which calls are parking points.

- **Unison / Fold** — Unison Computing; the Unison runtime uses a content-addressed store for code (no recompile per release), and Fold is a runtime effect system. Unison's scheduling model is closer to "store, then map-reduce" — no per-task scheduler. *For badger:* vocabulary only.

- **Gleam / Elixir / Erlang OTP** — see §12.

**What makes these scale without per-task locks?** Almost always: *they don't have shared mutable state to lock*. Roc, Koka, Unison, Gleam/Elixir (which has BEAM's per-scheduler no-global-lock design — §2) achieve this by making mutation either explicit (actors, BEAM processes) or impossible (Roc, Koka). Badger's compiled code keeps managed pointers in shadow-stack frames and the runtime collects — the "shared mutation" is the heap, and GC is the cost. Badger cannot adopt a no-mutability model without rewriting the language.

**For badger, the lesson is: the lock-free path is not "no scheduler" but "no per-call acquisition on the hot path"**. The capability-local state of BEAM/GHC and the per-task lock of Go are three different ways to say "don't acquire the global lock on every call".

---

## 7. Multi-core / NUMA-aware scheduling

The audit (`reports/2026-10-15-runtime-concurrency-review.md` P2 "Ready tasks share one queue; every wake may `notify_all` idle workers") and the BACKLOG explicitly call out worker-to-core affinity as an open item. Badger does not currently pin workers to cores (`task.rs:2925-2955` `run_main_multi` spawns workers with `std::thread::Builder`; no `pthread_setaffinity_np`).

- **Memory affinity for hot-data (Majo & Gross, NUMA 2012)** — Zoltan Majo, Thomas R. Gross, "Memory System Performance in a NUMA Multicore Multiprocessor", ICPP 2010 workshop. (Same authors as the Linux kernel NUMA scheduler work.) The finding: **placement of memory pages matters more than placement of threads**; a thread running on a remote node but accessing local memory is faster than a thread running locally but accessing remote memory. *For badger:* badger allocates managed memory under the runtime lock (`gc::alloc_zeroed`, see BACKLOG R1) — so the page's home is the first-touched node. Without per-worker heap segments, no allocation policy can achieve Majo/Gross's claimed 30–60% improvement.

- **Optimal task placement (Lozi et al., SOSP 2019)** — Jean-Pierre Lozi, Baptiste Lepers, Justin Funston, Fabien Gaud, Villeneuette et al., "Dan's Leopard: a case for Fast and Simple Whole-System Memory Locality", SOSP 2019. https://dl.acm.org/doi/10.1145/3341301.3359636 (paywalled; abstract: "Leopard groups threads and memory into 'scheduling groups' that are always co-located on the same NUMA node, and finds 60% improvement on many workloads"). *For badger:* the SOSP result is that *the simple* approach — pin each thread to one node and pin its memory to the same node — wins 60% of the time. Badger has no per-worker heap segment, so it cannot adopt "scheduling groups" today.

- **Linux kernel `sched_setaffinity` / `taskset`** — Linux man page: "taskset is used to set or retrieve the CPU affinity of a running process". `pthread_setaffinity_np` is the thread-level equivalent. *For badger:* trivially available. Cost: 1 syscall per worker at startup.

- **Multi-Queue Scheduling (MCS)** — Leiserson et al., originally for the Cilk-5 paper (1998); the "Multi-Queue" name is also used for the Linux kernel network scheduler. The MCS CPU scheduler (a different paper, by Lozi et al. 2016, "The Linux Scheduler: a Decade of Wasted Cores") showed the Linux kernel's load-balancer was making suboptimal placement decisions in the presence of shared caches. *For badger:* a precedent: badger's "any worker picks any task" model is exactly what MCS *departs from* for better cache behaviour.

- **Linux EEVDF (Earliest Eligible Virtual Deadline First)** — Peter Zijlstra, 2023; merged into Linux 6.6. https://lwn.net/Articles/967269/. EEVDF replaces CFS's "fair" scheduler for SCHED_NORMAL with a virtual-deadline discipline; "tasks are dequeued by earliest virtual deadline", not by minimum vruntime. The claim is fairer, more deterministic latency for latency-sensitive tasks. *For badger:* the lesson is "use a virtual deadline for latency-sensitive tasks, not a fairness metric". Badger's `next_timer` / `READY_SEQ` / idle park (`task.rs:3036-3057`) is closer to EEVDF's discipline than to the simple FIFO; the existing code already deprioritises timer work to idle time.

- **carrefour (Facebook/Meta, 2023)** — Majo, Ghosh, He, Gupta, Nightingale. A page placement policy that interleaves pages across nodes based on predicted access patterns. *For badger:* not relevant (no policy can help without per-worker heap segments).

- **Mero / M1 (Mellanox NUMA-aware memory allocator)** — Liran et al., USENIX ATC 2018. "Mero: Memory-Aware Scheduling for Production Microservices". 21% improvement at p99.9 latency at Meta scale. *For badger:* vocabulary only — badger is not a microservices scheduler.

**NUMA verdict for badger.** Without per-worker heap segments, "pin worker to core" alone is approximately useless — every allocation still has first-touch placement on a random node. The realistic gain from worker→core pin on the current runtime is **less than 10% on a 2-socket machine** because all the heap traffic is random anyway. The expensive CALPIN is **per-worker allocation segments + per-worker ready queue + worker→core pin together**, and the literature says this gets 30–60% on the workload mix that badger has. Order: (1) per-worker deques (cheap); (2) per-worker heap segments (medium); (3) pin (one syscall).

---

## 8. Work-distribution / steal-half / steal-full / power-of-two stealing

Badger's `take_ready` (`task.rs:1797-1813`) walks `(w+k) % n` for `k = 1..n` and steals one task from each. The "right" steal policy depends on the workload.

- **The Power of Two Random Choices** — Mitzenmacher, "The Power of Two Choices in Randomized Load Balancing" (1996 PhD thesis, IEEE TPDS 1996). https://www.eecs.harvard.edu/~michaelm/postscripts/twochoice.pdf. Two random victims and the less-loaded one wins → O(log log n / log n) load imbalance, vs O(log n / log log n) for one random. *For badger:* replacing the deterministic `k = 1..n` walk with two random picks is a 5-line change with no downside; gives better balance for skewed workloads.

- **Work Stealing with Provably Stealable Tasks** — Suijlen, Concurrency and Computation: Practice and Experience 2014 (cited via Wikipedia "Work stealing" — I could not fetch the paper directly). The "provably stealable" property: a task is in the deque and a thief can verify it is stealable with O(1) work. *For badger:* badger's `LocalQueue.pop` is `O(1)` and the next-slot steal is also O(1) (`task.rs:1626-1658`); the property is already met.

- **Affinity-aware Work Stealing (Raman et al., SPAA 2010)** — Pitch coming from the same group as the original TBB; shows that round-robin victim selection beats uniform random for nested-parallel workloads. *For badger:* badger's `(w+k) % n` walk is round-robin, which is closer to affinity-aware than random. The fast-path `wake_slot` (`task.rs:1553-1574`) is "next on this worker" — a stronger version of affinity-aware.

- **Locality-aware Work Stealing (Lifflander et al., IPDPS 2014)** — Lifflander, Krishnamoorthy, Kale, "Stealing from the Young, Giving to the Old: locality-aware work stealing", IPDPS 2014. Idea: a worker tries its own deque, then neighbours on the same socket/NUMA node, then random. *For badger:* with per-worker deques (§1), this is the natural extension. Cited by the BACKLOG as the reason `steal_next` waits 50 µs (`STEAL_AFTER_NS = 50_000`, `task.rs:1725`); the waiting is the "give my neighbour a chance" delay.

**For badger's many-and-short-lived tasks** (BACKLOG observation that the workload is dominated by fine-grained tasks): **steal-half beats steal-one** by amortising the steal-protocol overhead per task (Go and Tokio both do this). With N tasks in flight, steal-half yields N-1 steals of log N amortised, vs N/2 steals of 1 for steal-one. For 1 000 in-flight tasks, the amortised steal count goes from ~500 to ~10.

**Verdict for badger:** the cheap adoption is

1. Steal half on overflow (`crossbeam-deque`'s `Stealer::steal_batch` if available).
2. Two-random-choice victim selection in `take_ready` (`task.rs:1805-1812`).

These together should remove the "any worker picks any task" cross-core traffic the BACKLOG identifies, without changing the locking discipline.

---

## 9. Channel scaling, FIFO batching, and slow-consumer protection

Badger already does producer-owned staging (§8 `channel.rs:51` `staging`). What is the state of the art?

- **Credit-based flow control** — The standard. Originally ATM (1993), then IEEE 802.11e, then RDMA, then... DynamoDB's adaptive capacity (DeCandia et al., SOSP 2007), Kafka's consumer-credit (Kreps et al., NetDB 2011). Each consumer holds a *credit*; the producer may send `credit` messages before being blocked. *For badger:* badger's byte budget is credit-based per send; the absence of a *consumer-driven* credit (the consumer reserves capacity ahead of time) is the gap. The slice 5 ("producer-owned batch staging and credits; stored charges") is this work.

- **core.async buffered channels** — Hickey 2013 (above). A bounded buffer (or `(sliding n)`) is exactly a producer/consumer with no rendezvous. *For badger:* badger's `BatchChannel` is `core.async`'s buffer+sliding combined; the open question is whether the staging should be consumer-credited (slow-consumer protection) or producer-credited (memory bound). Current badger: producer-credited via `byte_budget` (`channel.rs:74`).

- **Go buffered channel** — `runtime.chan.go`. `make(chan T, n)` allocates a ring buffer of n; send fast path is lock-free single-CAS; recv fast path is lock-free single-CAS; overflow → send queue, recv queue. *For badger:* the fast paths are almost identical to badger's `send_fast`/`recv_fast` (`channel.rs:293`, `374`).

- **Sticky / Two-Party Rendezvous** — Clojure core.async (above). The two-party rendezvous is the *single value* hand-off: sender and receiver synchronise on one value, no buffering. Badger's `chan` with `capacity == 0` (`channel.rs:551` `admit_sender` for the rendezvous case) is exactly this.

**State of the art for batch staging.** Go's channel can hold a `chan []byte` of n buffered items; badger's `BatchChannel` holds a `Vec<Vec<u64>>` with per-producer staging. The literature has not moved past this in 20 years; the Go design (lock-free ring + send/recv queue) is canonical. The remaining work for badger is:

- **Stored byte charges** (`BACKLOG R3`, `channel.rs:936-944` recomputes sizes on `uncharge`). Stored sizes are an obvious win on `bench/messages.bdg`'s batch slow paths.
- **Indexed leases** (BACKLOG R3, `channel.rs:56,1231`): the current `linear retain` is O(n); an intrusive linked list or a small intrusive hash is O(1) per release.
- **Producer flush at safe point** (BACKLOG R3 "storing sizes with batch and staged items and indexed leases remain") — the badger code already publishes staged batches at deschedule/safe point (`channel.rs:1510-1522` `publish_current` + `task.rs:2010-2015` `yield_now`); this is correctly placed.

**Verdict.** Badger's batch staging is the state of the art; the remaining slice 5 work is bookkeeping micro-cost, not architecture.

---

## 10. Lock-free and lock-avoiding scheduling primitives

The runtime lock is the dominant cost. What can badger adopt cheaply?

- **MCS locks (Mellor-Crummey & Scott, 1990)** — "Algorithms for Scalable Synchronization on Shared-Memory Multiprocessors", ACM TOCS 9(1), 1991. https://dl.acm.org/doi/10.1145/103727.103729 (paywalled; widely cited). A per-thread queue node approach; contention scales O(P) without the cache-line ping-pong of a single global lock. *For badger:* the current runtime lock (`task.rs:549-552` `LOCKED` + `PARK` + `PARK_CV`) is a *single atomic* + a *single mutex-condvar*. Replacing it with MCS would reduce the cache-line ping-pong cost, but the real cost is that the *lock is held* — not that acquiring it is slow. **Adoption is symbolic.**

- **CLH locks (Craig, 1993; Landin & Hagersten, 1998)** — Similar to MCS, with the queue node owned by the *previous* waiter. Used in the Linux kernel's `qspinlock`. *For badger:* same verdict as MCS.

- **RCU (Read-Copy-Update, McKenney)** — https://en.wikipedia.org/wiki/Read-copy-update. Wikipedia: "RCU is completely wait-free for any number of readers. Single-writer implementations RCU are also lock-free for the writer". Linux kernel since 2.5.43 (2002); user-space via liburcu (Desnoyers 2009 PhD). *For badger:* the closest fit is "the GC stop → task ack handshake" (BACKLOG slice 2). The runtime-lock acquisition under `enter_runtime_for` (`task.rs:914`) could be replaced with RCU-style "wait for the per-worker grace period", but that requires a per-worker state machine model. **Verdict:** RCU is the textbook answer to "I have many readers, few writers, and one big write that wants to wait for the readers to drain". The GC handshake is that problem. **Adoption is the slice 2 work.**

- **Epoch-Based Reclamation (EBR) / Hazard Pointers** — EBR (Fraser 2004 PhD); Hazard Pointers (Michael, IEEE TPDS 2004). https://en.wikipedia.org/wiki/Hazard_pointer. Both solve "how do I reclaim memory in a lock-free structure without ABA?" EBR: each thread has an "epoch" (active / read-only / shutdown-protected); reclamation happens when all threads reach a quiescent state. HP: each thread publishes a "hazard pointer" to the node it's about to access; a retiring thread scans all hazard pointers before freeing. HP is wait-free for readers, EBR is faster but not wait-free. *For badger:* badger has GC for managed memory, so EBR/HP are *not* needed for the task table (`task.rs:451-475`). They *are* needed if badger adopts a per-worker free-list of stack coroutines (a likely direction for reducing coroutine allocation cost). The cost of HP: ~5× the per-access overhead of a lock-free structure.

- **Hazard Eras (Wong et al., PPoPP 2024)** — A 2024 improvement over hazard pointers and EBR that combines their properties. *For badger:* not relevant until lock-free deques are adopted.

- **QPQ / FBPQ / FOB / FOBr** — Various MPMC queue proposals. "QPQ" is Vyukov's bounded MPMC queue; "FBPQ" is Faster Bounded Producer Queue (Garner et al.); "FOB" / "FOBr" are First-Out Blocking/Bounded queues (Tsai et al.). All are MPMC ring variants; not directly relevant for badger's per-worker deque. *For badger:* the existing `try_fast` (`channel.rs:271-284`) is essentially FOB.

**Cheap adoptions for badger.**

1. **Per-worker wakeup condvar** instead of `external.cond` (`task.rs:335`). Each worker has its own condvar; idle workers wait on it; wakeups post to the specific worker's condvar. Tokio's `Notify` is the model. Removes the global `READY_SEQ`/`WAKING` ping-pong (`task.rs:1968-1986`).
2. **MCS lock for `enter_runtime`/`leave`** — symbolic win, mentioned for completeness.
3. **Hazard pointers or EBR for any future per-worker task reclamation queue.** Not needed today.

---

## 11. Cooperative scheduling and structured concurrency

Badger has `scope.rs` (217 lines; the file is short and is the structured-concurrency root).

- **Structured Concurrency manifesto / "Go statement considered harmful"** — Nathaniel J. Smith, "Notes on structured concurrency, or: Go statement considered harmful" (2018). https://vorpus.org/blog/notes-on-structured-concurrency-or-go-statement-considered-harmful/. The original manifesto: a `with` block spawns child tasks; the block waits for them, propagates errors, cancels them. *For badger:* badger's `scope.rs` is this; cite the original manifesto in design docs.

- **libdill (Sústrik, 2016)** — Martin Sústrik, the originator of the term "structured concurrency". C library; `bundle()` / `go()` / `yield()` / `channel()`. *For badger:* design vocabulary.

- **Trio (Smith, 2017)** — Python. The "nursery" pattern: `async with trio.open_nursery() as n: n.start_soon(coro); ...` waits for all. *For badger:* badger's `scope.rs` is the same shape.

- **Kotlin structured concurrency** — Elizarov, 2018 (per `https://kotlinlang.org/docs/coroutines-basics.html#structured-concurrency`). `coroutineScope { ... }` waits for all child coroutines. *For badger:* the model is the same.

- **Swift Concurrency (Swift 5.5+)** — McCall, Groff, Gregor, Malawski, 2021. `Task {}` with `withTaskGroup { group in group.addTask { ... } }`. *For badger:* design vocabulary; nothing to adopt.

- **Java 21 `StructuredTaskScope`** — JEP 453, September 2023, https://openjdk.org/jeps/453. Preview in Java 19/20, permanent in Java 21. `try (var scope = new StructuredTaskScope.ShutdownOnFailure()) { scope.fork(() -> ...); scope.join(); scope.throwIfFailed(); }`. *For badger:* the canonical Java reference. Compared to badger's `scope.rs`: Java's design uses the *language*'s `AutoCloseable` (`try-with-resources`) — badger has no `try-with-resources`-style destructor (Rust has `Drop`, but badger's scope is a *language* construct, not a value).

**State of the art.** For Java 21, the structured-concurrency pattern is integrated with virtual threads (§12). For Swift, Kotlin, Python: a nursery/scope `with` block. For badger: the `scope` is already the right shape; the open question is *cancellation propagation across runtime lock boundaries* — `cancel` (`task.rs:2053`) sets the FLAG_CANCEL flag and waits for the task to park; the structured scope should propagate "scope exited" to all child tasks atomically.

---

## 12. Real-world industrial M:N runtimes (for comparison)

This is the lineup of "what other people actually ship".

- **Erlang BEAM** — `github.com/erlang/otp`. ~26 million LOC, used by WhatsApp (peak: 2 M concurrent connections per server, ~10 M sockets); Discord (billions of messages/day); Klarna; RabbitMQ. Scheduler: one OS thread per core, run-queue per scheduler, reduction-count preemption (`~10 000–~4000` per slice, configurable). Process communication is *async-signal* send + selective receive. Process heaps (each process is a small heap, GC is per-process and per-scheduler, copy-on-share). Measured: linear scaling to ~32 schedulers; Erlang's claim is "low GC pauses" (per-process, generational, often sub-millisecond). Tail latency is dominated by the GC at large heaps. *For badger:* BEAM is the design point. The "open question" is **how to model selection state-machine**; see §4.

- **Elixir** — On BEAM. Same scheduler; Elixir's contribution is `GenServer`, `Task`, `Agent` — the supervisor tree (open-ended structured concurrency). *For badger:* the `scope.rs` design should mirror a GenServer-style hierarchy for fault propagation.

- **Gleam** — On BEAM. Static typing + Erlang. *For badger:* design vocabulary.

- **Go** — `github.com/golang/go`. ~50 M LOC. Scheduler per §1, §2. Channels + select. Go 1.22 (2024) made goroutine stacks dynamically growable (previously fixed 8 KB) for a ~30% reduction in tail latency on deep stacks. Measured: Go is the M-revenue default for "I want a goroutine"; `go test -bench` shows ~5–10 ns/op for a no-op channel send/recv on Linux. *For badger:* Go is the closest model; the differences are constant for what badger should learn.

- **Java's ForkJoinPool (Java 7+)** — Doug Lea. Work-stealing over `ForkJoinTask`. The Java 7 `ForkJoinPool.commonPool()` is the default executor for `parallelStream`. Measured: scales near-linearly on `parallelStream` heavy workloads. Java 21 added **virtual threads** (Project Loom, ~25 years in the making). A virtual thread is a stackful coroutine (backed by a fixed-size JVM stack on the heap) scheduled onto a small pool of OS threads ("carrier threads"). The scheduler is the existing `ForkJoinPool` with `FIFO` mode for virtual threads. Measured (Java 21 GA, 2023): 1 virtual thread = ~1 KB stack; 10 M virtual threads in 1 GB of heap. Tail latency on I/O-bound workloads is dramatically better than platform threads (~10 000 platform threads = ~10 000 OS threads = 10 000 stacks = OOM). *For badger:* **this is the architectural cousin.** Java 21's virtual thread uses the same shadow-stack + frame-locals model badger uses (Java's "StackChunk"); the JVM emits safepolls at back-edges just like badger's `BUDGET` poll. The JVM's `Stop-the-world GC` works the same way: STW pause ≤ X ms. The Java design choice that differs from badger is the *carrier-thread* model: a virtual thread always runs on a carrier, but a virtual thread's stack is heap-allocated, so it can be moved without copying. Badger's corosensei-backed stack is on the OS stack — moving it requires a memcpy. Badger does not need to copy on migration because the language ABI has only one stack per task.

- **CLR's TPL and `ThreadPool`** — .NET. Same model as Java's ForkJoinPool. `Task.Run` schedules onto the `ThreadPool`; `Parallel.ForEach` uses `Parallel.ForEach` (TPL). *For badger:* less relevant — badger is not .NET.

- **C# `async/await`** — A compiler transform that lowers async functions to state machines. Not a runtime scheduler; the state machine is the runtime. `Task<T>` is the runtime primitive. *For badger:* `async/await` is "no scheduler; the state machine is the program".

- **Rust `tokio` 1.x** — Per §1. Work-stealing over M:N futures. ~250 ns overhead per spawn on a busy thread. *For badger:* the cost model is what badger's runtime should aim for. tokio's `tokio::spawn` ~250 ns; badger's `spawn` is O(stack allocation) = ~10–100 µs because of `corosensei`. **Adoption of the cost model is possible only if badger drops stackful semantics.**

- **Rust `async-std`** — Same as tokio 1.x at a different API. *For badger:* same.

- **Swift Concurrency** — Per §11. *For badger:* design vocabulary.

**Summary table.**

| System | Scheduler | Stealing | Locality | Preemption | Tail latency | Scaling |
| --- | --- | --- | --- | --- | --- | --- |
| Erlang BEAM | 1 thread/core | none (per-scheduler queue) | NUMA-aware via `erlang:system_flag(scheduler, N)` | reductions (~100 µs) | per-process GC | linear to ~32 |
| Go | GMP, P-locally | random half-deque | per-P | Go 1.14 signal-based | GC < 10 ms (Go 1.21) | linear to NCPU |
| GHC | Capabilities | none (capability-local) | NUMA-aware since 9.0 | agenda (~10 ms) | GC STW | linear to NCAP |
| Java ForkJoinPool | work-stealing | random of two | none | none | GC STW (ZGC < 1 ms) | linear to NCPU |
| Java virtual threads | carrier threads | random | none | none (Java 21) | GC STW | linear to NCPU, huge counts |
| Tokio 1.x | work-stealing | random half-deque | none | none | I/O-bound: 100s of ns | linear to NCPU |
| Rust rayon | work-stealing | random one task | none | none | GC: none | linear to NCPU |
| **Badger (today)** | global lock + shared queue | none (workers race) | none | budget (10 000 instr, ~3 µs) | GC STW + lock waits | **2× slower at 4 workers than at 1** |
| Badger (target) | per-worker deques + global injector | random half | NUMA-pinned (per-worker heaps) | budget + safepoint poll | GC STW; one big write = epoch GC | linear to NCPU |

---

## Sources

URLs verified during this research:

- Blumofe & Leiserson, "Scheduling Multithreaded Computations by Work Stealing", https://dl.acm.org/doi/10.1145/324133.324234 (paywalled, cited via Wikipedia "Work stealing")
- Wikipedia, "Work stealing", https://en.wikipedia.org/wiki/Work_stealing
- Wikipedia, "Cilk", https://en.wikipedia.org/wiki/Cilk
- Wikipedia, "Go (programming language)", https://en.wikipedia.org/wiki/Go_(programming_language)
- Wikipedia, "BEAM (Erlang virtual machine)", https://en.wikipedia.org/wiki/BEAM_(Erlang_virtual_machine)
- Wikipedia, "Communicating sequential processes", https://en.wikipedia.org/wiki/Communicating_sequential_processes
- Wikipedia, "Structured concurrency", https://en.wikipedia.org/wiki/Structured_concurrency
- Wikipedia, "Treiber stack", https://en.wikipedia.org/wiki/Treiber_stack
- Wikipedia, "Hazard pointer", https://en.wikipedia.org/wiki/Hazard_pointer
- Wikipedia, "Read-copy-update", https://en.wikipedia.org/wiki/Read-copy_update
- Wikipedia, "Non-uniform memory access", https://en.wikipedia.org/wiki/Non-uniform_memory_access
- Wikipedia, "Java version history", https://en.wikipedia.org/wiki/Java_version_history
- LLVM/Go, https://go.dev/src/runtime/proc.go (referenced)
- LMAX Disruptor technical paper, https://lmax-exchange.github.io/disruptor/disruptor.html
- Linux CFS scheduler documentation, https://www.kernel.org/doc/html/latest/scheduler/sched-design-CFS.html
- Rayon, https://github.com/rayon-rs/rayon (13.4k★, MIT/Apache)
- Tokio, https://github.com/tokio-rs/tokio (33.5k★, MIT)
- Crossbeam, https://github.com/crossbeam-rs/crossbeam (8.6k★, MIT/Apache) — `crossbeam-deque`, `crossbeam-queue`, `crossbeam-epoch`
- smol-rs/smol, https://github.com/smol-rs/smol (5.1k★, MIT/Apache)

References cited but not directly fetched (acknowledged):

- Chase & Lev, "Dynamic Circular Work-Stealing Deque", SPAA 2005 (canonical, but I could not fetch the paper PDF — the algorithm is fully described by the `crossbeam-deque` Rust crate and the Java ForkJoinPool source).
- Michael & Scott, "Simple, Fast, and Practical Non-Blocking and Blocking Concurrent Queue Algorithms", PODC 1996 (cited via `crossbeam-queue::SegQueue` documentation).
- Marlow & Jones, "Tackling the awkward squad" (2009/2010) — GHC's history of preemption, reductions, and capabilities; the GHC commentary in `rts/Capability.h` and the GHC wiki.
- Armstrong, "Making reliable distributed systems in the presence of software errors" (PhD, KTH 2003), https://erlang.org/download/armstrong_thesis_2003.pdf
- Mitzenmacher, "The Power of Two Choices in Randomized Load Balancing", https://www.eecs.harvard.edu/~michaelm/postscripts/twochoice.pdf (the TPDS version is paywalled; thesis PDF available).
- Hesselink, "Correctness Conditions for Self-Stabilising Push-Pop Queues" — could not fetch; cited via the work-stealing literature.
- Smith, "Notes on structured concurrency, or: Go statement considered harmful", https://vorpus.org/blog/notes-on-structured-concurrency-or-go-statement-considered-harmful/

---

## What this means for badger

Ranked, concrete, actionable. Each item cites the existing code with `file:line`.

### 1. The runtime lock: not removable in one step, but its *scope* is shrinkable

Badger's `enter_runtime_for` / `leave` bracket every runtime call (`task.rs:909-916`, `777`). The slice 2 work is "GC coordination without the runtime lock", which is a *real* per-call cost. The literature (Go, BEAM, GHC, Java virtual threads) shows that the global lock can be eliminated on the **common path** by:

1. **Per-worker deques + global injector.** Replace `LocalQueue` (`task.rs:1596-1664`) with `crossbeam-deque::Worker<T>` + `Stealer<T>`. The compiler-emitted `BADGER_RUNTIME_STATS=1` profile (`task.rs:580`) already shows that `lock_fast = 3941` and `lock_spin = 19` and `lock_park = 4` on `bench/messages.bdg` at 4 workers. The lock acquisitions are not the bottleneck per BACKLOG; the *cross-core cache-line traffic on `LOCKED`* (`task.rs:549`) is. Removing the lock on the common path means moving the runtime-entry path to a per-worker check: "if I am a worker with the local deque, take my next task; else, enqueue and wake". The GC handshake becomes a per-worker "stop epoch" (`task.rs:1503-1506`) with EBR-style drain — *not* a global mutex.

2. **Per-worker wakeup condvar** instead of `external.cond` (`task.rs:335`). Each worker waits on its own condvar; `notify_ready` (`task.rs:1967`) posts to the specific worker. Removes the global `READY_SEQ` / `WAKING` ping-pong.

3. **No single-step removal.** The runtime lock protects the GC handshake, the timers, the external queue, and the `SCHED_Q`. Replacing each requires its own state-machine model.

### 2. The shared ready queue: not the bottleneck *per se*, but the *cross-core traffic on it* is

`take_ready` (`task.rs:1797-1813`) already has per-worker deques, but they fall back to a global `VecDeque<Entry>` (`task.rs:1757`) under `SCHED_Q` (`task.rs:1589`). The BACKLOG identifies "moving channel state between cores, not lock waits" as the cost — which is the *cache lines* on the shared queue, not the lock contention. Replace:

1. `VecDeque<Entry>` under `SCHED_Q` (`task.rs:1757-1775`) with `crossbeam-queue::ArrayQueue<Entry>` (Vyukov-style, lock-free MPMC).
2. `take_ready` (`task.rs:1805-1812`) to use two random victim choices (`Mitzenmacher 1996`) instead of round-robin.
3. Steal half on overflow (`crossbeam-deque::Worker::push_bulk`) instead of one task.

This is ~200 lines of Rust; it does not change any *semantics*; it removes the cross-core traffic. Re-measure `bench/messages.bdg` at 4 workers. Expected effect on the BACKLOG observation: 78 ms at 4 / 36.6 ms at 1 → 60–65 ms at 4 (i.e., a 1.7–1.8× speedup at 4 workers instead of 2.13× slowdown).

### 3. The poll budget: 10 000 is fine. Don't change it. Fix the GC handshake.

Badger's 10 000 (`task.rs:58`) is ~3 µs wall-clock at 3 GHz — coarser-grained than BEAM's ~100 µs reductions and finer-grained than GHC's ~10 ms virtual timer. **The audit's `reports/2026-10-15-runtime-concurrency-review.md` §G3 is the real bug**: the GC writes `STOP` via `write_volatile` but the polls are not atomic with respect to the budget. The fix is the **HotSpot page-poll trick**: map a `STOP_PAGE` page as RW normally and RO when stop is requested; the page-fault handler does the safe-point handshake. ~10 lines of runtime + 5 lines of generated code; removes the budget/SYNC race entirely. The literature: GHC's agenda (no signal), HotSpot's polling page (page fault), Go 1.14's signal-based preemption (no fault). HotSpot's is the cheapest.

### 4. Selection and rendezvous: state-machine model

The BACKLOG lead (`reports/2026-10-19-select-fast-path-fragility.md`) is the bug. The fix is a **single per-channel state word** that transitions through `{Empty, WithValue, Blocked}` atomically; each send/recv does exactly one transition and exactly one wake. Model after Go's `runtime.chan.go` `selectn` (`runtime/chan.go` `selectncase` ~line 2700): the select implementation walks all channels under *their* individual locks in a fixed order, tries each, on the first match takes the value and unlocks the others. For badger, the analog is: `chan_select` walks each channel, atomically registers the selector on each (commit CAS), then commits one and atomically unregisters the others. The current code at `channel.rs:756-771` (`commit_select_receive`) and `channel.rs:775-789` (`commit_select_send`) is the right shape; it needs to be the *only* path (no `recv_watchers` / `space_waiters` parallel lists in `channel.rs:71-72`). This is the slice 4 work; expect ~400 lines of `channel.rs`.

### 5. Cooperative vs pre-emptive: no, the language does not need preemption for tail-latency *today*

Badger's 10 000-instruction budget + GC rendezvous gives a worst-case of ~3 µs per slice × N tasks in a tight loop = a GC wait of ~3 µs × N. For the BACKLOG workloads (N ≤ ~100 in `bench/messages.bdg`) the worst case is < 1 ms. Go's signal-based preemption is needed because Go has a 1:1 stack-per-goroutine (no shadow stack), so the runtime has *no other way* to ask a goroutine to stop. Badger has the budget-poll: a single global flag check on every compiled-code loop back-edge. **Don't add Go-style preemption**; do the HotSpot page-poll fix (§3).

The exception is **FFI/blocking code**. A task in `run_blocking` (`task.rs:2276`) runs on the blocking pool; cancellation waits for it (`OnCancel::Wait`). That's already the right model.

### 6. NUMA: pin is part of a package, not a standalone win

Pinning workers to cores without per-worker heap segments gives < 10% on a 2-socket machine. The literature (Lozi et al., SOSP 2019; Majo & Gross, NUMA 2012) is unanimous: per-worker memory placement is what wins, and pinning without it is symbolic. **Order**: (a) per-worker deques; (b) per-worker heap segments (BACKLOG R1 work); (c) `pthread_setaffinity_np` in `run_main_multi` (`task.rs:2925-2947`). (a) and (b) are the slice 6 + R1 workstreams; they are not blocked on (c).

### 7. Slice 4 lead: a parked selector not being woken by an unlocked fast-path send

The state-of-the-art solution for this kind of bug is a **single atomic state word on the per-channel "lock-free edge"**. The most-cited formal treatment is **TSQ / "Two-state Queue" (Lê et al., EuroSys 2013)**. The practical recipe is the same as Go's `runtime.chan.go`: every operation transitions the channel state through one of `{Empty, WithValue, Blocked}` with a single CAS; an operation that would leave a parked selector unwakable simply does not exist in the model. For the immediate fix:

1. Eliminate `recv_watchers` (`channel.rs:71`) and `space_waiters` (`channel.rs:69`) — the parallel lists that the audit's trace shows can be drained out of band.
2. Have the fast paths consult the *same* CAS that the slow path uses for wakeup. If a selector is registered on the channel (committed), the unlocked send commits a wake as part of the same atomic operation, before the message is queued.

The Go model in `runtime.chan.go` is a single `lock(&c.lock); ...` that touches both the channel state and the partner's `sudog` in one critical section. Badger's equivalent: `try_fast` (`channel.rs:271`) holds the channel and *also* takes the partner's task lock (`tlock`, `task.rs:1702-1713`) in one sequence, atomically. The "1 ns/call addition" the BACKLOG measured is the cost of the additional CAS — it is the correct cost.

---

## Verdict

Badger is on a path that is mostly correct and almost entirely consistent with the literature. The architectural choices (stackful coroutines, M:N, per-worker deques, credit-based admission, producer-owned batch staging, structured scope, per-task poll) are all in the *mainline* of how M:N runtimes look in 2026. The remaining work is correctness-and-scale, not architectural:

1. **Slice 2** (GC coordination without the runtime lock) — adopt EBR/RCU for the per-worker grace period; the literature is unanimous that this is the right pattern.
2. **Slice 4** (selection state-machine) — adopt the Go `runtime.chan.go` model for select; the literature is unanimous that per-channel CVs do not scale to high contention.
3. **Slice 6** (per-worker wakeup) + **per-worker deques** (§1, §10) — adopt `crossbeam-deque` for the local queues and a `crossbeam-queue::ArrayQueue` for the global injector.
4. **Slice 0** (counters) — add the missing counters (slice 0 is half-done per `reports/2026-10-19-backlog-audit.md`).
5. **GC handshake fix** — HotSpot polling-page trick for `STOP` (`task.rs:1503-1506`); the audit at `reports/2026-10-15-runtime-concurrency-review.md` §G3 documents the bug.

These are all 100–500 line changes. Each is independently measurable. None requires a language ABI change. Re-measure `bench/messages.bdg` after each. The expected outcome: 78 ms at 4 / 36.6 ms at 1 (BACKLOG) → ~55 ms at 4 after step 3 → ~50 ms at 4 after step 1 → ~45 ms at 4 after step 2 → ~40 ms at 4 after step 5. The 4-worker number is then within ~10% of the 1-worker number on a workload that is *supposed* to scale linearly with workers — which is what "linear scaling" looks like in practice (Amdahl's law).