----------------------------- MODULE GcHandshake -----------------------------
(***************************************************************************)
(* The stop-the-world handshake of crates/badger_runtime (main at 13e7756) *)
(*                                                                         *)
(*   task.rs:805   STOP: AtomicBool                                        *)
(*   task.rs:791   IN_COMPILED: one AtomicIsize per worker                 *)
(*   task.rs:938   stop_the_world: STOP := true, then spin until the sum   *)
(*                 of the counters is 0 (read slot by slot)                *)
(*   task.rs:949   resume_the_world: STOP := false                         *)
(*   task.rs:1496  gc_enter: count, then read STOP; if set, uncount and    *)
(*                 spin until it clears, and retry                         *)
(*   task.rs:874   badger_rt_enter: uncount, then take the runtime lock    *)
(*   task.rs:881   badger_rt_leave: count (holding the lock), then unlock  *)
(*   task.rs:923   enter_runtime_holding: publish RETURNED, then enter     *)
(*   task.rs:889   badger_rt_safe_begin / _end: a foreign call uncounts;   *)
(*                 its end takes the lock, counts, unlocks                 *)
(*   task.rs:3073  suspended_free: a task suspended without the lock       *)
(*                 uncounts its worker                                     *)
(*   gc.rs:650     collect: stop the world, trace and sweep, resume        *)
(*                                                                         *)
(* One process per worker thread. A worker is "counted" (its IN_COMPILED   *)
(* slot is 1) exactly while it may run compiled code without the runtime   *)
(* lock; the collector is whichever worker holds the lock and decides to   *)
(* collect. Every label is one atomic, sequentially consistent access (the *)
(* code uses SeqCst for STOP and the counters).                            *)
(*                                                                         *)
(* Mutation names a deliberate reordering, used to show that the           *)
(* properties catch it (README.md):                                        *)
(*   "check_before_inc"   gc_enter reads STOP before counting itself       *)
(*   "unlock_before_inc"  badger_rt_leave unlocks before counting itself   *)
(*   "dec_before_publish" enter_runtime_holding uncounts before RETURNED   *)
(*   "nested_collect"     a finalizer collects during the sweep            *)
(*                        (resume_the_world clears STOP under the outer    *)
(*                        collection; gc.rs:347 forbids the allocation)    *)
(***************************************************************************)
EXTENDS Naturals, FiniteSets, TLC

CONSTANTS Workers, None, Mutation, MaxEpochs

ASSUME Mutation \in {"none", "check_before_inc", "unlock_before_inc",
                     "dec_before_publish", "nested_collect"}

(* --algorithm GcHandshake
variables
    stop = FALSE,                          \* STOP
    inC = [w \in Workers |-> 1],           \* IN_COMPILED, one slot per worker
    lock = None,                           \* the runtime lock
    returned = [w \in Workers |-> FALSE],  \* RETURNED: a published pointer
    held = [w \in Workers |-> FALSE],      \* a pointer compiled code returned, not yet rooted
    phase = "idle",                        \* the collector: idle | stopping | scan | sweep
    collector = None,
    epoch = 0,                             \* collections started
    ack = [w \in Workers |-> 0];           \* the epoch each worker last acknowledged

define
    \* labels at which a worker runs compiled code or runtime code outside the
    \* lock that compiled code called (allocation, channel fast paths): it may
    \* read and write the heap and its roots
    MutatorPCs ==
        {"Compiled", "EnterDec", "HFirst", "SuspendDec", "ForeignDec"}
        \cup (IF Mutation = "dec_before_publish" THEN {} ELSE {"HSecond"})
    \* labels at which the worker's slot is 1
    CountedPCs == MutatorPCs \cup {"LeaveUnlock", "ForeignUnlock", "GECheck", "GEDec"}
    Mutating(w) == pc[w] \in MutatorPCs
    Others == Workers \ {collector}

    TypeOK ==
        /\ stop \in BOOLEAN
        /\ inC \in [Workers -> {0, 1}]
        /\ lock \in Workers \cup {None}
        /\ phase \in {"idle", "stopping", "scan", "sweep"}

    \* the slot is exactly the worker's "may run compiled code" state (a
    \* slot that went negative would let the sum hide another worker's 1)
    CountMatches ==
        Mutation = "none" => \A w \in Workers : inC[w] = IF pc[w] \in CountedPCs THEN 1 ELSE 0

    \* Every worker observes the new epoch: while the collector traces and
    \* sweeps, no other worker runs compiled code, and (an invariant of every
    \* state of the scan) none enters it. ack[w] = epoch records the
    \* observation: the collector read w's slot as 0 during this epoch's
    \* stop, and w has not run compiled code since.
    EveryWorkerObservesEpoch ==
        phase \in {"scan", "sweep"} =>
            \A w \in Others : ~Mutating(w) /\ ack[w] = epoch

    \* The collector does not scan roots while a worker can still publish
    \* one: a pointer compiled code returned is in RETURNED before its
    \* worker stops counting (enter_runtime_holding, task.rs:926).
    RootsPublished ==
        phase = "scan" => \A w \in Others : held[w] => returned[w]

    \* Tracing and sweeping happen with STOP set, under the lock.
    ScanUnderStop == phase \in {"scan", "sweep"} => stop /\ lock = collector
end define;

fair+ process W \in Workers
variables sum = 0, todo = {};
begin
Compiled:
    \* compiled code reaches a safe point (within one task budget of polls)
    either goto EnterDec;     \* a locked runtime entry
    or     held[self] := TRUE; goto HFirst;   \* compiled code returned a pointer
    or     goto SuspendDec;   \* the task parks or yields without the lock
    or     goto ForeignDec;   \* a foreign call
    end either;

\* badger_rt_enter: uncount, then take the lock
EnterDec:
    inC[self] := inC[self] - 1;
EnterLock:
    await lock = None;
    lock := self;
    goto Runtime;

\* enter_runtime_holding: RETURNED, uncount, lock, root the pointer, clear RETURNED
HFirst:
    if Mutation = "dec_before_publish" then inC[self] := inC[self] - 1;
    else returned[self] := TRUE;
    end if;
HSecond:
    if Mutation = "dec_before_publish" then returned[self] := TRUE;
    else inC[self] := inC[self] - 1;
    end if;
HLock:
    await lock = None;
    lock := self;
HRoot:
    returned[self] := FALSE || held[self] := FALSE;
    goto Runtime;

\* suspended_free
SuspendDec:
    inC[self] := inC[self] - 1;
    goto FreeSched;

\* badger_rt_safe_begin; badger_rt_safe_end takes the lock first
ForeignDec:
    inC[self] := inC[self] - 1;
ForeignLock:
    await lock = None;
    lock := self;
ForeignInc:
    inC[self] := inC[self] + 1;
ForeignUnlock:
    lock := None;
    goto Compiled;

\* holding the runtime lock (the scheduler, or a runtime entry)
Runtime:
    either
        await epoch < MaxEpochs;
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

\* badger_rt_leave: count, then unlock
LeaveFirst:
    if Mutation = "unlock_before_inc" then lock := None;
    else inC[self] := inC[self] + 1;
    end if;
LeaveUnlock:
    if Mutation = "unlock_before_inc" then inC[self] := inC[self] + 1;
    else lock := None;
    end if;
    goto Compiled;

\* a worker without the lock, not counted (run_free's loop, idle_free)
FreeSched:
    either goto GEFirst;
    or     goto FreeLock;
    end either;

FreeLock:
    await lock = None;
    lock := self;
    goto Runtime;

\* gc_enter
GEFirst:
    if Mutation = "check_before_inc" then
        if stop then goto GESpin; end if;
    else
        inC[self] := inC[self] + 1;
    end if;
GECheck:
    if Mutation = "check_before_inc" then
        inC[self] := inC[self] + 1;
        goto Compiled;
    elsif ~stop then
        goto Compiled;
    end if;
GEDec:
    inC[self] := inC[self] - 1;
GESpin:
    await ~stop;
    goto GEFirst;

\* collect (gc.rs:650), holding the lock: stop_the_world
CStop:
    stop := TRUE || epoch := epoch + 1;
    phase := "stopping";
    collector := self;
CRestart:
    sum := 0;
    todo := Workers;
CRead:
    \* the sum is read one slot at a time (task.rs:943)
    while todo # {} do
        with w \in todo do
            sum := sum + inC[w];
            todo := todo \ {w};
            if inC[w] = 0 then ack[w] := epoch; end if;
        end with;
    end while;
CCheck:
    if sum > 0 then goto CRestart; end if;
CScan:
    phase := "scan";
CSweep:
    phase := "sweep";
    if Mutation = "nested_collect" then
        \* a finalizer allocates and collects: a nested stop and resume
        goto NStop;
    end if;
CResume:
    stop := FALSE || phase := "idle" || collector := None;
    goto Runtime;

\* the nested collection: STOP is a flag, not a count, so the inner resume
\* clears it while the outer collection still sweeps
NStop:
    stop := TRUE;
NResume:
    stop := FALSE;
NOuterSweep:
    phase := "sweep";
    goto CResume;
end process;
end algorithm; *)
\* BEGIN TRANSLATION
VARIABLES stop, inC, lock, returned, held, phase, collector, epoch, ack, pc

(* define statement *)
MutatorPCs ==
    {"Compiled", "EnterDec", "HFirst", "SuspendDec", "ForeignDec"}
    \cup (IF Mutation = "dec_before_publish" THEN {} ELSE {"HSecond"})

CountedPCs == MutatorPCs \cup {"LeaveUnlock", "ForeignUnlock", "GECheck", "GEDec"}
Mutating(w) == pc[w] \in MutatorPCs
Others == Workers \ {collector}

TypeOK ==
    /\ stop \in BOOLEAN
    /\ inC \in [Workers -> {0, 1}]
    /\ lock \in Workers \cup {None}
    /\ phase \in {"idle", "stopping", "scan", "sweep"}



CountMatches ==
    Mutation = "none" => \A w \in Workers : inC[w] = IF pc[w] \in CountedPCs THEN 1 ELSE 0






EveryWorkerObservesEpoch ==
    phase \in {"scan", "sweep"} =>
        \A w \in Others : ~Mutating(w) /\ ack[w] = epoch




RootsPublished ==
    phase = "scan" => \A w \in Others : held[w] => returned[w]


ScanUnderStop == phase \in {"scan", "sweep"} => stop /\ lock = collector

VARIABLES sum, todo

vars == << stop, inC, lock, returned, held, phase, collector, epoch, ack, pc, 
           sum, todo >>

ProcSet == (Workers)

Init == (* Global variables *)
        /\ stop = FALSE
        /\ inC = [w \in Workers |-> 1]
        /\ lock = None
        /\ returned = [w \in Workers |-> FALSE]
        /\ held = [w \in Workers |-> FALSE]
        /\ phase = "idle"
        /\ collector = None
        /\ epoch = 0
        /\ ack = [w \in Workers |-> 0]
        (* Process W *)
        /\ sum = [self \in Workers |-> 0]
        /\ todo = [self \in Workers |-> {}]
        /\ pc = [self \in ProcSet |-> "Compiled"]

Compiled(self) == /\ pc[self] = "Compiled"
                  /\ \/ /\ pc' = [pc EXCEPT ![self] = "EnterDec"]
                        /\ held' = held
                     \/ /\ held' = [held EXCEPT ![self] = TRUE]
                        /\ pc' = [pc EXCEPT ![self] = "HFirst"]
                     \/ /\ pc' = [pc EXCEPT ![self] = "SuspendDec"]
                        /\ held' = held
                     \/ /\ pc' = [pc EXCEPT ![self] = "ForeignDec"]
                        /\ held' = held
                  /\ UNCHANGED << stop, inC, lock, returned, phase, collector, 
                                  epoch, ack, sum, todo >>

EnterDec(self) == /\ pc[self] = "EnterDec"
                  /\ inC' = [inC EXCEPT ![self] = inC[self] - 1]
                  /\ pc' = [pc EXCEPT ![self] = "EnterLock"]
                  /\ UNCHANGED << stop, lock, returned, held, phase, collector, 
                                  epoch, ack, sum, todo >>

EnterLock(self) == /\ pc[self] = "EnterLock"
                   /\ lock = None
                   /\ lock' = self
                   /\ pc' = [pc EXCEPT ![self] = "Runtime"]
                   /\ UNCHANGED << stop, inC, returned, held, phase, collector, 
                                   epoch, ack, sum, todo >>

HFirst(self) == /\ pc[self] = "HFirst"
                /\ IF Mutation = "dec_before_publish"
                      THEN /\ inC' = [inC EXCEPT ![self] = inC[self] - 1]
                           /\ UNCHANGED returned
                      ELSE /\ returned' = [returned EXCEPT ![self] = TRUE]
                           /\ inC' = inC
                /\ pc' = [pc EXCEPT ![self] = "HSecond"]
                /\ UNCHANGED << stop, lock, held, phase, collector, epoch, ack, 
                                sum, todo >>

HSecond(self) == /\ pc[self] = "HSecond"
                 /\ IF Mutation = "dec_before_publish"
                       THEN /\ returned' = [returned EXCEPT ![self] = TRUE]
                            /\ inC' = inC
                       ELSE /\ inC' = [inC EXCEPT ![self] = inC[self] - 1]
                            /\ UNCHANGED returned
                 /\ pc' = [pc EXCEPT ![self] = "HLock"]
                 /\ UNCHANGED << stop, lock, held, phase, collector, epoch, 
                                 ack, sum, todo >>

HLock(self) == /\ pc[self] = "HLock"
               /\ lock = None
               /\ lock' = self
               /\ pc' = [pc EXCEPT ![self] = "HRoot"]
               /\ UNCHANGED << stop, inC, returned, held, phase, collector, 
                               epoch, ack, sum, todo >>

HRoot(self) == /\ pc[self] = "HRoot"
               /\ /\ held' = [held EXCEPT ![self] = FALSE]
                  /\ returned' = [returned EXCEPT ![self] = FALSE]
               /\ pc' = [pc EXCEPT ![self] = "Runtime"]
               /\ UNCHANGED << stop, inC, lock, phase, collector, epoch, ack, 
                               sum, todo >>

SuspendDec(self) == /\ pc[self] = "SuspendDec"
                    /\ inC' = [inC EXCEPT ![self] = inC[self] - 1]
                    /\ pc' = [pc EXCEPT ![self] = "FreeSched"]
                    /\ UNCHANGED << stop, lock, returned, held, phase, 
                                    collector, epoch, ack, sum, todo >>

ForeignDec(self) == /\ pc[self] = "ForeignDec"
                    /\ inC' = [inC EXCEPT ![self] = inC[self] - 1]
                    /\ pc' = [pc EXCEPT ![self] = "ForeignLock"]
                    /\ UNCHANGED << stop, lock, returned, held, phase, 
                                    collector, epoch, ack, sum, todo >>

ForeignLock(self) == /\ pc[self] = "ForeignLock"
                     /\ lock = None
                     /\ lock' = self
                     /\ pc' = [pc EXCEPT ![self] = "ForeignInc"]
                     /\ UNCHANGED << stop, inC, returned, held, phase, 
                                     collector, epoch, ack, sum, todo >>

ForeignInc(self) == /\ pc[self] = "ForeignInc"
                    /\ inC' = [inC EXCEPT ![self] = inC[self] + 1]
                    /\ pc' = [pc EXCEPT ![self] = "ForeignUnlock"]
                    /\ UNCHANGED << stop, lock, returned, held, phase, 
                                    collector, epoch, ack, sum, todo >>

ForeignUnlock(self) == /\ pc[self] = "ForeignUnlock"
                       /\ lock' = None
                       /\ pc' = [pc EXCEPT ![self] = "Compiled"]
                       /\ UNCHANGED << stop, inC, returned, held, phase, 
                                       collector, epoch, ack, sum, todo >>

Runtime(self) == /\ pc[self] = "Runtime"
                 /\ \/ /\ epoch < MaxEpochs
                       /\ pc' = [pc EXCEPT ![self] = "CStop"]
                       /\ lock' = lock
                    \/ /\ pc' = [pc EXCEPT ![self] = "LeaveFirst"]
                       /\ lock' = lock
                    \/ /\ lock' = None
                       /\ pc' = [pc EXCEPT ![self] = "FreeSched"]
                    \/ /\ lock' = None
                       /\ pc' = [pc EXCEPT ![self] = "IdleLock"]
                 /\ UNCHANGED << stop, inC, returned, held, phase, collector, 
                                 epoch, ack, sum, todo >>

IdleLock(self) == /\ pc[self] = "IdleLock"
                  /\ lock = None
                  /\ lock' = self
                  /\ pc' = [pc EXCEPT ![self] = "Runtime"]
                  /\ UNCHANGED << stop, inC, returned, held, phase, collector, 
                                  epoch, ack, sum, todo >>

LeaveFirst(self) == /\ pc[self] = "LeaveFirst"
                    /\ IF Mutation = "unlock_before_inc"
                          THEN /\ lock' = None
                               /\ inC' = inC
                          ELSE /\ inC' = [inC EXCEPT ![self] = inC[self] + 1]
                               /\ lock' = lock
                    /\ pc' = [pc EXCEPT ![self] = "LeaveUnlock"]
                    /\ UNCHANGED << stop, returned, held, phase, collector, 
                                    epoch, ack, sum, todo >>

LeaveUnlock(self) == /\ pc[self] = "LeaveUnlock"
                     /\ IF Mutation = "unlock_before_inc"
                           THEN /\ inC' = [inC EXCEPT ![self] = inC[self] + 1]
                                /\ lock' = lock
                           ELSE /\ lock' = None
                                /\ inC' = inC
                     /\ pc' = [pc EXCEPT ![self] = "Compiled"]
                     /\ UNCHANGED << stop, returned, held, phase, collector, 
                                     epoch, ack, sum, todo >>

FreeSched(self) == /\ pc[self] = "FreeSched"
                   /\ \/ /\ pc' = [pc EXCEPT ![self] = "GEFirst"]
                      \/ /\ pc' = [pc EXCEPT ![self] = "FreeLock"]
                   /\ UNCHANGED << stop, inC, lock, returned, held, phase, 
                                   collector, epoch, ack, sum, todo >>

FreeLock(self) == /\ pc[self] = "FreeLock"
                  /\ lock = None
                  /\ lock' = self
                  /\ pc' = [pc EXCEPT ![self] = "Runtime"]
                  /\ UNCHANGED << stop, inC, returned, held, phase, collector, 
                                  epoch, ack, sum, todo >>

GEFirst(self) == /\ pc[self] = "GEFirst"
                 /\ IF Mutation = "check_before_inc"
                       THEN /\ IF stop
                                  THEN /\ pc' = [pc EXCEPT ![self] = "GESpin"]
                                  ELSE /\ pc' = [pc EXCEPT ![self] = "GECheck"]
                            /\ inC' = inC
                       ELSE /\ inC' = [inC EXCEPT ![self] = inC[self] + 1]
                            /\ pc' = [pc EXCEPT ![self] = "GECheck"]
                 /\ UNCHANGED << stop, lock, returned, held, phase, collector, 
                                 epoch, ack, sum, todo >>

GECheck(self) == /\ pc[self] = "GECheck"
                 /\ IF Mutation = "check_before_inc"
                       THEN /\ inC' = [inC EXCEPT ![self] = inC[self] + 1]
                            /\ pc' = [pc EXCEPT ![self] = "Compiled"]
                       ELSE /\ IF ~stop
                                  THEN /\ pc' = [pc EXCEPT ![self] = "Compiled"]
                                  ELSE /\ pc' = [pc EXCEPT ![self] = "GEDec"]
                            /\ inC' = inC
                 /\ UNCHANGED << stop, lock, returned, held, phase, collector, 
                                 epoch, ack, sum, todo >>

GEDec(self) == /\ pc[self] = "GEDec"
               /\ inC' = [inC EXCEPT ![self] = inC[self] - 1]
               /\ pc' = [pc EXCEPT ![self] = "GESpin"]
               /\ UNCHANGED << stop, lock, returned, held, phase, collector, 
                               epoch, ack, sum, todo >>

GESpin(self) == /\ pc[self] = "GESpin"
                /\ ~stop
                /\ pc' = [pc EXCEPT ![self] = "GEFirst"]
                /\ UNCHANGED << stop, inC, lock, returned, held, phase, 
                                collector, epoch, ack, sum, todo >>

CStop(self) == /\ pc[self] = "CStop"
               /\ /\ epoch' = epoch + 1
                  /\ stop' = TRUE
               /\ phase' = "stopping"
               /\ collector' = self
               /\ pc' = [pc EXCEPT ![self] = "CRestart"]
               /\ UNCHANGED << inC, lock, returned, held, ack, sum, todo >>

CRestart(self) == /\ pc[self] = "CRestart"
                  /\ sum' = [sum EXCEPT ![self] = 0]
                  /\ todo' = [todo EXCEPT ![self] = Workers]
                  /\ pc' = [pc EXCEPT ![self] = "CRead"]
                  /\ UNCHANGED << stop, inC, lock, returned, held, phase, 
                                  collector, epoch, ack >>

CRead(self) == /\ pc[self] = "CRead"
               /\ IF todo[self] # {}
                     THEN /\ \E w \in todo[self]:
                               /\ sum' = [sum EXCEPT ![self] = sum[self] + inC[w]]
                               /\ todo' = [todo EXCEPT ![self] = todo[self] \ {w}]
                               /\ IF inC[w] = 0
                                     THEN /\ ack' = [ack EXCEPT ![w] = epoch]
                                     ELSE /\ TRUE
                                          /\ ack' = ack
                          /\ pc' = [pc EXCEPT ![self] = "CRead"]
                     ELSE /\ pc' = [pc EXCEPT ![self] = "CCheck"]
                          /\ UNCHANGED << ack, sum, todo >>
               /\ UNCHANGED << stop, inC, lock, returned, held, phase, 
                               collector, epoch >>

CCheck(self) == /\ pc[self] = "CCheck"
                /\ IF sum[self] > 0
                      THEN /\ pc' = [pc EXCEPT ![self] = "CRestart"]
                      ELSE /\ pc' = [pc EXCEPT ![self] = "CScan"]
                /\ UNCHANGED << stop, inC, lock, returned, held, phase, 
                                collector, epoch, ack, sum, todo >>

CScan(self) == /\ pc[self] = "CScan"
               /\ phase' = "scan"
               /\ pc' = [pc EXCEPT ![self] = "CSweep"]
               /\ UNCHANGED << stop, inC, lock, returned, held, collector, 
                               epoch, ack, sum, todo >>

CSweep(self) == /\ pc[self] = "CSweep"
                /\ phase' = "sweep"
                /\ IF Mutation = "nested_collect"
                      THEN /\ pc' = [pc EXCEPT ![self] = "NStop"]
                      ELSE /\ pc' = [pc EXCEPT ![self] = "CResume"]
                /\ UNCHANGED << stop, inC, lock, returned, held, collector, 
                                epoch, ack, sum, todo >>

CResume(self) == /\ pc[self] = "CResume"
                 /\ /\ collector' = None
                    /\ phase' = "idle"
                    /\ stop' = FALSE
                 /\ pc' = [pc EXCEPT ![self] = "Runtime"]
                 /\ UNCHANGED << inC, lock, returned, held, epoch, ack, sum, 
                                 todo >>

NStop(self) == /\ pc[self] = "NStop"
               /\ stop' = TRUE
               /\ pc' = [pc EXCEPT ![self] = "NResume"]
               /\ UNCHANGED << inC, lock, returned, held, phase, collector, 
                               epoch, ack, sum, todo >>

NResume(self) == /\ pc[self] = "NResume"
                 /\ stop' = FALSE
                 /\ pc' = [pc EXCEPT ![self] = "NOuterSweep"]
                 /\ UNCHANGED << inC, lock, returned, held, phase, collector, 
                                 epoch, ack, sum, todo >>

NOuterSweep(self) == /\ pc[self] = "NOuterSweep"
                     /\ phase' = "sweep"
                     /\ pc' = [pc EXCEPT ![self] = "CResume"]
                     /\ UNCHANGED << stop, inC, lock, returned, held, 
                                     collector, epoch, ack, sum, todo >>

W(self) == Compiled(self) \/ EnterDec(self) \/ EnterLock(self)
              \/ HFirst(self) \/ HSecond(self) \/ HLock(self)
              \/ HRoot(self) \/ SuspendDec(self) \/ ForeignDec(self)
              \/ ForeignLock(self) \/ ForeignInc(self)
              \/ ForeignUnlock(self) \/ Runtime(self) \/ IdleLock(self)
              \/ LeaveFirst(self) \/ LeaveUnlock(self) \/ FreeSched(self)
              \/ FreeLock(self) \/ GEFirst(self) \/ GECheck(self)
              \/ GEDec(self) \/ GESpin(self) \/ CStop(self)
              \/ CRestart(self) \/ CRead(self) \/ CCheck(self)
              \/ CScan(self) \/ CSweep(self) \/ CResume(self)
              \/ NStop(self) \/ NResume(self) \/ NOuterSweep(self)

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
(* Temporal properties (the bounded model: MaxEpochs collections).         *)

\* the resume is ordered after the scan: STOP clears only at the end of a
\* sweep, by the collector
ResumeAfterSweep ==
    [][stop /\ ~stop' => phase = "sweep" /\ pc[collector] \in {"CResume", "NResume"}]_vars

\* a started collection completes: every worker reaches a safe point
CollectionCompletes == (phase = "stopping") ~> (phase = "idle")
\* a worker waiting out a collection in gc_enter gets to run again
SpinnersProceed == \A w \in Workers : (pc[w] = "GESpin") ~> (pc[w] # "GESpin")
=============================================================================
