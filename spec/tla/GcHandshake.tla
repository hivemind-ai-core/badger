----------------------------- MODULE GcHandshake -----------------------------
(***************************************************************************)
(* The stop-the-world handshake of crates/badger_runtime (6cd8e06): a      *)
(* quiescent-state grace period (QSBR)                                     *)
(*                                                                         *)
(*   gc_handshake.rs:25   EPOCH: even while workers may run compiled code, *)
(*                        odd from a collector's request until its resume  *)
(*   gc_handshake.rs:30   DEPTH: the lock holder's stops in progress       *)
(*   gc_handshake.rs:38   OBSERVED: per worker, the even epoch it observed *)
(*                        going online, or OFFLINE (above every epoch)     *)
(*   gc_handshake.rs:48   online: read EPOCH; if even, publish it in the   *)
(*                        slot, re-read EPOCH; if it moved, go offline;    *)
(*                        spin while it is odd, and retry                  *)
(*   gc_handshake.rs:69   online_locked: publish EPOCH (lock held)         *)
(*   gc_handshake.rs:79   offline: slot := OFFLINE                         *)
(*   gc_handshake.rs:93   stop_the_world: if DEPTH was 0, EPOCH := EPOCH+1 *)
(*                        and wait until each slot >= EPOCH (one slot at   *)
(*                        a time)                                          *)
(*   gc_handshake.rs:109  resume_the_world: the outermost one advances     *)
(*                        EPOCH again                                      *)
(*   task.rs:874   badger_rt_enter: offline, then take the runtime lock    *)
(*   task.rs:881   badger_rt_leave: online_locked, then unlock             *)
(*   task.rs:922   enter_runtime_holding: publish RETURNED, then enter     *)
(*   task.rs:889   badger_rt_safe_begin / _end: a foreign call goes        *)
(*                 offline; its end takes the lock, online_locked, unlock  *)
(*   task.rs:3254  suspended_free: a task suspended without the lock       *)
(*                 takes its worker offline                                *)
(*   task.rs:1503  gc_enter: online, before a worker without the lock      *)
(*                 runs compiled code                                      *)
(*   gc.rs:650     collect: stop the world, trace and sweep, resume        *)
(*   gc.rs:1339    for_each_object_of: a stop and resume of its own, which *)
(*                 nests inside a collection                               *)
(*                                                                         *)
(* One process per worker thread. A worker is "online" while it may run    *)
(* compiled code without the runtime lock; the collector is whichever      *)
(* worker holds the lock and decides to collect. Every label is one        *)
(* atomic, sequentially consistent access (the code uses SeqCst for EPOCH  *)
(* and the slots; DEPTH is touched only by the lock holder).               *)
(*                                                                         *)
(* Mutation names a deliberate defect, used to show that the properties    *)
(* catch it (README.md):                                                   *)
(*   "online_no_publish"      online returns without publishing the slot   *)
(*   "online_no_recheck"      online publishes the slot and does not       *)
(*                            re-read the epoch                            *)
(*   "online_no_backout"      online, seeing a stop after publishing,      *)
(*                            waits without going offline                  *)
(*   "unlock_before_online"   badger_rt_leave unlocks, then goes online    *)
(*   "offline_before_publish" enter_runtime_holding goes offline before    *)
(*                            it publishes RETURNED                        *)
(*   "read_before_advance"    the collector waits for the slots, then      *)
(*                            advances the epoch                           *)
(*   "scan_before_grace"      the collector reads each slot once and goes  *)
(*                            on, acknowledged or not                      *)
(*   "nested_grace"           no DEPTH: a nested stop advances the epoch   *)
(*                            and waits again, a nested resume advances it *)
(*   "nested_resume_ends"     DEPTH on the stop only: every resume         *)
(*                            advances the epoch                           *)
(*   "poll_acks"              compiled code at a poll publishes the        *)
(*                            current epoch and runs on (urcu-qsbr's       *)
(*                            quiescent-state announcement)                *)
(* and one variant that is not a defect:                                   *)
(*   "recheck_parity"         online's re-read tests only that the epoch   *)
(*                            is even, not that it is unchanged            *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets, TLC

CONSTANTS Workers, None, Mutation, MaxCollections

ASSUME Mutation \in {"none", "online_no_publish", "online_no_recheck",
                     "online_no_backout", "unlock_before_online",
                     "offline_before_publish", "read_before_advance",
                     "scan_before_grace", "nested_grace",
                     "nested_resume_ends", "poll_acks", "recheck_parity"}

\* OFFLINE: above every epoch the bounded model reaches (a collection
\* advances the epoch at most four times, under "nested_grace")
Offline == 4 * MaxCollections + 1

(* --algorithm GcHandshake
variables
    epoch = 0,                             \* EPOCH
    depth = 0,                             \* DEPTH
    slot = [w \in Workers |-> 0],          \* OBSERVED: every worker starts online
    seen = [w \in Workers |-> 0],          \* the even epoch each worker observed going online (0 offline)
    lock = None,                           \* the runtime lock
    returned = [w \in Workers |-> FALSE],  \* RETURNED: a published pointer
    held = [w \in Workers |-> FALSE],      \* a pointer compiled code returned, not yet rooted
    phase = "idle",                        \* the collector: idle | stopping | scan | sweep
    collector = None,
    ack = [w \in Workers |-> 0];           \* the epoch whose grace period last read w as acknowledged

define
    Even(n) == n % 2 = 0
    \* labels at which a worker runs compiled code or runtime code outside the
    \* lock that compiled code called (allocation, channel fast paths): it may
    \* read and write the heap and its roots
    MutatorPCs ==
        {"Compiled", "EnterOff", "HFirst", "SuspendOff", "ForeignOff"}
        \cup (IF Mutation = "offline_before_publish" THEN {} ELSE {"HSecond"})
    \* labels at which the worker's slot holds the epoch it observed: online,
    \* the step before the unlock that follows online_locked, and online's
    \* window between the publication and the back-out
    PublishedPCs ==
        MutatorPCs \cup {"ForeignUnlock", "GERecheck", "GEBack"}
        \cup (IF Mutation = "unlock_before_online" THEN {} ELSE {"LeaveSecond"})
    \* online's window: the slot is published, the re-read not yet done
    BackoutPCs == {"GERecheck", "GEBack"}
    Mutating(w) == pc[w] \in MutatorPCs
    Others == Workers \ {collector}

    TypeOK ==
        /\ epoch \in 0..(Offline - 1)
        /\ depth \in 0..2
        /\ slot \in [Workers -> 0..Offline]
        /\ lock \in Workers \cup {None}
        /\ phase \in {"idle", "stopping", "scan", "sweep"}

    \* a slot is OFFLINE exactly while its worker is in a quiescent state,
    \* and otherwise the even epoch that worker observed going online
    SlotMatches ==
        \A w \in Workers :
            IF pc[w] \in PublishedPCs THEN slot[w] = seen[w] /\ Even(seen[w])
                                      ELSE slot[w] = Offline

    \* Every worker observes the new epoch: while the collector traces and
    \* sweeps, no other worker runs compiled code, and (an invariant of every
    \* state of the scan) none enters it. Each was read as acknowledged by
    \* this epoch's grace period (ack[w] = epoch), and its slot still
    \* acknowledges the epoch, except in online's window, which backs out.
    EveryWorkerObservesEpoch ==
        phase \in {"scan", "sweep"} =>
            \A w \in Others :
                /\ ~Mutating(w)
                /\ ack[w] = epoch
                /\ slot[w] >= epoch \/ pc[w] \in BackoutPCs

    \* The collector does not scan roots while a worker can still publish
    \* one: a pointer compiled code returned is in RETURNED before its
    \* worker goes offline (enter_runtime_holding, task.rs:926).
    RootsPublished ==
        phase = "scan" => \A w \in Others : held[w] => returned[w]

    \* Tracing and sweeping happen with the world stopped (the epoch odd),
    \* under the lock, inside a stop.
    ScanUnderStop ==
        phase \in {"scan", "sweep"} => ~Even(epoch) /\ depth >= 1 /\ lock = collector

    \* The claim in stop_the_world's comment (gc_handshake.rs:91): "a slot
    \* read as acknowledged stays so for the stop". Not checked by default:
    \* online's window breaks it (README.md, "Findings").
    AcksStay ==
        (collector # None /\ ~Even(epoch)) =>
            \A w \in Workers : ack[w] = epoch => slot[w] >= epoch
end define;

fair+ process W \in Workers
variables todo = {}, target = 0;
begin
Compiled:
    \* compiled code reaches a safe point (within one task budget of polls)
    either goto EnterOff;     \* a locked runtime entry
    or     held[self] := TRUE; goto HFirst;   \* compiled code returned a pointer
    or     goto SuspendOff;   \* the task parks or yields without the lock
    or     goto ForeignOff;   \* a foreign call
    or     \* the mutation: a poll acknowledges the epoch and runs on
           await Mutation = "poll_acks" /\ slot[self] # epoch;
           slot[self] := epoch;
           goto Compiled;
    end either;

\* badger_rt_enter: offline, then take the lock
EnterOff:
    slot[self] := Offline || seen[self] := 0;
EnterLock:
    await lock = None;
    lock := self;
    goto Runtime;

\* enter_runtime_holding: RETURNED, offline, lock, root the pointer, clear RETURNED
HFirst:
    if Mutation = "offline_before_publish" then slot[self] := Offline || seen[self] := 0;
    else returned[self] := TRUE;
    end if;
HSecond:
    if Mutation = "offline_before_publish" then returned[self] := TRUE;
    else slot[self] := Offline || seen[self] := 0;
    end if;
HLock:
    await lock = None;
    lock := self;
HRoot:
    returned[self] := FALSE || held[self] := FALSE;
    goto Runtime;

\* suspended_free
SuspendOff:
    slot[self] := Offline || seen[self] := 0;
    goto FreeSched;

\* badger_rt_safe_begin; badger_rt_safe_end takes the lock first
ForeignOff:
    slot[self] := Offline || seen[self] := 0;
ForeignLock:
    await lock = None;
    lock := self;
ForeignOnline:
    \* online_locked: the lock excludes a stop, so its load and its swap are
    \* one step
    slot[self] := epoch || seen[self] := epoch;
ForeignUnlock:
    lock := None;
    goto Compiled;

\* holding the runtime lock (the scheduler, or a runtime entry), offline
Runtime:
    either
        await epoch < 2 * MaxCollections;   \* the bound: MaxCollections stops
        goto CStop;
    or
        goto LeaveFirst;
    or
        \* run_free: release the lock, run a task that suspended without it
        lock := None;
        goto FreeSched;
    or
        \* idle: wait for work without the lock
        lock := None;
        goto IdleLock;
    end either;

IdleLock:
    await lock = None;
    lock := self;
    goto Runtime;

\* badger_rt_leave: online_locked, then unlock
LeaveFirst:
    if Mutation = "unlock_before_online" then lock := None;
    else slot[self] := epoch || seen[self] := epoch;
    end if;
LeaveSecond:
    if Mutation = "unlock_before_online" then slot[self] := epoch || seen[self] := epoch;
    else lock := None;
    end if;
    goto Compiled;

\* a worker without the lock, offline (run_free's loop, idle_free)
FreeSched:
    either goto GERead;
    or     goto FreeLock;
    end either;

FreeLock:
    await lock = None;
    lock := self;
    goto Runtime;

\* gc_enter: online (gc_handshake.rs:48)
GERead:
    if Even(epoch) then seen[self] := epoch;
    else goto GESpin;
    end if;
GEPub:
    if Mutation # "online_no_publish" then slot[self] := seen[self]; end if;
    if Mutation = "online_no_recheck" then goto Compiled; end if;
GERecheck:
    if epoch = seen[self] \/ (Mutation = "recheck_parity" /\ Even(epoch)) then
        goto Compiled;
    end if;
GEBack:
    if Mutation # "online_no_backout" then slot[self] := Offline || seen[self] := 0; end if;
GESpin:
    await Even(epoch);
    goto GERead;

\* collect (gc.rs:650), holding the lock: stop_the_world
CStop:
    depth := depth + 1;
    phase := "stopping";
    collector := self;
    target := epoch + 1;
    todo := Workers;
    if Mutation = "read_before_advance" then goto CWait; end if;
CAdvance:
    epoch := target;
    if Mutation = "read_before_advance" then goto CScan; end if;
CWait:
    \* the grace period: each slot, one at a time, until it acknowledges the
    \* epoch (gc_handshake.rs:99); the model reads them in every order
    while todo # {} do
        with w \in todo do
            if Mutation = "scan_before_grace" then
                if slot[w] >= target then ack[w] := target; end if;
            else
                await slot[w] >= target;
                ack[w] := target;
            end if;
            todo := todo \ {w};
        end with;
    end while;
    if Mutation = "read_before_advance" then goto CAdvance; end if;
CScan:
    phase := "scan";
CSweep:
    phase := "sweep";
    \* a nested stop and resume during the collection (the collector
    \* inspecting the heap), or the resume
    either goto NStop;
    or     goto CResume;
    end either;

\* the nested stop: DEPTH > 0, so no second grace period
NStop:
    if Mutation = "nested_grace" then
        target := epoch + 1;
        todo := Workers;
    else
        depth := depth + 1;
        goto NResume;
    end if;
NAdvance:
    epoch := target;
NWait:
    while todo # {} do
        with w \in todo do
            await slot[w] >= target;
            todo := todo \ {w};
        end with;
    end while;
\* the nested resume: not the outermost, so the world stays stopped
NResume:
    if Mutation = "nested_grace" then
        epoch := epoch + 1;
    else
        if Mutation = "nested_resume_ends" then epoch := epoch + 1; end if;
        depth := depth - 1;
    end if;
NOuterSweep:
    \* the outer collection sweeps on
    phase := "sweep";
CResume:
    \* resume_the_world: the outermost resume ends the stop
    if depth = 1 then epoch := epoch + 1; end if;
    depth := depth - 1;
    phase := "idle";
    collector := None;
    target := 0;
    goto Runtime;
end process;
end algorithm; *)
\* BEGIN TRANSLATION
VARIABLES epoch, depth, slot, seen, lock, returned, held, phase, collector, 
          ack, pc

(* define statement *)
Even(n) == n % 2 = 0



MutatorPCs ==
    {"Compiled", "EnterOff", "HFirst", "SuspendOff", "ForeignOff"}
    \cup (IF Mutation = "offline_before_publish" THEN {} ELSE {"HSecond"})



PublishedPCs ==
    MutatorPCs \cup {"ForeignUnlock", "GERecheck", "GEBack"}
    \cup (IF Mutation = "unlock_before_online" THEN {} ELSE {"LeaveSecond"})

BackoutPCs == {"GERecheck", "GEBack"}
Mutating(w) == pc[w] \in MutatorPCs
Others == Workers \ {collector}

TypeOK ==
    /\ epoch \in 0..(Offline - 1)
    /\ depth \in 0..2
    /\ slot \in [Workers -> 0..Offline]
    /\ lock \in Workers \cup {None}
    /\ phase \in {"idle", "stopping", "scan", "sweep"}



SlotMatches ==
    \A w \in Workers :
        IF pc[w] \in PublishedPCs THEN slot[w] = seen[w] /\ Even(seen[w])
                                  ELSE slot[w] = Offline






EveryWorkerObservesEpoch ==
    phase \in {"scan", "sweep"} =>
        \A w \in Others :
            /\ ~Mutating(w)
            /\ ack[w] = epoch
            /\ slot[w] >= epoch \/ pc[w] \in BackoutPCs




RootsPublished ==
    phase = "scan" => \A w \in Others : held[w] => returned[w]



ScanUnderStop ==
    phase \in {"scan", "sweep"} => ~Even(epoch) /\ depth >= 1 /\ lock = collector




AcksStay ==
    (collector # None /\ ~Even(epoch)) =>
        \A w \in Workers : ack[w] = epoch => slot[w] >= epoch

VARIABLES todo, target

vars == << epoch, depth, slot, seen, lock, returned, held, phase, collector, 
           ack, pc, todo, target >>

ProcSet == (Workers)

Init == (* Global variables *)
        /\ epoch = 0
        /\ depth = 0
        /\ slot = [w \in Workers |-> 0]
        /\ seen = [w \in Workers |-> 0]
        /\ lock = None
        /\ returned = [w \in Workers |-> FALSE]
        /\ held = [w \in Workers |-> FALSE]
        /\ phase = "idle"
        /\ collector = None
        /\ ack = [w \in Workers |-> 0]
        (* Process W *)
        /\ todo = [self \in Workers |-> {}]
        /\ target = [self \in Workers |-> 0]
        /\ pc = [self \in ProcSet |-> "Compiled"]

Compiled(self) == /\ pc[self] = "Compiled"
                  /\ \/ /\ pc' = [pc EXCEPT ![self] = "EnterOff"]
                        /\ UNCHANGED <<slot, held>>
                     \/ /\ held' = [held EXCEPT ![self] = TRUE]
                        /\ pc' = [pc EXCEPT ![self] = "HFirst"]
                        /\ slot' = slot
                     \/ /\ pc' = [pc EXCEPT ![self] = "SuspendOff"]
                        /\ UNCHANGED <<slot, held>>
                     \/ /\ pc' = [pc EXCEPT ![self] = "ForeignOff"]
                        /\ UNCHANGED <<slot, held>>
                     \/ /\ Mutation = "poll_acks" /\ slot[self] # epoch
                        /\ slot' = [slot EXCEPT ![self] = epoch]
                        /\ pc' = [pc EXCEPT ![self] = "Compiled"]
                        /\ held' = held
                  /\ UNCHANGED << epoch, depth, seen, lock, returned, phase, 
                                  collector, ack, todo, target >>

EnterOff(self) == /\ pc[self] = "EnterOff"
                  /\ /\ seen' = [seen EXCEPT ![self] = 0]
                     /\ slot' = [slot EXCEPT ![self] = Offline]
                  /\ pc' = [pc EXCEPT ![self] = "EnterLock"]
                  /\ UNCHANGED << epoch, depth, lock, returned, held, phase, 
                                  collector, ack, todo, target >>

EnterLock(self) == /\ pc[self] = "EnterLock"
                   /\ lock = None
                   /\ lock' = self
                   /\ pc' = [pc EXCEPT ![self] = "Runtime"]
                   /\ UNCHANGED << epoch, depth, slot, seen, returned, held, 
                                   phase, collector, ack, todo, target >>

HFirst(self) == /\ pc[self] = "HFirst"
                /\ IF Mutation = "offline_before_publish"
                      THEN /\ /\ seen' = [seen EXCEPT ![self] = 0]
                              /\ slot' = [slot EXCEPT ![self] = Offline]
                           /\ UNCHANGED returned
                      ELSE /\ returned' = [returned EXCEPT ![self] = TRUE]
                           /\ UNCHANGED << slot, seen >>
                /\ pc' = [pc EXCEPT ![self] = "HSecond"]
                /\ UNCHANGED << epoch, depth, lock, held, phase, collector, 
                                ack, todo, target >>

HSecond(self) == /\ pc[self] = "HSecond"
                 /\ IF Mutation = "offline_before_publish"
                       THEN /\ returned' = [returned EXCEPT ![self] = TRUE]
                            /\ UNCHANGED << slot, seen >>
                       ELSE /\ /\ seen' = [seen EXCEPT ![self] = 0]
                               /\ slot' = [slot EXCEPT ![self] = Offline]
                            /\ UNCHANGED returned
                 /\ pc' = [pc EXCEPT ![self] = "HLock"]
                 /\ UNCHANGED << epoch, depth, lock, held, phase, collector, 
                                 ack, todo, target >>

HLock(self) == /\ pc[self] = "HLock"
               /\ lock = None
               /\ lock' = self
               /\ pc' = [pc EXCEPT ![self] = "HRoot"]
               /\ UNCHANGED << epoch, depth, slot, seen, returned, held, phase, 
                               collector, ack, todo, target >>

HRoot(self) == /\ pc[self] = "HRoot"
               /\ /\ held' = [held EXCEPT ![self] = FALSE]
                  /\ returned' = [returned EXCEPT ![self] = FALSE]
               /\ pc' = [pc EXCEPT ![self] = "Runtime"]
               /\ UNCHANGED << epoch, depth, slot, seen, lock, phase, 
                               collector, ack, todo, target >>

SuspendOff(self) == /\ pc[self] = "SuspendOff"
                    /\ /\ seen' = [seen EXCEPT ![self] = 0]
                       /\ slot' = [slot EXCEPT ![self] = Offline]
                    /\ pc' = [pc EXCEPT ![self] = "FreeSched"]
                    /\ UNCHANGED << epoch, depth, lock, returned, held, phase, 
                                    collector, ack, todo, target >>

ForeignOff(self) == /\ pc[self] = "ForeignOff"
                    /\ /\ seen' = [seen EXCEPT ![self] = 0]
                       /\ slot' = [slot EXCEPT ![self] = Offline]
                    /\ pc' = [pc EXCEPT ![self] = "ForeignLock"]
                    /\ UNCHANGED << epoch, depth, lock, returned, held, phase, 
                                    collector, ack, todo, target >>

ForeignLock(self) == /\ pc[self] = "ForeignLock"
                     /\ lock = None
                     /\ lock' = self
                     /\ pc' = [pc EXCEPT ![self] = "ForeignOnline"]
                     /\ UNCHANGED << epoch, depth, slot, seen, returned, held, 
                                     phase, collector, ack, todo, target >>

ForeignOnline(self) == /\ pc[self] = "ForeignOnline"
                       /\ /\ seen' = [seen EXCEPT ![self] = epoch]
                          /\ slot' = [slot EXCEPT ![self] = epoch]
                       /\ pc' = [pc EXCEPT ![self] = "ForeignUnlock"]
                       /\ UNCHANGED << epoch, depth, lock, returned, held, 
                                       phase, collector, ack, todo, target >>

ForeignUnlock(self) == /\ pc[self] = "ForeignUnlock"
                       /\ lock' = None
                       /\ pc' = [pc EXCEPT ![self] = "Compiled"]
                       /\ UNCHANGED << epoch, depth, slot, seen, returned, 
                                       held, phase, collector, ack, todo, 
                                       target >>

Runtime(self) == /\ pc[self] = "Runtime"
                 /\ \/ /\ epoch < 2 * MaxCollections
                       /\ pc' = [pc EXCEPT ![self] = "CStop"]
                       /\ lock' = lock
                    \/ /\ pc' = [pc EXCEPT ![self] = "LeaveFirst"]
                       /\ lock' = lock
                    \/ /\ lock' = None
                       /\ pc' = [pc EXCEPT ![self] = "FreeSched"]
                    \/ /\ lock' = None
                       /\ pc' = [pc EXCEPT ![self] = "IdleLock"]
                 /\ UNCHANGED << epoch, depth, slot, seen, returned, held, 
                                 phase, collector, ack, todo, target >>

IdleLock(self) == /\ pc[self] = "IdleLock"
                  /\ lock = None
                  /\ lock' = self
                  /\ pc' = [pc EXCEPT ![self] = "Runtime"]
                  /\ UNCHANGED << epoch, depth, slot, seen, returned, held, 
                                  phase, collector, ack, todo, target >>

LeaveFirst(self) == /\ pc[self] = "LeaveFirst"
                    /\ IF Mutation = "unlock_before_online"
                          THEN /\ lock' = None
                               /\ UNCHANGED << slot, seen >>
                          ELSE /\ /\ seen' = [seen EXCEPT ![self] = epoch]
                                  /\ slot' = [slot EXCEPT ![self] = epoch]
                               /\ lock' = lock
                    /\ pc' = [pc EXCEPT ![self] = "LeaveSecond"]
                    /\ UNCHANGED << epoch, depth, returned, held, phase, 
                                    collector, ack, todo, target >>

LeaveSecond(self) == /\ pc[self] = "LeaveSecond"
                     /\ IF Mutation = "unlock_before_online"
                           THEN /\ /\ seen' = [seen EXCEPT ![self] = epoch]
                                   /\ slot' = [slot EXCEPT ![self] = epoch]
                                /\ lock' = lock
                           ELSE /\ lock' = None
                                /\ UNCHANGED << slot, seen >>
                     /\ pc' = [pc EXCEPT ![self] = "Compiled"]
                     /\ UNCHANGED << epoch, depth, returned, held, phase, 
                                     collector, ack, todo, target >>

FreeSched(self) == /\ pc[self] = "FreeSched"
                   /\ \/ /\ pc' = [pc EXCEPT ![self] = "GERead"]
                      \/ /\ pc' = [pc EXCEPT ![self] = "FreeLock"]
                   /\ UNCHANGED << epoch, depth, slot, seen, lock, returned, 
                                   held, phase, collector, ack, todo, target >>

FreeLock(self) == /\ pc[self] = "FreeLock"
                  /\ lock = None
                  /\ lock' = self
                  /\ pc' = [pc EXCEPT ![self] = "Runtime"]
                  /\ UNCHANGED << epoch, depth, slot, seen, returned, held, 
                                  phase, collector, ack, todo, target >>

GERead(self) == /\ pc[self] = "GERead"
                /\ IF Even(epoch)
                      THEN /\ seen' = [seen EXCEPT ![self] = epoch]
                           /\ pc' = [pc EXCEPT ![self] = "GEPub"]
                      ELSE /\ pc' = [pc EXCEPT ![self] = "GESpin"]
                           /\ seen' = seen
                /\ UNCHANGED << epoch, depth, slot, lock, returned, held, 
                                phase, collector, ack, todo, target >>

GEPub(self) == /\ pc[self] = "GEPub"
               /\ IF Mutation # "online_no_publish"
                     THEN /\ slot' = [slot EXCEPT ![self] = seen[self]]
                     ELSE /\ TRUE
                          /\ slot' = slot
               /\ IF Mutation = "online_no_recheck"
                     THEN /\ pc' = [pc EXCEPT ![self] = "Compiled"]
                     ELSE /\ pc' = [pc EXCEPT ![self] = "GERecheck"]
               /\ UNCHANGED << epoch, depth, seen, lock, returned, held, phase, 
                               collector, ack, todo, target >>

GERecheck(self) == /\ pc[self] = "GERecheck"
                   /\ IF epoch = seen[self] \/ (Mutation = "recheck_parity" /\ Even(epoch))
                         THEN /\ pc' = [pc EXCEPT ![self] = "Compiled"]
                         ELSE /\ pc' = [pc EXCEPT ![self] = "GEBack"]
                   /\ UNCHANGED << epoch, depth, slot, seen, lock, returned, 
                                   held, phase, collector, ack, todo, target >>

GEBack(self) == /\ pc[self] = "GEBack"
                /\ IF Mutation # "online_no_backout"
                      THEN /\ /\ seen' = [seen EXCEPT ![self] = 0]
                              /\ slot' = [slot EXCEPT ![self] = Offline]
                      ELSE /\ TRUE
                           /\ UNCHANGED << slot, seen >>
                /\ pc' = [pc EXCEPT ![self] = "GESpin"]
                /\ UNCHANGED << epoch, depth, lock, returned, held, phase, 
                                collector, ack, todo, target >>

GESpin(self) == /\ pc[self] = "GESpin"
                /\ Even(epoch)
                /\ pc' = [pc EXCEPT ![self] = "GERead"]
                /\ UNCHANGED << epoch, depth, slot, seen, lock, returned, held, 
                                phase, collector, ack, todo, target >>

CStop(self) == /\ pc[self] = "CStop"
               /\ depth' = depth + 1
               /\ phase' = "stopping"
               /\ collector' = self
               /\ target' = [target EXCEPT ![self] = epoch + 1]
               /\ todo' = [todo EXCEPT ![self] = Workers]
               /\ IF Mutation = "read_before_advance"
                     THEN /\ pc' = [pc EXCEPT ![self] = "CWait"]
                     ELSE /\ pc' = [pc EXCEPT ![self] = "CAdvance"]
               /\ UNCHANGED << epoch, slot, seen, lock, returned, held, ack >>

CAdvance(self) == /\ pc[self] = "CAdvance"
                  /\ epoch' = target[self]
                  /\ IF Mutation = "read_before_advance"
                        THEN /\ pc' = [pc EXCEPT ![self] = "CScan"]
                        ELSE /\ pc' = [pc EXCEPT ![self] = "CWait"]
                  /\ UNCHANGED << depth, slot, seen, lock, returned, held, 
                                  phase, collector, ack, todo, target >>

CWait(self) == /\ pc[self] = "CWait"
               /\ IF todo[self] # {}
                     THEN /\ \E w \in todo[self]:
                               /\ IF Mutation = "scan_before_grace"
                                     THEN /\ IF slot[w] >= target[self]
                                                THEN /\ ack' = [ack EXCEPT ![w] = target[self]]
                                                ELSE /\ TRUE
                                                     /\ ack' = ack
                                     ELSE /\ slot[w] >= target[self]
                                          /\ ack' = [ack EXCEPT ![w] = target[self]]
                               /\ todo' = [todo EXCEPT ![self] = todo[self] \ {w}]
                          /\ pc' = [pc EXCEPT ![self] = "CWait"]
                     ELSE /\ IF Mutation = "read_before_advance"
                                THEN /\ pc' = [pc EXCEPT ![self] = "CAdvance"]
                                ELSE /\ pc' = [pc EXCEPT ![self] = "CScan"]
                          /\ UNCHANGED << ack, todo >>
               /\ UNCHANGED << epoch, depth, slot, seen, lock, returned, held, 
                               phase, collector, target >>

CScan(self) == /\ pc[self] = "CScan"
               /\ phase' = "scan"
               /\ pc' = [pc EXCEPT ![self] = "CSweep"]
               /\ UNCHANGED << epoch, depth, slot, seen, lock, returned, held, 
                               collector, ack, todo, target >>

CSweep(self) == /\ pc[self] = "CSweep"
                /\ phase' = "sweep"
                /\ \/ /\ pc' = [pc EXCEPT ![self] = "NStop"]
                   \/ /\ pc' = [pc EXCEPT ![self] = "CResume"]
                /\ UNCHANGED << epoch, depth, slot, seen, lock, returned, held, 
                                collector, ack, todo, target >>

NStop(self) == /\ pc[self] = "NStop"
               /\ IF Mutation = "nested_grace"
                     THEN /\ target' = [target EXCEPT ![self] = epoch + 1]
                          /\ todo' = [todo EXCEPT ![self] = Workers]
                          /\ pc' = [pc EXCEPT ![self] = "NAdvance"]
                          /\ depth' = depth
                     ELSE /\ depth' = depth + 1
                          /\ pc' = [pc EXCEPT ![self] = "NResume"]
                          /\ UNCHANGED << todo, target >>
               /\ UNCHANGED << epoch, slot, seen, lock, returned, held, phase, 
                               collector, ack >>

NAdvance(self) == /\ pc[self] = "NAdvance"
                  /\ epoch' = target[self]
                  /\ pc' = [pc EXCEPT ![self] = "NWait"]
                  /\ UNCHANGED << depth, slot, seen, lock, returned, held, 
                                  phase, collector, ack, todo, target >>

NWait(self) == /\ pc[self] = "NWait"
               /\ IF todo[self] # {}
                     THEN /\ \E w \in todo[self]:
                               /\ slot[w] >= target[self]
                               /\ todo' = [todo EXCEPT ![self] = todo[self] \ {w}]
                          /\ pc' = [pc EXCEPT ![self] = "NWait"]
                     ELSE /\ pc' = [pc EXCEPT ![self] = "NResume"]
                          /\ todo' = todo
               /\ UNCHANGED << epoch, depth, slot, seen, lock, returned, held, 
                               phase, collector, ack, target >>

NResume(self) == /\ pc[self] = "NResume"
                 /\ IF Mutation = "nested_grace"
                       THEN /\ epoch' = epoch + 1
                            /\ depth' = depth
                       ELSE /\ IF Mutation = "nested_resume_ends"
                                  THEN /\ epoch' = epoch + 1
                                  ELSE /\ TRUE
                                       /\ epoch' = epoch
                            /\ depth' = depth - 1
                 /\ pc' = [pc EXCEPT ![self] = "NOuterSweep"]
                 /\ UNCHANGED << slot, seen, lock, returned, held, phase, 
                                 collector, ack, todo, target >>

NOuterSweep(self) == /\ pc[self] = "NOuterSweep"
                     /\ phase' = "sweep"
                     /\ pc' = [pc EXCEPT ![self] = "CResume"]
                     /\ UNCHANGED << epoch, depth, slot, seen, lock, returned, 
                                     held, collector, ack, todo, target >>

CResume(self) == /\ pc[self] = "CResume"
                 /\ IF depth = 1
                       THEN /\ epoch' = epoch + 1
                       ELSE /\ TRUE
                            /\ epoch' = epoch
                 /\ depth' = depth - 1
                 /\ phase' = "idle"
                 /\ collector' = None
                 /\ target' = [target EXCEPT ![self] = 0]
                 /\ pc' = [pc EXCEPT ![self] = "Runtime"]
                 /\ UNCHANGED << slot, seen, lock, returned, held, ack, todo >>

W(self) == Compiled(self) \/ EnterOff(self) \/ EnterLock(self)
              \/ HFirst(self) \/ HSecond(self) \/ HLock(self)
              \/ HRoot(self) \/ SuspendOff(self) \/ ForeignOff(self)
              \/ ForeignLock(self) \/ ForeignOnline(self)
              \/ ForeignUnlock(self) \/ Runtime(self) \/ IdleLock(self)
              \/ LeaveFirst(self) \/ LeaveSecond(self) \/ FreeSched(self)
              \/ FreeLock(self) \/ GERead(self) \/ GEPub(self)
              \/ GERecheck(self) \/ GEBack(self) \/ GESpin(self)
              \/ CStop(self) \/ CAdvance(self) \/ CWait(self)
              \/ CScan(self) \/ CSweep(self) \/ NStop(self)
              \/ NAdvance(self) \/ NWait(self) \/ NResume(self)
              \/ NOuterSweep(self) \/ CResume(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in Workers: W(self))
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ \A self \in Workers : SF_vars(W(self))

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION

-----------------------------------------------------------------------------
(* Temporal properties (the bounded model: MaxCollections collections).    *)

\* the resume is ordered after the scan: the stop ends (the epoch turns
\* even) only at the end of a sweep, at the collector's outermost resume
ResumeAfterSweep ==
    [][~Even(epoch) /\ Even(epoch') =>
        /\ phase = "sweep"
        /\ pc[collector] = "CResume"
        /\ depth = 1]_vars

\* a started collection completes: every worker reaches a safe point
CollectionCompletes == (phase = "stopping") ~> (phase = "idle")
\* a worker waiting out a collection in online gets to run again
SpinnersProceed == \A w \in Workers : (pc[w] = "GESpin") ~> (pc[w] # "GESpin")
=============================================================================
