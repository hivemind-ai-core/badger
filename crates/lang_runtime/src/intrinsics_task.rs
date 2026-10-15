//! Task, scope, channel, time, and cleanup intrinsics (uniform ABI).

use crate::channel::{self, RecvResult, SendResult};
use crate::desc::{Kind, TypeDesc};
use crate::gc::Roots;
use crate::intrinsics::{Targs, rdesc, targ};
use crate::string::new_string;
use crate::task::{self, Cleanup, Exit, TaskCtx};
use crate::{make_record, make_variant, scope};

fn exit_word(id: u64) -> Option<Exit> {
    task::exit_of(id as u32)
}

/// `SendError<T>` result for an unsent value.
fn send_err(ret: &'static TypeDesc, v: u64, reason: &str) -> u64 {
    let r = Roots::new();
    let err_desc = ret.variant(1).map(|v| v.fields[0].desc).expect("send error type");
    if err_desc.field("value").is_some_and(|f| f.desc.is_ptr()) {
        r.add(v);
    }
    let e = r.add(make_record(err_desc, &[("value", v), ("reason", crate::string::symbol(reason))]));
    make_variant(ret, "err", &[e])
}

fn ok_unit(ret: &'static TypeDesc) -> u64 {
    make_variant(ret, "ok", &[0])
}

pub fn panic_info_record(desc: &'static TypeDesc, p: &task::PanicInfo) -> u64 {
    let r = Roots::new();
    let m = r.add(new_string(&p.message));
    let l = r.add(new_string(&p.location));
    let notes_desc = desc.field("notes").map(|f| f.desc);
    match notes_desc {
        Some(nd) => {
            let list = r.add(crate::list::new(nd));
            for n in &p.notes {
                let w = r.add(new_string(n));
                crate::list::push(list, w);
            }
            make_record(desc, &[("message", m), ("location", l), ("notes", list)])
        }
        None => make_record(desc, &[("message", m), ("location", l)]),
    }
}

/// Whether a channel fast path may run: several workers, and the caller
/// does not hold the runtime lock (it was called as an unlocked entry).
fn fast_ok() -> bool {
    task::multi() && !task::holds_lock()
}

/// Run `f` holding the runtime lock, taking it if this unlocked entry was
/// called without it.
fn locked<R>(f: impl FnOnce() -> R) -> R {
    if task::holds_lock() {
        return f();
    }
    task::enter_runtime();
    let r = f();
    task::leave_runtime();
    r
}

fn send_result(ret: *const TypeDesc, v: u64, r: SendResult) -> u64 {
    match r {
        SendResult::Sent => ok_unit(rdesc(ret)),
        SendResult::Closed => send_err(rdesc(ret), v, "channel/closed"),
        SendResult::Full => send_err(rdesc(ret), v, "channel/full"),
        SendResult::TooLarge => send_err(rdesc(ret), v, "channel/too_large"),
    }
}

macro_rules! task_intrinsics {
    ($( fn $name:ident($ctx:ident, $targs:ident, $ret:ident $(, $arg:ident)*) $body:block )*) => {
        $(
            #[allow(unused_variables)]
            pub extern "C" fn $name($ctx: *mut TaskCtx, $targs: Targs, $ret: *const TypeDesc $(, $arg: u64)*) -> u64 $body
        )*
        pub fn symbols() -> Vec<(&'static str, *const u8)> {
            vec![$((concat!("lang_rt_i_", stringify!($name)), $name as *const u8)),*]
        }
    };
}

task_intrinsics! {
    // ---------------------------------------------------------------- scopes & tasks
    fn scope_open(ctx, targs, ret) { scope::open() }
    fn scope_close(ctx, targs, ret, s, cancel) { scope::close(s, cancel & 1 != 0); 0 }
    fn scope_defer(ctx, targs, ret, s, f) { scope::defer(s, f); 0 }
    fn task_spawn(ctx, targs, ret, s, f, captured) {
        let t = targ(targs, 0);
        scope::spawn(s, f, captured & 1 != 0, t.is_ptr()) as u64
    }
    fn task_join(ctx, targs, ret, id, captured) {
        if captured & 1 != 0 {
            return task_exit(ctx, targs, ret, id);
        }
        task::await_task(id as u32, false);
        match exit_word(id) {
            Some(Exit::Returned(w)) => w,
            Some(Exit::Failed(p)) => task::panic_current(format!("joined task failed: {}", p.message), p.location),
            _ => task::panic_current("joined a cancelled task".into(), String::new()),
        }
    }
    fn task_exit(ctx, targs, ret, id) {
        task::await_task(id as u32, false);
        let ret = rdesc(ret);
        match exit_word(id) {
            Some(Exit::Returned(w)) => make_variant(ret, "returned", &[w]),
            Some(Exit::Failed(p)) => {
                let info_desc = ret.variant(ret.variant_index("failed").unwrap()).unwrap().fields[0].desc;
                let r = Roots::new();
                let info = r.add(panic_info_record(info_desc, &p));
                make_variant(ret, "failed", &[info])
            }
            _ => {
                let reason = task::task(id as u32).map(|t| t.cancel_reason).unwrap_or(task::CancelReason::Requested);
                let vi = ret.variant_index("cancelled").unwrap();
                let rdesc_ = ret.variant(vi).unwrap().fields[0].desc;
                let r = Roots::new();
                let why = r.add(make_variant(rdesc_, reason.name(), &[]));
                make_variant(ret, "cancelled", &[why])
            }
        }
    }
    // Wait for fallible tasks in any order; on the first `err`, cancel and
    // settle every unfinished sibling and return the failing index (else -1).
    fn task_try_join_all(ctx, targs, ret, ids) {
        let ids: Vec<u32> = (0..crate::list::len(ids)).map(|i| crate::list::get(ids, i) as u32).collect();
        loop {
            for (i, &id) in ids.iter().enumerate() {
                if let Some(Exit::Returned(w)) = task::exit_of(id) {
                    let (tag, _) = crate::enum_parts(w);
                    if tag == 1 {
                        for &other in ids.iter().filter(|o| **o != id) {
                            task::cancel_with(other, task::CancelReason::SiblingError);
                        }
                        for &other in ids.iter().filter(|o| **o != id) {
                            task::await_task(other, true);
                        }
                        return i as u64;
                    }
                }
                if let Some(Exit::Failed(p)) = task::exit_of(id) {
                    task::panic_current(format!("child task failed: {}", p.message), p.location);
                }
            }
            if ids.iter().all(|id| task::is_done(*id)) {
                return u64::MAX;
            }
            let me = task::current_id();
            for &id in &ids {
                if let Some(t) = task::task(id)
                    && !t.joiners.contains(&me) {
                        t.joiners.push(me);
                    }
            }
            task::park_for("join");
        }
    }
    fn task_done(ctx, targs, ret, id) { task::is_done(id as u32) as u64 }
    // attach a cleanup failure to the current task's primary defect
    fn attach_note(ctx, targs, ret, note) {
        task::attach_note(crate::string::str_of(note).to_string());
        0
    }
    fn type_id(ctx, targs, ret) { targ(targs, 0) as *const TypeDesc as u64 }
    fn task_cancel_timeout(ctx, targs, ret, id) { task::cancel_with(id as u32, task::CancelReason::Timeout); 0 }
    fn task_cancel(ctx, targs, ret, id) { task::cancel(id as u32); 0 }
    fn task_settle(ctx, targs, ret, id) { task::await_task(id as u32, true); 0 }
    fn task_yield(ctx, targs, ret) { task::yield_now(); 0 }
    fn task_sleep(ctx, targs, ret, nanos) {
        task::sleep(std::time::Duration::from_nanos((nanos as i64).max(0) as u64));
        0
    }
    fn task_id(ctx, targs, ret) { task::current_id() as u64 }
    fn task_worker(ctx, targs, ret) { task::worker_index() as u64 }
    fn task_workers(ctx, targs, ret) { task::workers() as u64 }
    fn time_now(ctx, targs, ret) {
        static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        if let Some(t) = *crate::intrinsics_os::MANUAL_CLOCK.lock().unwrap() {
            return t as u64;
        }
        let s = START.get_or_init(std::time::Instant::now);
        s.elapsed().as_nanos() as u64
    }
    fn time_wall_nanos(ctx, targs, ret) {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0)
    }

    // ---------------------------------------------------------------- cleanup
    fn cleanup_push(ctx, targs, ret, f) { task::push_cleanup(Cleanup::Closure(f)) }
    fn cleanup_remove(ctx, targs, ret, h) { task::remove_cleanup(h); 0 }
    fn task_stage_push(ctx, targs, ret, name) {
        if let Some(t) = task::current() {
            t.stages.push(crate::string::str_of(name).to_string());
        }
        0
    }
    fn task_stage_pop(ctx, targs, ret) {
        if let Some(t) = task::current() {
            t.stages.pop();
        }
        0
    }
    fn iter_on_close(ctx, targs, ret, it, f) { task::iter_on_close(it, f); 0 }
    fn iter_has_close(ctx, targs, ret, it) { task::iter_has_close(it) as u64 }
    fn iter_close(ctx, targs, ret, it) { task::iter_close(it); 0 }
    fn chan_guard(ctx, targs, ret, ch) {
        let r = Roots::new();
        r.add(ch);
        // keep the channel alive while the guard is registered
        task::current().unwrap().roots.push(ch);
        task::push_cleanup(Cleanup::Native(Box::new(move |_| channel::close(ch))))
    }
    fn chan_unguard(ctx, targs, ret, h, ch) {
        task::remove_cleanup(h);
        let t = task::current().unwrap();
        if let Some(i) = t.roots.iter().rposition(|w| *w == ch) {
            t.roots.remove(i);
        }
        0
    }

    // ---------------------------------------------------------------- channels
    fn chan_new(ctx, targs, ret, capacity, max_batch, batched, byte_budget) {
        let ch = channel::new(targ(targs, 0), capacity as usize, max_batch as usize, batched & 1 != 0);
        channel::data(ch).byte_budget = byte_budget as usize;
        ch
    }
    // Fair selection over homogeneous operations: (kinds, channels, values)
    // lists; returns `Selected<T>`.
    // a rotating start position for fair selection
    fn select_seq(ctx, targs, ret) {
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
    fn chan_select(ctx, targs, ret, kinds, chans, values, start, timeout) {
        let ret = rdesc(ret);
        let g = Roots::new();
        g.add(kinds);
        g.add(chans);
        g.add(values);
        let n = crate::list::len(kinds);
        let deadline = if (timeout as i64) >= 0 { Some(std::time::Instant::now() + std::time::Duration::from_nanos(timeout)) } else { None };
        if n == 0 {
            return make_variant(ret, "timeout", &[]);
        }
        loop {
            for k in 0..n {
                let i = (start as usize + k) % n;
                let ch = crate::list::get(chans, i);
                if crate::list::get(kinds, i) & 1 == 0 {
                    // kind 2 receives one published message of a batch channel
                    let got = if crate::list::get(kinds, i) == 2 { channel::recv_one(ch, false) } else { channel::recv(ch, false) };
                    match got {
                        RecvResult::Value(v) => {
                            let some = g.add(make_variant(ret.variant(0).unwrap().fields[1].desc, "some", &[v]));
                            return make_variant(ret, "received", &[i as u64, some]);
                        }
                        RecvResult::Closed => {
                            let none = make_variant(ret.variant(0).unwrap().fields[1].desc, "none", &[]);
                            return make_variant(ret, "received", &[i as u64, none]);
                        }
                        RecvResult::Empty => {}
                    }
                } else {
                    let v = crate::list::get(values, i);
                    match channel::send(ch, v, false) {
                        SendResult::Sent => return make_variant(ret, "sent", &[i as u64]),
                        // the send can never commit: report it, keeping the value
                        SendResult::Closed => return make_variant(ret, "closed", &[i as u64]),
                        SendResult::TooLarge => return make_variant(ret, "too_large", &[i as u64]),
                        _ => {}
                    }
                }
            }
            if let Some(dl) = deadline
                && std::time::Instant::now() >= dl {
                    return make_variant(ret, "timeout", &[]);
                }
            for i in 0..n {
                let ch = crate::list::get(chans, i);
                if crate::list::get(kinds, i) & 1 == 0 { channel::watch_recv(ch) } else { channel::watch_space(ch) }
            }
            // a rendezvous partner may commit one of these cases while we wait
            offer_cases(kinds, chans, |i| crate::list::get(values, i));
            match deadline {
                Some(dl) => { task::sleep(dl.saturating_duration_since(std::time::Instant::now())); }
                None => { task::park_for("select"); }
            }
            for i in 0..n {
                channel::unwatch(crate::list::get(chans, i));
            }
            if let Some((i, v)) = take_committed(kinds) {
                if crate::list::get(kinds, i) & 1 == 1 {
                    return make_variant(ret, "sent", &[i as u64]);
                }
                let some = g.add(make_variant(ret.variant(0).unwrap().fields[1].desc, "some", &[v]));
                return make_variant(ret, "received", &[i as u64, some]);
            }
        }
    }
    fn type_desc(ctx, targs, ret) { targ(targs, 0) as *const TypeDesc as u64 }
    // Heterogeneous selection: each case has its own message type. Values
    // travel boxed in `Option<T>` objects whose descriptors come with the
    // cases; exactly one case commits. Returns `{index, value}` (the received
    // `Option<T>`, or nothing for a send / `-1` on timeout).
    fn chan_select_cases(ctx, targs, ret, kinds, chans, values, odescs, start, timeout) {
        let ret = rdesc(ret);
        let g = Roots::new();
        g.add(kinds);
        g.add(chans);
        g.add(values);
        g.add(odescs);
        let n = crate::list::len(kinds);
        let deadline = if (timeout as i64) >= 0 { Some(std::time::Instant::now() + std::time::Duration::from_nanos(timeout)) } else { None };
        let result = |i: i64, v: u64| make_record(ret, &[("index", i as u64), ("value", v)]);
        if n == 0 {
            return result(-1, 0);
        }
        // send cases on closed channels drop out of contention
        let mut dead = vec![false; n];
        loop {
            for k in 0..n {
                let i = (start as usize + k) % n;
                if dead[i] {
                    continue;
                }
                let ch = crate::list::get(chans, i);
                let od = unsafe { &*(crate::list::get(odescs, i) as *const TypeDesc) };
                if crate::list::get(kinds, i) & 1 == 0 {
                    let got = if crate::list::get(kinds, i) == 2 { channel::recv_one(ch, false) } else { channel::recv(ch, false) };
                    match got {
                        RecvResult::Value(v) => {
                            let some = g.add(make_variant(od, "some", &[v]));
                            return result(i as i64, some);
                        }
                        RecvResult::Closed => {
                            let none = g.add(make_variant(od, "none", &[]));
                            return result(i as i64, none);
                        }
                        RecvResult::Empty => {}
                    }
                } else {
                    let boxed = crate::list::get(values, i);
                    let v = crate::enum_parts(boxed).1[0];
                    match channel::send(ch, v, false) {
                        SendResult::Sent => return result(i as i64, boxed),
                        // can never commit (closed, or larger than the budget)
                        SendResult::Closed | SendResult::TooLarge => dead[i] = true,
                        _ => {}
                    }
                }
            }
            if dead.iter().all(|d| *d) {
                return result(-1, 0);
            }
            if let Some(dl) = deadline
                && std::time::Instant::now() >= dl
            {
                return result(-1, 0);
            }
            for i in (0..n).filter(|i| !dead[*i]) {
                let ch = crate::list::get(chans, i);
                if crate::list::get(kinds, i) & 1 == 0 { channel::watch_recv(ch) } else { channel::watch_space(ch) }
            }
            offer_cases(kinds, chans, |i| if dead[i] { 0 } else { crate::enum_parts(crate::list::get(values, i)).1.first().copied().unwrap_or(0) });
            if let Some(t) = task::current() {
                t.select_offer.retain(|c| !dead[c.1]);
            }
            match deadline {
                Some(dl) => {
                    task::sleep(dl.saturating_duration_since(std::time::Instant::now()));
                }
                None => {
                    task::park_for("select");
                }
            }
            for i in (0..n).filter(|i| !dead[*i]) {
                channel::unwatch(crate::list::get(chans, i));
            }
            if let Some((i, v)) = take_committed(kinds) {
                if crate::list::get(kinds, i) & 1 == 1 {
                    return result(i as i64, crate::list::get(values, i));
                }
                let od = unsafe { &*(crate::list::get(odescs, i) as *const TypeDesc) };
                let some = g.add(make_variant(od, "some", &[v]));
                return result(i as i64, some);
            }
        }
    }
    // prompt send/receive are unlocked entries: a fast path completes
    // without the runtime lock when no other task is involved, otherwise
    // the locked path decides
    fn chan_send(ctx, targs, ret, ch, v) {
        if fast_ok() && channel::send_fast(ch, v) {
            return ok_unit(rdesc(ret));
        }
        locked(|| send_result(ret, v, channel::send(ch, v, true)))
    }
    fn chan_try_send(ctx, targs, ret, ch, v) {
        if fast_ok() && channel::send_fast(ch, v) {
            return ok_unit(rdesc(ret));
        }
        locked(|| send_result(ret, v, channel::send(ch, v, false)))
    }
    fn chan_recv(ctx, targs, ret, ch) {
        if fast_ok()
            && let Some(v) = channel::recv_fast(ch)
        {
            let r = Roots::new();
            if channel::data_elem(ch).is_ptr() {
                r.add(v);
            }
            return make_variant(rdesc(ret), "some", &[v]);
        }
        locked(|| match channel::recv(ch, true) {
            RecvResult::Value(v) => make_variant(rdesc(ret), "some", &[v]),
            _ => make_variant(rdesc(ret), "none", &[]),
        })
    }
    fn chan_try_recv(ctx, targs, ret, ch) {
        if fast_ok()
            && let Some(v) = channel::recv_fast(ch)
        {
            let r = Roots::new();
            if channel::data_elem(ch).is_ptr() {
                r.add(v);
            }
            return make_variant(rdesc(ret), "value", &[v]);
        }
        locked(|| match channel::recv(ch, false) {
            RecvResult::Value(v) => make_variant(rdesc(ret), "value", &[v]),
            RecvResult::Empty => make_variant(rdesc(ret), "empty", &[]),
            RecvResult::Closed => make_variant(rdesc(ret), "closed", &[]),
        })
    }
    fn chan_close(ctx, targs, ret, ch) { channel::close(ch); 0 }
    fn chan_len(ctx, targs, ret, ch) {
        let d = channel::data(ch);
        (d.queue.len() + d.batches.iter().map(std::collections::VecDeque::len).sum::<usize>()) as u64
    }
    // batch append and single receive are unlocked entries too
    fn batch_append(ctx, targs, ret, ch, v) {
        if fast_ok() && channel::append_fast(ch, v) {
            return ok_unit(rdesc(ret));
        }
        locked(|| match channel::append(ch, v, true) {
            SendResult::Sent => ok_unit(rdesc(ret)),
            SendResult::TooLarge => send_err(rdesc(ret), v, "channel/too_large"),
            _ => send_err(rdesc(ret), v, "channel/closed"),
        })
    }
    fn batch_try_append(ctx, targs, ret, ch, v) {
        if fast_ok() && channel::append_fast(ch, v) {
            return ok_unit(rdesc(ret));
        }
        locked(|| match channel::append(ch, v, false) {
            SendResult::Sent => ok_unit(rdesc(ret)),
            SendResult::TooLarge => send_err(rdesc(ret), v, "channel/too_large"),
            SendResult::Full => send_err(rdesc(ret), v, "channel/full"),
            SendResult::Closed => send_err(rdesc(ret), v, "channel/closed"),
        })
    }
    fn batch_recv_one(ctx, targs, ret, ch) {
        if fast_ok()
            && let Some(v) = channel::recv_one_fast(ch)
        {
            let r = Roots::new();
            if channel::data_elem(ch).is_ptr() {
                r.add(v);
            }
            return make_variant(rdesc(ret), "some", &[v]);
        }
        locked(|| match channel::recv_one(ch, true) {
            RecvResult::Value(v) => make_variant(rdesc(ret), "some", &[v]),
            _ => make_variant(rdesc(ret), "none", &[]),
        })
    }
    fn batch_try_recv(ctx, targs, ret, ch) {
        if fast_ok()
            && let Some(v) = channel::recv_one_fast(ch)
        {
            let r = Roots::new();
            if channel::data_elem(ch).is_ptr() {
                r.add(v);
            }
            return make_variant(rdesc(ret), "value", &[v]);
        }
        locked(|| match channel::recv_one(ch, false) {
            RecvResult::Value(v) => make_variant(rdesc(ret), "value", &[v]),
            RecvResult::Empty => make_variant(rdesc(ret), "empty", &[]),
            RecvResult::Closed => make_variant(rdesc(ret), "closed", &[]),
        })
    }
    fn chan_close_receiver(ctx, targs, ret, ch) { channel::close_receiver(ch); 0 }
    fn chan_configure(ctx, targs, ret, ch, discard, latency) {
        let d = channel::data(ch);
        d.discard_on_close = discard & 1 == 1;
        d.flush_latency = std::time::Duration::from_nanos(latency);
        0
    }
    fn batch_flush(ctx, targs, ret, ch) { channel::flush(ch); 0 }
    fn batch_recv(ctx, targs, ret, ch) {
        match channel::recv_batch(ch) {
            Some(b) => make_variant(rdesc(ret), "some", &[b]),
            None => make_variant(rdesc(ret), "none", &[]),
        }
    }
    fn batch_len(ctx, targs, ret, b) {
        let bt = channel::batch(b);
        if bt.released {
            task::panic_current("batch used after release".into(), String::new());
        }
        bt.items.len() as u64
    }
    fn batch_get(ctx, targs, ret, b, i) {
        let bt = channel::batch(b);
        if bt.released {
            task::panic_current("batch used after release".into(), String::new());
        }
        bt.items[i as usize]
    }
    fn batch_release(ctx, targs, ret, b) { channel::release(b); 0 }
    fn batches_published(ctx, targs, ret, ch) { channel::data(ch).published_batches }
}

#[allow(dead_code)]
fn is_list(d: &TypeDesc) -> bool {
    matches!(d.kind, Kind::List { .. })
}

/// Post a parked selection's cases so a rendezvous partner can commit one.
fn offer_cases(kinds: u64, chans: u64, value: impl Fn(usize) -> u64) {
    let n = crate::list::len(kinds);
    if let Some(t) = task::current() {
        t.select_done = None;
        t.select_offer = (0..n)
            .map(|i| (crate::list::get(chans, i), i, crate::list::get(kinds, i) & 1 == 1, value(i)))
            .collect();
    }
}

/// The case a partner committed while the selection was parked, with the
/// received value (withdrawing the offer either way).
fn take_committed(kinds: u64) -> Option<(usize, u64)> {
    let t = task::current()?;
    t.select_offer.clear();
    let i = t.select_done.take()?;
    let (v, _) = task::take_delivered();
    let _ = kinds;
    Some((i, v))
}
