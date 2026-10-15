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

pub struct ChanData {
    pub elem: &'static TypeDesc,
    pub capacity: usize,
    pub max_batch: usize,
    pub batched: bool,
    pub closed: bool,
    /// Prompt channel: queued messages. Batch channel: unused.
    pub queue: VecDeque<u64>,
    /// Batch channel: sealed batches awaiting a receiver.
    pub batches: VecDeque<Vec<u64>>,
    /// Batch channel: per-producer staging (accepted, not yet published).
    pub staging: FxHashMap<TaskId, Vec<u64>>,
    /// Messages in leased (received, unreleased) batches.
    pub leased: usize,
    /// Unreleased leases and the tasks holding them (released when the
    /// holder finishes; traced so a lease outlives its last reference).
    pub leases: Vec<(TaskId, u64)>,
    pub recv_waiters: VecDeque<TaskId>,
    /// Waiting senders with their (still owned) values.
    pub send_waiters: VecDeque<(TaskId, u64)>,
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
        self.queue.len()
            + self.batches.iter().map(Vec::len).sum::<usize>()
            + self.staging.values().map(Vec::len).sum::<usize>()
            + self.leased
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
    d.send_waiters.iter().for_each(|(_, w)| f(*w));
}

/// Live channels for inspection: (sequence number, object address).
static mut REGISTRY: Vec<(u64, u64)> = Vec::new();
static mut NEXT_SEQ: u64 = 1;

/// Inspection snapshot of every live channel (JSON array).
#[allow(static_mut_refs)]
pub fn snapshot_json() -> String {
    let items: Vec<String> = unsafe { REGISTRY.iter() }
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
        REGISTRY.retain(|&(_, w)| w != o as u64);
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

pub fn data<'a>(ch: u64) -> &'a mut ChanData {
    unsafe { &mut **(obj(ch).payload() as *mut *mut ChanData) }
}

#[allow(static_mut_refs)]
pub fn new(elem: &'static TypeDesc, capacity: usize, max_batch: usize, batched: bool) -> u64 {
    let w = gc::alloc(chan_desc(), 8);
    unsafe {
        REGISTRY.push((NEXT_SEQ, w));
        NEXT_SEQ += 1;
    }
    let d = Box::new(ChanData {
        elem,
        capacity,
        max_batch: max_batch.max(1),
        batched,
        closed: false,
        queue: VecDeque::new(),
        batches: VecDeque::new(),
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

fn live(t: TaskId) -> bool {
    task::task(t).is_some_and(|t| t.ctx.flags & 1 == 0) && !task::is_done(t)
}

fn wake_one_receiver(d: &mut ChanData) {
    while let Some(r) = d.recv_waiters.pop_front() {
        if live(r) {
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
            while let Some(r) = d.recv_waiters.pop_front() {
                if live(r) {
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
        d.send_waiters.push_back((me, v));
        task::park_for("channel send");
        let (_, status) = task::take_delivered();
        let d = data(ch);
        if status == 1 {
            return SendResult::Sent;
        }
        // not taken: withdraw our reservation (the value is still ours)
        d.send_waiters.retain(|(t, _)| *t != me);
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

/// Admit one waiting sender's value after space opens.
fn admit_sender(d: &mut ChanData) {
    while let Some((s, v)) = d.send_waiters.pop_front() {
        if live(s) {
            if d.capacity == 0 {
                // rendezvous: the receiver takes it directly (handled by caller)
                d.send_waiters.push_front((s, v));
                return;
            }
            d.queue.push_back(v);
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
        while let Some((s, v)) = d.send_waiters.pop_front() {
            if live(s) {
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
        d.recv_waiters.push_back(me);
        task::park_for("channel receive");
        let (v, status) = task::take_delivered();
        if status == 1 {
            return RecvResult::Value(v);
        }
        data(ch).recv_waiters.retain(|t| *t != me);
    }
}

pub fn close(ch: u64) {
    let d = data(ch);
    if d.batched {
        if d.discard_on_close {
            // the configured disposal: staged data is released, not published
            let staged: Vec<Vec<u64>> = d.staging.drain().map(|(_, v)| v).collect();
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
    for r in std::mem::take(&mut d.recv_waiters) {
        task::wake(r, task::Wake::Normal);
    }
    for (s, _) in std::mem::take(&mut d.send_waiters) {
        task::wake_with(s, 0, false, 2);
    }
    wake_capacity(d);
    wake_space(d);
    for t in std::mem::take(&mut d.recv_watchers) {
        task::wake(t, task::Wake::Normal);
    }
}

/// Close from the consumer side: no further messages are accepted, and
/// every pending message (queued, staged, published) is released once.
/// Waiting senders get their values back with a closed error.
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
}

/// Register the current task to be woken when `ch` may have space.
pub fn watch_space(ch: u64) {
    data(ch).space_waiters.push_back(task::current_id());
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
    d.batches.push_back(staged);
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
            d.leased += items.len();
            unsafe {
                let p = obj(b).payload();
                *(p as *mut *mut Batch) = Box::into_raw(Box::new(Batch { items, released: false }));
                *(p.add(8) as *mut u64) = ch;
            }
            data(ch).leases.push((task::current_id(), b));
            return Some(b);
        }
        if d.closed && d.staging.values().all(Vec::is_empty) {
            return None;
        }
        let me = task::current_id();
        d.recv_waiters.push_back(me);
        task::park_for("batch receive");
        data(ch).recv_waiters.retain(|t| *t != me);
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
        while d.batches.front().is_some_and(Vec::is_empty) {
            d.batches.pop_front();
        }
        if let Some(front) = d.batches.front_mut() {
            let v = front.remove(0);
            if front.is_empty() {
                d.batches.pop_front();
            }
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
        d.recv_waiters.push_back(me);
        task::park_for("batch receive");
        data(ch).recv_waiters.retain(|t| *t != me);
    }
}

/// A finished task's unreleased leases return their capacity (a lease is
/// released by the cleanup of the task that received it).
#[allow(static_mut_refs)]
pub fn release_leases_of(id: TaskId) {
    let chans: Vec<u64> = unsafe { REGISTRY.iter().map(|&(_, w)| w).collect() };
    for ch in chans {
        let held: Vec<u64> = data(ch).leases.iter().filter(|(t, _)| *t == id).map(|(_, b)| *b).collect();
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
    items.clear();
    if d.pool.len() < 4 {
        d.pool.push(items);
    }
    wake_capacity(d);
}
