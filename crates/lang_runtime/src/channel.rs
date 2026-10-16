//! Bounded channels and batch channels.
//!
//! A prompt `Channel<T>` publishes each send immediately (capacity zero is a
//! rendezvous). A `BatchChannel<T>` accepts appends into per-producer staging
//! owned by the runtime and publishes sealed batches when a batch fills, on
//! flush/close, and whenever the producing task schedules out (parks, yields,
//! or finishes) — so staged messages are never stranded. Admission counts
//! staged, queued, and leased messages against capacity; a producer flushes
//! its own staging before waiting for capacity.

use std::collections::VecDeque;

use rustc_hash::FxHashMap;

use crate::desc::{Kind, TypeDesc};
use crate::gc::{self, Header, obj};
use crate::task::{self, TaskId};

pub struct Batch {
    pub items: Vec<u64>,
    pub released: bool,
}

/// A channel's state. Every field except `state` and the creation-time
/// constants (`elem`, `capacity`, `max_batch`, `batched`, `byte_budget`) is
/// read and written only by the channel's current owner: an unlocked fast
/// path holding it (`try_fast`), or runtime-lock code that claimed it
/// ([`data`]). The two never overlap, so no counter or queue is shared
/// between them unsynchronized.
pub struct ChanData {
    /// Creation number (inspection lists channels in creation order).
    pub seq: u64,
    /// Ownership with several workers: 0 free, 1 held by an unlocked fast
    /// path, 2 claimed by code holding the runtime lock (until it releases
    /// that lock).
    pub state: std::sync::atomic::AtomicU8,
    pub elem: &'static TypeDesc,
    pub capacity: usize,
    pub max_batch: usize,
    pub batched: bool,
    pub closed: bool,
    /// Prompt channel: queued messages. Batch channel: unused.
    pub queue: VecDeque<u64>,
    /// Batch channel: sealed batches awaiting a receiver (single-message
    /// receives pop from the front one).
    pub batches: VecDeque<VecDeque<u64>>,
    /// Batch channel: staged, published, and leased messages (admission
    /// occupancy, kept current instead of recounted).
    pub accepted: usize,
    /// Batch channel: per-producer staging (accepted, not yet published).
    pub staging: FxHashMap<TaskId, Vec<u64>>,
    /// Messages in leased (received, unreleased) batches.
    pub leased: usize,
    /// Unreleased leases and the tasks holding them (released when the
    /// holder finishes; traced so a lease outlives its last reference).
    pub leases: Vec<(TaskId, u64)>,
    /// Parked receivers and the wait each registered (see `task::waits`).
    /// (task, its wait, its record: see `task::deliver`)
    pub recv_waiters: VecDeque<(TaskId, u64, usize)>,
    /// Waiting senders with their (still owned) values.
    /// Parked senders with their values and byte charges.
    /// (task, value, byte charge, its wait, its record)
    pub send_waiters: VecDeque<(TaskId, u64, usize, u64, usize)>,
    /// Batch producers waiting for capacity.
    pub cap_waiters: VecDeque<TaskId>,
    /// Recycled page buffers (bounded reserve).
    pub pool: Vec<Vec<u64>>,
    pub published_batches: u64,
    /// Tasks (e.g. in `select`) waiting for capacity without committing a value.
    pub space_waiters: VecDeque<TaskId>,
    /// Tasks (e.g. in `select`) notified when a message may be available.
    pub recv_watchers: VecDeque<TaskId>,
    /// Admission byte budget (0 = unlimited) and bytes currently queued.
    pub byte_budget: usize,
    pub bytes: usize,
    /// Sizes of queued messages (prompt channels with a byte budget).
    pub sizes: VecDeque<usize>,
    /// Batch channels: discard (instead of publish) staged data on close.
    pub discard_on_close: bool,
    /// Batch channels: publish staged data at the next safe point once it
    /// is this old.
    pub flush_latency: std::time::Duration,
    /// When each producer's current staging began.
    pub staged_at: FxHashMap<TaskId, std::time::Instant>,
}

impl ChanData {
    fn count(&self) -> usize {
        debug_assert_eq!(
            self.accepted,
            self.batches.iter().map(VecDeque::len).sum::<usize>()
                + self.staging.values().map(Vec::len).sum::<usize>()
                + self.leased,
            "batch channel occupancy drifted"
        );
        self.queue.len() + self.accepted
    }
}

fn chan_desc() -> &'static TypeDesc {
    static D: std::sync::OnceLock<&'static TypeDesc> = std::sync::OnceLock::new();
    D.get_or_init(|| {
        TypeDesc::new(
            "Channel",
            Kind::Opaque { trace: Some(trace_chan), finalize: Some(finalize_chan), args: vec![] },
        )
        .leak()
    })
}

fn batch_desc() -> &'static TypeDesc {
    static D: std::sync::OnceLock<&'static TypeDesc> = std::sync::OnceLock::new();
    D.get_or_init(|| {
        TypeDesc::new(
            "Batch",
            Kind::Opaque { trace: Some(trace_batch), finalize: Some(finalize_batch), args: vec![] },
        )
        .leak()
    })
}

fn trace_chan(o: *mut Header, f: &mut dyn FnMut(u64)) {
    let d = unsafe { &*(*((*o).payload() as *const *mut ChanData)) };
    d.leases.iter().for_each(|(_, b)| f(*b));
    if !d.elem.is_ptr() {
        return;
    }
    d.queue.iter().for_each(|w| f(*w));
    d.batches.iter().flatten().for_each(|w| f(*w));
    d.staging.values().flatten().for_each(|w| f(*w));
    d.send_waiters.iter().for_each(|(_, w, _, _, _)| f(*w));
}

/// Channel numbers for inspection, in creation order.
static NEXT_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Inspection snapshot of every live channel (JSON array).
#[allow(static_mut_refs)]
pub fn snapshot_json() -> String {
    // found on the heap: keeping a shared registry of live channels made
    // creating and finalizing them contend across workers
    let mut live: Vec<(u64, u64)> = vec![];
    gc::for_each_object_of(chan_desc(), &mut |w| {
        let p = unsafe { *(obj(w).payload() as *const *mut ChanData) };
        if !p.is_null() {
            live.push((unsafe { (*p).seq }, w));
        }
    });
    live.sort_unstable();
    let items: Vec<String> = live
        .iter()
        .map(|&(seq, w)| {
            let d = data(w);
            let staged: usize = d.staging.values().map(Vec::len).sum();
            format!(
                "{{\"id\":{seq},\"kind\":\"{}\",\"element\":{},\"capacity\":{},\"queued\":{},\"staged\":{staged},\"leased\":{},\"closed\":{},\"waiting_senders\":{},\"waiting_receivers\":{},\"bytes\":{}}}",
                if d.batched { "batch" } else { "prompt" },
                // the prelude's names for the default numeric types
                crate::intrinsics_io::json_quote(match d.elem.name.as_str() {
                    "I64" => "Int",
                    "F64" => "Float",
                    n => n,
                }),
                d.capacity,
                d.count(),
                d.leased,
                d.closed,
                d.send_waiters.len() + d.cap_waiters.len(),
                d.recv_waiters.len(),
                d.bytes,
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}

#[allow(static_mut_refs)]
fn finalize_chan(o: *mut Header) {
    unsafe {
        (*CLAIMED.0.get()).retain(|w| *w != o as u64);
        let p = (*o).payload() as *mut *mut ChanData;
        if !(*p).is_null() {
            drop(Box::from_raw(*p));
            *p = std::ptr::null_mut();
        }
    }
}

/// Batch payload: `*mut Batch`, then the owning channel object.
fn trace_batch(o: *mut Header, f: &mut dyn FnMut(u64)) {
    unsafe {
        let p = (*o).payload();
        // a lease object not yet filled in (allocated before its batch)
        let bp = *(p as *const *mut Batch);
        if bp.is_null() {
            return;
        }
        let b = &*bp;
        let ch = *(p.add(8) as *const u64);
        f(ch);
        if data(ch).elem.is_ptr() {
            b.items.iter().for_each(|w| f(*w));
        }
    }
}

fn finalize_batch(o: *mut Header) {
    unsafe {
        let p = (*o).payload() as *mut *mut Batch;
        if !(*p).is_null() {
            drop(Box::from_raw(*p));
            *p = std::ptr::null_mut();
        }
    }
}

/// A channel's state. With several workers, code holding the runtime lock
/// claims the channel on first access (waiting out an unlocked fast path),
/// and keeps it until the lock is released.
pub fn data<'a>(ch: u64) -> &'a mut ChanData {
    let d = raw(ch);
    if task::multi() && task::holds_lock() {
        claim(ch, d);
    }
    d
}

fn raw<'a>(ch: u64) -> &'a mut ChanData {
    unsafe { &mut **(obj(ch).payload() as *mut *mut ChanData) }
}

/// Channels claimed by the runtime lock's holder (only it touches this).
struct Claims(std::cell::UnsafeCell<Vec<u64>>);
unsafe impl Sync for Claims {}
static CLAIMED: Claims = Claims(std::cell::UnsafeCell::new(Vec::new()));

fn claim(ch: u64, d: &ChanData) {
    use std::sync::atomic::Ordering;
    // one thread holds the runtime lock: a claim already made is its own
    if d.state.load(Ordering::Acquire) == 2 {
        return;
    }
    while d.state.compare_exchange_weak(0, 2, Ordering::Acquire, Ordering::Relaxed).is_err() {
        std::hint::spin_loop();
    }
    unsafe { (*CLAIMED.0.get()).push(ch) };
}

/// Release every channel claimed under the runtime lock (called as the
/// lock is released).
pub fn release_claims() {
    let claimed = unsafe { &mut *CLAIMED.0.get() };
    for ch in claimed.drain(..) {
        raw(ch).state.store(0, std::sync::atomic::Ordering::Release);
    }
}

/// A channel's element type (fixed at creation: read without claiming).
pub fn data_elem(ch: u64) -> &'static TypeDesc {
    raw(ch).elem
}

/// Take a channel for an unlocked fast path, if nothing else holds it.
/// Another fast path holds a channel for a few instructions, and a claim by
/// the runtime lock's holder lasts until it releases that lock: wait for
/// either briefly, with backoff, rather than falling back to the runtime
/// lock (one fallback would claim the channel and make every other
/// operation on it fall back too). The wait is bounded: the lock's holder
/// may be a collection waiting for this worker to reach a safe point, which
/// the fallback is.
fn try_fast(d: &ChanData) -> bool {
    use std::sync::atomic::Ordering;
    let mut spins = 1u32;
    for _ in 0..24 {
        if d.state.compare_exchange_weak(0, 1, Ordering::Acquire, Ordering::Relaxed).is_ok() {
            return true;
        }
        for _ in 0..spins {
            std::hint::spin_loop();
        }
        spins = (spins * 2).min(256);
    }
    false
}

fn end_fast(d: &ChanData) {
    d.state.store(0, std::sync::atomic::Ordering::Release);
}

/// Unlocked prompt send: queue `v` when no task waits on the channel (no
/// one to wake), it has room, and admission is by count alone. `false`
/// means the locked path must decide.
pub fn send_fast(ch: u64, v: u64) -> bool {
    let d = raw(ch);
    // a byte budget (fixed at creation) charges the message's size, which
    // depends only on the value: measured before taking the channel
    let size = if d.byte_budget > 0 { admission_size(d.elem, v) } else { 0 };
    if !try_fast(d) {
        return false;
    }
    let mut ok = false;
    let fits = d.byte_budget == 0 || (size <= d.byte_budget && d.bytes + size <= d.byte_budget);
    if !d.closed && !d.batched && fits && d.recv_watchers.is_empty() && d.send_waiters.is_empty() {
        // hand off to a parked receiver (the channel is held, so the
        // receiver cannot finish and leave it meanwhile)
        while let Some((_, wait, p)) = d.recv_waiters.pop_front() {
            if task::deliver(p, wait, v, d.elem.is_ptr(), 1) {
                ok = true;
                break;
            }
        }
        if !ok && d.capacity > 0 && d.queue.len() < d.capacity {
            d.queue.push_back(v);
            gc::barrier(ch);
            d.bytes += size;
            d.sizes.push_back(size);
            ok = true;
        }
    }
    end_fast(d);
    ok
}

/// Unlocked batch append: add `v` to the calling producer's existing
/// staging when it does not fill the batch (no publication, so no one to
/// wake) and admission is by count alone.
pub fn append_fast(ch: u64, v: u64) -> bool {
    let d = raw(ch);
    if !try_fast(d) {
        return false;
    }
    let me = task::current_id();
    let ok = d.batched
        && !d.closed
        && d.byte_budget == 0
        && d.queue.len() + d.accepted < d.capacity
        && d.staging.get(&me).is_some_and(|st| st.len() + 1 < d.max_batch);
    if ok {
        d.staging.get_mut(&me).expect("staging").push(v);
        gc::barrier(ch);
        d.accepted += 1;
    }
    end_fast(d);
    ok
}

/// Unlocked single-message batch receive: take the oldest published
/// message when no producer waits for capacity (no one to wake).
pub fn recv_one_fast(ch: u64) -> Option<u64> {
    let d = raw(ch);
    if !try_fast(d) {
        return None;
    }
    let mut v = None;
    if d.batched && d.byte_budget == 0 && d.cap_waiters.is_empty() {
        while d.batches.front().is_some_and(VecDeque::is_empty) {
            d.batches.pop_front();
        }
        if let Some(front) = d.batches.front_mut() {
            v = front.pop_front();
            if front.is_empty() {
                d.batches.pop_front();
            }
            d.accepted -= 1;
        }
    }
    end_fast(d);
    v
}

/// Unlocked prompt receive: take the oldest message when no task waits to
/// send or for space (no one to admit or wake). `None` means the locked
/// path must decide.
pub fn recv_fast(ch: u64) -> Option<u64> {
    let d = raw(ch);
    if !try_fast(d) {
        return None;
    }
    let mut v = None;
    if !d.batched && d.space_waiters.is_empty() {
        if let Some(x) = d.queue.pop_front() {
            if let Some(sz) = d.sizes.pop_front() {
                d.bytes -= sz;
            }
            v = Some(x);
            // a sender parked on the full queue moves into the freed slot
            admit_sender_fast(d);
        } else {
            // capacity zero or a drained queue: take a parked sender's value
            while let Some((_, x, _, wait, p)) = d.send_waiters.pop_front() {
                if task::deliver(p, wait, 0, false, 1) {
                    v = Some(x);
                    break;
                }
            }
        }
    }
    end_fast(d);
    v
}

/// What a waiting send or receive without the runtime lock did.
pub enum Free {
    /// Completed (a received message; 0 for a send).
    Done(u64),
    /// Not attempted: the locked path decides.
    Locked,
    /// Parked, and woken without completing (closing, cancellation): the
    /// locked path decides after [`withdraw`].
    Woken,
}

/// A prompt receive that waits, without the runtime lock: takes a message
/// as [`recv_fast`] does, or else registers and parks without the lock
/// ([`task::park_free`]) until a sender hands it one.
pub fn recv_free(ch: u64) -> Free {
    if !task::can_suspend_free() {
        return Free::Locked;
    }
    let d = raw(ch);
    if !try_fast(d) {
        return Free::Locked;
    }
    if d.batched || !d.space_waiters.is_empty() {
        end_fast(d);
        return Free::Locked;
    }
    if let Some(x) = d.queue.pop_front() {
        if let Some(sz) = d.sizes.pop_front() {
            d.bytes -= sz;
        }
        admit_sender_fast(d);
        end_fast(d);
        return Free::Done(x);
    }
    while let Some((_, x, _, wait, p)) = d.send_waiters.pop_front() {
        if task::deliver(p, wait, 0, false, 1) {
            end_fast(d);
            return Free::Done(x);
        }
    }
    if d.closed || !task::begin_park_free(1) {
        end_fast(d);
        return Free::Locked;
    }
    let _r = gc::Roots::new();
    _r.add(ch);
    d.recv_waiters.push_back((task::current_id(), task::begin_wait(), task::current_ptr()));
    let added = note_waiting(ch);
    end_fast(d);
    task::park_free();
    match task::take_delivered() {
        (v, 1) => {
            done_waiting(ch, added);
            Free::Done(v)
        }
        _ => Free::Woken,
    }
}

/// A prompt send that waits, without the runtime lock: hands `v` to a
/// parked receiver or queues it as [`send_fast`] does, or else registers
/// with it and parks without the lock until a receiver takes it.
pub fn send_free(ch: u64, v: u64) -> Free {
    if !task::can_suspend_free() {
        return Free::Locked;
    }
    let d = raw(ch);
    if d.byte_budget > 0 || !try_fast(d) {
        return Free::Locked;
    }
    if d.closed || d.batched || !d.recv_watchers.is_empty() {
        end_fast(d);
        return Free::Locked;
    }
    while let Some((_, wait, p)) = d.recv_waiters.pop_front() {
        if task::deliver(p, wait, v, d.elem.is_ptr(), 1) {
            end_fast(d);
            return Free::Done(0);
        }
    }
    if d.capacity > 0 && d.queue.len() < d.capacity && d.send_waiters.is_empty() {
        d.queue.push_back(v);
        gc::barrier(ch);
        d.sizes.push_back(0);
        end_fast(d);
        return Free::Done(0);
    }
    if !task::begin_park_free(2) {
        end_fast(d);
        return Free::Locked;
    }
    let _r = gc::Roots::new();
    _r.add(ch);
    if d.elem.is_ptr() {
        _r.add(v);
    }
    d.send_waiters.push_back((task::current_id(), v, 0, task::begin_wait(), task::current_ptr()));
    gc::barrier(ch);
    let added = note_waiting(ch);
    end_fast(d);
    task::park_free();
    match task::take_delivered() {
        (_, 1) => {
            done_waiting(ch, added);
            Free::Done(0)
        }
        _ => Free::Woken,
    }
}

/// Withdraw the running task's registrations with `ch` as a waiting sender
/// or receiver (runtime lock held), after a wait without the lock ended
/// without completing. Such waits are on prompt channels, where waiting
/// is the task's only registration (no leases or staging), so the task
/// also forgets the channel and its end does not search it again.
pub fn withdraw(ch: u64) {
    let me = task::current_id();
    let d = data(ch);
    debug_assert!(!d.batched, "waits without the lock are on prompt channels");
    remove_waiter(&mut d.recv_waiters, |w| w.0, me);
    remove_waiter(&mut d.send_waiters, |w| w.0, me);
    done_waiting(ch, true);
}

/// [`admit_sender`] for a fast path holding the channel.
fn admit_sender_fast(d: &mut ChanData) {
    while let Some((s, v, size, wait, p)) = d.send_waiters.pop_front() {
        let fits = d.byte_budget == 0 || d.bytes + size <= d.byte_budget;
        if d.capacity == 0 || !fits || d.queue.len() >= d.capacity {
            d.send_waiters.push_front((s, v, size, wait, p));
            return;
        }
        if task::deliver(p, wait, 0, false, 1) {
            d.queue.push_back(v);
            d.bytes += size;
            d.sizes.push_back(size);
            return;
        }
    }
}

#[allow(static_mut_refs)]
pub fn new(elem: &'static TypeDesc, capacity: usize, max_batch: usize, batched: bool) -> u64 {
    let w = gc::alloc(chan_desc(), 8);
    let d = Box::new(ChanData {
        seq: NEXT_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        state: std::sync::atomic::AtomicU8::new(0),
        elem,
        capacity,
        max_batch: max_batch.max(1),
        batched,
        closed: false,
        queue: VecDeque::new(),
        batches: VecDeque::new(),
        accepted: 0,
        staging: FxHashMap::default(),
        leased: 0,
        leases: vec![],
        recv_waiters: VecDeque::new(),
        send_waiters: VecDeque::new(),
        cap_waiters: VecDeque::new(),
        pool: vec![],
        published_batches: 0,
        space_waiters: VecDeque::new(),
        recv_watchers: VecDeque::new(),
        byte_budget: 0,
        bytes: 0,
        sizes: VecDeque::new(),
        discard_on_close: false,
        flush_latency: std::time::Duration::from_millis(1),
        staged_at: FxHashMap::default(),
    });
    unsafe { *(obj(w).payload() as *mut *mut ChanData) = Box::into_raw(d) };
    w
}

fn wake_one_receiver(d: &mut ChanData) {
    while let Some((r, wait, _)) = d.recv_waiters.pop_front() {
        if task::waits(r, wait) {
            task::wake(r, task::Wake::Normal);
            return;
        }
    }
}

fn wake_capacity(d: &mut ChanData) {
    for t in std::mem::take(&mut d.cap_waiters) {
        task::wake(t, task::Wake::Normal);
    }
}

pub enum SendResult {
    Sent,
    Closed,
    Full,
    /// A single message larger than the channel's byte budget.
    TooLarge,
}

/// Admission size of a message: direct byte storage is charged precisely,
/// other payloads by their structural size.
pub fn admission_size(d: &TypeDesc, w: u64) -> usize {
    match &d.kind {
        Kind::String | Kind::Bytes | Kind::Symbol => crate::string::bytes_of(w).len(),
        Kind::Record { fields, .. } => {
            let p = unsafe { obj(w).payload() };
            fields
                .iter()
                .map(|f| {
                    if f.desc.is_ptr() {
                        admission_size(f.desc, unsafe { f.repr.load(p.add(f.offset as usize)) })
                    } else {
                        f.repr.size() as usize
                    }
                })
                .sum()
        }
        Kind::List { elem } => (0..crate::list::len(w))
            .map(|i| {
                if elem.is_ptr() {
                    admission_size(elem, crate::list::get(w, i))
                } else {
                    elem.repr().size() as usize
                }
            })
            .sum(),
        // an enum (`Option`, `Result`, user enums): its tag and the chosen
        // variant's fields
        Kind::Enum { .. } => {
            let (tag, fields) = crate::enum_parts(w);
            let v = d.variant(tag);
            4 + v
                .map(|v| {
                    v.fields
                        .iter()
                        .zip(fields)
                        .map(
                            |(f, x)| {
                                if f.desc.is_ptr() {
                                    admission_size(f.desc, x)
                                } else {
                                    f.repr.size() as usize
                                }
                            },
                        )
                        .sum::<usize>()
                })
                .unwrap_or(0)
        }
        // maps and sets: their keys and values
        Kind::Map { key, val } => crate::map::entries(w)
            .into_iter()
            .map(|(k, v)| {
                let ks = if key.is_ptr() { admission_size(key, k) } else { key.repr().size() as usize };
                let vs = if val.is_ptr() { admission_size(val, v) } else { val.repr().size() as usize };
                ks + vs
            })
            .sum(),
        Kind::Set { elem } => crate::map::entries(w)
            .into_iter()
            .map(|(k, _)| if elem.is_ptr() { admission_size(elem, k) } else { elem.repr().size() as usize })
            .sum(),
        // a buffer's (or array's) elements
        _ if d.is_ptr() && w != 0 && crate::buffer::is_buffer(w) => {
            let b = crate::buffer::data(w);
            let elem = b.elem;
            b.items
                .clone()
                .into_iter()
                .map(|x| if elem.is_ptr() { admission_size(elem, x) } else { elem.repr().size() as usize })
                .sum()
        }
        _ => d.repr().size() as usize,
    }
}

fn wake_space(d: &mut ChanData) {
    for t in std::mem::take(&mut d.space_waiters) {
        task::wake(t, task::Wake::Normal);
    }
}

/// Prompt send. Waits for capacity (or a receiver when capacity is zero).
pub fn send(ch: u64, v: u64, wait: bool) -> SendResult {
    let me = task::current_id();
    let _r = gc::Roots::new();
    _r.add(ch);
    if data(ch).elem.is_ptr() {
        _r.add(v);
    }
    let size = if data(ch).byte_budget > 0 { admission_size(data(ch).elem, v) } else { 0 };
    loop {
        let d = data(ch);
        if d.closed {
            return SendResult::Closed;
        }
        if d.byte_budget > 0 && size > d.byte_budget {
            return SendResult::TooLarge;
        }
        let fits = d.byte_budget == 0 || d.bytes + size <= d.byte_budget;
        // hand off directly to a waiting receiver
        if fits {
            while let Some((r, wait, _)) = d.recv_waiters.pop_front() {
                if task::waits(r, wait) {
                    task::wake_with(r, v, d.elem.is_ptr(), 1);
                    return SendResult::Sent;
                }
            }
            // a rendezvous with a selection parked on a receive of this channel
            if d.capacity == 0 && commit_select_receive(ch, v) {
                return SendResult::Sent;
            }
        }
        if d.queue.len() < d.capacity && fits {
            d.queue.push_back(v);
            gc::barrier(ch);
            d.bytes += size;
            d.sizes.push_back(size);
            for t in std::mem::take(&mut d.recv_watchers) {
                task::wake(t, task::Wake::Normal);
            }
            return SendResult::Sent;
        }
        if !wait && !fits {
            return SendResult::Full;
        }
        if !wait {
            return SendResult::Full;
        }
        d.send_waiters.push_back((me, v, size, task::begin_wait(), task::current_ptr()));
        gc::barrier(ch);
        let added = note_waiting(ch);
        task::park_for("channel send");
        let (_, status) = task::take_delivered();
        let d = data(ch);
        if status == 1 {
            done_waiting(ch, added);
            return SendResult::Sent;
        }
        // not taken: withdraw our reservation (the value is still ours)
        remove_waiter(&mut d.send_waiters, |w| w.0, me);
        done_waiting(ch, added);
        if status == 2 || d.closed {
            return SendResult::Closed;
        }
    }
}

/// A parked selection that offers to receive from `ch` takes `v` (that
/// case commits on its behalf).
fn commit_select_receive(ch: u64, v: u64) -> bool {
    let me = task::current_id();
    let watchers: Vec<TaskId> = data(ch).recv_watchers.iter().copied().collect();
    for t in watchers {
        let Some(tk) = task::task(t) else { continue };
        if t == me || !task::is_parked(t) || tk.select_done.is_some() {
            continue;
        }
        let Some(&(_, i, _, _)) = tk.select_offer.iter().find(|c| c.0 == ch && !c.2) else { continue };
        tk.select_done = Some(i);
        task::wake_with(t, v, data(ch).elem.is_ptr(), 1);
        return true;
    }
    false
}

/// A parked selection that offers to send on `ch` gives its value (that
/// case commits on its behalf).
fn commit_select_send(ch: u64) -> Option<u64> {
    let me = task::current_id();
    let waiters: Vec<TaskId> = data(ch).space_waiters.iter().copied().collect();
    for t in waiters {
        let Some(tk) = task::task(t) else { continue };
        if t == me || !task::is_parked(t) || tk.select_done.is_some() {
            continue;
        }
        let Some(&(_, i, _, v)) = tk.select_offer.iter().find(|c| c.0 == ch && c.2) else { continue };
        tk.select_done = Some(i);
        task::wake_with(t, 0, false, 1);
        return Some(v);
    }
    None
}

pub enum RecvResult {
    Value(u64),
    Closed,
    Empty,
}

/// Admit one waiting sender's value after space opens, charging its bytes
/// (a sender whose value does not fit the budget yet keeps waiting, first
/// in line).
fn admit_sender(d: &mut ChanData) {
    while let Some((s, v, size, wait, p)) = d.send_waiters.pop_front() {
        if task::waits(s, wait) {
            let fits = d.byte_budget == 0 || d.bytes + size <= d.byte_budget;
            if d.capacity == 0 || !fits || d.queue.len() >= d.capacity {
                // rendezvous: the receiver takes it directly (handled by caller)
                d.send_waiters.push_front((s, v, size, wait, p));
                return;
            }
            d.queue.push_back(v);
            d.bytes += size;
            d.sizes.push_back(size);
            task::wake_with(s, 0, false, 1);
            return;
        }
    }
}

pub fn recv(ch: u64, wait: bool) -> RecvResult {
    let _r = gc::Roots::new();
    _r.add(ch);
    loop {
        let d = data(ch);
        if let Some(v) = d.queue.pop_front() {
            if let Some(sz) = d.sizes.pop_front() {
                d.bytes -= sz;
            }
            admit_sender(d);
            wake_space(d);
            return RecvResult::Value(v);
        }
        // rendezvous / waiting sender with capacity zero
        while let Some((s, v, _, wait, _)) = d.send_waiters.pop_front() {
            if task::waits(s, wait) {
                task::wake_with(s, 0, false, 1);
                return RecvResult::Value(v);
            }
        }
        // a rendezvous with a selection parked on a send to this channel
        if d.capacity == 0
            && let Some(v) = commit_select_send(ch)
        {
            return RecvResult::Value(v);
        }
        if d.closed {
            return RecvResult::Closed;
        }
        if !wait {
            return RecvResult::Empty;
        }
        let me = task::current_id();
        d.recv_waiters.push_back((me, task::begin_wait(), task::current_ptr()));
        let added = note_waiting(ch);
        task::park_for("channel receive");
        let (v, status) = task::take_delivered();
        if status == 1 {
            done_waiting(ch, added);
            return RecvResult::Value(v);
        }
        remove_waiter(&mut data(ch).recv_waiters, |w| w.0, me);
        done_waiting(ch, added);
    }
}

pub fn close(ch: u64) {
    let d = data(ch);
    if d.batched {
        if d.discard_on_close {
            // the configured disposal: staged data is released, not published
            let staged: Vec<Vec<u64>> = d.staging.drain().map(|(_, v)| v).collect();
            d.accepted -= staged.iter().map(Vec::len).sum::<usize>();
            for items in staged {
                uncharge(ch, &items);
            }
            data(ch).staged_at.clear();
        } else {
            // staged messages already belong to the runtime: publish them all
            let producers: Vec<TaskId> = d.staging.keys().copied().collect();
            for p in producers {
                publish(ch, p);
            }
        }
    }
    let d = data(ch);
    d.closed = true;
    for (r, wait, _) in std::mem::take(&mut d.recv_waiters) {
        if task::waits(r, wait) {
            task::wake(r, task::Wake::Normal);
        }
    }
    for (s, _, _, wait, _) in std::mem::take(&mut d.send_waiters) {
        if task::waits(s, wait) {
            task::wake_with(s, 0, false, 2);
        }
    }
    wake_capacity(d);
    wake_space(d);
    for t in std::mem::take(&mut d.recv_watchers) {
        task::wake(t, task::Wake::Normal);
    }
}

/// Unlocked close of a prompt channel no selection watches: parked
/// receivers wake to find it closed, parked senders get their values back
/// as closed. `false`: the locked path must close it.
pub fn close_fast(ch: u64) -> bool {
    let d = raw(ch);
    if !try_fast(d) {
        return false;
    }
    let ok =
        !d.batched && d.recv_watchers.is_empty() && d.space_waiters.is_empty() && d.cap_waiters.is_empty();
    if ok {
        d.closed = true;
        for (_, wait, p) in std::mem::take(&mut d.recv_waiters) {
            task::deliver(p, wait, 0, false, 0);
        }
        for (_, _, _, wait, p) in std::mem::take(&mut d.send_waiters) {
            task::deliver(p, wait, 0, false, 2);
        }
    }
    end_fast(d);
    ok
}

/// Close from the consumer side: no further messages are accepted, and
/// every pending message (queued, staged, published) is released once.
/// Waiting senders get their values back with a closed error.
pub fn close_receiver(ch: u64) {
    let d = data(ch);
    let queued: Vec<u64> = d.queue.drain(..).collect();
    let mut batches: Vec<Vec<u64>> = d.batches.drain(..).map(Vec::from).collect();
    batches.extend(d.staging.drain().map(|(_, v)| v));
    d.accepted -= batches.iter().map(Vec::len).sum::<usize>();
    d.staged_at.clear();
    d.sizes.clear();
    d.bytes = 0;
    drop(queued);
    drop(batches);
    close(ch);
}

/// Remove the byte charge of released or discarded messages.
fn uncharge(ch: u64, items: &[u64]) {
    let d = data(ch);
    if d.byte_budget == 0 {
        return;
    }
    let total: usize = items.iter().map(|w| admission_size(d.elem, *w)).sum();
    d.bytes = d.bytes.saturating_sub(total);
}

/// Safe-point hook: publish staging that is older than the flush latency.
pub fn flush_due(producer: TaskId, chans: &[u64]) {
    let now = std::time::Instant::now();
    for &ch in chans {
        let d = data(ch);
        if let Some(at) = d.staged_at.get(&producer)
            && now.duration_since(*at) >= d.flush_latency
        {
            publish(ch, producer);
        }
    }
}

/// Register the current task to be woken when `ch` may have a message.
pub fn watch_recv(ch: u64) {
    data(ch).recv_watchers.push_back(task::current_id());
    let _ = note_waiting(ch);
}

/// Register the current task to be woken when `ch` may have space.
pub fn watch_space(ch: u64) {
    data(ch).space_waiters.push_back(task::current_id());
    let _ = note_waiting(ch);
}

pub fn unwatch(ch: u64) {
    let me = task::current_id();
    let d = data(ch);
    d.recv_watchers.retain(|t| *t != me);
    d.space_waiters.retain(|t| *t != me);
}

// ------------------------------------------------------------------ batches

/// Publish a producer's staged messages as one sealed batch.
pub fn publish(ch: u64, producer: TaskId) {
    let d = data(ch);
    d.staged_at.remove(&producer);
    let Some(staged) = d.staging.remove(&producer) else { return };
    if staged.is_empty() {
        return;
    }
    d.batches.push_back(VecDeque::from(staged));
    d.published_batches += 1;
    wake_one_receiver(d);
    // selections waiting on this channel
    for t in std::mem::take(&mut d.recv_watchers) {
        task::wake(t, task::Wake::Normal);
    }
}

/// Scheduler hook: a task scheduled out; publish its staged batches.
pub fn deschedule_hook(producer: TaskId, chans: &[u64]) {
    for &ch in chans {
        publish(ch, producer);
    }
    if let Some(t) = task::task(producer) {
        t.on_deschedule.clear();
    }
}

pub fn install_hook() {
    task::rt().deschedule_hook = Some(deschedule_hook);
    task::rt().safepoint_hook = Some(flush_due);
}

/// Accept a message into the producer's staging, waiting for capacity
/// (`wait`) or reporting `Full` (`try_append`).
pub fn append(ch: u64, v: u64, wait: bool) -> SendResult {
    install_hook();
    let me = task::current_id();
    let _r = gc::Roots::new();
    _r.add(ch);
    if data(ch).elem.is_ptr() {
        _r.add(v);
    }
    let size = if data(ch).byte_budget > 0 { admission_size(data(ch).elem, v) } else { 0 };
    loop {
        let d = data(ch);
        if d.closed {
            return SendResult::Closed;
        }
        // staged, published, and leased data all count against admission
        if d.byte_budget > 0 && size > d.byte_budget {
            return SendResult::TooLarge;
        }
        let fits = d.byte_budget == 0 || d.bytes + size <= d.byte_budget;
        if d.count() < d.capacity && fits {
            d.bytes += size;
            let page = d.pool.pop().unwrap_or_default();
            d.staged_at.entry(me).or_insert_with(std::time::Instant::now);
            let st = d.staging.entry(me).or_insert(page);
            st.push(v);
            gc::barrier(ch);
            d.accepted += 1;
            let full = st.len() >= d.max_batch;
            if let Some(t) = task::current()
                && !t.on_deschedule.contains(&ch)
            {
                t.on_deschedule.push(ch);
            }
            if full {
                publish(ch, me);
            }
            return SendResult::Sent;
        }
        // flush our own staging before waiting, so the receiver sees work
        publish(ch, me);
        if !wait {
            return SendResult::Full;
        }
        let d = data(ch);
        d.cap_waiters.push_back(me);
        let _ = note_waiting(ch);
        task::park_for("batch capacity");
    }
}

pub fn flush(ch: u64) {
    publish(ch, task::current_id());
}

/// Receive a leased sealed batch, or `None` after close and drain.
pub fn recv_batch(ch: u64) -> Option<u64> {
    let _r = gc::Roots::new();
    _r.add(ch);
    loop {
        let d = data(ch);
        if !d.batches.is_empty() {
            // allocate the lease object first: the allocation may collect,
            // and the messages must still be reachable from the channel then
            let b = gc::alloc(batch_desc(), 16);
            _r.add(b);
            let d = data(ch);
            // another receiver may have taken it while this one allocated
            let Some(items) = d.batches.pop_front() else { continue };
            let items: Vec<u64> = items.into();
            d.leased += items.len();
            unsafe {
                let p = obj(b).payload();
                *(p as *mut *mut Batch) = Box::into_raw(Box::new(Batch { items, released: false }));
                *(p.add(8) as *mut u64) = ch;
            }
            data(ch).leases.push((task::current_id(), b));
            gc::barrier(ch);
            let _ = note_waiting(ch);
            return Some(b);
        }
        if d.closed && d.staging.values().all(Vec::is_empty) {
            return None;
        }
        let me = task::current_id();
        d.recv_waiters.push_back((me, task::begin_wait(), task::current_ptr()));
        let _ = note_waiting(ch);
        task::park_for("batch receive");
        remove_waiter(&mut data(ch).recv_waiters, |w| w.0, me);
    }
}

/// Receive one published message of a batch channel (the per-message
/// endpoint operation): `Closed` after close and drain, `Empty` when
/// nothing is published and `wait` is false.
pub fn recv_one(ch: u64, wait: bool) -> RecvResult {
    let _r = gc::Roots::new();
    _r.add(ch);
    loop {
        let d = data(ch);
        while d.batches.front().is_some_and(VecDeque::is_empty) {
            d.batches.pop_front();
        }
        if let Some(front) = d.batches.front_mut() {
            let v = front.pop_front().expect("nonempty batch");
            if front.is_empty() {
                d.batches.pop_front();
            }
            d.accepted -= 1;
            uncharge(ch, &[v]);
            wake_capacity(data(ch));
            return RecvResult::Value(v);
        }
        if d.closed && d.staging.values().all(Vec::is_empty) {
            return RecvResult::Closed;
        }
        if !wait {
            return RecvResult::Empty;
        }
        let me = task::current_id();
        d.recv_waiters.push_back((me, task::begin_wait(), task::current_ptr()));
        let _ = note_waiting(ch);
        task::park_for("batch receive");
        remove_waiter(&mut data(ch).recv_waiters, |w| w.0, me);
    }
}

/// A channel's creation number (inspection, deadlock reports).
pub fn seq_of(ch: u64) -> Option<u64> {
    let p = unsafe { *(obj(ch).payload() as *const *mut ChanData) };
    (!p.is_null()).then(|| unsafe { (*p).seq })
}

/// Remember that the current task is registered with `ch` (a waiter,
/// watcher, or lease holder), so its end withdraws the registration.
fn note_waiting(ch: u64) -> bool {
    if let Some(t) = task::current()
        && !t.chan_regs.contains(&ch)
    {
        t.chan_regs.push(ch);
        return true;
    }
    false
}

/// A wait that [`note_waiting`] recorded (`added`) ended with its
/// registration gone (taken by the waker, or withdrawn): forget the
/// channel again, so the task's end does not search it.
fn done_waiting(ch: u64, added: bool) {
    if added
        && let Some(t) = task::current()
        && let Some(i) = t.chan_regs.iter().rposition(|c| *c == ch)
    {
        t.chan_regs.swap_remove(i);
    }
}

/// Remove task `id`'s entry from a list of parked senders or receivers (a
/// task waits at most once in each). Searched from both ends: waiters
/// that leave in or against the order they arrived, as when a scope
/// cancels many tasks blocked on one channel, cost O(1) each instead of a
/// scan of every waiter (which made cancelling n of them O(n²)).
fn remove_waiter<T>(q: &mut VecDeque<T>, task_of: impl Fn(&T) -> TaskId, id: TaskId) {
    let n = q.len();
    let found = (0..n.div_ceil(2)).find_map(|k| {
        if task_of(&q[k]) == id {
            Some(k)
        } else if task_of(&q[n - 1 - k]) == id {
            Some(n - 1 - k)
        } else {
            None
        }
    });
    if let Some(i) = found {
        q.remove(i);
    }
    debug_assert!(!q.iter().any(|w| task_of(w) == id), "a task waits at most once in a waiter list");
}

/// A finished task leaves the channels it was registered with: a waiter
/// abandoned by cancellation is withdrawn (with the value it offered), and
/// its unreleased leases return their capacity (a lease is released by
/// the cleanup of the task that received it).
pub fn task_finished(id: TaskId) {
    let Some(t) = task::task(id) else { return };
    for ch in std::mem::take(&mut t.chan_regs) {
        let d = data(ch);
        remove_waiter(&mut d.send_waiters, |w| w.0, id);
        remove_waiter(&mut d.recv_waiters, |w| w.0, id);
        d.cap_waiters.retain(|t| *t != id);
        d.space_waiters.retain(|t| *t != id);
        d.recv_watchers.retain(|t| *t != id);
        let held: Vec<u64> = d.leases.iter().filter(|(t, _)| *t == id).map(|(_, b)| *b).collect();
        for b in held {
            release(b);
        }
    }
}

pub fn batch<'a>(b: u64) -> &'a mut Batch {
    unsafe { &mut **(obj(b).payload() as *mut *mut Batch) }
}

fn batch_chan(b: u64) -> u64 {
    unsafe { *(obj(b).payload().add(8) as *const u64) }
}

/// Release a lease: its messages stop counting against capacity and the
/// page buffer returns to the bounded pool.
pub fn release(b: u64) {
    let bt = batch(b);
    if bt.released {
        return;
    }
    bt.released = true;
    let mut items = std::mem::take(&mut bt.items);
    uncharge(batch_chan(b), &items);
    let d = data(batch_chan(b));
    d.leases.retain(|(_, x)| *x != b);
    d.leased -= items.len();
    d.accepted -= items.len();
    items.clear();
    if d.pool.len() < 4 {
        d.pool.push(items);
    }
    wake_capacity(d);
}
