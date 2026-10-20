---------------------------- MODULE ChannelBadger ----------------------------
(***************************************************************************)
(* The per-channel state machine of crates/badger_runtime/src/channel.rs   *)
(* (the Go-style protocol of 9f1ff4c and c4750f8), one channel, a bounded  *)
(* set of tasks.                                                           *)
(*                                                                         *)
(* The channel. One atomic word (`ChanData::state`, channel.rs:70) holds   *)
(* the owner (free, an unlocked fast path, the runtime lock's claim), the  *)
(* phase (EMPTY, VALUE, BLOCKED), the closed bit and, with one worker, the *)
(* STALE bit; c.own, c.pub, c.pclosed and c.stale are those fields (a fast *)
(* path's ownership is never visible: its body is one step). Every task    *)
(* registered with the channel is in one list (`Waiters`, channel.rs:125), *)
(* c.ws, in arrival order, with a count per kind, c.cnt.                   *)
(*                                                                         *)
(* Granularity. A fast path (try_fast .. end_fast) owns the channel from   *)
(* its compare-and-swap to its releasing store, so its body is one atomic  *)
(* step that is enabled only while no claim is held (c.own = "free"); a    *)
(* try_fast that gives up is the same step taking its "decline" branch,    *)
(* and so is a recv_fast / recv_one_fast that declines on a published      *)
(* EMPTY phase after one load. Code holding the runtime lock claims the    *)
(* channel on first `data()` (channel.rs:500, claim at 527) and keeps the  *)
(* claim until the lock is released (release_claims, channel.rs:553, from  *)
(* task.rs:783), which publishes the phase; so a locked body is a step (or *)
(* a run of steps) during which no fast path can run. With one worker      *)
(* nothing claims: `data()` marks the word STALE and the next fast path    *)
(* decides from the contents (its end_fast clears STALE). Task-state       *)
(* transitions (park, wake, deliver) are atomic, as they are under each    *)
(* task's own lock (`with_task`). Interleaving semantics: every shared     *)
(* access is sequentially consistent and reads the current value.         *)
(*                                                                         *)
(* Workers: with one worker (Workers = 1) at most one task is Running and  *)
(* a task runs until it parks or finishes; the waits without the lock      *)
(* (send_free, recv_free) do not exist (task.rs:1422 can_suspend_free).    *)
(* With several workers every task has its own worker and every           *)
(* interleaving of steps is possible.                                      *)
(*                                                                         *)
(* Impl = "current" is the code as it is. Impl = "contract" is the         *)
(* Go-style hand-off as OPTIMIZATION_BACKLOG.md 1.2 asked for it, written  *)
(* before the refactor: it differs from the code only where the code is    *)
(* stricter or adds a device the contract does not need -- a publication   *)
(* may wake and forget the watchers, and no receive declines on the        *)
(* published phase. Mutation names a deliberate defect, used to show that  *)
(* the properties catch it (see README.md); three of them put back the     *)
(* defects of the code before the refactor (findings F1, F2, F3).          *)
(***************************************************************************)
EXTENDS Naturals, Sequences, FiniteSets, TLC

CONSTANTS
    Tasks,          \* task identities (a symmetry set)
    NoTask,         \* "nobody holds the runtime lock"
    Kinds,          \* subset of {"prompt", "batch"}: the channel kind is chosen at Init
    Capacities,     \* prompt capacities to explore (0 = rendezvous)
    BatchCapacity,  \* admission capacity of a batch channel
    MaxBatch,       \* messages per sealed batch (a producer appends this many)
    Ops,            \* operations each sender, receiver and selection performs
    WorkerCounts,   \* subset of {1, 3}: one worker, or one worker per task
    Impl,           \* "current" | "contract"
    Mutation,       \* "none" or a deliberate defect (below)
    MaxCancels,     \* cancellations the environment may inject
    MaxSpurious     \* wakes without a delivery (a timer, another select case)

Mutations ==
    { "none",
      "fast_ignores_watchers",  \* send_fast queues past a registered selection
      "select_no_claim",        \* the claim is dropped between probe and registration
      "no_stale",               \* one worker: locked code does not mark the word STALE
      "one_wake_per_batch",     \* F2: a publication wakes one receiver
      "cancel_retracts",        \* F1: a cancellation undoes a completed hand-off
      "queue_forgets_watchers"  \* F3: a send commits to a selection only at
    }                           \*     capacity 0; queueing wakes and forgets watchers

ASSUME Kinds \subseteq {"prompt", "batch"} /\ Kinds # {}
ASSUME WorkerCounts \subseteq {1, 3} /\ WorkerCounts # {}
ASSUME Impl \in {"current", "contract"}
ASSUME Mutation \in Mutations

VARIABLES
    cfg,    \* [kind, cap, workers]: fixed at Init
    c,      \* the channel (ChanData and its word)
    ts,     \* the tasks
    lock,   \* holder of the runtime lock, or NoTask
    res,    \* fate of every value a sender owns
    env     \* injected interrupts so far

vars == <<cfg, c, ts, lock, res, env>>

-----------------------------------------------------------------------------
(* Values: a sender t sends <<t, 1>>; a batch producer appends <<t, 1>>,   *)
(* <<t, 2>>.                                                               *)
Msgs == MaxBatch
Vals == {<<t, k>> : t \in Tasks, k \in 1..(IF Ops > MaxBatch THEN Ops ELSE MaxBatch)}
Multi == cfg.workers > 1
Cap == cfg.cap

Range(s) == {s[i] : i \in DOMAIN s}
Min(S) == CHOOSE x \in S : \A y \in S : x <= y
RECURSIVE SumLens(_)
SumLens(s) == IF s = <<>> THEN 0 ELSE Len(Head(s)) + SumLens(Tail(s))
Flatten(s) == UNION {Range(s[i]) : i \in DOMAIN s}

\* s without the elements at the indices S, in order. By value (the waiter
\* entries are distinct, OneEntryPerKind), with the built-in SelectSeq: a
\* recursive filter made the liveness runs ten times slower
DropIdx(s, S) == LET E == {s[i] : i \in S} IN SelectSeq(s, LAMBDA e : e \notin E)

-----------------------------------------------------------------------------
(* The waiter list (Waiters, channel.rs:125). An entry is a Waiter: its    *)
(* task, its kind, the wait it registered for (task::begin_wait) and, for  *)
(* a sender, its value. The counts are kept as the code keeps them         *)
(* (CountsMatch checks them against the list).                             *)

WaitKinds == {"Recv", "Send", "WatchRecv", "Capacity"}
NoVal == <<>>
W(t, k, g, v) == [t |-> t, k |-> k, g |-> g, v |-> v]

\* Once({e(x) : x \in {v}}) is e(v) with v evaluated once: TLC passes operator
\* arguments and LET definitions unevaluated and, when it evaluates an action
\* or ENABLED for fairness, re-evaluates them at every use, which nested
\* updates multiply; a bound variable is a value (the liveness runs are
\* several times faster for it)
Once(S) == CHOOSE x \in S : TRUE

OfKind(cx, k) == {i \in DOMAIN cx.ws : cx.ws[i].k = k}
TasksOf(cx, k) == {cx.ws[i].t : i \in OfKind(cx, k)}

\* task::waits / task::deliver: still in the wait it registered for
\* (parked or parking, that wait's generation), not cancelled
LiveW(tsx, e) ==
    /\ ~tsx[e.t].cancel
    /\ tsx[e.t].gen = e.g
    /\ tsx[e.t].st \in {"Parked", "Parking"}
LiveTasks(cx, k, tsx) == {cx.ws[i].t : i \in {j \in OfKind(cx, k) : LiveW(tsx, cx.ws[j])}}

\* index of the first live entry of kind k (0: none)
FirstLive(cx, k, tsx) ==
    LET I == {i \in OfKind(cx, k) : LiveW(tsx, cx.ws[i])} IN IF I = {} THEN 0 ELSE Min(I)

\* `while let Some(w) = pop(k) { if live { serve; break } }`: every entry of
\* kind k up to the i-th, which is served (every entry of kind k when i = 0)
PopTo(cx, k, i) ==
    Once({LET S == {j \in OfKind(x, k) : i = 0 \/ j <= i}
          IN [x EXCEPT !.ws = DropIdx(@, S), !.cnt[k] = @ - Cardinality(S)] : x \in {cx}})
Push(cx, e) == [cx EXCEPT !.ws = Append(@, e), !.cnt[e.k] = @ + 1]
\* Waiters::remove (a task has at most one entry of a kind: OneEntryPerKind)
RemoveTK(cx, t, k) ==
    Once({LET S == {j \in OfKind(x, k) : x.ws[j].t = t}
          IN IF S = {} THEN x ELSE [x EXCEPT !.ws = DropIdx(@, {Min(S)}), !.cnt[k] = @ - 1]
          : x \in {cx}})
RemoveTask(cx, t) ==
    RemoveTK(RemoveTK(RemoveTK(RemoveTK(cx, t, "Recv"), t, "Send"), t, "WatchRecv"), t, "Capacity")
DrainKind(cx, k) == Once({[x EXCEPT !.ws = DropIdx(@, OfKind(x, k)), !.cnt[k] = 0] : x \in {cx}})
TakeAll(cx) == [cx EXCEPT !.ws = <<>>, !.cnt = [k \in WaitKinds |-> 0]]

-----------------------------------------------------------------------------
(* The word. Whoever owns the channel publishes the phase of what it       *)
(* leaves as it lets go (ChanData::word, channel.rs:281).                  *)

Word(cx) ==
    IF cx.ws # <<>> THEN "BLOCKED"
    ELSE IF cx.queue # <<>> \/ cx.batches # <<>> THEN "VALUE"
    ELSE "EMPTY"

\* end_fast (channel.rs:629): store FREE | word(), which clears STALE
EndFast(cx) ==
    Once({[x EXCEPT !.own = "free", !.pub = Word(x), !.pclosed = x.closed, !.stale = FALSE] : x \in {cx}})

\* data() under the runtime lock: several workers claim (the compare-and-swap
\* keeps the phase bits); one worker marks the word STALE
MarkStale(cx) == IF Mutation = "no_stale" THEN cx ELSE [cx EXCEPT !.stale = TRUE]
Claim(cx) == IF Multi THEN [cx EXCEPT !.own = "claim"] ELSE MarkStale(cx)
\* the end of a locked region that used the channel: release_claims publishes
Rel(cx) ==
    IF Multi THEN Once({[x EXCEPT !.own = "free", !.pub = Word(x), !.pclosed = x.closed] : x \in {cx}})
    ELSE MarkStale(cx)

\* published() (channel.rs:577): recv_fast and recv_one_fast decline on a
\* published EMPTY phase after one load, without taking the channel (owned
\* or not: a claim keeps the phase it found)
DeclinesOnPhase(cx) == Impl = "current" /\ ~cx.stale /\ cx.pub = "EMPTY"

-----------------------------------------------------------------------------
(* Wakes and deliveries.                                                   *)

\* wake_waiting / wake_task: Parked -> Ready, Parking -> pending wake,
\* otherwise dropped (never recorded as an early wake of a running task)
WakeOne(tsx, t) ==
    Once({CASE x[t].st = "Parked"  -> [x EXCEPT ![t].st = "Ready"]
            [] x[t].st = "Parking" -> [x EXCEPT ![t].pw = TRUE]
            [] OTHER               -> x
          : x \in {tsx}})
WakeSet(tsx, S) == Once({[t \in Tasks |-> IF t \in S THEN WakeOne(x, t)[t] ELSE x[t]] : x \in {tsx}})
\* wake_with / deliver: the value (a set: {} or {v}) and status, then the wake
Give(tsx, t, V, s) == WakeOne([tsx EXCEPT ![t].dv = V, ![t].dst = s], t)
GiveSet(tsx, S, s) ==
    Once({[t \in Tasks |-> IF t \in S THEN Give(x, t, {}, s)[t] ELSE x[t]] : x \in {tsx}})

\* wake_receivers (channel.rs:1012): pop Recv entries in order, waking the
\* live ones, until n are woken: <<indices popped, tasks>>
RECURSIVE WakeRecv(_, _, _, _, _)
WakeRecv(ws, i, tsx, n, S) ==
    IF n = 0 \/ i > Len(ws) THEN <<S, tsx>>
    ELSE IF ws[i].k # "Recv" THEN WakeRecv(ws, i + 1, tsx, n, S)
    ELSE IF LiveW(tsx, ws[i]) THEN WakeRecv(ws, i + 1, WakeOne(tsx, ws[i].t), n - 1, S \cup {i})
    ELSE WakeRecv(ws, i + 1, tsx, n, S \cup {i})

\* publish (channel.rs:1472): seal the producer's staging, wake a receiver
\* per message, then wake every watcher (wake_watchers, which keeps them;
\* the contract may also forget them)
Publish(cx, tsx, p) ==
    IF cx.staging[p] = <<>> THEN <<cx, tsx>>
    ELSE LET n  == IF Mutation = "one_wake_per_batch" THEN 1 ELSE Len(cx.staging[p])
         IN Once({LET c1 == [cx EXCEPT !.batches = Append(@, cx.staging[p]), !.staging[p] = <<>>,
                                       !.ws = DropIdx(@, wr[1]), !.cnt["Recv"] = @ - Cardinality(wr[1])]
                      t1 == WakeSet(wr[2], TasksOf(c1, "WatchRecv"))
                  IN IF Impl = "contract" THEN <<DrainKind(c1, "WatchRecv"), t1>> ELSE <<c1, t1>>
                  : wr \in {WakeRecv(cx.ws, 1, tsx, n, {})}})
RECURSIVE PublishAll(_, _, _)
PublishAll(cx, tsx, S) ==
    IF S = {} THEN <<cx, tsx>>
    ELSE LET p == CHOOSE x \in S : TRUE
             r == Publish(cx, tsx, p)
         IN PublishAll(r[1], r[2], S \ {p})

\* the front published message of a batch channel
PopBatch(cx) ==
    LET b == Head(cx.batches)
    IN IF Len(b) = 1 THEN [cx EXCEPT !.batches = Tail(@)]
       ELSE [cx EXCEPT !.batches[1] = Tail(b)]

\* staged + published messages (ChanData::count; nothing is leased here)
RECURSIVE SumOver(_, _)
SumOver(f, S) ==
    IF S = {} THEN 0
    ELSE LET x == CHOOSE y \in S : TRUE IN Len(f[x]) + SumOver(f, S \ {x})
Count(cx) == SumLens(cx.batches) + SumOver(cx.staging, Tasks)

\* a task leaves (park() honoring a cancellation): a producer's staged
\* messages are published first (after_resume, Signal::Abandon), then
\* task_finished (channel.rs:1684) withdraws every registration
Leave(cx, tsx, t) ==
    Once({<< Rel(RemoveTask(p[1], t)),
             [p[2] EXCEPT ![t].st = "Done", ![t].pc = "Done", ![t].dv = {}, ![t].dst = 0] >>
          : p \in {Publish(cx, tsx, t)}})

\* park() after a wait (task.rs:1360, interrupts_after_wait at 1386): a
\* pending cancellation is honored unless a waker completed the operation
\* (status 1); the mutation is the code before c4750f8
Honors(t) == ts[t].cancel /\ (ts[t].dst # 1 \/ Mutation = "cancel_retracts")

\* at most one task runs with one worker
CpuFree == Multi \/ \A t \in Tasks : ts[t].st # "Running"

-----------------------------------------------------------------------------
Init ==
    /\ cfg \in [kind : Kinds, cap : Capacities \cup {BatchCapacity}, workers : WorkerCounts]
    /\ cfg.kind = "batch" => cfg.cap = BatchCapacity
    /\ cfg.kind = "prompt" => cfg.cap \in Capacities
    /\ c = [closed |-> FALSE, own |-> "free", pub |-> "EMPTY", pclosed |-> FALSE, stale |-> FALSE,
            queue |-> <<>>, batches |-> <<>>, staging |-> [t \in Tasks |-> <<>>],
            ws |-> <<>>, cnt |-> [k \in WaitKinds |-> 0]]
    /\ ts = [t \in Tasks |->
               [pc |-> "Start", role |-> "none", st |-> "Ready", gen |-> 0,
                cancel |-> FALSE, pw |-> FALSE, dv |-> {}, dst |-> 0,
                sel |-> FALSE, offer |-> FALSE, got |-> {}, gotClosed |-> FALSE,
                k |-> 1, woken |-> FALSE]]
    /\ lock = NoTask
    /\ res = [v \in Vals |-> "pending"]
    /\ env = [cancels |-> 0, spurious |-> 0]

PromptRoles == {"send", "recv", "select", "close"}
BatchRoles == {"append", "recvone", "select", "close"}
Entry == [send |-> "SFast", recv |-> "RFast", select |-> "SelLock",
          close |-> "CFast", append |-> "AFast", recvone |-> "OFast"]

Start(t) ==
    /\ ts[t].pc = "Start" /\ ts[t].st = "Ready" /\ CpuFree
    /\ \E r \in (IF cfg.kind = "prompt" THEN PromptRoles ELSE BatchRoles) :
          ts' = [ts EXCEPT ![t].role = r, ![t].st = "Running", ![t].pc = Entry[r]]
    /\ UNCHANGED <<cfg, c, lock, res, env>>

Running(t, l) == ts[t].pc = l /\ ts[t].st = "Running"
Goto(tsx, t, l) == [tsx EXCEPT ![t].pc = l]
Finish(tsx, t) == [tsx EXCEPT ![t].st = "Done", ![t].pc = "Done"]
\* an operation completed: the next one of the task's Ops, or the end
Complete(tsx, t) ==
    Once({IF x[t].k < Ops
          THEN [x EXCEPT ![t].k = @ + 1, ![t].st = "Running", ![t].pc = Entry[x[t].role]]
          ELSE Finish(x, t)
          : x \in {tsx}})

\* a step of a parked task's worker resuming it under the runtime lock
\* (run_one), ending the wait (end_wait)
ResumeLocked(t, l) ==
    /\ ts[t].pc = l /\ ts[t].st = "Ready" /\ lock = NoTask /\ CpuFree

-----------------------------------------------------------------------------
(* Prompt send: chan_send (intrinsics_task.rs:479) = send_fast, then       *)
(* send_free, then the locked send.                                        *)

\* send_fast (channel.rs:678): only while no selection offers to receive
\* and no sender waits
SFast(t) ==
    LET v == <<t, ts[t].k>>
        next == IF Multi THEN "SFree" ELSE "SLock"
        unserved == (IF Mutation = "fast_ignores_watchers" THEN 0 ELSE c.cnt["WatchRecv"])
                    + c.cnt["Send"]
        serves == ~c.closed /\ unserved = 0
        i == FirstLive(c, "Recv", ts)
        c0 == PopTo(c, "Recv", i)
    IN /\ Running(t, "SFast")
       /\ IF c.own # "free" THEN
             /\ ts' = Goto(ts, t, next)
             /\ UNCHANGED <<c, res>>
          ELSE IF serves /\ i > 0 THEN
             \* hand off to a parked receiver
             /\ c' = EndFast(c0)
             /\ ts' = Complete(Give(ts, c.ws[i].t, {v}, 1), t)
             /\ res' = [res EXCEPT ![v] = "sent"]
          ELSE IF serves /\ Cap > 0 /\ Len(c.queue) < Cap THEN
             /\ c' = EndFast([c0 EXCEPT !.queue = Append(@, v)])
             /\ ts' = Complete(ts, t)
             /\ res' = [res EXCEPT ![v] = "sent"]
          ELSE IF serves THEN
             /\ c' = EndFast(c0)
             /\ ts' = Goto(ts, t, next)
             /\ UNCHANGED res
          ELSE
             /\ c' = EndFast(c)
             /\ ts' = Goto(ts, t, next)
             /\ UNCHANGED res
       /\ UNCHANGED <<cfg, lock, env>>

\* send_free (channel.rs:900): several workers only
SFree(t) ==
    LET v == <<t, ts[t].k>>
        i == FirstLive(c, "Recv", ts)
        c0 == PopTo(c, "Recv", i)
    IN /\ Running(t, "SFree")
       /\ IF c.own # "free" THEN
             /\ ts' = Goto(ts, t, "SLock")
             /\ UNCHANGED <<c, res>>
          ELSE IF c.closed \/ c.cnt["WatchRecv"] > 0 THEN
             /\ c' = EndFast(c)
             /\ ts' = Goto(ts, t, "SLock")
             /\ UNCHANGED res
          ELSE IF i > 0 THEN
             /\ c' = EndFast(c0)
             /\ ts' = Complete(Give(ts, c.ws[i].t, {v}, 1), t)
             /\ res' = [res EXCEPT ![v] = "sent"]
          ELSE IF Cap > 0 /\ Len(c.queue) < Cap /\ c.cnt["Send"] = 0 THEN
             /\ c' = EndFast([c0 EXCEPT !.queue = Append(@, v)])
             /\ ts' = Complete(ts, t)
             /\ res' = [res EXCEPT ![v] = "sent"]
          ELSE IF ts[t].cancel THEN       \* begin_park_free refuses
             /\ c' = EndFast(c0)
             /\ ts' = Goto(ts, t, "SLock")
             /\ UNCHANGED res
          ELSE
             /\ c' = EndFast(Push(c0, W(t, "Send", ts[t].gen, v)))
             /\ ts' = [ts EXCEPT ![t].st = "Parking", ![t].pw = FALSE, ![t].pc = "SParkFree"]
             /\ UNCHANGED res
       /\ UNCHANGED <<cfg, lock, env>>

\* the worker after the task suspended without the lock (suspended_free,
\* task.rs:3215): a wake that arrived while parking makes it ready
ParkFree(t, l, next) ==
    /\ ts[t].pc = l /\ ts[t].st = "Parking"
    /\ ts' = [ts EXCEPT ![t].st = IF ts[t].pw THEN "Ready" ELSE "Parked",
                        ![t].pw = FALSE, ![t].pc = next]
    /\ UNCHANGED <<cfg, c, lock, res, env>>

\* run_free resumes it without the lock; take_delivered
SFreeResume(t) ==
    LET v == <<t, ts[t].k>> IN
    /\ ts[t].pc = "SFreeResume" /\ ts[t].st = "Ready" /\ CpuFree
    /\ IF ts[t].dst = 1 THEN
          /\ ts' = Complete([ts EXCEPT ![t].gen = @ + 1, ![t].dv = {}, ![t].dst = 0], t)
          /\ res' = [res EXCEPT ![v] = "sent"]
       ELSE
          /\ ts' = [ts EXCEPT ![t].st = "Running", ![t].gen = @ + 1, ![t].dv = {},
                              ![t].dst = 0, ![t].woken = TRUE, ![t].pc = "SLock"]
          /\ UNCHANGED res
    /\ UNCHANGED <<cfg, c, lock, env>>

\* take the runtime lock (and the claim); after a wait without the lock that
\* ended without completing: withdraw (channel.rs:953), then honor a
\* cancellation (after_free_wait, intrinsics_task.rs:90)
LockStep(t, l, body) ==
    LET cw == IF ts[t].woken THEN RemoveTK(RemoveTK(c, t, "Recv"), t, "Send") ELSE c IN
    /\ Running(t, l) /\ lock = NoTask
    /\ IF ts[t].woken /\ ts[t].cancel THEN
          LET r == Leave(cw, ts, t)
          IN /\ c' = r[1] /\ ts' = r[2] /\ lock' = NoTask
       ELSE
          /\ c' = Claim(cw)
          /\ ts' = [ts EXCEPT ![t].woken = FALSE, ![t].pc = body]
          /\ lock' = t
    /\ UNCHANGED <<cfg, res, env>>

\* parked selections offering a receive on this channel, still in the wait
\* they registered for (commit_select_receive, channel.rs:1204; select_offer)
SelCands(cx, tsx, me) ==
    {i \in OfKind(cx, "WatchRecv") :
        LET s == cx.ws[i].t IN
        s # me /\ LiveW(tsx, cx.ws[i]) /\ ~tsx[s].sel /\ tsx[s].offer}

\* the locked send (channel.rs:1135), holding the lock and the claim
SBody(t) ==
    LET v == <<t, ts[t].k>>
        i == FirstLive(c, "Recv", ts)
        c0 == PopTo(c, "Recv", i)
        J == SelCands(c0, ts, t)
        old == Mutation = "queue_forgets_watchers"
    IN /\ Running(t, "SBody")
       /\ IF c.closed THEN
             /\ c' = Rel(c) /\ lock' = NoTask
             /\ ts' = Complete(ts, t)
             /\ res' = [res EXCEPT ![v] = "closed"]
          ELSE IF i > 0 THEN
             /\ c' = Rel(c0) /\ lock' = NoTask
             /\ ts' = Complete(Give(ts, c.ws[i].t, {v}, 1), t)
             /\ res' = [res EXCEPT ![v] = "sent"]
          ELSE IF (~old \/ Cap = 0) /\ J # {} THEN
             \* a parked selection commits, at every capacity; its
             \* registration stays (the selection removes it)
             LET s == c0.ws[Min(J)].t IN
             /\ c' = Rel(c0) /\ lock' = NoTask
             /\ ts' = Complete(Give([ts EXCEPT ![s].sel = TRUE], s, {v}, 1), t)
             /\ res' = [res EXCEPT ![v] = "sent"]
          ELSE IF Len(c.queue) < Cap THEN
             \* queue; every watcher still registered is not parked and
             \* probes again before it parks (the mutation: wake and forget)
             LET cq == [c0 EXCEPT !.queue = Append(@, v)] IN
             /\ c' = Rel(IF old THEN DrainKind(cq, "WatchRecv") ELSE cq)
             /\ lock' = NoTask
             /\ ts' = Complete(IF old THEN WakeSet(ts, TasksOf(c0, "WatchRecv")) ELSE ts, t)
             /\ res' = [res EXCEPT ![v] = "sent"]
          ELSE IF ts[t].cancel THEN
             \* park() honors the cancellation: the task leaves, keeping its value
             LET r == Leave(c0, ts, t) IN
             /\ c' = r[1] /\ ts' = r[2] /\ lock' = NoTask
             /\ UNCHANGED res
          ELSE
             /\ c' = Rel(Push(c0, W(t, "Send", ts[t].gen, v)))
             /\ lock' = NoTask
             /\ ts' = [ts EXCEPT ![t].st = "Parked", ![t].pc = "SResume"]
             /\ UNCHANGED res
       /\ UNCHANGED <<cfg, env>>

SResume(t) ==
    LET v == <<t, ts[t].k>>
        t1 == [ts EXCEPT ![t].st = "Running", ![t].gen = @ + 1, ![t].dv = {}, ![t].dst = 0]
    IN /\ ResumeLocked(t, "SResume")
       /\ IF Honors(t) THEN
             LET r == Leave(c, t1, t) IN
             /\ c' = r[1] /\ ts' = r[2] /\ lock' = NoTask
             /\ UNCHANGED res
          ELSE IF ts[t].dst = 1 THEN
             /\ c' = Rel(c) /\ lock' = NoTask
             /\ ts' = Complete(t1, t)
             /\ res' = [res EXCEPT ![v] = "sent"]
          ELSE IF ts[t].dst = 2 \/ c.closed THEN
             /\ c' = Rel(RemoveTK(c, t, "Send")) /\ lock' = NoTask
             /\ ts' = Complete(t1, t)
             /\ res' = [res EXCEPT ![v] = "closed"]
          ELSE
             /\ c' = Claim(RemoveTK(c, t, "Send")) /\ lock' = t
             /\ ts' = Goto(t1, t, "SBody")
             /\ UNCHANGED res
       /\ UNCHANGED <<cfg, env>>

-----------------------------------------------------------------------------
(* Prompt receive: chan_recv = recv_fast, then recv_free, then the locked  *)
(* recv.                                                                   *)

\* pop the queue's head and move the first live parked sender's value into
\* the freed slot (admit_sender_fast / admit_sender): <<value, c', ts'>>
TakeQueued(cx, tsx) ==
    LET i == FirstLive(cx, "Send", tsx)
        c1 == PopTo([cx EXCEPT !.queue = Tail(@)], "Send", i)
    IN IF i > 0
       THEN << Head(cx.queue), [c1 EXCEPT !.queue = Append(@, cx.ws[i].v)],
               Give(tsx, cx.ws[i].t, {}, 1) >>
       ELSE << Head(cx.queue), c1, tsx >>

Got(tsx, t, v) == Complete([tsx EXCEPT ![t].got = @ \cup {v}], t)

\* the receive half every prompt receive shares (recv_fast's body, recv's
\* first two branches): a queued value, or a parked sender's
TryTake(cx, tsx, t, fin(_)) ==
    LET i == FirstLive(cx, "Send", tsx) IN
    IF cx.queue # <<>> THEN
        \E r \in {TakeQueued(cx, tsx)} : /\ c' = fin(r[2]) /\ ts' = Got(r[3], t, r[1])
    ELSE
        /\ c' = fin(PopTo(cx, "Send", i))
        /\ ts' = Got(Give(tsx, cx.ws[i].t, {}, 1), t, cx.ws[i].v)
CanTake(cx, tsx) == cx.queue # <<>> \/ FirstLive(cx, "Send", tsx) > 0

\* recv_fast (channel.rs:792)
RFast(t) ==
    /\ Running(t, "RFast")
    /\ IF DeclinesOnPhase(c) \/ c.own # "free" THEN
          /\ c' = c
          /\ ts' = Goto(ts, t, IF Multi THEN "RFree" ELSE "RLock")
       ELSE IF CanTake(c, ts) THEN
          /\ TryTake(c, ts, t, EndFast)
       ELSE
          /\ c' = EndFast(PopTo(c, "Send", 0))
          /\ ts' = Goto(ts, t, IF Multi THEN "RFree" ELSE "RLock")
    /\ UNCHANGED <<cfg, lock, res, env>>

\* recv_free (channel.rs:850)
RFree(t) ==
    /\ Running(t, "RFree")
    /\ IF c.own # "free" THEN
          /\ ts' = Goto(ts, t, "RLock") /\ c' = c
       ELSE IF CanTake(c, ts) THEN
          /\ TryTake(c, ts, t, EndFast)
       ELSE IF c.closed \/ ts[t].cancel THEN
          /\ c' = EndFast(PopTo(c, "Send", 0))
          /\ ts' = Goto(ts, t, "RLock")
       ELSE
          /\ c' = EndFast(Push(PopTo(c, "Send", 0), W(t, "Recv", ts[t].gen, NoVal)))
          /\ ts' = [ts EXCEPT ![t].st = "Parking", ![t].pw = FALSE, ![t].pc = "RParkFree"]
    /\ UNCHANGED <<cfg, lock, res, env>>

RFreeResume(t) ==
    /\ ts[t].pc = "RFreeResume" /\ ts[t].st = "Ready" /\ CpuFree
    /\ IF ts[t].dst = 1 THEN
          ts' = Got([ts EXCEPT ![t].gen = @ + 1, ![t].dv = {}, ![t].dst = 0], t,
                    CHOOSE x \in ts[t].dv : TRUE)
       ELSE
          ts' = [ts EXCEPT ![t].st = "Running", ![t].gen = @ + 1, ![t].dv = {},
                           ![t].dst = 0, ![t].woken = TRUE, ![t].pc = "RLock"]
    /\ UNCHANGED <<cfg, c, lock, res, env>>

\* the locked recv (channel.rs:1281)
RBody(t) ==
    LET c0 == PopTo(c, "Send", 0) IN
    /\ Running(t, "RBody")
    /\ IF CanTake(c, ts) THEN
          /\ TryTake(c, ts, t, Rel) /\ lock' = NoTask
       ELSE IF c.closed THEN
          /\ c' = Rel(c0) /\ lock' = NoTask
          /\ ts' = Complete([ts EXCEPT ![t].gotClosed = TRUE], t)
       ELSE IF ts[t].cancel THEN
          LET r == Leave(c0, ts, t) IN
          /\ c' = r[1] /\ ts' = r[2] /\ lock' = NoTask
       ELSE
          /\ c' = Rel(Push(c0, W(t, "Recv", ts[t].gen, NoVal)))
          /\ lock' = NoTask
          /\ ts' = [ts EXCEPT ![t].st = "Parked", ![t].pc = "RResume"]
    /\ UNCHANGED <<cfg, res, env>>

RResume(t) ==
    LET t1 == [ts EXCEPT ![t].st = "Running", ![t].gen = @ + 1, ![t].dv = {}, ![t].dst = 0] IN
    /\ ResumeLocked(t, "RResume")
    /\ IF Honors(t) THEN
          LET r == Leave(c, t1, t) IN /\ c' = r[1] /\ ts' = r[2] /\ lock' = NoTask
       ELSE IF ts[t].dst = 1 THEN
          /\ c' = c /\ lock' = NoTask
          /\ ts' = Got(t1, t, CHOOSE x \in ts[t].dv : TRUE)
       ELSE
          /\ c' = Claim(RemoveTK(c, t, "Recv")) /\ lock' = t
          /\ ts' = Goto(t1, t, "RBody")
    /\ UNCHANGED <<cfg, res, env>>

-----------------------------------------------------------------------------
(* Close: chan_close = close_fast, then the locked close.                  *)

\* close_fast (channel.rs:1375): prompt channels no selection watches and no
\* producer waits on
CFast(t) ==
    /\ Running(t, "CFast")
    /\ IF c.own = "free" /\ cfg.kind = "prompt"
          /\ c.cnt["WatchRecv"] = 0 /\ c.cnt["Capacity"] = 0 THEN
          /\ c' = EndFast([TakeAll(c) EXCEPT !.closed = TRUE])
          /\ ts' = Finish(GiveSet(GiveSet(ts, LiveTasks(c, "Recv", ts), 0),
                                  LiveTasks(c, "Send", ts), 2), t)
       ELSE IF c.own = "free" THEN
          /\ c' = EndFast(c) /\ ts' = Goto(ts, t, "CLock")
       ELSE
          /\ c' = c /\ ts' = Goto(ts, t, "CLock")
    /\ UNCHANGED <<cfg, lock, res, env>>

\* close (channel.rs:1327): publish every producer's staging, then wake and
\* remove every registration
CLock(t) ==
    LET p  == PublishAll(c, ts, Tasks)
        c1 == p[1]
        t1 == WakeSet(p[2], LiveTasks(c1, "Recv", p[2]))
        t2 == GiveSet(t1, LiveTasks(c1, "Send", p[2]), 2)
        t3 == WakeSet(t2, TasksOf(c1, "Capacity") \cup TasksOf(c1, "WatchRecv"))
    IN /\ Running(t, "CLock") /\ lock = NoTask
       /\ c' = Rel([TakeAll(c1) EXCEPT !.closed = TRUE])
       /\ ts' = Finish(t3, t)
       /\ UNCHANGED <<cfg, lock, res, env>>

-----------------------------------------------------------------------------
(* Select over one receive case (chan_select, intrinsics_task.rs:321): a   *)
(* locked entry. Probe, register (watch_recv, offer_cases), park; on wake  *)
(* unwatch, take a committed case or probe again.                          *)

SelLock(t) ==
    /\ Running(t, "SelLock") /\ lock = NoTask
    /\ lock' = t
    /\ c' = Claim(c)
    /\ ts' = Goto(ts, t, "SelProbe")
    /\ UNCHANGED <<cfg, res, env>>

\* recv(ch, false) / recv_one(ch, false); a batch receive wakes and removes
\* the producers waiting for capacity (wake_capacity, channel.rs:1025)
SelProbe(t) ==
    /\ Running(t, "SelProbe")
    /\ IF cfg.kind = "prompt" /\ CanTake(c, ts) THEN
          /\ TryTake(c, ts, t, Rel) /\ lock' = NoTask
       ELSE IF cfg.kind = "batch" /\ c.batches # <<>> THEN
          /\ c' = Rel(DrainKind(PopBatch(c), "Capacity")) /\ lock' = NoTask
          /\ ts' = Got(WakeSet(ts, TasksOf(c, "Capacity")), t, Head(Head(c.batches)))
       ELSE IF c.closed /\ \A x \in Tasks : c.staging[x] = <<>> THEN
          /\ c' = Rel(PopTo(c, "Send", 0)) /\ lock' = NoTask
          /\ ts' = Complete([ts EXCEPT ![t].gotClosed = TRUE], t)
       ELSE
          \* the mutation drops the claim between the probe and the registration
          LET c0 == PopTo(c, "Send", 0) IN
          /\ c' = IF Mutation = "select_no_claim" THEN Rel(c0) ELSE c0
          /\ ts' = Goto(ts, t, "SelWatch")
          /\ lock' = lock
    /\ UNCHANGED <<cfg, res, env>>

\* watch_recv (channel.rs:1449), offer_cases, park_for
SelWatch(t) ==
    /\ Running(t, "SelWatch")
    /\ IF ts[t].cancel THEN
          LET r == Leave(c, ts, t) IN /\ c' = r[1] /\ ts' = r[2]
       ELSE
          /\ c' = Rel(Push(c, W(t, "WatchRecv", ts[t].gen, NoVal)))
          /\ ts' = [ts EXCEPT ![t].offer = TRUE, ![t].sel = FALSE,
                              ![t].st = "Parked", ![t].pc = "SelResume"]
    /\ lock' = NoTask
    /\ UNCHANGED <<cfg, res, env>>

\* park returns; unwatch (channel.rs:1462); take_committed
SelResume(t) ==
    LET t1 == [ts EXCEPT ![t].st = "Running", ![t].gen = @ + 1, ![t].offer = FALSE] IN
    /\ ResumeLocked(t, "SelResume")
    /\ IF Honors(t) THEN
          LET r == Leave(c, t1, t) IN /\ c' = r[1] /\ ts' = r[2] /\ lock' = NoTask
       ELSE IF ts[t].sel THEN
          /\ c' = Rel(RemoveTK(c, t, "WatchRecv")) /\ lock' = NoTask
          /\ ts' = Got([t1 EXCEPT ![t].sel = FALSE, ![t].dv = {}, ![t].dst = 0], t,
                       CHOOSE x \in ts[t].dv : TRUE)
       ELSE
          /\ c' = Claim(RemoveTK(c, t, "WatchRecv")) /\ lock' = t
          /\ ts' = Goto(t1, t, "SelProbe")
    /\ UNCHANGED <<cfg, res, env>>

-----------------------------------------------------------------------------
(* Batch channel: a producer appends MaxBatch messages (batch_append =     *)
(* append_fast, then the locked append) and finishes, which publishes its  *)
(* staging; a receiver takes one published message (recv_one_fast, then   *)
(* the locked recv_one).                                                   *)

NextMsg(tsx, t) ==
    IF tsx[t].k < Msgs THEN [tsx EXCEPT ![t].k = @ + 1, ![t].pc = "AFast"]
    ELSE [tsx EXCEPT ![t].pc = "AFinish"]

\* append_fast (channel.rs:723)
AFast(t) ==
    LET v == <<t, ts[t].k>> IN
    /\ Running(t, "AFast")
    /\ IF c.own # "free" THEN
          /\ c' = c /\ ts' = Goto(ts, t, "ALock") /\ UNCHANGED res
       ELSE IF /\ ~c.closed /\ Count(c) < Cap
               /\ c.staging[t] # <<>> /\ Len(c.staging[t]) + 1 < MaxBatch THEN
          /\ c' = EndFast([c EXCEPT !.staging[t] = Append(@, v)])
          /\ ts' = NextMsg(ts, t)
          /\ res' = [res EXCEPT ![v] = "sent"]
       ELSE
          /\ c' = EndFast(c) /\ ts' = Goto(ts, t, "ALock") /\ UNCHANGED res
    /\ UNCHANGED <<cfg, lock, env>>

ALock(t) ==
    /\ Running(t, "ALock") /\ lock = NoTask
    /\ lock' = t /\ c' = Claim(c)
    /\ ts' = Goto(ts, t, "ABody")
    /\ UNCHANGED <<cfg, res, env>>

\* append (channel.rs:1511)
ABody(t) ==
    LET v == <<t, ts[t].k>> IN
    /\ Running(t, "ABody")
    /\ IF c.closed THEN
          /\ c' = Rel(c) /\ lock' = NoTask
          /\ ts' = NextMsg(ts, t)
          /\ res' = [res EXCEPT ![v] = "closed"]
       ELSE IF Count(c) < Cap THEN
          LET c1 == [c EXCEPT !.staging[t] = Append(@, v)]
              p  == IF Len(c1.staging[t]) >= MaxBatch THEN Publish(c1, ts, t) ELSE <<c1, ts>>
          IN /\ c' = Rel(p[1]) /\ lock' = NoTask
             /\ ts' = NextMsg(p[2], t)
             /\ res' = [res EXCEPT ![v] = "sent"]
       ELSE
          \* flush the producer's own staging, then wait for capacity
          LET p == Publish(c, ts, t) IN
          IF ts[t].cancel THEN
             LET r == Leave(p[1], p[2], t) IN
             /\ c' = r[1] /\ ts' = r[2] /\ lock' = NoTask /\ UNCHANGED res
          ELSE
             /\ c' = Rel(Push(p[1], W(t, "Capacity", ts[t].gen, NoVal))) /\ lock' = NoTask
             /\ ts' = [p[2] EXCEPT ![t].st = "Parked", ![t].pc = "AResume"]
             /\ UNCHANGED res
    /\ UNCHANGED <<cfg, env>>

AResume(t) ==
    LET t1 == [ts EXCEPT ![t].st = "Running", ![t].gen = @ + 1] IN
    /\ ResumeLocked(t, "AResume")
    /\ IF Honors(t) THEN
          LET r == Leave(c, t1, t) IN /\ c' = r[1] /\ ts' = r[2] /\ lock' = NoTask
       ELSE /\ c' = Claim(RemoveTK(c, t, "Capacity")) /\ lock' = t
            /\ ts' = Goto(t1, t, "ABody")
    /\ UNCHANGED <<cfg, res, env>>

\* the task finishes: the deschedule hook publishes what it staged
AFinish(t) ==
    LET p == Publish(c, ts, t) IN
    /\ Running(t, "AFinish") /\ lock = NoTask
    /\ c' = Rel(p[1]) /\ ts' = Finish(p[2], t)
    /\ UNCHANGED <<cfg, lock, res, env>>

\* recv_one_fast (channel.rs:753)
OFast(t) ==
    /\ Running(t, "OFast")
    /\ IF DeclinesOnPhase(c) \/ c.own # "free" THEN
          /\ c' = c /\ ts' = Goto(ts, t, "OLock")
       ELSE IF c.cnt["Capacity"] = 0 /\ c.batches # <<>> THEN
          /\ c' = EndFast(PopBatch(c))
          /\ ts' = Got(ts, t, Head(Head(c.batches)))
       ELSE
          /\ c' = EndFast(c) /\ ts' = Goto(ts, t, "OLock")
    /\ UNCHANGED <<cfg, lock, res, env>>

OLock(t) ==
    /\ Running(t, "OLock") /\ lock = NoTask
    /\ lock' = t /\ c' = Claim(c)
    /\ ts' = Goto(ts, t, "OBody")
    /\ UNCHANGED <<cfg, res, env>>

\* recv_one (channel.rs:1618)
OBody(t) ==
    /\ Running(t, "OBody")
    /\ IF c.batches # <<>> THEN
          /\ c' = Rel(DrainKind(PopBatch(c), "Capacity")) /\ lock' = NoTask
          /\ ts' = Got(WakeSet(ts, TasksOf(c, "Capacity")), t, Head(Head(c.batches)))
       ELSE IF c.closed /\ \A x \in Tasks : c.staging[x] = <<>> THEN
          /\ c' = Rel(c) /\ lock' = NoTask
          /\ ts' = Complete([ts EXCEPT ![t].gotClosed = TRUE], t)
       ELSE IF ts[t].cancel THEN
          LET r == Leave(c, ts, t) IN /\ c' = r[1] /\ ts' = r[2] /\ lock' = NoTask
       ELSE
          /\ c' = Rel(Push(c, W(t, "Recv", ts[t].gen, NoVal))) /\ lock' = NoTask
          /\ ts' = [ts EXCEPT ![t].st = "Parked", ![t].pc = "OResume"]
    /\ UNCHANGED <<cfg, res, env>>

OResume(t) ==
    LET t1 == [ts EXCEPT ![t].st = "Running", ![t].gen = @ + 1] IN
    /\ ResumeLocked(t, "OResume")
    /\ IF Honors(t) THEN
          LET r == Leave(c, t1, t) IN /\ c' = r[1] /\ ts' = r[2] /\ lock' = NoTask
       ELSE /\ c' = Claim(RemoveTK(c, t, "Recv")) /\ lock' = t
            /\ ts' = Goto(t1, t, "OBody")
    /\ UNCHANGED <<cfg, res, env>>

-----------------------------------------------------------------------------
(* The environment: a cancellation (cancel_with, task.rs:2160: set the     *)
(* flag, then wake_waiting with Cancelled) and a wake that delivers        *)
(* nothing (a timer, a case on another channel). Both take the runtime     *)
(* lock; with one worker they run from the scheduler, between tasks.       *)

EnvOk == lock = NoTask /\ CpuFree

Cancel(t) ==
    /\ EnvOk /\ env.cancels < MaxCancels
    /\ ts[t].st # "Done" /\ ts[t].pc # "Start" /\ ~ts[t].cancel
    /\ ts' = WakeOne([ts EXCEPT ![t].cancel = TRUE], t)
    /\ env' = [env EXCEPT !.cancels = @ + 1]
    /\ UNCHANGED <<cfg, c, lock, res>>

Spurious(t) ==
    /\ EnvOk /\ env.spurious < MaxSpurious
    /\ ts[t].st = "Parked"
    /\ ts' = WakeOne(ts, t)
    /\ env' = [env EXCEPT !.spurious = @ + 1]
    /\ UNCHANGED <<cfg, c, lock, res>>

-----------------------------------------------------------------------------
TaskStep(t) ==
    \/ Start(t)
    \/ SFast(t) \/ SFree(t) \/ ParkFree(t, "SParkFree", "SFreeResume") \/ SFreeResume(t)
    \/ LockStep(t, "SLock", "SBody") \/ SBody(t) \/ SResume(t)
    \/ RFast(t) \/ RFree(t) \/ ParkFree(t, "RParkFree", "RFreeResume") \/ RFreeResume(t)
    \/ LockStep(t, "RLock", "RBody") \/ RBody(t) \/ RResume(t)
    \/ CFast(t) \/ CLock(t)
    \/ SelLock(t) \/ SelProbe(t) \/ SelWatch(t) \/ SelResume(t)
    \/ AFast(t) \/ ALock(t) \/ ABody(t) \/ AResume(t) \/ AFinish(t)
    \/ OFast(t) \/ OLock(t) \/ OBody(t) \/ OResume(t)

EnvStep == \E t \in Tasks : Cancel(t) \/ Spurious(t)

Next == (\E t \in Tasks : TaskStep(t)) \/ EnvStep

\* tasks are scheduled fairly; a step that needs the runtime lock gets it
\* eventually (strong fairness); the environment is not fair
Spec == Init /\ [][Next]_vars /\ \A t \in Tasks : SF_vars(TaskStep(t))

-----------------------------------------------------------------------------
(* Properties.                                                             *)

TypeOK ==
    /\ lock \in Tasks \cup {NoTask}
    /\ c.own \in {"free", "claim"}
    /\ c.pub \in {"EMPTY", "VALUE", "BLOCKED"}
    /\ c.cnt \in [WaitKinds -> Nat]
    /\ \A i \in DOMAIN c.ws : c.ws[i].t \in Tasks /\ c.ws[i].k \in WaitKinds
    /\ \A t \in Tasks : ts[t].st \in {"Ready", "Running", "Parking", "Parked", "Done"}
    /\ res \in [Vals -> {"pending", "sent", "closed"}]

\* receive-side waits, and the registration each one keeps
RecvWaitPCs == {"RParkFree", "RFreeResume", "RResume", "SelResume", "OResume"}
HasEntry(t, k, g) == \E i \in DOMAIN c.ws : c.ws[i].t = t /\ c.ws[i].k = k /\ c.ws[i].g = g
Registered(t) ==
    CASE ts[t].pc \in {"RParkFree", "RFreeResume", "RResume", "OResume"} ->
            HasEntry(t, "Recv", ts[t].gen)
      [] ts[t].pc = "SelResume" -> HasEntry(t, "WatchRecv", ts[t].gen)
      [] ts[t].pc \in {"SParkFree", "SFreeResume", "SResume"} -> HasEntry(t, "Send", ts[t].gen)
      [] ts[t].pc = "AResume" -> HasEntry(t, "Capacity", ts[t].gen)
      [] OTHER -> FALSE
\* parked (or parking with no wake pending): no wake is on its way
Waiting(t) ==
    \/ ts[t].st = "Parked"
    \/ ts[t].st = "Parking" /\ ~ts[t].pw
RecvWaiting(t) == Waiting(t) /\ ts[t].pc \in RecvWaitPCs
\* what a receiver could take now: queued or published messages, or a
\* parked sender's value
Avail ==
    IF cfg.kind = "prompt"
    THEN Len(c.queue) + Cardinality(LiveTasks(c, "Send", ts))
    ELSE SumLens(c.batches)
Deliverable == Avail > 0 \/ c.closed

(* 2. No lost wake-up (invariant): the pending-wake relation. A receiver   *)
(* that waits with no wake pending never coexists with a closed channel,   *)
(* nor with a value it could take that no other receiver is on its way to: *)
(* every step that makes a value available hands it over or leaves a wake *)
(* pending (Ready, or Parking with pw). A prompt value is handed over, so  *)
(* none may be available while a receiver waits. A batch publication      *)
(* wakes a receiver per message to retry (wake_receivers), so a published  *)
(* message may wait for a woken receiver that has not run yet: Retrying    *)
(* counts the receivers on their way (woken in a batch receive or a        *)
(* selection and not about to honor a cancellation, or holding the lock in *)
(* its body). With at most two receivers (3 tasks) the bound is 0 there    *)
(* too; it matters from three receivers on.                                *)
Retrying ==
    {u \in Tasks :
        \/ /\ ts[u].pc \in {"OResume", "SelResume"} /\ ts[u].st = "Ready"
           /\ ~(ts[u].cancel /\ ts[u].dst # 1)
        \/ ts[u].pc \in {"OBody", "SelProbe"} /\ lock = u}
Owed == IF cfg.kind = "prompt" THEN 0 ELSE Cardinality(Retrying)
NoLostWake == \A t \in Tasks : RecvWaiting(t) => (~c.closed /\ Avail <= Owed)

(* 4. Registration is not lost (invariant): a task that waits on the       *)
(* channel with no wake pending is registered there with its current wait. *)
RegisteredWhileWaiting ==
    \A t \in Tasks : Waiting(t) => Registered(t)

(* 3. Exactly once. Holders(v): every place value v can be (the queue, a   *)
(* sealed batch, a producer's staging, a parked sender's entry, a          *)
(* receiver's delivered slot, a completed receive). At most one, always;   *)
(* exactly one once its send completed; none after it was refused.         *)
InFlight(v) == {t \in Tasks : v \in ts[t].dv /\ ts[t].dst = 1}
Holders(v) ==
    Cardinality({i \in DOMAIN c.queue : c.queue[i] = v})
    + Cardinality({i \in OfKind(c, "Send") : c.ws[i].v = v})
    + (IF v \in Flatten(c.batches) THEN 1 ELSE 0)
    + (IF \E t \in Tasks : v \in Range(c.staging[t]) THEN 1 ELSE 0)
    + Cardinality(InFlight(v))
    + Cardinality({t \in Tasks : v \in ts[t].got})
AtMostOnce == \A v \in Vals : Holders(v) <= 1
ExactlyOnce ==
    \A v \in Vals :
        /\ Holders(v) <= 1
        /\ res[v] = "sent" => Holders(v) = 1
        /\ res[v] = "closed" => Holders(v) = 0

(* 1. One step at a time (action property, prompt channels): the abstract  *)
(* state of OPTIMIZATION_BACKLOG.md 1.2 moves to a neighbour or to Closed. *)
ChanState ==
    IF c.closed THEN "Closed"
    ELSE IF Avail > 0 THEN "WithValue"
    ELSE IF \E t \in Tasks : RecvWaiting(t) THEN "Blocked"
    ELSE "Empty"
Adjacent(a, b) ==
    \/ a = b \/ b = "Closed"
    \/ {a, b} = {"Empty", "WithValue"} \/ {a, b} = {"Empty", "Blocked"}
OneStep == [][cfg.kind = "prompt" => Adjacent(ChanState, ChanState')]_vars

(* 4'. A registration is served, not forgotten (action property, prompt):  *)
(* a waiting receiver stops waiting only by being handed a value (or a     *)
(* committed case), by the channel closing, by cancellation, or by an      *)
(* environment wake -- not by a send that wakes it and forgets it.         *)
RegistrationServed ==
    [][cfg.kind = "prompt" =>
        \A t \in Tasks :
            (RecvWaiting(t) /\ ~RecvWaiting(t)' /\ ~EnvStep)
                => (ts'[t].dst = 1 \/ ts'[t].sel \/ c'.closed \/ ts'[t].cancel
                    \/ ts'[t].pc \notin RecvWaitPCs)]_vars

(* 4''. A selection's registration is removed only by the selection        *)
(* (unwatch, or its end) or by the close (action property, both kinds):    *)
(* queueing, publishing and committing wake a watcher without forgetting   *)
(* it (channel.rs:38-43). A step of another task never changes this        *)
(* task's pc, so "the selection's own step" is a step that changes it.     *)
HasWatch(t) == \E i \in OfKind(c, "WatchRecv") : c.ws[i].t = t
WatchKept ==
    [][\A t \in Tasks : (HasWatch(t) /\ ~HasWatch(t)') => (c'.closed \/ ts'[t].pc # ts[t].pc)]_vars

(* The waiter list and the word (invariants): the per-kind counts the fast *)
(* paths read are the list's (CountsMatch); a task has at most one entry   *)
(* of each kind (the debug_assert in Waiters::push); and whenever the      *)
(* channel is free and, with one worker, not STALE, the word's phase and   *)
(* closed bit are those of the contents (the claim in ChanData's doc,      *)
(* channel.rs:224-227).                                                    *)
CountsMatch == \A k \in WaitKinds : c.cnt[k] = Cardinality(OfKind(c, k))
OneEntryPerKind ==
    \A i, j \in DOMAIN c.ws : i # j => ~(c.ws[i].t = c.ws[j].t /\ c.ws[i].k = c.ws[j].k)
PhasePublished ==
    c.own = "free" /\ ~c.stale => (c.pub = Word(c) /\ c.pclosed = c.closed)

(* 2'. Every blocked receive is eventually woken (liveness), unless no     *)
(* task is left that could still send to or close the channel and nothing *)
(* is deliverable. The literal "every blocked recv is eventually woken" is *)
(* false for a receiver nobody sends to; this is the strongest form that   *)
(* is true of a correct channel.                                           *)
Producers == {t \in Tasks : ts[t].role \in {"send", "append", "close", "none"}}
Starved == ~Deliverable /\ \A t \in Producers : ts[t].st = "Done"
EveryBlockedRecvWoken ==
    \A t \in Tasks : RecvWaiting(t) ~> (~RecvWaiting(t) \/ Starved)

Symmetry == Permutations(Tasks)

\* a compact view of a trace (add `ALIAS TraceAlias` to a configuration)
TraceAlias ==
    [cfg |-> cfg, lock |-> lock, env |-> env,
     tasks |-> [t \in Tasks |-> <<ts[t].role, ts[t].pc, ts[t].st,
                                  IF ts[t].cancel THEN "cancel" ELSE "",
                                  ts[t].dv, ts[t].dst, ts[t].got>>],
     closed |-> c.closed, word |-> <<c.own, c.pub, c.stale>>, queue |-> c.queue,
     batches |-> c.batches, staging |-> c.staging,
     ws |-> [i \in DOMAIN c.ws |-> <<c.ws[i].t, c.ws[i].k, c.ws[i].g>>]]
=============================================================================
