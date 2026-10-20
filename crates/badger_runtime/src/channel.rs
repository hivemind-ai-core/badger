//! Bounded channels and batch channels.
//!
//! A prompt `Channel<T>` publishes each send immediately (capacity zero is a
//! rendezvous). A `BatchChannel<T>` accepts appends into per-producer staging
//! owned by the runtime and publishes sealed batches when a batch fills, on
//! flush/close, and whenever the producing task schedules out (parks, yields,
//! or finishes) — so staged messages are never stranded. Admission counts
//! staged, queued, and leased messages against capacity; a producer flushes
//! its own staging before waiting for capacity.
//!
//! # The state machine
//!
//! A channel is in one phase at a time: `EMPTY` (nothing to take, nobody
//! registered), `VALUE` (messages to take, nobody registered) or `BLOCKED`
//! (a task is registered: a parked receiver or sender, a selection, a
//! producer waiting for capacity), and it may be closed. The phase, the
//! closed bit and the channel's owner share one atomic word
//! ([`ChanData::state`]); every task registered with the channel is in one
//! list ([`Waiters`]), in arrival order.
//!
//! Each operation linearizes at the one atomic write that makes it the
//! channel's owner: the compare-and-swap in [`try_fast`] for an unlocked
//! fast path, the one in [`claim`] for code holding the runtime lock (with
//! one worker, the single runtime thread owns every channel between task
//! switches). Everything it decides, it decides as owner from the
//! channel's contents, and the store that ends the ownership publishes the
//! new phase. A fast path and a slow path therefore never both act on the
//! same event: whichever takes the channel first sees the other's effects
//! completely or not at all.
//!
//! The contract the operations keep (`spec/tla/ChannelBadger.tla`):
//!
//! - a value reaches a receiver: a send hands it to the first live parked
//!   receiver, else commits it to a parked selection offering to receive,
//!   else queues it; a queued value is taken by the next receive, and a
//!   selection that is registered but not parked probes again before it
//!   parks;
//! - a registration is not forgotten: only its task removes it (when its
//!   wait ends, or when it finishes), except that the operation serving an
//!   entry removes it (a receiver or sender handed its value, a parked
//!   receiver or producer woken to retry, which registers again if it must
//!   wait), and closing (the last transition) wakes and removes every
//!   entry; a selection's entries are never served this way;
//! - a handed-over value stays handed over: a cancellation arriving after
//!   the hand-off is honored after the operation completes.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

use rustc_hash::FxHashMap;

use crate::desc::{Kind, TypeDesc};
use crate::gc::{self, Header, obj};
use crate::task::{self, TaskId};

pub struct Batch {
    pub items: Vec<u64>,
    pub released: bool,
}

impl Drop for Batch {
    fn drop(&mut self) {
        // An unreleased lease still owns its messages' buffer; a released
        // one moved them to the pool (`mem::take` left this empty).
        release_buf::<u64>(self.items.capacity());
    }
}

/// The bits of [`ChanData::state`].
pub mod word {
    /// Owner: nobody, an unlocked fast path ([`super::try_fast`]), or code
    /// holding the runtime lock ([`super::claim`]) until it releases it.
    pub const FREE: u8 = 0;
    pub const FAST: u8 = 1;
    pub const CLAIMED: u8 = 2;
    pub const OWNER: u8 = 0b11;
    /// Phase: nothing to take and nobody registered; messages to take and
    /// nobody registered; a task registered (see [`super::Waiters`]).
    pub const EMPTY: u8 = 0;
    pub const VALUE: u8 = 1 << 2;
    pub const BLOCKED: u8 = 2 << 2;
    pub const PHASE: u8 = 0b11 << 2;
    pub const CLOSED: u8 = 1 << 4;
    /// One worker: code that needs no claim has used the channel since the
    /// phase was published (cleared by the next fast path's release).
    pub const STALE: u8 = 1 << 5;
}

/// What a registered task waits for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WaitKind {
    /// A parked receiver: a sender hands it the message.
    Recv = 0,
    /// A parked sender with its (still owned) value: a receiver takes it.
    Send = 1,
    /// A selection offering to receive: a send commits to it while it is
    /// parked ([`commit_select_receive`]); otherwise it probes again.
    WatchRecv = 2,
    /// A selection offering to send: woken when space may have opened.
    WatchSpace = 3,
    /// A batch producer waiting for capacity.
    Capacity = 4,
}

/// A task registered with a channel.
#[derive(Clone, Copy, Debug)]
pub struct Waiter {
    pub task: TaskId,
    pub kind: WaitKind,
    /// The wait it registered for (`task::begin_wait`): it is served only
    /// while still in that wait (`task::waits`, `task::deliver`).
    pub wait: u64,
    /// Its record (`task::current_ptr`), for a fast path's `task::deliver`.
    pub rec: usize,
    /// A sender's value and its byte charge.
    pub value: u64,
    pub size: usize,
}

/// Every task registered with a channel, in arrival order, with a count
/// per kind. An operation serves the first entry of the kind it completes,
/// so each kind keeps its arrival order; a task has at most one entry of
/// each kind.
#[derive(Default)]
pub struct Waiters {
    list: VecDeque<Waiter>,
    counts: [usize; 5],
}

impl Waiters {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    #[inline]
    pub fn count(&self, k: WaitKind) -> usize {
        self.counts[k as usize]
    }

    pub fn iter(&self) -> impl Iterator<Item = &Waiter> {
        self.list.iter()
    }

    fn push(&mut self, w: Waiter) {
        debug_assert!(
            !self.list.iter().any(|x| x.task == w.task && x.kind == w.kind),
            "a task waits at most once of each kind"
        );
        self.counts[w.kind as usize] += 1;
        self.list.push_back(w);
    }

    /// Put back an entry [`Waiters::pop`] took, ahead of its kind.
    fn push_front(&mut self, w: Waiter) {
        self.counts[w.kind as usize] += 1;
        self.list.push_front(w);
    }

    /// Remove and return the first entry of kind `k`.
    #[inline]
    fn pop(&mut self, k: WaitKind) -> Option<Waiter> {
        if self.counts[k as usize] == 0 {
            return None;
        }
        self.counts[k as usize] -= 1;
        if self.list.front().is_some_and(|w| w.kind == k) {
            return self.list.pop_front();
        }
        let i = self.list.iter().position(|w| w.kind == k).expect("counted entry");
        self.list.remove(i)
    }

    /// Remove task `id`'s entry of kind `k`. Searched from both ends:
    /// waiters that leave in or against the order they arrived, as when a
    /// scope cancels many tasks blocked on one channel, cost O(1) each
    /// instead of a scan of every waiter (which made cancelling n of them
    /// O(n²)).
    fn remove(&mut self, id: TaskId, k: WaitKind) {
        if self.counts[k as usize] == 0 {
            return;
        }
        let q = &self.list;
        let n = q.len();
        let is = |w: &Waiter| w.task == id && w.kind == k;
        let found = (0..n.div_ceil(2)).find_map(|j| {
            if is(&q[j]) {
                Some(j)
            } else if is(&q[n - 1 - j]) {
                Some(n - 1 - j)
            } else {
                None
            }
        });
        if let Some(i) = found {
            self.list.remove(i);
            self.counts[k as usize] -= 1;
        }
    }

    /// Remove every entry of task `id`.
    fn remove_task(&mut self, id: TaskId) {
        for k in [WaitKind::Recv, WaitKind::Send, WaitKind::WatchRecv, WaitKind::WatchSpace, WaitKind::Capacity] {
            self.remove(id, k);
        }
    }

    /// Remove every entry (closing the channel).
    fn take_all(&mut self) -> VecDeque<Waiter> {
        self.counts = [0; 5];
        std::mem::take(&mut self.list)
    }
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
    /// Owner, phase and closed bits ([`word`]). Whenever the owner is
    /// `FREE` (and, with one worker, `STALE` is clear) the phase and closed
    /// bits are [`ChanData::word`] of the contents: every owner publishes
    /// them as it lets go.
    pub state: AtomicU8,
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
    /// Every task registered with the channel.
    pub waiters: Waiters,
    /// Recycled page buffers (bounded reserve).
    pub pool: Vec<Vec<u64>>,
    pub published_batches: u64,
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

    /// The phase and closed bits of the contents.
    fn word(&self) -> u8 {
        let phase = if !self.waiters.is_empty() {
            word::BLOCKED
        } else if !self.queue.is_empty() || !self.batches.is_empty() {
            word::VALUE
        } else {
            word::EMPTY
        };
        phase | if self.closed { word::CLOSED } else { 0 }
    }

    /// Register the running task.
    fn register(&mut self, kind: WaitKind, value: u64, size: usize) {
        self.waiters.push(Waiter {
            task: task::current_id(),
            kind,
            wait: task::begin_wait(),
            rec: task::current_ptr(),
            value,
            size,
        });
    }
}

/// Native bytes a buffer of `cap` elements occupies.
fn buf_bytes<T>(cap: usize) -> usize {
    cap * std::mem::size_of::<T>()
}

/// Charge the storage a `T` buffer gains when its capacity grows from `was`
/// to `now`. Only the growth is charged; a buffer keeps its charge until
/// [`release_buf`] releases its whole capacity as the native storage is
/// freed.
fn charge_growth<T>(was: usize, now: usize) {
    if now > was {
        gc::external_alloc(buf_bytes::<T>(now - was));
    }
}

/// Charge a `T` buffer's whole capacity, taken over from a container whose
/// storage was uncharged (a conversion allocates a fresh buffer).
fn charge_buffer<T>(cap: usize) {
    if cap > 0 {
        gc::external_alloc(buf_bytes::<T>(cap));
    }
}

/// Release a `T` buffer's whole capacity from the external counters, as its
/// native storage is freed.
fn release_buf<T>(cap: usize) {
    if cap > 0 {
        gc::external_free(buf_bytes::<T>(cap));
    }
}

/// Push onto a charged channel `Vec`, reporting any capacity growth.
fn push_charged<T>(v: &mut Vec<T>, x: T) {
    let was = v.capacity();
    v.push(x);
    charge_growth::<T>(was, v.capacity());
}

/// `VecDeque::push_back` onto a charged channel buffer, reporting any
/// capacity growth.
fn push_back_charged<T>(q: &mut VecDeque<T>, x: T) {
    let was = q.capacity();
    q.push_back(x);
    charge_growth::<T>(was, q.capacity());
}

/// Drop the front (empty) sealed batch, releasing its native storage.
fn drop_front_batch(batches: &mut VecDeque<VecDeque<u64>>) {
    if let Some(b) = batches.pop_front() {
        release_buf::<u64>(b.capacity());
    }
}

impl Drop for ChanData {
    fn drop(&mut self) {
        // Every buffer the channel still holds: queued messages, message
        // sizes, leases, sealed batches (outer and inner), per-producer
        // staging, and the recycled page pool. Buffers released while the
        // channel lived were freed then; the waiter lists hold no charge.
        let mut bytes = buf_bytes::<u64>(self.queue.capacity())
            + buf_bytes::<usize>(self.sizes.capacity())
            + buf_bytes::<(TaskId, u64)>(self.leases.capacity())
            + buf_bytes::<VecDeque<u64>>(self.batches.capacity())
            + buf_bytes::<Vec<u64>>(self.pool.capacity());
        bytes += self.batches.iter().map(|b| buf_bytes::<u64>(b.capacity())).sum::<usize>();
        bytes += self.staging.values().map(|v| buf_bytes::<u64>(v.capacity())).sum::<usize>();
        bytes += self.pool.iter().map(|v| buf_bytes::<u64>(v.capacity())).sum::<usize>();
        if bytes > 0 {
            gc::external_free(bytes);
        }
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
    d.waiters.iter().filter(|w| w.kind == WaitKind::Send).for_each(|w| f(w.value));
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
                d.waiters.count(WaitKind::Send) + d.waiters.count(WaitKind::Capacity),
                d.waiters.count(WaitKind::Recv),
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
/// and keeps it until the lock is released. With one worker nothing claims
/// (no fast path runs meanwhile): the channel is marked `STALE` instead, so
/// the next fast path decides from its contents, not the published phase.
pub fn data<'a>(ch: u64) -> &'a mut ChanData {
    let d = raw(ch);
    if task::multi() {
        if task::holds_lock() {
            claim(ch, d);
        }
    } else {
        let w = d.state.load(Ordering::Relaxed);
        if w & word::STALE == 0 {
            d.state.store(w | word::STALE, Ordering::Relaxed);
        }
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

/// Make the runtime lock's holder the channel's owner: the
/// compare-and-swap from `FREE` is the linearization point of every
/// operation on the channel until the lock is released.
fn claim(ch: u64, d: &ChanData) {
    let mut w = d.state.load(Ordering::Acquire);
    loop {
        match w & word::OWNER {
            // one thread holds the runtime lock: a claim already made is its own
            word::CLAIMED => return,
            word::FREE => match d.state.compare_exchange_weak(
                w,
                w | word::CLAIMED,
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => w = x,
            },
            _ => {
                std::hint::spin_loop();
                w = d.state.load(Ordering::Acquire);
            }
        }
    }
    unsafe { (*CLAIMED.0.get()).push(ch) };
}

/// Release every channel claimed under the runtime lock (called as the
/// lock is released), publishing each one's phase.
pub fn release_claims() {
    let claimed = unsafe { &mut *CLAIMED.0.get() };
    for ch in claimed.drain(..) {
        let d = raw(ch);
        d.state.store(word::FREE | d.word(), Ordering::Release);
    }
}

/// A channel's element type (fixed at creation: read without claiming).
pub fn data_elem(ch: u64) -> &'static TypeDesc {
    raw(ch).elem
}

/// A channel's word as its last owner published it (read without claiming:
/// inspection and tests).
pub fn word_of(ch: u64) -> u8 {
    raw(ch).state.load(Ordering::Acquire)
}

/// The phase and closed bits a fast path may decline on without taking the
/// channel, from a load of its word: `None` while stale (one worker: only
/// the contents tell). They may be out of date by the time it acts, which a
/// decline tolerates: the path it hands the operation to decides from the
/// contents.
fn published(w: u8) -> Option<u8> {
    (w & word::STALE == 0).then_some(w & (word::PHASE | word::CLOSED))
}

/// Take a channel for an unlocked fast path, if nothing else holds it (`w`:
/// a load of its word). Another fast path holds a channel for a few
/// instructions, and a claim by the runtime lock's holder lasts until it
/// releases that lock: wait for either briefly, with backoff, rather than
/// falling back to the runtime lock (one fallback would claim the channel
/// and make every other operation on it fall back too). The wait is
/// bounded: the lock's holder may be a collection waiting for this worker
/// to reach a safe point, which the fallback is.
///
/// The successful compare-and-swap is the fast path's linearization point:
/// it keeps the phase bits it read, so it succeeds only on the state the
/// last owner published, and from then until [`end_fast`] nothing else
/// reads or writes the channel.
#[inline]
fn try_fast(d: &ChanData, w: u8) -> bool {
    (w & word::OWNER == word::FREE
        && d.state.compare_exchange(w, w | word::FAST, Ordering::Acquire, Ordering::Relaxed).is_ok())
        || try_fast_spin(d)
}

/// [`try_fast`] after its first attempt failed: wait with backoff.
#[cold]
#[inline(never)]
fn try_fast_spin(d: &ChanData) -> bool {
    let started = task::stats_on().then(std::time::Instant::now);
    let mut spun = 0u64;
    let mut spins = 1u32;
    for _ in 1..24 {
        for _ in 0..spins {
            std::hint::spin_loop();
        }
        spun += spins as u64;
        spins = (spins * 2).min(256);
        let w = d.state.load(Ordering::Relaxed);
        if w & word::OWNER == word::FREE
            && d.state.compare_exchange_weak(w, w | word::FAST, Ordering::Acquire, Ordering::Relaxed).is_ok()
        {
            return true;
        }
    }
    if let Some(t) = started {
        stat_fallback(fallback::BUSY, spun, t.elapsed().as_nanos() as u64);
    }
    false
}

/// End a fast path: publish the phase of what it leaves and free the
/// channel (the release that makes its effects visible to the next owner),
/// then tell idle workers of the tasks it made ready.
fn end_fast(d: &ChanData) {
    d.state.store(word::FREE | d.word(), Ordering::Release);
    task::notify_deferred();
}

/// Why a fast path declined (`BADGER_RUNTIME_STATS=1`): counted on the
/// decline path from what the fast path read, never changing what it does.
mod fallback {
    /// Another fast path or a claim held the channel past the backoff.
    pub const BUSY: usize = 0;
    /// A task waits on the channel that the fast path does not serve.
    pub const WAITER: usize = 1;
    /// No room, nothing to take, closed, or admission not by count alone.
    pub const STATE: usize = 2;
    pub const NAMES: [&str; 3] = ["channel busy", "a task waits on the channel", "no room, or nothing to take"];
}

static FALLBACKS: [AtomicU64; 3] = [const { AtomicU64::new(0) }; 3];
static FALLBACK_SPINS: AtomicU64 = AtomicU64::new(0);
static FALLBACK_SPIN_NS: AtomicU64 = AtomicU64::new(0);

/// Count a fast-path decline (statistics on only).
fn stat_fallback(reason: usize, spins: u64, spin_ns: u64) {
    if task::stats_on() {
        FALLBACKS[reason].fetch_add(1, Ordering::Relaxed);
        FALLBACK_SPINS.fetch_add(spins, Ordering::Relaxed);
        FALLBACK_SPIN_NS.fetch_add(spin_ns, Ordering::Relaxed);
    }
}

/// The fast-path declines, printed with the runtime statistics.
pub fn print_fallbacks() {
    let total: u64 = FALLBACKS.iter().map(|c| c.load(Ordering::Relaxed)).sum();
    eprintln!(
        "[runtime stats] channel fast path declined {total} times ({} spin iterations, {:.3} ms spinning)",
        FALLBACK_SPINS.load(Ordering::Relaxed),
        FALLBACK_SPIN_NS.load(Ordering::Relaxed) as f64 / 1e6
    );
    for (c, name) in FALLBACKS.iter().zip(fallback::NAMES) {
        eprintln!("[runtime stats]   {:>9} x {name}", c.load(Ordering::Relaxed));
    }
}

/// Unlocked prompt send: hand `v` to the first live parked receiver, or
/// else queue it when it has room and admission is by count alone; only
/// while no selection offers to receive and no sender waits (the locked
/// path commits to the one and keeps the order behind the other). `false`
/// means the locked path must decide.
///
/// Linearizes at [`try_fast`]'s compare-and-swap.
pub fn send_fast(ch: u64, v: u64) -> bool {
    let d = raw(ch);
    // a byte budget (fixed at creation) charges the message's size, which
    // depends only on the value: measured before taking the channel
    let size = if d.byte_budget > 0 { admission_size(d.elem, v) } else { 0 };
    if !try_fast(d, d.state.load(Ordering::Relaxed)) {
        return false;
    }
    let mut ok = false;
    let fits = d.byte_budget == 0 || (size <= d.byte_budget && d.bytes + size <= d.byte_budget);
    let unserved = if d.waiters.is_empty() {
        0
    } else {
        d.waiters.count(WaitKind::WatchRecv) + d.waiters.count(WaitKind::Send)
    };
    if !d.closed && !d.batched && fits && unserved == 0 {
        // the channel is held, so the receiver cannot finish and leave it
        // meanwhile
        while let Some(r) = d.waiters.pop(WaitKind::Recv) {
            if task::deliver_held(r.rec, r.wait, v, d.elem.is_ptr(), 1) {
                ok = true;
                break;
            }
        }
        if !ok && d.capacity > 0 && d.queue.len() < d.capacity {
            push_back_charged(&mut d.queue, v);
            gc::barrier(ch);
            d.bytes += size;
            push_back_charged(&mut d.sizes, size);
            ok = true;
        }
    }
    let why = (!ok && task::stats_on()).then_some(if unserved > 0 { fallback::WAITER } else { fallback::STATE });
    end_fast(d);
    if let Some(r) = why {
        stat_fallback(r, 0, 0);
    }
    ok
}

/// Unlocked batch append: add `v` to the calling producer's existing
/// staging when it does not fill the batch (no publication, so no one to
/// wake) and admission is by count alone.
///
/// Linearizes at [`try_fast`]'s compare-and-swap.
pub fn append_fast(ch: u64, v: u64) -> bool {
    let d = raw(ch);
    if !try_fast(d, d.state.load(Ordering::Relaxed)) {
        return false;
    }
    let me = task::current_id();
    let ok = d.batched
        && !d.closed
        && d.byte_budget == 0
        && d.queue.len() + d.accepted < d.capacity
        && d.staging.get(&me).is_some_and(|st| st.len() + 1 < d.max_batch);
    if ok {
        push_charged(d.staging.get_mut(&me).expect("staging"), v);
        gc::barrier(ch);
        d.accepted += 1;
    }
    let why = (!ok && task::stats_on()).then_some(fallback::STATE);
    end_fast(d);
    if let Some(r) = why {
        stat_fallback(r, 0, 0);
    }
    ok
}

/// Unlocked single-message batch receive: take the oldest published
/// message when no producer waits for capacity (no one to wake).
///
/// Linearizes at [`try_fast`]'s compare-and-swap, or, when the published
/// phase is `EMPTY` (nothing published, nobody registered), declines at
/// the load of the word without taking the channel.
pub fn recv_one_fast(ch: u64) -> Option<u64> {
    let d = raw(ch);
    let w = d.state.load(Ordering::Acquire);
    if published(w).is_some_and(|p| p & word::PHASE == word::EMPTY) {
        stat_fallback(fallback::STATE, 0, 0);
        return None;
    }
    if !try_fast(d, w) {
        return None;
    }
    let mut v = None;
    let waiting = d.waiters.count(WaitKind::Capacity);
    if d.batched && d.byte_budget == 0 && waiting == 0 {
        while d.batches.front().is_some_and(VecDeque::is_empty) {
            drop_front_batch(&mut d.batches);
        }
        if let Some(front) = d.batches.front_mut() {
            v = front.pop_front();
            if front.is_empty() {
                drop_front_batch(&mut d.batches);
            }
            d.accepted -= 1;
        }
    }
    let why = (v.is_none() && task::stats_on()).then_some(if waiting > 0 { fallback::WAITER } else { fallback::STATE });
    end_fast(d);
    if let Some(r) = why {
        stat_fallback(r, 0, 0);
    }
    v
}

/// Unlocked prompt receive: take the oldest message, or a parked sender's
/// value, when no selection waits for space (no one to wake). `None` means
/// the locked path must decide.
///
/// Linearizes at [`try_fast`]'s compare-and-swap, or, when the published
/// phase is `EMPTY` (nothing queued, nobody registered), declines at the
/// load of the word without taking the channel.
pub fn recv_fast(ch: u64) -> Option<u64> {
    let d = raw(ch);
    let w = d.state.load(Ordering::Acquire);
    if published(w).is_some_and(|p| p & word::PHASE == word::EMPTY) {
        stat_fallback(fallback::STATE, 0, 0);
        return None;
    }
    if !try_fast(d, w) {
        return None;
    }
    let mut v = None;
    let watching = if d.waiters.is_empty() { 0 } else { d.waiters.count(WaitKind::WatchSpace) };
    if !d.batched && watching == 0 {
        if let Some(x) = d.queue.pop_front() {
            if let Some(sz) = d.sizes.pop_front() {
                d.bytes -= sz;
            }
            v = Some(x);
            // a sender parked on the full queue moves into the freed slot
            if !d.waiters.is_empty() {
                admit_sender_fast(d);
            }
        } else {
            // capacity zero or a drained queue: take a parked sender's value
            while let Some(s) = d.waiters.pop(WaitKind::Send) {
                if task::deliver_held(s.rec, s.wait, 0, false, 1) {
                    v = Some(s.value);
                    break;
                }
            }
        }
    }
    let why = (v.is_none() && task::stats_on()).then_some(if watching > 0 { fallback::WAITER } else { fallback::STATE });
    end_fast(d);
    if let Some(r) = why {
        stat_fallback(r, 0, 0);
    }
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
///
/// Linearizes at [`try_fast`]'s compare-and-swap: taking a message, or
/// registering (published as `BLOCKED` by [`end_fast`]) and then completing
/// when a sender's hand-off delivers to it.
pub fn recv_free(ch: u64) -> Free {
    if !task::can_suspend_free() {
        return Free::Locked;
    }
    let d = raw(ch);
    if !try_fast(d, d.state.load(Ordering::Relaxed)) {
        return Free::Locked;
    }
    if d.batched || d.waiters.count(WaitKind::WatchSpace) > 0 {
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
    while let Some(s) = d.waiters.pop(WaitKind::Send) {
        if task::deliver_held(s.rec, s.wait, 0, false, 1) {
            end_fast(d);
            return Free::Done(s.value);
        }
    }
    if d.closed || !task::begin_park_free(1) {
        end_fast(d);
        return Free::Locked;
    }
    let _r = gc::Roots::new();
    _r.add(ch);
    d.register(WaitKind::Recv, 0, 0);
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
///
/// Linearizes at [`try_fast`]'s compare-and-swap, as [`recv_free`] does.
pub fn send_free(ch: u64, v: u64) -> Free {
    if !task::can_suspend_free() {
        return Free::Locked;
    }
    let d = raw(ch);
    if d.byte_budget > 0 || !try_fast(d, d.state.load(Ordering::Relaxed)) {
        return Free::Locked;
    }
    if d.closed || d.batched || d.waiters.count(WaitKind::WatchRecv) > 0 {
        end_fast(d);
        return Free::Locked;
    }
    while let Some(r) = d.waiters.pop(WaitKind::Recv) {
        if task::deliver_held(r.rec, r.wait, v, d.elem.is_ptr(), 1) {
            end_fast(d);
            return Free::Done(0);
        }
    }
    if d.capacity > 0 && d.queue.len() < d.capacity && d.waiters.count(WaitKind::Send) == 0 {
        push_back_charged(&mut d.queue, v);
        gc::barrier(ch);
        push_back_charged(&mut d.sizes, 0);
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
    d.register(WaitKind::Send, v, 0);
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
    d.waiters.remove(me, WaitKind::Recv);
    d.waiters.remove(me, WaitKind::Send);
    done_waiting(ch, true);
}

/// [`admit_sender`] for a fast path holding the channel.
fn admit_sender_fast(d: &mut ChanData) {
    while let Some(s) = d.waiters.pop(WaitKind::Send) {
        let fits = d.byte_budget == 0 || d.bytes + s.size <= d.byte_budget;
        if d.capacity == 0 || !fits || d.queue.len() >= d.capacity {
            d.waiters.push_front(s);
            return;
        }
        if task::deliver_held(s.rec, s.wait, 0, false, 1) {
            push_back_charged(&mut d.queue, s.value);
            d.bytes += s.size;
            push_back_charged(&mut d.sizes, s.size);
            return;
        }
    }
}

#[allow(static_mut_refs)]
pub fn new(elem: &'static TypeDesc, capacity: usize, max_batch: usize, batched: bool) -> u64 {
    let w = gc::alloc(chan_desc(), 8);
    let d = Box::new(ChanData {
        seq: NEXT_SEQ.fetch_add(1, Ordering::Relaxed),
        state: AtomicU8::new(word::FREE | word::EMPTY),
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
        waiters: Waiters::default(),
        pool: vec![],
        published_batches: 0,
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

/// Wake up to `n` live parked receivers (each removed: it was served, and
/// registers again if another receiver took the message first).
fn wake_receivers(d: &mut ChanData, mut n: usize) {
    while n > 0
        && let Some(r) = d.waiters.pop(WaitKind::Recv)
    {
        if task::waits(r.task, r.wait) {
            task::wake(r.task, task::Wake::Normal);
            n -= 1;
        }
    }
}

/// Wake the producers waiting for capacity (each removed: it retries, and
/// registers again if there is still no room).
fn wake_capacity(d: &mut ChanData) {
    while let Some(w) = d.waiters.pop(WaitKind::Capacity) {
        task::wake(w.task, task::Wake::Normal);
    }
}

/// Wake every selection registered with kind `k` without removing it: a
/// selection probes all its cases again when it runs, and removes its own
/// registrations ([`unwatch`]) only then, so none is lost meanwhile.
fn wake_watchers(d: &ChanData, k: WaitKind) {
    for w in d.waiters.iter().filter(|w| w.kind == k) {
        task::wake(w.task, task::Wake::Normal);
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
        // maps and sets: their keys and values, walked in place (an entry
        // list here would allocate on every admission and every uncharge)
        Kind::Map { key, val } => {
            let (kptr, vptr) = (key.is_ptr(), val.is_ptr());
            let (ks, vs) = (key.repr().size() as usize, val.repr().size() as usize);
            let mut total = 0usize;
            crate::map::for_each(w, &mut |k, v| {
                total += if kptr { admission_size(key, k) } else { ks };
                total += if vptr { admission_size(val, v) } else { vs };
            });
            total
        }
        Kind::Set { elem } => {
            let eptr = elem.is_ptr();
            let es = elem.repr().size() as usize;
            let mut total = 0usize;
            crate::map::for_each(w, &mut |k, _| total += if eptr { admission_size(elem, k) } else { es });
            total
        }
        // a buffer's (or array's) elements
        _ if d.is_ptr() && w != 0 && crate::buffer::is_buffer(w) => {
            let b = crate::buffer::data(w);
            let elem = b.elem;
            b.items
                .iter()
                .map(|x| if elem.is_ptr() { admission_size(elem, x) } else { elem.repr().size() as usize })
                .sum()
        }
        _ => d.repr().size() as usize,
    }
}

fn wake_space(d: &ChanData) {
    wake_watchers(d, WaitKind::WatchSpace);
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
            while let Some(r) = d.waiters.pop(WaitKind::Recv) {
                if task::waits(r.task, r.wait) {
                    task::wake_with(r.task, v, d.elem.is_ptr(), 1);
                    return SendResult::Sent;
                }
            }
            // a selection parked on a receive of this channel commits to it
            // (at every capacity: a queued value would wake it only to
            // probe again, and a wake to it may arrive while it is not
            // parked)
            if commit_select_receive(ch, v) {
                return SendResult::Sent;
            }
        }
        // queued: every selection still watching this channel is not
        // parked (or committed elsewhere), and probes again before it parks
        if d.queue.len() < d.capacity && fits {
            push_back_charged(&mut d.queue, v);
            gc::barrier(ch);
            d.bytes += size;
            push_back_charged(&mut d.sizes, size);
            return SendResult::Sent;
        }
        if !wait && !fits {
            return SendResult::Full;
        }
        if !wait {
            return SendResult::Full;
        }
        d.register(WaitKind::Send, v, size);
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
        d.waiters.remove(me, WaitKind::Send);
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
    let d = data(ch);
    if d.waiters.count(WaitKind::WatchRecv) == 0 {
        return false;
    }
    for w in d.waiters.iter().filter(|w| w.kind == WaitKind::WatchRecv && w.task != me) {
        if let Some((tk, i, _)) = select_offer(w, ch, false) {
            tk.select_done = Some(i);
            task::wake_with(w.task, v, d.elem.is_ptr(), 1);
            return true;
        }
    }
    false
}

/// A parked selection that offers to send on `ch` gives its value (that
/// case commits on its behalf).
fn commit_select_send(ch: u64) -> Option<u64> {
    let me = task::current_id();
    let d = data(ch);
    if d.waiters.count(WaitKind::WatchSpace) == 0 {
        return None;
    }
    for w in d.waiters.iter().filter(|w| w.kind == WaitKind::WatchSpace && w.task != me) {
        if let Some((tk, i, v)) = select_offer(w, ch, true) {
            tk.select_done = Some(i);
            task::wake_with(w.task, 0, false, 1);
            return Some(v);
        }
    }
    None
}

/// The selection registered as `w`, if it is still in the wait it
/// registered for (parked, not cancelled), nothing committed it yet, and it
/// offers a case on `ch` (a send case when `send`): its record, the case's
/// index, and the offered value.
fn select_offer(w: &Waiter, ch: u64, send: bool) -> Option<(&'static mut task::Task, usize, u64)> {
    if !task::waits(w.task, w.wait) {
        return None;
    }
    let tk = task::task(w.task)?;
    if tk.select_done.is_some() {
        return None;
    }
    let &(_, i, _, v) = tk.select_offer.iter().find(|c| c.0 == ch && c.2 == send)?;
    Some((tk, i, v))
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
    while let Some(s) = d.waiters.pop(WaitKind::Send) {
        if task::waits(s.task, s.wait) {
            let fits = d.byte_budget == 0 || d.bytes + s.size <= d.byte_budget;
            if d.capacity == 0 || !fits || d.queue.len() >= d.capacity {
                // rendezvous: the receiver takes it directly (handled by caller)
                d.waiters.push_front(s);
                return;
            }
            push_back_charged(&mut d.queue, s.value);
            d.bytes += s.size;
            push_back_charged(&mut d.sizes, s.size);
            task::wake_with(s.task, 0, false, 1);
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
        while let Some(s) = d.waiters.pop(WaitKind::Send) {
            if task::waits(s.task, s.wait) {
                task::wake_with(s.task, 0, false, 1);
                return RecvResult::Value(s.value);
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
        d.register(WaitKind::Recv, 0, 0);
        let added = note_waiting(ch);
        task::park_for("channel receive");
        let (v, status) = task::take_delivered();
        if status == 1 {
            done_waiting(ch, added);
            return RecvResult::Value(v);
        }
        data(ch).waiters.remove(me, WaitKind::Recv);
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
                release_buf::<u64>(items.capacity());
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
    // the last transition: every registration is woken and removed (each
    // waiter, and each selection's probe, finds the channel closed)
    let d = data(ch);
    d.closed = true;
    for w in d.waiters.take_all() {
        match w.kind {
            WaitKind::Recv => {
                if task::waits(w.task, w.wait) {
                    task::wake(w.task, task::Wake::Normal);
                }
            }
            WaitKind::Send => {
                if task::waits(w.task, w.wait) {
                    task::wake_with(w.task, 0, false, 2);
                }
            }
            WaitKind::Capacity | WaitKind::WatchSpace | WaitKind::WatchRecv => {
                task::wake(w.task, task::Wake::Normal)
            }
        }
    }
}

/// Unlocked close of a prompt channel no selection watches: parked
/// receivers wake to find it closed, parked senders get their values back
/// as closed. `false`: the locked path must close it.
///
/// Linearizes at [`try_fast`]'s compare-and-swap.
pub fn close_fast(ch: u64) -> bool {
    let d = raw(ch);
    if !try_fast(d, d.state.load(Ordering::Relaxed)) {
        return false;
    }
    let ok = !d.batched
        && d.waiters.count(WaitKind::WatchRecv) == 0
        && d.waiters.count(WaitKind::WatchSpace) == 0
        && d.waiters.count(WaitKind::Capacity) == 0;
    if ok {
        d.closed = true;
        for w in d.waiters.take_all() {
            let status = if w.kind == WaitKind::Send { 2 } else { 0 };
            task::deliver_held(w.rec, w.wait, 0, false, status);
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
    // the messages move into uncharged temporaries: release each buffer's
    // charge as it is emptied (the channel's own buffers keep theirs).
    let queued: Vec<u64> = d.queue.drain(..).collect();
    let mut batches: Vec<Vec<u64>> = d
        .batches
        .drain(..)
        .map(|b| {
            release_buf::<u64>(b.capacity());
            Vec::from(b)
        })
        .collect();
    batches.extend(d.staging.drain().map(|(_, v)| {
        release_buf::<u64>(v.capacity());
        v
    }));
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

/// Register the current task (a selection) to be committed to, or woken,
/// when `ch` may have a message. It stays registered until [`unwatch`].
pub fn watch_recv(ch: u64) {
    data(ch).register(WaitKind::WatchRecv, 0, 0);
    let _ = note_waiting(ch);
}

/// Register the current task (a selection) to be woken when `ch` may have
/// space. It stays registered until [`unwatch`].
pub fn watch_space(ch: u64) {
    data(ch).register(WaitKind::WatchSpace, 0, 0);
    let _ = note_waiting(ch);
}

/// Withdraw the current task's selection registrations with `ch`.
pub fn unwatch(ch: u64) {
    let me = task::current_id();
    let d = data(ch);
    d.waiters.remove(me, WaitKind::WatchRecv);
    d.waiters.remove(me, WaitKind::WatchSpace);
}

// ------------------------------------------------------------------ batches

/// Publish a producer's staged messages as one sealed batch.
pub fn publish(ch: u64, producer: TaskId) {
    let d = data(ch);
    d.staged_at.remove(&producer);
    let Some(staged) = d.staging.remove(&producer) else { return };
    if staged.is_empty() {
        release_buf::<u64>(staged.capacity());
        return;
    }
    // the buffer moves into an uncharged temporary (`VecDeque::from`), which
    // may allocate a fresh one: release the old charge, charge the new.
    release_buf::<u64>(staged.capacity());
    let batch = VecDeque::from(staged);
    charge_buffer::<u64>(batch.capacity());
    let n = batch.len();
    push_back_charged(&mut d.batches, batch);
    d.published_batches += 1;
    // a receiver per message (a single-message receiver takes one)
    wake_receivers(d, n);
    // selections waiting on this channel
    wake_watchers(d, WaitKind::WatchRecv);
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
            // a page popped from the pool carries its charge into staging;
            // one dropped because the entry exists releases its own.
            let st = match d.staging.entry(me) {
                std::collections::hash_map::Entry::Occupied(e) => {
                    release_buf::<u64>(page.capacity());
                    e.into_mut()
                }
                std::collections::hash_map::Entry::Vacant(e) => e.insert(page),
            };
            push_charged(st, v);
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
        d.register(WaitKind::Capacity, 0, 0);
        let _ = note_waiting(ch);
        task::park_for("batch capacity");
        data(ch).waiters.remove(me, WaitKind::Capacity);
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
            // the buffer moves into an uncharged `Vec` (which may allocate a
            // fresh one): release the old charge, charge the new.
            release_buf::<u64>(items.capacity());
            let items: Vec<u64> = items.into();
            charge_buffer::<u64>(items.capacity());
            d.leased += items.len();
            unsafe {
                let p = obj(b).payload();
                *(p as *mut *mut Batch) = Box::into_raw(Box::new(Batch { items, released: false }));
                *(p.add(8) as *mut u64) = ch;
            }
            push_charged(&mut data(ch).leases, (task::current_id(), b));
            gc::barrier(ch);
            let _ = note_waiting(ch);
            return Some(b);
        }
        if d.closed && d.staging.values().all(Vec::is_empty) {
            return None;
        }
        let me = task::current_id();
        d.register(WaitKind::Recv, 0, 0);
        let _ = note_waiting(ch);
        task::park_for("batch receive");
        data(ch).waiters.remove(me, WaitKind::Recv);
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
            drop_front_batch(&mut d.batches);
        }
        if let Some(front) = d.batches.front_mut() {
            let v = front.pop_front().expect("nonempty batch");
            if front.is_empty() {
                drop_front_batch(&mut d.batches);
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
        d.register(WaitKind::Recv, 0, 0);
        let _ = note_waiting(ch);
        task::park_for("batch receive");
        data(ch).waiters.remove(me, WaitKind::Recv);
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

/// A finished task leaves the channels it was registered with: a waiter
/// abandoned by cancellation is withdrawn (with the value it offered), and
/// its unreleased leases return their capacity (a lease is released by
/// the cleanup of the task that received it).
pub fn task_finished(id: TaskId) {
    let Some(t) = task::task(id) else { return };
    for ch in std::mem::take(&mut t.chan_regs) {
        let d = data(ch);
        d.waiters.remove_task(id);
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
        // the page keeps its charge; it returns to the pool's account
        push_charged(&mut d.pool, items);
    } else {
        release_buf::<u64>(items.capacity());
    }
    wake_capacity(d);
}
