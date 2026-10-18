# Codebase Robustness & Memory Safety Audit

**Date:** 2026-10-15
**Scope:** `crates/lang_runtime` (GC, task scheduler, channels, intrinsics, FFI), with
spot checks of `lang_codegen`, `lang_driver`, `lang_typeck`, `lang_mir`.
**Goal:** Surface candidate bugs related to memory safety, garbage collection, race
conditions, and general correctness. Items are *candidates* — each needs targeted
reproduction before being treated as a confirmed defect.

Severity is my estimate of exploit/impact likelihood on real workloads.

---

## Summary

| #  | Severity | File | One-liner |
|----|----------|------|-----------|
| C1 | Critical | `runtime/task.rs` | `IN_COMPILED` accounting starts at 0 and is never bumped to the worker count; stop-the-world wait is broken unless `call_closure_returning` *always* runs first (and the cleanup coroutine violates that). |
| C2 | High     | `runtime/lib.rs` | `make_record` / `make_enum` allocate without rooting the result; callers must root before any subsequent allocation. The convention is honored in-tree, but the helper itself is unsafe by construction. |
| C3 | High     | `runtime/scope.rs` | A defecting `defer` aborts `close()` mid-cleanup; the scope record, children, and `SCOPES` entry leak. |
| C4 | High     | `runtime/channel.rs` | `close_receiver` drops channel-owned pointer data before `close()`; relies on the fact that `close()` and the surrounding code do not allocate. |
| C5 | Medium   | `runtime/list.rs` | `push` allocates a page that is unrooted between `gc::alloc` and `data(w).pages.push(page)`. Currently safe (no alloc between), but a single future alloc in that window is a UAF. |
| C6 | Medium   | `runtime/codec.rs` | `gc::alloc` inside the encoding decoder (line ~298) roots only the just-allocated object; intermediate strings/buffers are held only in stack locals. |
| C7 | Medium   | `runtime/intrinsics_task.rs` | `chan_guard`'s cleanup captures the channel `u64` and closes it on drop, but the cleanup runs only when the task exits — so the channel is *not* closed when the guard's lexical scope ends. |
| C8 | Medium   | `runtime/intrinsics_task.rs` | `select_offer` is overwritten on every loop iteration of `chan_select`, but a partner may have already read it before the parking task resets it; this can commit a stale offer. |
| C9 | Medium   | `runtime/task.rs` | `harvest_events` / `wait_events` drain the same queue under different lock paths; a missed `notify_one` is possible if the runtime lock is held between draining and the waiter reaching its `wait`. |
| C10| Medium   | `runtime/task.rs` | `await_task` on a cancelled task only re-checks state once per loop iteration; a cancel that races after the check but before the park can deadlock until the cleanup grace expires. |
| C11| Low–Med  | `runtime/task.rs` | `STACK_POOL` is a `static mut` Vec accessed from every task without synchronization beyond the runtime lock. OK while single-threaded, fragile. |
| C12| Low–Med  | `runtime/http.rs` | Connection mpsc uses `recv_timeout` with no retry; partial writes after `Out::End` and on error silently drop the rest of a streamed response. |
| C13| Low      | `runtime/value.rs` | `eq`/`hash` on `Dynamic` recursively calls `dynamic_desc` on each operand; a cycle (Dynamic containing Dynamic) is not detected. |
| C14| Low      | `runtime/lib.rs` | `call_closure_returning` returns `r` and *then* calls `enter_runtime_holding`; in multi-worker mode the runtime lock is acquired *after* the managed pointer is no longer in any compiled-code frame. The `RETURNED` slot covers the gap, but only if the compiler emits `lang_rt_enter` around the call. Verify codegen path. |
| C15| Low      | `runtime/intrinsics_task.rs` | `chan_close_receiver` drains under the runtime lock with no root for the released messages; relies on the immediate `close()` having no alloc — see C4. |
| C16| Low      | `runtime/task.rs` | `chaos_pop` indexes `r.ready` with a modulo of a `u64`; the cast to `usize` is safe on supported platforms but should be documented. |

Typeck/MIR/codegen and driver were spot-checked and nothing rose to the same level.
The `lang_runtime` GC itself (mark/sweep) looks correct under verification, but
several allocation sites around it are fragile.

---

## C1 — `IN_COMPILED` initialization and cleanup-coroutine window

**File:** `crates/lang_runtime/src/task.rs:374, 446-471, 502-517`

`IN_COMPILED` is an `AtomicIsize` initialised to 0 and never assigned a starting
value of `workers()`. It is incremented in `lang_rt_leave` and decremented in
`lang_rt_enter`. The stop-the-world wait is

```rust
while IN_COMPILED.load(Ordering::SeqCst) > 0 {
    std::hint::spin_loop();
    std::thread::yield_now();
}
```

For the wait to ever fire, `IN_COMPILED` must become positive. With the
single-coroutine-per-task design, that only happens via `lang_rt_leave`, which is
called from `call_closure_returning` (`lib.rs:77`) before running the closure.

Two concerns:

1. **Cleanup coroutines** do not call `call_closure_returning`. Their body
   (`run_cleanups`, `task.rs:701`) runs native cleanups directly. Native cleanups
   may allocate (e.g., `channel::close` does not, but others might). Under the
   runtime lock, that allocation may trigger a collection, which calls
   `stop_the_world`. If the runtime lock is held by another worker that is
   between `lang_rt_leave` and the next `lang_rt_enter`, `IN_COMPILED` is
   positive and the wait will fire — but `IN_COMPILED` only reflects workers
   that have *already* left the runtime. A worker that is in compiled code but
   has not yet executed any `lang_rt_leave` does not contribute. With `>= 2`
   workers, this is a real concern if a task is started on worker B and the
   first leave/enter transition has not happened.

2. **The accounting has no clamp.** If a code path forgets to pair a `leave`
   with an `enter` (or vice versa), `IN_COMPILED` drifts and the wait becomes
   permanently satisfied or permanently blocked.

The two together mean: the stop-the-world relies on `lang_rt_leave` always
having fired before any other worker observes a "this worker is in compiled
code" state. The single-worker test passes because `multi()` short-circuits the
wait. Multi-worker correctness depends on every entry into compiled code being
preceded by a `lang_rt_leave`.

**Verification idea:** a targeted test that runs many small allocations from a
freshly-spawned task on worker N while another worker holds the runtime lock.
Under `LANG_GC_STRESS=1`, look for missed roots or freed objects reachable from
worker N's compiled frame.

---

## C2 — `make_record` / `make_enum` allocate without rooting the result

**File:** `crates/lang_runtime/src/lib.rs:89-151`

```rust
let w = gc::alloc(desc, desc.payload_size() as usize);  // may collect
unsafe {
    let p = gc::obj(w).payload();
    ...
}
w
```

`w` is added to the *inputs*' `Roots` guard but not to its own result. Between
this function returning and the caller rooting `w` (e.g., `let out = r.add(...)`),
if any allocation runs, GC may free `w`. The pattern is followed everywhere in
the runtime I checked, but the helper itself offers no safety net. Under
`LANG_GC_STRESS=1` a future caller that forgets the `r.add(...)` becomes a UAF.

**Recommendation:** either root `w` in the helper (with the same `Roots` guard
that already exists for inputs) or rename to `make_record_unchecked` to make
the contract explicit.

---

## C3 — `scope::close` leaks on defer failure

**File:** `crates/lang_runtime/src/scope.rs:104-136`

```rust
pub fn close(id: u64, cancel: bool) {
    ...
    task::remove_cleanup(handle);
    // deferred cleanups, after every child has settled
    let defers = std::mem::take(&mut get(id).unwrap().defers);
    for h in defers.into_iter().rev() {
        if let Some(Cleanup::Closure(w)) = task::remove_cleanup(h) {
            let ctx = task::ctx_of(task::current_id());
            task::current().unwrap().roots.push(w);
            unsafe { crate::call_closure(ctx, w, &[]) };
            task::current().unwrap().roots.pop();
        }
    }
    let s = scopes().1.remove(&id).unwrap();
    for c in &s.children {
        task::remove_task(*c);
    }
    if let Some(f) = s.failure {
        task::panic_current(f.message, f.location);
    }
}
```

If a defer calls `panic_current`, the task is abandoned via `abandon` →
`Signal::Abandon` → `run_one` runs the remaining cleanups, then `finish()`
calls `remove_task` for children. But:

- The scope record was removed from `SCOPES` only at the end of `close()`. If
  abandon fires mid-loop, that removal is skipped.
- `task::remove_task` is *not* called for the children that were `await_task`'d
  in step 3-4. `finish()` only removes the *current* task from `rt.tasks`, not
  its siblings.
- The remaining `defers` (those with higher indices than the failing one) run
  via `run_cleanups`, but their handles were already removed from
  `t.cleanups` by the prior iteration. So `run_cleanups` sees an empty list and
  finishes. The scope cleanup was also already removed, so `settle(id, true)`
  is never invoked for the abandoned close.

Net result: the `ScopeRec` leaks, child `Task` entries leak in `rt.tasks`, and
the children remain parked (they were cancelled and awaited earlier, so they
should be `Done` — but their entries are still in `rt.tasks`).

**Reproduction:** open a scope, register two `defer` closures where the first
panics, then exit the scope normally. Inspect `rt.tasks` afterwards.

---

## C4 — `close_receiver` drops pointer data before a no-alloc `close()`

**File:** `crates/lang_runtime/src/channel.rs:539-550`

```rust
pub fn close_receiver(ch: u64) {
    let d = data(ch);
    let queued: Vec<u64> = d.queue.drain(..).collect();
    let mut batches: Vec<Vec<u64>> = d.batches.drain(..).collect();
    batches.extend(d.staging.drain().map(|(_, v)| v));
    d.staged_at.clear();
    d.sizes.clear();
    d.bytes = 0;
    drop(queued);
    drop(batches);
    close(ch);
}
```

After `drop(queued)` and `drop(batches)`, the managed pointers they contained
are no longer reachable via the channel (or anywhere else, unless the user
holds them). Between these drops and `close()`, there is no allocation, so no GC
runs — the pointers are still physically valid. `close()` itself does not
allocate.

If anyone ever adds an allocation to `close()` or inserts a logging hook between
the `drop` calls, this becomes a UAF on `close()`'s `send_waiters` wake-up
(which iterates `send_waiters` and does not dereference pointer data — but the
*next* allocation would be against freed objects). Mark as a fragile invariant.

---

## C5 — `list::push` page allocation is not rooted between alloc and link

**File:** `crates/lang_runtime/src/list.rs:120-152`

```rust
if need_page {
    let _r = gc::Roots::new();
    _r.add(w);
    if elem.is_ptr() { _r.add(v); }
    let page = gc::alloc(page_desc(), PAGE_HDR + cap * size.max(8));   // (1)
    unsafe {
        let p = obj(page).payload();
        *(p as *mut *const TypeDesc) = elem;
    }
    data(w).pages.push(page);                                            // (2)
}
```

The page allocated at (1) is not in any root between (1) and (2). It is only in
the local `page` binding. Under `LANG_GC_STRESS=1` (which collects before every
allocation) the safety relies on there being no allocation between (1) and (2).
The current code has only a pointer store — fragile.

`append` (`list.rs:199-235`) has the same shape for the `copy` page.

The comment on `recv_batch` (`channel.rs:689-690`) explicitly calls out the
equivalent hazard and roots the result. `list::push` and `list::append` lack
that protection.

**Recommendation:** add the page to `_r` immediately after `gc::alloc` (or use a
single combined `Roots` that holds both `w` and the new `page`).

---

## C6 — `codec::encode` allocator rooting

**File:** `crates/lang_runtime/src/codec.rs` (around line 298)

The decoder allocates intermediate objects with `gc::alloc` and then loops,
potentially allocating more, without re-rooting the intermediate results. Spot
checks showed that *most* call sites pass through `root(...)`, but the
readability of the root invariant here is low and a future change is likely to
drop a `root()` call.

A full read of `codec.rs` is recommended; this audit only confirmed the
allocation pattern, not every code path.

---

## C7 — `chan_guard` cleanup runs at task exit, not at scope exit

**File:** `crates/lang_runtime/src/intrinsics_task.rs:184-198`

```rust
fn chan_guard(ctx, targs, ret, ch) {
    let r = Roots::new();
    r.add(ch);
    task::current().unwrap().roots.push(ch);
    task::push_cleanup(Cleanup::Native(Box::new(move |_| channel::close(ch))))
}
```

The cleanup is registered on the *task*, not on a scope. It runs only when the
task exits (or `chan_unguard` is called). If the user's intent was a
lexically-scoped guard (`{ let _g = chan_guard(c); ... }`), the channel is not
closed when the block ends.

Compare with `scope::defer`, which does run at scope close. If `chan_guard` was
intended to mimic RAII for the lifetime of a scope, the current design is
surprising. Worth either documenting the contract or changing the
implementation to register on the current scope.

---

## C8 — `select_offer` may commit a stale case

**File:** `crates/lang_runtime/src/intrinsics_task.rs:468-476`

```rust
fn offer_cases(kinds: u64, chans: u64, value: impl Fn(usize) -> u64) {
    let n = crate::list::len(kinds);
    if let Some(t) = task::current() {
        t.select_done = None;
        t.select_offer = (0..n)
            .map(|i| (crate::list::get(chans, i), i, crate::list::get(kinds, i) & 1 == 1, value(i)))
            .collect();
    }
}
```

`offer_cases` is called *after* the loop polls every case (`chan_select`,
`intrinsics_task.rs:262`). If a partner committed a case in that poll and woke
this task before `offer_cases` runs, the new offer will overwrite the
committed `select_done`. But `take_committed` (`intrinsics_task.rs:480`) reads
`select_done` after `park_for`, so as long as the partner committed *before*
this task parked, the index is still in `select_done` and `take_committed`
returns it. Then `select_offer.clear()` is called. OK on this path.

However, if the partner committed *while* `offer_cases` was running (single
step, but the partner is on another worker), the cleared `select_offer` and
the reset `select_done = None` race with the partner's read. The
`commit_select_receive` / `commit_select_send` helpers (`channel.rs:398-430`)
read `tk.select_offer.iter().find(...)` and check `tk.select_done.is_some()`.
If `select_done` is `None` at that moment, the partner skips this task. But if
the partner *did* commit and set `select_done`, but then this task's
`offer_cases` resets it, the partner's wakeup is lost. The next park in the
select task will re-offer. So no permanent loss, but spurious retries.

The fix is probably to set `select_offer` *before* the first poll, or to use a
sequence number so the partner can detect a stale offer.

---

## C9 — Drain/notify race in the external queue

**File:** `crates/lang_runtime/src/task.rs:1697-1724, 1775-1822`

`harvest_events` and `wait_events` both drain `external.queue` under
`q.queue.lock()`. `notify_ready` (`task.rs:816`) holds the same mutex and calls
`cond.notify_all`. If `harvest_events` is called while `wait_events`'s
`wait_timeout` is parked but before it has acquired the lock (impossible — the
mutex is held during the wait), there is a missed-wakeup window: a producer
posts an event after `wait_events` checked `g.is_empty()` and before it parks.
That event arrives while the producer holds `q.queue.lock()`; the producer's
`notify_one` finds no waiters (the consumer is between unlock and cond.wait)
and the consumer then sleeps. The event sits in the queue until the deadline.

`READY_SEQ` is supposed to prevent this:

```rust
let seq = READY_SEQ.load(Ordering::SeqCst);
...
if g.is_empty() && READY_SEQ.load(Ordering::SeqCst) == seq && !SHUTDOWN.load(Ordering::SeqCst) {
    let _ = q.cond.wait_timeout(g, wait).unwrap();
}
```

But `READY_SEQ` is only bumped in `notify_ready` (which is gated on `IDLE > 0`).
If no worker is idle, the producer does not bump `READY_SEQ`, so a sleeping
worker has no way to know a non-task event was posted. External events include
`External::Callback` and `External::Scheduler`, which are not gated by
`notify_ready`. Look at `external_queue.post` — it only does
`notify_one`. So an external callback posted while no worker is idle will sit
in the queue until a task becomes ready and bumps `READY_SEQ`.

**Reproduction:** post an `External::Callback` from a blocking-pool thread
while all tasks are parked. The callback should run on the next event harvest,
but may not.

---

## C10 — `await_task` cancel/finish race

**File:** `crates/lang_runtime/src/task.rs:1262-1284`

```rust
pub fn await_task(id: TaskId, shielded: bool) {
    loop {
        if is_done(id) || task(id).is_none() { return; }
        if task(id).is_some_and(|t| {
            t.ctx.flags & FLAG_CANCEL != 0
                || t.state == State::Cleaning
                    && t.pending_exit.as_ref().is_some_and(|e| matches!(e, Exit::Cancelled))
        }) {
            await_cancelled(&[id]);
            return;
        }
        let me = current_id();
        task(id).unwrap().joiners.push(me);
        if shielded { park_shielded() } else { park_for("join") };
    }
}
```

A task that is cancelled *after* the `is_some_and` check but *before*
`joiners.push(me)` will park this waiter forever (no one to wake it). The
cleanup grace timeout in `await_cancelled` will eventually fire and fail the
caller, but only after up to `GRACE_MS`. This is a benign delay, not a
correctness bug, but it does mean `await_task` on a concurrently-cancelled task
is not immediate.

---

## C11 — `STACK_POOL` is `static mut`

**File:** `crates/lang_runtime/src/task.rs:672-668`

`STACK_POOL: Vec<DefaultStack>` is `static mut`. All access is gated by the
runtime lock, so within the runtime model this is fine. But every `unsafe`
block is a footgun for a future maintainer who adds a code path that touches
the pool outside the lock. Consider wrapping it in a `Mutex<Vec<...>>` or
moving it into `Runtime`.

---

## C12 — HTTP streaming drops on partial writes

**File:** `crates/lang_runtime/src/http.rs:233-256`

```rust
loop {
    match rx.recv_timeout(Duration::from_secs(60)) {
        Ok(Out::Part(chunk)) => {
            if s.write_all(&chunk).is_err() {
                break;
            }
            let _ = s.flush();
        }
        Ok(Out::End) => {
            let _ = s.write_all(b"0\r\n\r\n");
            break;
        }
        Ok(Out::Full(_)) | Err(_) => break,
    }
}
```

If the user sends a chunked response and then exits the scope without calling
`http_stream_end`, the cleanup sends `Out::Full(vec![])` (an empty Full — not a
valid chunked terminator). The connection thread matches `Out::Full(_)` and
breaks. The HTTP client sees a truncated chunked body with no terminal `0\r\n\r\n`.

Either route the cleanup's response to the `End` arm, or document that
`http_stream_end` must be called before the scope closes.

---

## C13 — `Dynamic` equality/hash can recurse

**File:** `crates/lang_runtime/src/value.rs:47-105, 137-207`

`eq` and `hash` on `Dynamic` dispatch to `dynamic_desc(w)` and recursively call
`eq`/`hash`. There is no cycle detection. A `Dynamic` value that wraps a
`Dynamic` (via record or enum) loops. The values themselves are not recursive
(the language likely does not allow recursive types in records), but a wrapper
that holds a `Dynamic` and the runtime stores it back into the wrapper is a
possibility if the runtime ever constructs such a value.

Low risk in practice because the language forbids recursive types and the
runtime never constructs self-referential Dynamic. Document the assumption.

---

## C14 — `call_closure_returning` ordering in multi-worker

**File:** `crates/lang_runtime/src/lib.rs:71-86`

```rust
task::leave_runtime();
let r = f(ctx, closure, args.as_ptr());
if ptr_result && r != 0 {
    task::enter_runtime_holding(r);
} else {
    task::enter_runtime();
}
r
```

The function returns `r` *after* `enter_runtime_holding`, which clears
`RETURNED[worker_index()]`. By that point, `r` is either in the caller's stack
or in `t.exit` (if the caller is the entry body). `t.exit` is traced by
`for_each_root`, so `r` is safe.

But the call site is `make_coroutine`'s entry body:

```rust
let r = unsafe { crate::call_closure_returning(ctx, t.entry, &args, t.result_is_ptr) };
let t = task(id).unwrap();
t.exit = Some(Exit::Returned(r));
```

If `call_closure_returning` returns and *then* a GC runs (from another worker's
allocation) before `t.exit = ...`, `r` is only in `RETURNED` which is cleared
on `lang_rt_enter`. Wait — `enter_runtime_holding` clears `RETURNED` *before*
calling `lang_rt_enter`? Let me re-read:

```rust
pub fn enter_runtime_holding(w: u64) {
    if multi() {
        let slot = &RETURNED[worker_index()];
        slot.store(w, Ordering::SeqCst);
        lang_rt_enter(std::ptr::null_mut());
        slot.store(0, Ordering::SeqCst);
    }
}
```

`slot.store(w)` first, then `lang_rt_enter` (which acquires the lock). So
between storing `w` and acquiring the lock, the worker is *not* holding the
runtime lock. Another worker could acquire the lock and run GC. GC traces
`RETURNED` (under the lock? no — `for_each_root` is called from `collect_inner`
which is called from `collect` which is called under the lock). So while worker
A is between `slot.store(w)` and `lang_rt_enter`, worker B holds the lock and
runs GC. GC traces `RETURNED` and finds `w`. Good — `w` is rooted.

After `lang_rt_enter` returns (worker A holds the lock), `slot.store(0)` clears
`RETURNED`. Now `w` is only in `r` (the local in `call_closure_returning`).
`call_closure_returning` returns `w` to the coroutine body. The coroutine body
does `t.exit = Some(Exit::Returned(r))`. Between `slot.store(0)` and
`t.exit = ...`, no allocation happens in the coroutine body (the assignment is
a memory write). So no GC. Safe.

OK this is sound, but the reasoning is subtle. A comment would help.

---

## C15 — `chan_close_receiver` aliasing of C4

Already covered by C4. Calling it out for completeness: `close_receiver` is the
only caller-facing path that drops pointer data without a root.

---

## C16 — `chaos_pop` cast

**File:** `crates/lang_runtime/src/task.rs:1361-1368`

`(chaos_next() % r.ready.len() as u64) as usize` — the `as u64` on
`r.ready.len()` (which is `usize`) is safe. The `as usize` from `u64` is safe
on 32-bit and 64-bit platforms. On a hypothetical 16-bit platform with
`r.ready.len() > u16::MAX` this would truncate, but the runtime targets 64-bit.
Mark as informational.

---

## Items I checked and did not flag

- `gc::collect` mark/sweep tracing of records, enums, lists, maps, buffers,
  strings, channels, batches. All trace functions look correct and use
  `Repr::Ptr.load` to read pointer words.
- `gc::obj` poison check on freed objects (`gc.rs:62-68`) — only active under
  `LANG_GC_VERIFY=1`, which is the right default.
- The runtime lock acquisition (`task.rs:343-371`) and `notify_ready`
  (`task.rs:816-823`) — sound under the assumption that all `notify_ready`
  calls happen under the lock (verified by call sites).
- `chan_guard` roots `ch` via `task.roots` (persistent) as well as the
  `Roots` guard. The `Roots` guard is dropped, but `ch` remains in `task.roots`.
- `ffi::pack` / `ffi::unpack` allocate the result of `unpack` under a `Roots`
  guard. Intermediate pointer-typed values are added to the guard. Sound.
- `http_listen` calls `gc::pin(serve)` and `http_stop` calls `gc::unpin`.
  Symmetric.
- The blocking pool (`task.rs:1067-1122`) uses a mutex+condvar correctly. The
  `idle` accounting is decremented before the worker thread takes the next job,
  which is the right order to avoid lost wakeups.
- `live::publish` uses `swap` with `AcqRel` and is documented as atomic. The
  `keep_existing` check is a TOCTOU window (`load` then `swap`), but the
  semantics ("keep the existing code if one is published") tolerate the race:
  two concurrent publishers with `keep_existing=true` will both see the
  existing value and skip, which is the desired outcome.
- `lang_codegen` emits `lang_rt_enter`/`lang_rt_leave` around every runtime
  call (`codegen/src/lib.rs:1114-1131`), and `lang_rt_safe_begin`/`end` around
  foreign calls. Matches the runtime's expectations.

---

## Recommended next steps

1. **Targeted reproducers** for C1, C3, C5, C7, C9. C1 and C9 should be
   reproducible under `LANG_CHAOS=<seed> LANG_GC_STRESS=1` with several
   workers. C3 needs a specific scope/defer pattern. C5 needs
   `LANG_GC_STRESS=1` plus a list that exceeds the page boundary. C7 is a
   behavioral test (does the channel close at the right time?). C9 needs an
   external-thread `External::Callback` posted while all tasks are parked.
2. **Harden the allocators** (C2, C5): have `make_record` / `make_enum` and
   `list::push` root their result before returning to the caller or pushing
   into the parent. This removes the "every caller must remember" footgun.
3. **Fix C3** by reordering scope close to either remove all resources first
   and run defers last, or to make defers abort-tolerant (run remaining
   cleanups on abandon and let the existing `run_cleanups` path handle them).
4. **Document** the contracts in C4, C7, C13, C14 so future changes do not
   silently break them.
5. **Optional: add a static `STACK_POOL` wrapper** (C11) to remove the
   `static mut`.

These are candidates, not confirmed defects. Each needs a focused reproducer
in the existing adversarial/stress test framework before being filed as a bug.