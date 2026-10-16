//! One-shot replies (`channel.reply()`): a single value from one task to
//! another, as in a request/response `call`. One small object: no queue,
//! no channel bookkeeping, no finalizer. Its payload is
//!
//! ```text
//! state | element descriptor | value | waiter task | waiter's wait number
//! ```
//!
//! The state word's [`LOCK`] bit guards the rest (held for a few
//! instructions, by either side, with or without the runtime lock).

use std::sync::atomic::{AtomicU64, Ordering};

use crate::desc::{Kind, TypeDesc};
use crate::gc::{self, Header, obj};
use crate::task;

const EMPTY: u64 = 0;
/// A value waits to be received.
const VALUE: u64 = 1;
/// The value was received (or handed to the waiting receiver).
const DONE: u64 = 2;
/// Closed before a value was sent.
const CLOSED: u64 = 3;
const LOCK: u64 = 1 << 8;

fn reply_desc() -> &'static TypeDesc {
    static D: std::sync::OnceLock<&'static TypeDesc> = std::sync::OnceLock::new();
    D.get_or_init(|| {
        TypeDesc::new("Reply", Kind::Opaque { trace: Some(trace_reply), finalize: None, args: vec![] }).leak()
    })
}

fn words(r: u64) -> *mut u64 {
    unsafe { obj(r).payload() as *mut u64 }
}

fn state(r: u64) -> &'static AtomicU64 {
    unsafe { &*(words(r) as *const AtomicU64) }
}

fn elem(r: u64) -> &'static TypeDesc {
    unsafe { &*(*words(r).add(1) as *const TypeDesc) }
}

fn trace_reply(o: *mut Header, f: &mut dyn FnMut(u64)) {
    let p = unsafe { (*o).payload() as *const u64 };
    let st = unsafe { *p } & !LOCK;
    let d = unsafe { &*(*p.add(1) as *const TypeDesc) };
    if st == VALUE && d.is_ptr() {
        f(unsafe { *p.add(2) });
    }
}

/// A new reply for values of type `elem`.
pub fn new(elem: &'static TypeDesc) -> u64 {
    let r = gc::alloc(reply_desc(), 40);
    unsafe { *words(r).add(1) = elem as *const TypeDesc as u64 };
    r
}

/// Hold the reply's lock; returns its state.
fn lock(r: u64) -> u64 {
    let s = state(r);
    loop {
        let cur = s.load(Ordering::Relaxed);
        if cur & LOCK == 0
            && s.compare_exchange_weak(cur, cur | LOCK, Ordering::Acquire, Ordering::Relaxed).is_ok()
        {
            return cur;
        }
        std::hint::spin_loop();
    }
}

fn unlock(r: u64, st: u64) {
    state(r).store(st, Ordering::Release);
}

/// Take the registered waiter (lock held).
fn take_waiter(r: u64) -> Option<(usize, u64)> {
    let w = words(r);
    let p = unsafe { *w.add(3) };
    if p == 0 {
        return None;
    }
    unsafe { *w.add(3) = 0 };
    Some((p as usize, unsafe { *w.add(4) }))
}

/// Send `v`: `false` when a value was sent already or the reply was closed
/// (the value is then the caller's again). The caller roots `r` and `v`.
pub fn send(r: u64, v: u64) -> bool {
    let st = lock(r);
    if st != EMPTY {
        unlock(r, st);
        return false;
    }
    let is_ptr = elem(r).is_ptr();
    if let Some((p, wait)) = take_waiter(r) {
        unlock(r, DONE);
        if task::deliver(p, wait, v, is_ptr, 1) {
            return true;
        }
        // the receiver stopped waiting (it is being cancelled): keep the value
        let _ = lock(r);
    }
    unsafe { *words(r).add(2) = v };
    if is_ptr {
        gc::barrier(r);
    }
    unlock(r, VALUE);
    true
}

/// Close before sending: a waiting receiver gets nothing. Closing after a
/// send changes nothing.
pub fn close(r: u64) {
    let st = lock(r);
    if st != EMPTY {
        unlock(r, st);
        return;
    }
    let waiter = take_waiter(r);
    unlock(r, CLOSED);
    if let Some((p, wait)) = waiter {
        task::deliver(p, wait, 0, false, 0);
    }
}

/// Without waiting: the value if one was sent and not yet received.
fn try_take(r: u64) -> Result<Option<u64>, ()> {
    let st = lock(r);
    match st {
        VALUE => {
            let v = unsafe { std::mem::take(&mut *words(r).add(2)) };
            unlock(r, DONE);
            Ok(Some(v))
        }
        DONE | CLOSED => {
            unlock(r, st);
            Ok(None)
        }
        _ => {
            unlock(r, st);
            Err(())
        }
    }
}

/// Receive: the value, waiting for it; `None` once closed without one or
/// already received. Cancellation interrupts the wait. The caller roots
/// `r`.
pub fn recv(r: u64) -> Option<u64> {
    let roots = gc::Roots::new();
    roots.add(r);
    loop {
        let st = lock(r);
        match st {
            VALUE | DONE | CLOSED => {
                unlock(r, st);
                return try_take(r).ok().flatten();
            }
            _ => {}
        }
        let me = task::current_ptr();
        let wait = task::begin_wait();
        let parked = if task::can_suspend_free() {
            if !task::begin_park_free(4) {
                unlock(r, st);
                interrupt();
                continue;
            }
            unsafe {
                *words(r).add(3) = me as u64;
                *words(r).add(4) = wait;
            }
            unlock(r, st);
            task::park_free();
            true
        } else {
            // a locked wait: marked about to park first, so a value sent
            // before it parks is kept for its park
            task::set_pre_park();
            unsafe {
                *words(r).add(3) = me as u64;
                *words(r).add(4) = wait;
            }
            unlock(r, st);
            let locked = !task::holds_lock();
            if locked {
                task::enter_runtime_for(task::reason::CHANNEL_SLOW);
            }
            // shielded: the registration is withdrawn before an interrupt
            task::park_shielded();
            if locked {
                task::leave_runtime();
            }
            true
        };
        if parked {
            let (v, status) = task::take_delivered();
            if status == 1 {
                return Some(v);
            }
            // woken without a value (closed, or cancelled): withdraw
            let st = lock(r);
            if unsafe { *words(r).add(3) } == me as u64 {
                unsafe { *words(r).add(3) = 0 };
            }
            unlock(r, st);
            interrupt();
        }
    }
}

/// Honor a pending cancellation or failure (under the runtime lock).
fn interrupt() {
    let locked = !task::holds_lock();
    if locked {
        task::enter_runtime_for(task::reason::CHANNEL_SLOW);
    }
    task::check_interrupts();
    if locked {
        task::leave_runtime();
    }
}
