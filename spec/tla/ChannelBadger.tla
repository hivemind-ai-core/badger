---------------------------- MODULE ChannelBadger ----------------------------
(***************************************************************************)
(* The per-channel state machine of crates/badger_runtime/src/channel.rs   *)
(* (against main at 13e7756), one channel, a bounded set of tasks.         *)
(*                                                                         *)
(* Granularity. A fast path (try_fast .. end_fast) owns the channel's      *)
(* `state` byte for its whole body, so its body is one atomic step that is *)
(* enabled only while no claim is held (c.own = "free"); a try_fast that   *)
(* gives up is the same step taking its "decline" branch. Code holding the *)
(* runtime lock claims the channel on first `data()` and keeps the claim   *)
(* until the lock is released (channel.rs:301-337, task.rs:777), so a      *)
(* locked body is a step (or a run of steps) during which no fast path can *)
(* run. Task-state transitions (park, wake, deliver) are atomic, as they   *)
(* are under each task's own lock (task.rs:1694 `with_task`).              *)
(* Interleaving semantics: every shared access is sequentially consistent  *)
(* and reads the current value (no stale reads, no compiler caching).      *)
(*                                                                         *)
(* Workers: with one worker (Workers = 1) at most one task is Running and  *)
(* a task runs until it parks or finishes; the waits without the lock      *)
(* (send_free, recv_free) do not exist (task.rs:1409 can_suspend_free) and *)
(* `data()` claims nothing. With several workers every task has its own    *)
(* worker and every interleaving of steps is possible.                     *)
(*                                                                         *)
(* Impl = "current" is the code as it is. Impl = "contract" is the         *)
(* Go-style hand-off (OPTIMIZATION_BACKLOG.md 1.2): a send that finds a    *)
(* parked selection commits to it at every capacity, a queued value does   *)
(* not wake-and-forget the watchers, and a published batch wakes one       *)
(* receiver per message. Mutation names a deliberate defect, used to show  *)
(* that the properties catch it (see README.md).                           *)
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
    Mutation,       \* "none" | "fast_ignores_watchers" | "select_no_claim"
    MaxCancels,     \* cancellations the environment may inject
    MaxSpurious     \* wakes without a delivery (a timer, another select case)

ASSUME Kinds \subseteq {"prompt", "batch"} /\ Kinds # {}
ASSUME WorkerCounts \subseteq {1, 3} /\ WorkerCounts # {}
ASSUME Impl \in {"current", "contract"}
ASSUME Mutation \in {"none", "fast_ignores_watchers", "select_no_claim"}

VARIABLES
    cfg,    \* [kind, cap, workers]: fixed at Init
    c,      \* the channel (ChanData)
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

\* waiter entries: receiver <<t, wait>>, sender <<t, wait, value>>
Without(s, t) == SelectSeq(s, LAMBDA e : e[1] # t)
WithoutT(s, t) == SelectSeq(s, LAMBDA x : x # t)
Drop(s, i) == SubSeq(s, i + 1, Len(s))

\* task::waits / task::deliver: still parked (or parking) in that wait, not cancelled
LiveW(tsx, e) ==
    /\ ~tsx[e[1]].cancel
    /\ tsx[e[1]].gen = e[2]
    /\ tsx[e[1]].st \in {"Parked", "Parking"}

\* index of the first live entry (the ones before it are popped as dead)
FirstLive(s, tsx) ==
    LET I == {i \in DOMAIN s : LiveW(tsx, s[i])} IN IF I = {} THEN 0 ELSE Min(I)

\* wake_waiting / wake_task: Parked -> Ready, Parking -> pending wake,
\* otherwise dropped (never recorded as an early wake of a running task)
WakeOne(tsx, t) ==
    CASE tsx[t].st = "Parked"  -> [tsx EXCEPT ![t].st = "Ready"]
      [] tsx[t].st = "Parking" -> [tsx EXCEPT ![t].pw = TRUE]
      [] OTHER                 -> tsx
WakeSet(tsx, S) == [t \in Tasks |-> IF t \in S THEN WakeOne(tsx, t)[t] ELSE tsx[t]]
\* wake_with / deliver: the value (a set: {} or {v}) and status, then the wake
Give(tsx, t, V, s) == WakeOne([tsx EXCEPT ![t].dv = V, ![t].dst = s], t)
GiveSet(tsx, S, s) ==
    [t \in Tasks |-> IF t \in S THEN Give(tsx, t, {}, s)[t] ELSE tsx[t]]
LiveTasks(s, tsx) == {s[i][1] : i \in {j \in DOMAIN s : LiveW(tsx, s[j])}}

\* publish (channel.rs:1081): seal the producer's staging, wake one receiver
\* (the contract: one per message), then wake and drain every watcher
RECURSIVE WakeRecv(_, _, _)
WakeRecv(s, tsx, n) ==
    IF n = 0 \/ s = <<>> THEN <<s, tsx>>
    ELSE IF LiveW(tsx, Head(s)) THEN WakeRecv(Tail(s), WakeOne(tsx, Head(s)[1]), n - 1)
    ELSE WakeRecv(Tail(s), tsx, n)
Publish(cx, tsx, p) ==
    IF cx.staging[p] = <<>> THEN <<cx, tsx>>
    ELSE LET n  == IF Impl = "contract" THEN Len(cx.staging[p]) ELSE 1
             wr == WakeRecv(cx.recvW, tsx, n)
         IN << [cx EXCEPT !.batches = Append(@, cx.staging[p]),
                          !.staging[p] = <<>>, !.recvW = wr[1], !.watch = <<>>],
               WakeSet(wr[2], Range(cx.watch)) >>
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

\* a task leaves: task_finished withdraws every registration; a producer's
\* staged messages are published first (after_resume, Signal::Abandon)
Leave(cx, tsx, t) ==
    LET p  == Publish(cx, tsx, t)
        c1 == p[1]
    IN << [c1 EXCEPT !.recvW = Without(@, t), !.sendW = Without(@, t),
                     !.capW = WithoutT(@, t), !.watch = WithoutT(@, t),
                     !.own = "free"],
          [p[2] EXCEPT ![t].st = "Done", ![t].pc = "Done", ![t].dv = {}, ![t].dst = 0] >>

\* claims are only made with several workers (channel.rs:303)
Claimed == IF Multi THEN "claim" ELSE "free"

\* at most one task runs with one worker
CpuFree == Multi \/ \A t \in Tasks : ts[t].st # "Running"

-----------------------------------------------------------------------------
Init ==
    /\ cfg \in [kind : Kinds, cap : Capacities \cup {BatchCapacity}, workers : WorkerCounts]
    /\ cfg.kind = "batch" => cfg.cap = BatchCapacity
    /\ cfg.kind = "prompt" => cfg.cap \in Capacities
    /\ c = [closed |-> FALSE, own |-> "free", queue |-> <<>>, batches |-> <<>>,
            staging |-> [t \in Tasks |-> <<>>],
            recvW |-> <<>>, sendW |-> <<>>, capW |-> <<>>, watch |-> <<>>]
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
    IF tsx[t].k < Ops
    THEN [tsx EXCEPT ![t].k = @ + 1, ![t].st = "Running", ![t].pc = Entry[tsx[t].role]]
    ELSE Finish(tsx, t)

\* a step of a parked task's worker resuming it under the runtime lock
\* (run_one), ending the wait (end_wait)
ResumeLocked(t, l) ==
    /\ ts[t].pc = l /\ ts[t].st = "Ready" /\ lock = NoTask /\ CpuFree

-----------------------------------------------------------------------------
(* Prompt send: chan_send (intrinsics_task.rs:479) = send_fast, then       *)
(* send_free, then the locked send.                                        *)

\* send_fast (channel.rs:374)
SFast(t) ==
    LET v == <<t, ts[t].k>>
        next == IF Multi THEN "SFree" ELSE "SLock"
        guard == /\ c.own = "free" /\ ~c.closed /\ c.sendW = <<>>
                 /\ (Mutation = "fast_ignores_watchers" \/ c.watch = <<>>)
        i == FirstLive(c.recvW, ts)
    IN /\ Running(t, "SFast")
       /\ IF guard /\ i > 0 THEN
             \* hand off to a parked receiver
             /\ c' = [c EXCEPT !.recvW = Drop(@, i)]
             /\ ts' = Complete(Give(ts, c.recvW[i][1], {v}, 1), t)
             /\ res' = [res EXCEPT ![v] = "sent"]
          ELSE IF guard /\ Cap > 0 /\ Len(c.queue) < Cap THEN
             /\ c' = [c EXCEPT !.recvW = <<>>, !.queue = Append(@, v)]
             /\ ts' = Complete(ts, t)
             /\ res' = [res EXCEPT ![v] = "sent"]
          ELSE IF guard THEN
             /\ c' = [c EXCEPT !.recvW = <<>>]
             /\ ts' = Goto(ts, t, next)
             /\ UNCHANGED res
          ELSE
             /\ ts' = Goto(ts, t, next)
             /\ UNCHANGED <<c, res>>
       /\ UNCHANGED <<cfg, lock, env>>

\* send_free (channel.rs:545): several workers only
SFree(t) ==
    LET v == <<t, ts[t].k>>
        i == FirstLive(c.recvW, ts)
        c0 == [c EXCEPT !.recvW = <<>>]
    IN /\ Running(t, "SFree")
       /\ IF c.own # "free" \/ c.closed \/ c.watch # <<>> THEN
             /\ ts' = Goto(ts, t, "SLock")
             /\ UNCHANGED <<c, res>>
          ELSE IF i > 0 THEN
             /\ c' = [c EXCEPT !.recvW = Drop(@, i)]
             /\ ts' = Complete(Give(ts, c.recvW[i][1], {v}, 1), t)
             /\ res' = [res EXCEPT ![v] = "sent"]
          ELSE IF Cap > 0 /\ Len(c.queue) < Cap /\ c.sendW = <<>> THEN
             /\ c' = [c0 EXCEPT !.queue = Append(@, v)]
             /\ ts' = Complete(ts, t)
             /\ res' = [res EXCEPT ![v] = "sent"]
          ELSE IF ts[t].cancel THEN       \* begin_park_free refuses
             /\ c' = c0
             /\ ts' = Goto(ts, t, "SLock")
             /\ UNCHANGED res
          ELSE
             /\ c' = [c0 EXCEPT !.sendW = Append(@, <<t, ts[t].gen, v>>)]
             /\ ts' = [ts EXCEPT ![t].st = "Parking", ![t].pw = FALSE, ![t].pc = "SParkFree"]
             /\ UNCHANGED res
       /\ UNCHANGED <<cfg, lock, env>>

\* the worker after the task suspended without the lock (suspended_free,
\* task.rs:3073): a wake that arrived while parking makes it ready
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
\* ended without completing: withdraw, then honor a cancellation
\* (after_free_wait, intrinsics_task.rs:90)
LockStep(t, l, body) ==
    /\ Running(t, l) /\ lock = NoTask
    /\ IF ts[t].woken /\ ts[t].cancel THEN
          LET r == Leave([c EXCEPT !.recvW = Without(@, t), !.sendW = Without(@, t)], ts, t)
          IN /\ c' = r[1] /\ ts' = r[2] /\ lock' = NoTask
       ELSE
          /\ c' = [c EXCEPT !.own = Claimed,
                            !.recvW = IF ts[t].woken THEN Without(@, t) ELSE @,
                            !.sendW = IF ts[t].woken THEN Without(@, t) ELSE @]
          /\ ts' = [ts EXCEPT ![t].woken = FALSE, ![t].pc = body]
          /\ lock' = t
    /\ UNCHANGED <<cfg, res, env>>

\* releasing the runtime lock releases every claim (release_claims)
Unlock(cx) == [cx EXCEPT !.own = "free"]

\* parked selections offering a receive on this channel (commit_select_receive)
SelCands(cx, tsx, me) ==
    {i \in DOMAIN cx.watch :
        LET s == cx.watch[i] IN
        s # me /\ tsx[s].st = "Parked" /\ ~tsx[s].sel /\ tsx[s].offer}

\* the locked send (channel.rs:771), holding the lock and the claim
SBody(t) ==
    LET v == <<t, ts[t].k>>
        i == FirstLive(c.recvW, ts)
        c0 == [c EXCEPT !.recvW = <<>>]
        J == SelCands(c, ts, t)
    IN /\ Running(t, "SBody")
       /\ IF c.closed THEN
             /\ c' = Unlock(c) /\ lock' = NoTask
             /\ ts' = Complete(ts, t)
             /\ res' = [res EXCEPT ![v] = "closed"]
          ELSE IF i > 0 THEN
             /\ c' = Unlock([c EXCEPT !.recvW = Drop(@, i)]) /\ lock' = NoTask
             /\ ts' = Complete(Give(ts, c.recvW[i][1], {v}, 1), t)
             /\ res' = [res EXCEPT ![v] = "sent"]
          ELSE IF (Cap = 0 \/ Impl = "contract") /\ J # {} THEN
             \* a rendezvous with a parked selection: it commits
             LET s == c.watch[Min(J)] IN
             /\ c' = Unlock(c0) /\ lock' = NoTask
             /\ ts' = Complete(Give([ts EXCEPT ![s].sel = TRUE], s, {v}, 1), t)
             /\ res' = [res EXCEPT ![v] = "sent"]
          ELSE IF Len(c.queue) < Cap THEN
             \* queue; the current code wakes every watcher and forgets them
             /\ c' = Unlock([c0 EXCEPT !.queue = Append(@, v),
                                       !.watch = IF Impl = "current" THEN <<>> ELSE @])
             /\ lock' = NoTask
             /\ ts' = Complete(IF Impl = "current" THEN WakeSet(ts, Range(c.watch)) ELSE ts, t)
             /\ res' = [res EXCEPT ![v] = "sent"]
          ELSE IF ts[t].cancel THEN
             \* park() honors the cancellation: the task leaves, keeping its value
             LET r == Leave(c0, ts, t) IN
             /\ c' = r[1] /\ ts' = r[2] /\ lock' = NoTask
             /\ UNCHANGED res
          ELSE
             /\ c' = Unlock([c0 EXCEPT !.sendW = Append(@, <<t, ts[t].gen, v>>)])
             /\ lock' = NoTask
             /\ ts' = [ts EXCEPT ![t].st = "Parked", ![t].pc = "SResume"]
             /\ UNCHANGED res
       /\ UNCHANGED <<cfg, env>>

SResume(t) ==
    LET v == <<t, ts[t].k>>
        t1 == [ts EXCEPT ![t].st = "Running", ![t].gen = @ + 1, ![t].dv = {}, ![t].dst = 0]
    IN /\ ResumeLocked(t, "SResume")
       /\ IF ts[t].cancel /\ ~(Impl = "contract" /\ ts[t].dst = 1) THEN
             LET r == Leave(c, t1, t) IN
             /\ c' = r[1] /\ ts' = r[2] /\ lock' = NoTask
             /\ UNCHANGED res
          ELSE IF ts[t].dst = 1 THEN
             /\ c' = c /\ lock' = NoTask
             /\ ts' = Complete(t1, t)
             /\ res' = [res EXCEPT ![v] = "sent"]
          ELSE IF ts[t].dst = 2 \/ c.closed THEN
             /\ c' = [c EXCEPT !.sendW = Without(@, t)] /\ lock' = NoTask
             /\ ts' = Complete(t1, t)
             /\ res' = [res EXCEPT ![v] = "closed"]
          ELSE
             /\ c' = [c EXCEPT !.sendW = Without(@, t), !.own = Claimed] /\ lock' = t
             /\ ts' = Goto(t1, t, "SBody")
             /\ UNCHANGED res
       /\ UNCHANGED <<cfg, env>>

-----------------------------------------------------------------------------
(* Prompt receive: chan_recv = recv_fast, then recv_free, then the locked  *)
(* recv.                                                                   *)

\* pop the queue's head and move the first live parked sender's value into
\* the freed slot (admit_sender_fast / admit_sender): <<value, c', ts'>>
TakeQueued(cx, tsx) ==
    LET i == FirstLive(cx.sendW, tsx)
        q1 == Tail(cx.queue)
    IN IF i > 0
       THEN << Head(cx.queue),
               [cx EXCEPT !.queue = Append(q1, cx.sendW[i][3]), !.sendW = Drop(@, i)],
               Give(tsx, cx.sendW[i][1], {}, 1) >>
       ELSE << Head(cx.queue), [cx EXCEPT !.queue = q1, !.sendW = <<>>], tsx >>

Got(tsx, t, v) == Complete([tsx EXCEPT ![t].got = @ \cup {v}], t)

\* the receive half every prompt receive shares (recv_fast's body, recv's
\* first two branches): a queued value, or a parked sender's
TryTake(cx, tsx, t, fin(_)) ==
    LET i == FirstLive(cx.sendW, tsx) IN
    IF cx.queue # <<>> THEN
        LET r == TakeQueued(cx, tsx) IN /\ c' = fin(r[2]) /\ ts' = Got(r[3], t, r[1])
    ELSE
        /\ c' = fin([cx EXCEPT !.sendW = Drop(@, i)])
        /\ ts' = Got(Give(tsx, cx.sendW[i][1], {}, 1), t, cx.sendW[i][3])
CanTake(cx, tsx) == cx.queue # <<>> \/ FirstLive(cx.sendW, tsx) > 0
Keep(cx) == cx
Rel(cx) == Unlock(cx)

RFast(t) ==
    /\ Running(t, "RFast")
    /\ IF c.own = "free" /\ CanTake(c, ts) THEN
          /\ TryTake(c, ts, t, Keep)
       ELSE
          /\ c' = IF c.own = "free" THEN [c EXCEPT !.sendW = <<>>] ELSE c
          /\ ts' = Goto(ts, t, IF Multi THEN "RFree" ELSE "RLock")
    /\ UNCHANGED <<cfg, lock, res, env>>

\* recv_free (channel.rs:497)
RFree(t) ==
    /\ Running(t, "RFree")
    /\ IF c.own # "free" THEN
          /\ ts' = Goto(ts, t, "RLock") /\ c' = c
       ELSE IF CanTake(c, ts) THEN
          /\ TryTake(c, ts, t, Keep)
       ELSE IF c.closed \/ ts[t].cancel THEN
          /\ c' = [c EXCEPT !.sendW = <<>>]
          /\ ts' = Goto(ts, t, "RLock")
       ELSE
          /\ c' = [c EXCEPT !.sendW = <<>>, !.recvW = Append(@, <<t, ts[t].gen>>)]
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

\* the locked recv (channel.rs:899)
RBody(t) ==
    /\ Running(t, "RBody")
    /\ IF CanTake(c, ts) THEN
          /\ TryTake(c, ts, t, Rel) /\ lock' = NoTask
       ELSE IF c.closed THEN
          /\ c' = Unlock([c EXCEPT !.sendW = <<>>]) /\ lock' = NoTask
          /\ ts' = Complete([ts EXCEPT ![t].gotClosed = TRUE], t)
       ELSE IF ts[t].cancel THEN
          LET r == Leave([c EXCEPT !.sendW = <<>>], ts, t) IN
          /\ c' = r[1] /\ ts' = r[2] /\ lock' = NoTask
       ELSE
          /\ c' = Unlock([c EXCEPT !.sendW = <<>>, !.recvW = Append(@, <<t, ts[t].gen>>)])
          /\ lock' = NoTask
          /\ ts' = [ts EXCEPT ![t].st = "Parked", ![t].pc = "RResume"]
    /\ UNCHANGED <<cfg, res, env>>

RResume(t) ==
    LET t1 == [ts EXCEPT ![t].st = "Running", ![t].gen = @ + 1, ![t].dv = {}, ![t].dst = 0] IN
    /\ ResumeLocked(t, "RResume")
    /\ IF ts[t].cancel /\ ~(Impl = "contract" /\ ts[t].dst = 1) THEN
          LET r == Leave(c, t1, t) IN /\ c' = r[1] /\ ts' = r[2] /\ lock' = NoTask
       ELSE IF ts[t].dst = 1 THEN
          /\ c' = c /\ lock' = NoTask
          /\ ts' = Got(t1, t, CHOOSE x \in ts[t].dv : TRUE)
       ELSE
          /\ c' = [c EXCEPT !.recvW = Without(@, t), !.own = Claimed] /\ lock' = t
          /\ ts' = Goto(t1, t, "RBody")
    /\ UNCHANGED <<cfg, res, env>>

-----------------------------------------------------------------------------
(* Close: chan_close = close_fast, then the locked close.                  *)

\* close_fast (channel.rs:987): prompt channels no selection watches
CFast(t) ==
    /\ Running(t, "CFast")
    /\ IF c.own = "free" /\ cfg.kind = "prompt" /\ c.watch = <<>> /\ c.capW = <<>> THEN
          /\ c' = [c EXCEPT !.closed = TRUE, !.recvW = <<>>, !.sendW = <<>>]
          /\ ts' = Finish(GiveSet(GiveSet(ts, LiveTasks(c.recvW, ts), 0),
                                  LiveTasks(c.sendW, ts), 2), t)
       ELSE
          /\ c' = c /\ ts' = Goto(ts, t, "CLock")
    /\ UNCHANGED <<cfg, lock, res, env>>

\* close (channel.rs:945): publish every producer's staging, then wake
\* everyone registered
CLock(t) ==
    LET p  == PublishAll(c, ts, Tasks)
        c1 == p[1]
        t1 == WakeSet(p[2], LiveTasks(c1.recvW, p[2]))
        t2 == GiveSet(t1, LiveTasks(c1.sendW, p[2]), 2)
        t3 == WakeSet(t2, Range(c1.capW) \cup Range(c1.watch))
    IN /\ Running(t, "CLock") /\ lock = NoTask
       /\ c' = [c1 EXCEPT !.closed = TRUE, !.recvW = <<>>, !.sendW = <<>>,
                          !.capW = <<>>, !.watch = <<>>]
       /\ ts' = Finish(t3, t)
       /\ UNCHANGED <<cfg, lock, res, env>>

-----------------------------------------------------------------------------
(* Select over one receive case (chan_select, intrinsics_task.rs:321): a   *)
(* locked entry. Probe, register (watch_recv, offer_cases), park; on wake  *)
(* unwatch, take a committed case or probe again.                          *)

SelLock(t) ==
    /\ Running(t, "SelLock") /\ lock = NoTask
    /\ lock' = t
    /\ c' = [c EXCEPT !.own = Claimed]
    /\ ts' = Goto(ts, t, "SelProbe")
    /\ UNCHANGED <<cfg, res, env>>

\* recv(ch, false) / recv_one(ch, false)
SelProbe(t) ==
    /\ Running(t, "SelProbe")
    /\ IF cfg.kind = "prompt" /\ CanTake(c, ts) THEN
          /\ TryTake(c, ts, t, Rel) /\ lock' = NoTask
       ELSE IF cfg.kind = "batch" /\ c.batches # <<>> THEN
          /\ c' = Unlock([PopBatch(c) EXCEPT !.capW = <<>>]) /\ lock' = NoTask
          /\ ts' = Got(WakeSet(ts, Range(c.capW)), t, Head(Head(c.batches)))
       ELSE IF c.closed /\ \A x \in Tasks : c.staging[x] = <<>> THEN
          /\ c' = Unlock([c EXCEPT !.sendW = <<>>]) /\ lock' = NoTask
          /\ ts' = Complete([ts EXCEPT ![t].gotClosed = TRUE], t)
       ELSE
          \* the mutation drops the claim between the probe and the registration
          /\ c' = [c EXCEPT !.sendW = <<>>,
                            !.own = IF Mutation = "select_no_claim" THEN "free" ELSE @]
          /\ ts' = Goto(ts, t, "SelWatch")
          /\ lock' = lock
    /\ UNCHANGED <<cfg, res, env>>

SelWatch(t) ==
    /\ Running(t, "SelWatch")
    /\ c.own \in {"free", "claim"}
    /\ IF ts[t].cancel THEN
          LET r == Leave(c, ts, t) IN /\ c' = r[1] /\ ts' = r[2]
       ELSE
          /\ c' = Unlock([c EXCEPT !.watch = Append(@, t)])
          /\ ts' = [ts EXCEPT ![t].offer = TRUE, ![t].sel = FALSE,
                              ![t].st = "Parked", ![t].pc = "SelResume"]
    /\ lock' = NoTask
    /\ UNCHANGED <<cfg, res, env>>

SelResume(t) ==
    LET t1 == [ts EXCEPT ![t].st = "Running", ![t].gen = @ + 1, ![t].offer = FALSE] IN
    /\ ResumeLocked(t, "SelResume")
    /\ IF ts[t].cancel /\ ~(Impl = "contract" /\ ts[t].sel) THEN
          LET r == Leave(c, t1, t) IN /\ c' = r[1] /\ ts' = r[2] /\ lock' = NoTask
       ELSE IF ts[t].sel THEN
          /\ c' = [c EXCEPT !.watch = WithoutT(@, t)] /\ lock' = NoTask
          /\ ts' = Got([t1 EXCEPT ![t].sel = FALSE, ![t].dv = {}, ![t].dst = 0], t,
                       CHOOSE x \in ts[t].dv : TRUE)
       ELSE
          /\ c' = [c EXCEPT !.watch = WithoutT(@, t), !.own = Claimed] /\ lock' = t
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

\* append_fast (channel.rs:408)
AFast(t) ==
    LET v == <<t, ts[t].k>> IN
    /\ Running(t, "AFast")
    /\ IF /\ c.own = "free" /\ ~c.closed /\ Count(c) < Cap
          /\ c.staging[t] # <<>> /\ Len(c.staging[t]) + 1 < MaxBatch
       THEN /\ c' = [c EXCEPT !.staging[t] = Append(@, v)]
            /\ ts' = NextMsg(ts, t)
            /\ res' = [res EXCEPT ![v] = "sent"]
       ELSE /\ c' = c /\ ts' = Goto(ts, t, "ALock") /\ UNCHANGED res
    /\ UNCHANGED <<cfg, lock, env>>

ALock(t) ==
    /\ Running(t, "ALock") /\ lock = NoTask
    /\ lock' = t /\ c' = [c EXCEPT !.own = Claimed]
    /\ ts' = Goto(ts, t, "ABody")
    /\ UNCHANGED <<cfg, res, env>>

\* append (channel.rs:1120)
ABody(t) ==
    LET v == <<t, ts[t].k>> IN
    /\ Running(t, "ABody")
    /\ IF c.closed THEN
          /\ c' = Unlock(c) /\ lock' = NoTask
          /\ ts' = NextMsg(ts, t)
          /\ res' = [res EXCEPT ![v] = "closed"]
       ELSE IF Count(c) < Cap THEN
          LET c1 == [c EXCEPT !.staging[t] = Append(@, v)]
              p  == IF Len(c1.staging[t]) >= MaxBatch THEN Publish(c1, ts, t) ELSE <<c1, ts>>
          IN /\ c' = Unlock(p[1]) /\ lock' = NoTask
             /\ ts' = NextMsg(p[2], t)
             /\ res' = [res EXCEPT ![v] = "sent"]
       ELSE
          \* flush the producer's own staging, then wait for capacity
          LET p == Publish(c, ts, t) IN
          IF ts[t].cancel THEN
             LET r == Leave(p[1], p[2], t) IN
             /\ c' = r[1] /\ ts' = r[2] /\ lock' = NoTask /\ UNCHANGED res
          ELSE
             /\ c' = Unlock([p[1] EXCEPT !.capW = Append(@, t)]) /\ lock' = NoTask
             /\ ts' = [p[2] EXCEPT ![t].st = "Parked", ![t].pc = "AResume"]
             /\ UNCHANGED res
    /\ UNCHANGED <<cfg, env>>

AResume(t) ==
    LET t1 == [ts EXCEPT ![t].st = "Running", ![t].gen = @ + 1] IN
    /\ ResumeLocked(t, "AResume")
    /\ IF ts[t].cancel THEN
          LET r == Leave(c, t1, t) IN /\ c' = r[1] /\ ts' = r[2] /\ lock' = NoTask
       ELSE /\ c' = [c EXCEPT !.own = Claimed] /\ lock' = t /\ ts' = Goto(t1, t, "ABody")
    /\ UNCHANGED <<cfg, res, env>>

\* the task finishes: the deschedule hook publishes what it staged
AFinish(t) ==
    LET p == Publish(c, ts, t) IN
    /\ Running(t, "AFinish") /\ lock = NoTask
    /\ c' = p[1] /\ ts' = Finish(p[2], t)
    /\ UNCHANGED <<cfg, lock, res, env>>

\* recv_one_fast (channel.rs:430)
OFast(t) ==
    /\ Running(t, "OFast")
    /\ IF c.own = "free" /\ c.capW = <<>> /\ c.batches # <<>> THEN
          /\ c' = PopBatch(c)
          /\ ts' = Got(ts, t, Head(Head(c.batches)))
       ELSE /\ c' = c /\ ts' = Goto(ts, t, "OLock")
    /\ UNCHANGED <<cfg, lock, res, env>>

OLock(t) ==
    /\ Running(t, "OLock") /\ lock = NoTask
    /\ lock' = t /\ c' = [c EXCEPT !.own = Claimed]
    /\ ts' = Goto(ts, t, "OBody")
    /\ UNCHANGED <<cfg, res, env>>

\* recv_one (channel.rs:1226)
OBody(t) ==
    /\ Running(t, "OBody")
    /\ IF c.batches # <<>> THEN
          /\ c' = Unlock([PopBatch(c) EXCEPT !.capW = <<>>]) /\ lock' = NoTask
          /\ ts' = Got(WakeSet(ts, Range(c.capW)), t, Head(Head(c.batches)))
       ELSE IF c.closed /\ \A x \in Tasks : c.staging[x] = <<>> THEN
          /\ c' = Unlock(c) /\ lock' = NoTask
          /\ ts' = Complete([ts EXCEPT ![t].gotClosed = TRUE], t)
       ELSE IF ts[t].cancel THEN
          LET r == Leave(c, ts, t) IN /\ c' = r[1] /\ ts' = r[2] /\ lock' = NoTask
       ELSE
          /\ c' = Unlock([c EXCEPT !.recvW = Append(@, <<t, ts[t].gen>>)]) /\ lock' = NoTask
          /\ ts' = [ts EXCEPT ![t].st = "Parked", ![t].pc = "OResume"]
    /\ UNCHANGED <<cfg, res, env>>

OResume(t) ==
    LET t1 == [ts EXCEPT ![t].st = "Running", ![t].gen = @ + 1] IN
    /\ ResumeLocked(t, "OResume")
    /\ IF ts[t].cancel THEN
          LET r == Leave(c, t1, t) IN /\ c' = r[1] /\ ts' = r[2] /\ lock' = NoTask
       ELSE /\ c' = [c EXCEPT !.recvW = Without(@, t), !.own = Claimed] /\ lock' = t
            /\ ts' = Goto(t1, t, "OBody")
    /\ UNCHANGED <<cfg, res, env>>

-----------------------------------------------------------------------------
(* The environment: a cancellation (cancel_with, task.rs:2018: set the     *)
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
    /\ \A t \in Tasks : ts[t].st \in {"Ready", "Running", "Parking", "Parked", "Done"}
    /\ res \in [Vals -> {"pending", "sent", "closed"}]

\* receive-side waits, and the registration each one keeps
RecvWaitPCs == {"RParkFree", "RFreeResume", "RResume", "SelResume", "OResume"}
Registered(t) ==
    CASE ts[t].pc \in {"RParkFree", "RFreeResume", "RResume", "OResume"} ->
            <<t, ts[t].gen>> \in Range(c.recvW)
      [] ts[t].pc = "SelResume" -> t \in Range(c.watch)
      [] ts[t].pc \in {"SParkFree", "SFreeResume", "SResume"} ->
            \E i \in DOMAIN c.sendW : c.sendW[i][1] = t /\ c.sendW[i][2] = ts[t].gen
      [] ts[t].pc = "AResume" -> t \in Range(c.capW)
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
    THEN Len(c.queue) + Cardinality(LiveTasks(c.sendW, ts))
    ELSE SumLens(c.batches)
Deliverable == Avail > 0 \/ c.closed

(* 2. No lost wake-up (invariant): the pending-wake relation. A receiver   *)
(* that waits with no wake pending never coexists with a value it could    *)
(* take or with a closed channel: every step that makes a value available  *)
(* hands it over or leaves a wake pending (Ready, or Parking with pw).     *)
NoLostWake == \A t \in Tasks : RecvWaiting(t) => ~Deliverable

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
    + Cardinality({i \in DOMAIN c.sendW : c.sendW[i][3] = v})
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
(* environment wake. The current send wakes every watcher and forgets them *)
(* (channel.rs:806-808, report point 2): that step violates this.          *)
RegistrationServed ==
    [][cfg.kind = "prompt" =>
        \A t \in Tasks :
            (RecvWaiting(t) /\ ~RecvWaiting(t)' /\ ~EnvStep)
                => (ts'[t].dst = 1 \/ ts'[t].sel \/ c'.closed \/ ts'[t].cancel
                    \/ ts'[t].pc \notin RecvWaitPCs)]_vars

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
=============================================================================
