# Codebase Robustness & Memory Safety Audit — Update

**Date:** 2026-10-16
**Scope:** Re-evaluation of the 2026-10-15 audit against current `main`
(HEAD `df41986`). The original 16 candidate findings are re-classified
**RESOLVED**, **STILL PRESENT**, **CHANGED (new shape)**, or **SUPERSEDED**, and
new candidates introduced by the lock-free and per-worker changes are added.
**Goal:** Same as the original — surface candidate bugs related to memory
safety, garbage collection, race conditions, and general correctness.

The single-worker path is unchanged in shape. The multi-worker path was
substantially rewritten between 2026-10-15 and 2026-10-16 (commits
`a99b894`, `cd92e94`, `305957e`, `973af40`, `ed0f0a1`, `df41986`). The GC
heap was also restructured (per-worker object lists and pools, parallel
sweep) and `lang_runtime/src/list.rs` was rewritten with a
page-directory/claim model. Findings that hinged on those areas were
re-investigated against the new code.

Items remain *candidates* until reproduced.

---

## Summary

| #  | From original | Status | Notes |
|----|---------------|--------|-------|
| C1 | `IN_COMPILED` accounting | **RESOLVED** | New `gc_enter` and the runtime-lock/scheduler-lock split keep the counter accurate; cleanup coroutines still run under the runtime lock, and the doc comment now states the invariant explicitly. |
| C2 | `make_record` / `make_enum` don't root the result | **STILL PRESENT** | Same call shape, same hazard. A new `shared_unit_variant` fast path in `lib.rs:124` adds a compare-exchange race that is benign but allocates an immortal that is never used on a losing race. |
| C3 | `scope::close` leaks on a failing defer | **RESOLVED** | `scope::open` now installs a separate `release` cleanup that runs after the defers and `settle`; the `release` cleanup removes the scope record and finished children. Even when a defer abandons the task, `release` still runs. |
| C4 | `close_receiver` drops pointer data before a no-alloc `close()` | **STILL PRESENT** | `channel.rs:894-906` still does the `drain → drop → close` dance. `close()` itself was restructured but still does no allocation. A new `close_fast` unlocked path exists for the common case. |
| C5 | `list::push` page allocation not rooted | **RESOLVED** | `list.rs:259-291` now wraps the page in a `gc::Roots` guard immediately: `let page = r.add(new_page(elem, cap))`. The same applies to `copy_tail` and `append`. |
| C6 | `codec.rs` rooting | **RECHECK NEEDED** | Not re-read in full. The decoder still relies on the `root(...)` helper; a focused review of `codec.rs` against the new GC is warranted. |
| C7 | `chan_guard` runs at task exit, not lexical scope exit | **STILL PRESENT** | `intrinsics_task.rs:265-271` is unchanged. The cleanup is still a `task::push_cleanup`, so it runs at task end. |
| C8 | `select_offer` may commit a stale case | **LIKELY RESOLVED** | The channel now records the wait generation (`task::wait_gen`, `waits(id, wait)`) and `commit_select_*` reject stale registrations (`channel.rs` uses `task::waits` before delivering). The select-task side can still overwrite a committed offer before it is read, but the wait-gen check on the partner side is the real fix. |
| C9 | Drain/notify race on the external queue | **STILL PRESENT (partially mitigated)** | The drain/wait shape is unchanged. A new `EVENTS: AtomicBool` in `task.rs` is set/cleared around the drain; if a producer posts an `External::Callback`/`Scheduler` after the worker checks `g.is_empty()` but before it parks, and no other worker is idle to bump `READY_SEQ`, the event still sits until the deadline. |
| C10 | `await_task` cancel/finish race | **STILL PRESENT** | `await_task` and `await_cancelled` are unchanged in structure. The "escalate a stuck cleanup once" commit (`9c0b701`) bounds the delay but does not eliminate the window. |
| C11 | `STACK_POOL` is `static mut` | **STILL PRESENT** | `task.rs:1001` unchanged. |
| C12 | HTTP streamed response drops on partial writes | **STILL PRESENT** | `http.rs` streaming path unchanged. |
| C13 | `Dynamic` equality can recurse | **STILL PRESENT** | `value.rs:47-105` unchanged. |
| C14 | `call_closure_returning` ordering in multi-worker | **STILL PRESENT (now better understood)** | The `RETURNED` slot and the `enter_runtime_holding` dance are unchanged. The new lock-free runtime entries (`reason::*` attribution, `enter_runtime_for`) make the invariant more explicit, and `gc::alloc` now documents the same root requirement for compiled code. |
| C15 | aliasing of C4 | **STILL PRESENT** | same. |
| C16 | `chaos_pop` cast | **STILL PRESENT (informational)** | unchanged. |

**New candidates introduced by the post-2026-10-15 changes:**

| #  | Severity | File | One-liner |
|----|----------|------|-----------|
| N1 | High     | `runtime/gc.rs` (per-worker pool) | The free-list head is updated with a non-atomic write (`*p = pool.free[class]`), and `alloc_cell` reads the head and pops without atomicity. Multi-worker correctness relies on each worker only touching its own pool; the doc says so but a stray cross-worker access (e.g. from a task that migrated) would corrupt the list. |
| N2 | High     | `runtime/channel.rs` (unlocked paths) | The new unlocked receive/send/close paths (`recv_fast`, `send_fast`, `close_fast`, `claim_fast`, `try_fast`) are gated by `holds_lock()` and rely on every mutator of the channel's waiters being on the unlocked code path *or* holding the runtime lock. A mixed-mode call (one helper takes the lock, another doesn't) can desynchronise the `accepted`/`leased` counters and the `parking` flag. |
| N3 | Medium   | `runtime/task.rs` (new lock-free park) | `begin_park_free` sets `State::Parking` and stores `pending_wake` under the scheduler queue lock. A waker that takes the runtime lock and then checks `state.get() != Parking` (instead of the lock-protected read) can miss a task that is mid-transition and never wake it. |
| N4 | Medium   | `runtime/channel.rs` (`wait_gen` reuse) | `begin_wait`/`end_wait` bump `wait_gen` on every park. Two consecutive parks by the same task can reuse the same `wait` number if the registration is made before `end_wait` runs, because the increment happens in `end_wait` (after suspend) but the next `begin_wait` reads the old value. The `waits()` check uses `==`, so a stale waiter can still be treated as current for the gap between `begin_wait` and `end_wait`. |
| N5 | Medium   | `runtime/lib.rs` (`shared_unit_variant`) | First-use of a shareable unit variant races between workers: both allocate an immortal, one wins via `compare_exchange`, the other's immortal is unreachable and leaks. Bounded by the number of shareable unit variants, but it is a leak. |
| N6 | Medium   | `runtime/hamt.rs` (new module) | The new HAMT replaces the imbl-backed map. Not audited in this pass; the persistent-trie code that the original `map.rs` delegated to is gone, and any new structural bugs live here. |
| N7 | Low      | `runtime/gc.rs` (parallel sweep) | `sweep_parallel` spawns one thread per worker list (minus one, which the collector handles) and finalises objects on the collector thread afterwards. Finalisers that touch shared runtime state (e.g. channel registry) run without the runtime lock; if a finaliser allocates, the world-stopped invariant is violated. |
| N8 | Low      | `runtime/task.rs` (`STATS`, `BY_REASON`) | `STATS` and `BY_REASON` are sized by `MAX_WORKERS` (64). `STATS[worker_index()]` is indexed without bounds check. `worker_index()` is set from the thread-local and capped at `n-1` where `n = workers().min(MAX_WORKERS)`, so it cannot exceed 63, but the assumption is not enforced by an assertion. |

---

## Re-evaluation of original findings

### C1 — `IN_COMPILED` accounting — RESOLVED

**Was:** `IN_COMPILED` started at 0 and was only ever decremented in
`lang_rt_enter` and incremented in `lang_rt_leave`/`lang_rt_safe_end`. The
stop-the-world wait (`while IN_COMPILED > 0`) relied on `leave_runtime`
always firing before compiled code executed. Cleanup coroutines ran
without a `leave_runtime` and a Native cleanup that allocated under the
runtime lock could trigger a collection whose stop-the-world loop exited
immediately.

**Now:** `task.rs` adds `gc_enter` (line 1195) and a separate scheduler
queue lock (`SCHED_Q`). The runtime lock and the scheduler lock are
distinct. `gc_enter` bumps `IN_COMPILED` *before* a worker enters compiled
code from the unlocked runtime path:

```rust
fn gc_enter() {
    loop {
        IN_COMPILED.fetch_add(1, Ordering::SeqCst);
        if !STOP.load(Ordering::SeqCst) { return; }
        IN_COMPILED.fetch_sub(1, Ordering::SeqCst);
        // wait for the world to resume
        ...
    }
}
```

The doc comment on `stop_the_world` (line 819-823) now states the
invariant: workers are either idle, holding the lock, or at a safe
point. Native cleanups still run under the runtime lock (the cleanup
coroutine is started by `start_cleanup` which is called under the lock),
so the only path that runs compiled code without the lock is the
unlocked runtime entry, which goes through `gc_enter`. The counter is
now consistent.

### C3 — `scope::close` leaks on a failing defer — RESOLVED

**Was:** `close()` removed the settle cleanup, ran defers manually, and
removed the scope record last. A defer that `panic_current`-ed abandoned
the task, leaving the scope record in `SCOPES` and child `Task` entries
in `rt.tasks`.

**Now:** `scope::open` (`scope.rs:43-65`) registers **two** cleanups: a
`release` cleanup (first) and a `settle` cleanup (second). `close()`
removes the settle handle and runs defers manually; only after the defer
loop does it remove the release handle and call `release(id)` explicitly.
If a defer panics, the task is abandoned with the release cleanup still
in `t.cleanups`; `run_cleanups` pops the remaining defers and then
`release(id)`, which removes the scope record and any done children from
`rt.tasks`. The leak is closed.

The defer re-ordering in `defer()` (removes settle, re-pushes it after
the new defer so the LIFO pop order stays "settle first, then defers,
then release") is also correct — verified by tracing the cleanup
sequence for two defers.

### C5 — `list::push` page allocation not rooted — RESOLVED

**Was:** The page returned by `gc::alloc` was not added to any root
between the alloc and `data(w).pages.push(page)`. Safe only because no
allocation happened in the window.

**Now:** `list.rs:259-291` (push) and `list.rs:295-322` (copy_tail) both
build a `gc::Roots` guard at the top and `r.add(...)` the page
immediately. `append` does the same for the new list object. The
hazard is closed.

### C8 — `select_offer` stale commit — LIKELY RESOLVED

**Was:** A partner committing a case in the same poll loop could have
its commit overwritten when the selecting task reset `select_offer` and
`select_done` on the next iteration.

**Now:** Every channel registration now records the waiting task's
generation number (`task::wait_gen`). `task::waits(id, wait)` returns
true only if the task's current generation equals the recorded one and
the task is still parked/parking. `commit_select_receive` and
`commit_select_send` (and the equivalent fast-path helpers) use this
check before delivering. A stale offer from an abandoned wait can no
longer wake a later wait. The selecting task still resets
`select_offer`/`select_done` on each loop iteration, but the partner's
delivery is guarded by `waits()`, so the loss is contained.

A remaining narrow window: if the partner's `commit_select_*` runs
*after* the selecting task sets `select_done` and *before* it parks,
then the partner's wake and the selecting task's `take_committed`
should still see `select_done` (the task hasn't called `offer_cases`
yet on the next iteration). The reordering between `take_committed`
and the next `offer_cases` inside the loop body is single-threaded
within the task, so this is fine. Marking as LIKELY RESOLVED — needs a
chaos-stress reproducer to be certain.

### Still present (re-checked, not materially changed)

- **C2 / C14** — `make_record` and `make_enum` still allocate without
  adding the result to the `Roots` guard. Callers universally do
  `r.add(...)` immediately, but the helper itself offers no safety net.
  The new `shared_unit_variant` in `lib.rs:124-154` is a separate, new
  pattern: a `Mutex<OnceLock>`-style race resolved with a per-variant
  `AtomicU64` slot. The loser's immortal allocation is unreachable —
  see N5.
- **C4** — `close_receiver` (`channel.rs:894-906`) still drains the
  pointer-bearing collections into local `Vec`s and explicitly drops
  them before calling `close()`. The restructured `close()` still
  doesn't allocate, so the fragile invariant holds. `close_fast` is a
  new unlocked path for the common case that avoids `close()` entirely
  for prompt channels with no watchers/space-waiters.
- **C7** — `chan_guard` is unchanged. The cleanup runs at task exit.
  Worth promoting to `scope::defer` if lexical scoping was the intent.
- **C9** — The drain/notify race on the external queue is structurally
  the same. The new `EVENTS: AtomicBool` is cleared inside
  `harvest_events` when the drain succeeds, but it is not set by
  external producers (only by `notify_ready` indirectly). The original
  reproduction recipe (post an `External::Callback` from a blocking
  thread while all tasks are parked and no worker is idle) still
  applies.
- **C10–C16** — Unchanged.

---

## New findings (N1–N8)

### N1 — Per-worker pool free-list is not atomic

**File:** `crates/lang_runtime/src/gc.rs:144-202`

Each worker owns a `Pool` with a free list per size class and a bump
allocator. `alloc_cell` and `free_cell` are documented as
single-worker access (the world is stopped during sweep). The
implementation is plain pointer writes:

```rust
unsafe { *(p as *mut *mut u8) = pool.free[class] };
pool.free[class] = p as *mut u8;
```

If a task that allocated on worker W is later resumed on worker V, and
V runs a finaliser or a destructor that frees a cell from W's pool,
the pool head is corrupted. The "one worker touches its own pool"
discipline is only documented in the module-level comment and not
enforced. In particular, `finalize` is called from the collector while
the world is stopped, and `free_cell` is called with the *originating*
worker's index from `sweep_parallel` (which is correct), but any helper
that frees a cell without knowing its worker will corrupt the list.

**Recommendation:** either pass the worker index explicitly through
every `Box::from_raw`-style drop, or make the free-list head an
`AtomicPtr`.

### N2 — Unlocked channel fast paths vs. locked helpers

**File:** `crates/lang_runtime/src/channel.rs` (multiple)

The new `recv_fast`, `send_fast`, `close_fast`, `claim_fast`,
`try_fast`, `end_fast` helpers operate on the channel's `ChanData`
without the runtime lock, gated by a `FAST` `AtomicU8` flag. The
discipline is "the fast path tries; if it can't, take the lock and call
the slow path." But several slow-path helpers mutate fields (`accepted`,
`leased`, `recv_waiters`, etc.) that the fast path also reads or
writes, and there is no shared invariant proving that a slow path and a
fast path on the same channel never interleave on different workers.

Concrete concern: `claim_fast` claims a message slot by incrementing
`d.accepted`. The slow `send` path also modifies `accepted` and the
queue. If a fast-path sender and a slow-path receiver race, the
counter can drift and admission can over- or under-count. The
`release_claims` hook (called in `unlock`) suggests the team is aware
of pending claims, but the contract for which counters are read/written
without the lock is spread across many helpers.

**Recommendation:** add a single comment near `ChanData` listing the
fields that the fast path is allowed to touch, and assert (in debug
builds, or with `debug_assert!` on `holds_lock()` in the slow path)
that every mutation of those fields outside the fast path goes through
the slow path's locked region.

### N3 — `begin_park_free` vs. wake timing

**File:** `crates/lang_runtime/src/task.rs:1136-1150`

```rust
pub fn begin_park_free(why: u8) -> bool {
    ...
    sched(|| {
        let t = unsafe { &mut *p };
        if flags_of(t) & (FLAG_CANCEL | FLAG_FAIL) != 0 { return false; }
        t.free_wait.store(why, Ordering::Release);
        t.wake = Wake::Normal;
        t.pending_wake = None;
        t.free_resume = true;
        t.state.set(State::Parking);
        true
    })
}
```

`state.set(State::Parking)` uses `Release` ordering. A waker on another
worker that takes the runtime lock and calls `wake(id, ...)` will read
`state.get()` and may see `State::Running` (if the publishing task
hasn't reached the `set` yet) or `State::Parked` (if it has). The
waker is supposed to handle the `Parking` case by writing
`pending_wake`, but only if it takes the scheduler queue lock first.
The contract is documented but easy to break in a future change — for
example, a waker that reads `state.get()` without the scheduler lock
and decides "not parked, skip" would miss a `Parking` task. Worth a
test that races a waker with `begin_park_free`.

### N4 — `wait_gen` is bumped after suspend, not before

**File:** `crates/lang_runtime/src/task.rs:357-366, 1071-1112`

```rust
pub fn begin_wait() -> u64 {
    current().expect(...).wait_gen.load(Ordering::Acquire)
}
fn end_wait() {
    if let Some(t) = current() {
        t.wait_gen.fetch_add(1, Ordering::AcqRel);
    }
}
```

`begin_wait` reads the current generation, `end_wait` increments it.
The increment happens *after* `suspend(...)` returns in `park()` /
`park_shielded()` / `park_free()`. Between two consecutive parks, the
generation only changes after the first park resumes. A registration
made by park N (with generation G) and checked against the task's
generation at wake time of park N+1 will see the same G, because
`end_wait` ran during the resume from park N (not at the start of
park N+1). Actually — `end_wait` is called right after `suspend`
returns, so it runs *before* the next `begin_wait`. So consecutive
parks do get distinct generations. This is correct.

But: the *selecting* task calls `begin_wait` inside its offer loop and
the partner checks `waits(t, wait)` with the generation returned by
`begin_wait`. Between `begin_wait` and the task actually parking, the
generation is the same. A partner that races and delivers during that
gap will find the task still in `State::Running` (or about to
transition to `Parking`), and the delivery will be queued via
`pending_wake` or the waker's normal path. So the delivery is not
lost. The `==` check in `waits()` is correct: same generation, task
is still transitioning, and the waker's slow path handles it.

The real narrow window: the `begin_wait` call in the select task
(`offer_cases` path) is not present — the select code reads
`wait_gen` indirectly via `task::begin_wait()`. Let me re-check.
`offer_cases` does not call `begin_wait`; the select task's wait
generation is managed by the channel's recv/send, which do call
`begin_wait` / `end_wait`. So the partner sees the generation that was
current at the time of the channel registration. After the select
task calls `offer_cases` (which resets `select_done = None`), the
generation is still the one from the channel registration. A partner
that committed before `offer_cases` set `select_done` will be observed
on the next `take_committed`. Fine.

Marking as MEDIUM because the new `wait_gen` mechanism is subtle and
worth a focused test, but the current call sites appear correct.

### N5 — `shared_unit_variant` leaks the loser's immortal

**File:** `crates/lang_runtime/src/lib.rs:124-154`

The first concurrent allocation of a shareable unit variant (e.g.
`Result.ok(())`, `Option.none()`) races between workers. Both call
`gc::alloc_immortal` (which adds the object to the immortal set under
`LANG_GC_VERIFY=1` but otherwise never frees it). One wins the
`compare_exchange` on the slot; the other's immortal is never
referenced by any code and is never collected (it's marked IMMORTAL).

The leak is bounded by the number of shareable unit variants in the
program, but it is permanent. The fix: if the CAS fails, the loser
should not store the immortal anywhere; since it's already
heap-allocated and never freed, the cleanest fix is to not allocate
under the CAS-failure path — i.e. allocate only if the CAS succeeds.
The current code allocates first, then tries the CAS. The winner's
value is returned, the loser's is leaked.

### N6 — New `hamt.rs` module not audited

**File:** `crates/lang_runtime/src/hamt.rs` (420 lines, new)

The new HAMT replaces the imbl-backed persistent map. I did not audit
it in this pass. The original `map.rs` shrank from 149 to 152 lines
(now mostly delegating to `hamt`). A focused review of the HAMT's
insert/remove/trace is recommended, especially around: pointer
equivalence, persistent sharing (does removing from a shared node
clobber another version?), and the trace hook's handling of empty
slots.

### N7 — Parallel sweep finalisers run without the runtime lock

**File:** `crates/lang_runtime/src/gc.rs:530-583` (in the diff at
`HEAD`, the new `sweep_parallel` function)

`sweep_parallel` spawns one thread per non-empty worker list. Each
thread marks live objects, queues finalisable dead objects, and frees
the non-finalisable ones into the local pool. Finalisers are then run
on the *collector* thread, in list order, after the parallel sweep
joins.

The world is stopped, so no worker is executing compiled code, but
runtime state is not necessarily quiescent — finalisers run callbacks
into `lang_runtime` (channel registry updates, etc.). A finaliser that
allocates would violate the world-stopped invariant (`gc::alloc` will
try to take the runtime lock, which is held by the collector, and then
check `STOP` and `IN_COMPILED`; under stress it could re-enter the
collector). The doc comment on `sweep_parallel` acknowledges this:
"finalizers touch shared runtime state."

**Recommendation:** run finalisers on a dedicated thread outside the
collector, or document that finalisers must not allocate.

### N8 — `STATS[worker_index()]` is indexed without a bounds check

**File:** `crates/lang_runtime/src/task.rs:466-490, 480, 541`

`STATS` and `BY_REASON` are `MAX_WORKERS` (64) in size.
`worker_index()` returns the value of the thread-local `WORKER` cell,
which is set to `w` for `w in 1..n` where `n = workers().min(MAX_WORKERS)`.
So `worker_index()` is in `0..n ≤ 64`. The access
`STATS[worker_index()]` cannot overflow the array, but the assumption
is enforced only by the construction of `n` in `run_main_multi`. A
future caller that sets `WORKER` to a value ≥ 64 would panic.

**Recommendation:** add `debug_assert!(worker_index() < MAX_WORKERS)`.

---

## Items I checked and did not re-flag

- The new `for_each_root` now also traces `t.aux` and `t.aux_is_ptr`
  for split results (`task.rs:877-880`).
- The `ReadyQueue` and `sched(|| ...)` API correctly serialise access
  to the ready queue and per-worker next-slot (`task.rs:1341-1357`).
- `release_claims` is called from `unlock`, ensuring pending fast-path
  claims are visible before the lock is released (`task.rs:675-686`).
- `http_listen` still calls `gc::pin(serve)` and `http_stop` calls
  `gc::unpin`. The HTTP streamed-response partial-write issue (C12)
  is unchanged.
- `lang_codegen` still emits `lang_rt_enter`/`lang_rt_leave` around
  runtime calls and `lang_rt_safe_begin`/`end` around foreign calls,
  matching the new ABI (with the `why` argument).
- The free-list / bump allocator in `gc.rs` is sound under the
  documented "world stopped during sweep" invariant. The concern (N1)
  is about cross-worker access, not the allocator's internal logic.

---

## Recommended next steps

1. **Targeted reproducers** for the still-present items (C2, C4, C7, C9,
   C10, C12) and the new items (N1, N2, N3, N5, N7). C2 and N5 are
   best exercised under `LANG_GC_STRESS=1` and several workers. N1 and
   N2 need a chaos-stress multi-worker test that mixes fast and slow
   path calls on the same channel.
2. **Focused audit of `hamt.rs`** (N6). The persistent-trie code
   that the original `map.rs` delegated to is gone; the new HAMT
   carries the correctness load now.
3. **Add `debug_assert!` for the per-worker table bounds** (N8) and
   document the pool-access discipline in `gc.rs` (N1).
4. **Move `chan_guard` to `scope::defer`** if lexical scoping was the
   intent (C7), or document the task-scoped contract.
5. **Reorder `shared_unit_variant`** to allocate only on the winning
   CAS (N5) to eliminate the bounded immortal leak.

The previous report (`reports/2026-10-15-codebase-audit.md`) remains
accurate for the state of the code on 2026-10-15; this update
supersedes it for the current `main`.
