//! The channel state machine's contract (`spec/tla/ChannelBadger.tla`), on
//! the real runtime (single worker, word ABI): the published phase follows
//! the contents; a send commits to a parked selection at every capacity and
//! the selection's registration is not forgotten; a value handed to a
//! receiver stays handed over when the receiver is cancelled before it runs;
//! a published batch wakes a receiver per message.
use badger_runtime::channel::{self, RecvResult, SendResult, WaitKind, word};
use badger_runtime::{Adapter, Kind, TaskCtx, TypeDesc, gc, task};
use std::sync::atomic::{AtomicU64, Ordering};

static CH: AtomicU64 = AtomicU64::new(0);
/// What the helper tasks saw (`u64::MAX`: nothing yet).
static GOT: [AtomicU64; 2] = [AtomicU64::new(u64::MAX), AtomicU64::new(u64::MAX)];
static SLOT: AtomicU64 = AtomicU64::new(0);

fn entry(adapter: Adapter) -> u64 {
    let desc =
        TypeDesc::new("state-closure", Kind::Opaque { trace: None, finalize: None, args: vec![] }).leak();
    let w = gc::alloc_immortal(desc, 16);
    unsafe {
        *(gc::obj(w).payload().add(8) as *mut usize) = adapter as usize;
    }
    w
}

fn int() -> &'static TypeDesc {
    badger_runtime::desc::builtin::int()
}

fn spawn(f: Adapter) -> task::TaskId {
    task::spawn_typed(entry(f), None, "helper".into(), false)
}

fn phase(ch: u64) -> u8 {
    let w = channel::word_of(ch);
    assert_eq!(w & word::OWNER, word::FREE, "nothing owns the channel between operations");
    w & word::PHASE
}

/// A selection with one receive case on `CH`, as `chan_select` registers
/// it; records the committed case and value.
extern "C" fn selection(_: *mut TaskCtx, _: u64, _: *const u64) -> u64 {
    let ch = CH.load(Ordering::Relaxed);
    channel::watch_recv(ch);
    let t = task::current().unwrap();
    t.select_done = None;
    t.select_offer = vec![(ch, 0, false, 0)];
    task::park_for("select");
    channel::unwatch(ch);
    let t = task::current().unwrap();
    t.select_offer.clear();
    if let Some(i) = t.select_done.take() {
        GOT[0].store(i as u64, Ordering::Relaxed);
        GOT[1].store(task::take_delivered().0, Ordering::Relaxed);
    }
    0
}

/// A locked prompt receive; records the value.
extern "C" fn receiver(_: *mut TaskCtx, _: u64, _: *const u64) -> u64 {
    if let RecvResult::Value(v) = channel::recv(CH.load(Ordering::Relaxed), true) {
        GOT[0].store(v, Ordering::Relaxed);
    }
    0
}

/// A single-message batch receive; records the value in its slot.
extern "C" fn batch_receiver(_: *mut TaskCtx, _: u64, _: *const u64) -> u64 {
    let slot = SLOT.fetch_add(1, Ordering::Relaxed) as usize;
    if let RecvResult::Value(v) = channel::recv_one(CH.load(Ordering::Relaxed), true) {
        GOT[slot].store(v, Ordering::Relaxed);
    }
    0
}

fn reset() {
    for g in &GOT {
        g.store(u64::MAX, Ordering::Relaxed);
    }
    SLOT.store(0, Ordering::Relaxed);
}

/// Every owner publishes the phase of what it leaves.
fn phase_follows_contents() {
    let roots = gc::Roots::new();
    let ch = roots.add(channel::new(int(), 2, 1, false));
    assert_eq!(phase(ch), word::EMPTY);
    assert!(channel::send_fast(ch, 1));
    assert_eq!(phase(ch), word::VALUE);
    assert_eq!(channel::recv_fast(ch), Some(1));
    assert_eq!(phase(ch), word::EMPTY);
    // nothing to take: declined on the published phase
    assert_eq!(channel::recv_fast(ch), None);
    assert_eq!(channel::word_of(ch) & word::OWNER, word::FREE);
    assert!(channel::close_fast(ch));
    assert_ne!(channel::word_of(ch) & word::CLOSED, 0);
}

/// A send to a channel with room commits to the parked selection watching
/// it (its value is not queued for it to race other receivers for), and
/// the selection stays registered until it withdraws.
fn send_commits_to_a_parked_selection() {
    reset();
    let roots = gc::Roots::new();
    let ch = roots.add(channel::new(int(), 4, 1, false));
    CH.store(ch, Ordering::Relaxed);
    let sel = spawn(selection);
    task::yield_now(); // the selection registers and parks
    assert_eq!(channel::data(ch).waiters.count(WaitKind::WatchRecv), 1);
    assert!(!channel::send_fast(ch, 7), "a fast send does not pass a watching selection");
    assert_eq!(phase(ch), word::BLOCKED);
    assert!(matches!(channel::send(ch, 7, false), SendResult::Sent));
    let d = channel::data(ch);
    assert!(d.queue.is_empty(), "the value went to the selection, not the queue");
    assert_eq!(d.waiters.count(WaitKind::WatchRecv), 1, "the registration is the selection's to withdraw");
    task::await_task(sel, false);
    assert_eq!((GOT[0].load(Ordering::Relaxed), GOT[1].load(Ordering::Relaxed)), (0, 7));
    assert!(channel::data(ch).waiters.is_empty());
    // withdrawn: the fast path queues again
    assert!(channel::send_fast(ch, 8));
    assert_eq!(phase(ch), word::VALUE);
}

/// A receiver handed a value and cancelled before it runs again completes
/// its receive: the value is not lost.
fn cancel_after_handoff_keeps_the_value() {
    reset();
    let roots = gc::Roots::new();
    let ch = roots.add(channel::new(int(), 1, 1, false));
    CH.store(ch, Ordering::Relaxed);
    let r = spawn(receiver);
    task::yield_now(); // the receiver parks
    assert!(matches!(channel::send(ch, 5, true), SendResult::Sent));
    task::cancel(r);
    task::await_task(r, false);
    assert_eq!(GOT[0].load(Ordering::Relaxed), 5, "the handed-over value reached the receiver");
    assert!(channel::data(ch).queue.is_empty());
}

/// Two receivers parked on a batch channel: a sealed batch of two wakes
/// both (one receiver per message).
fn publish_wakes_a_receiver_per_message() {
    reset();
    let roots = gc::Roots::new();
    let ch = roots.add(channel::new(int(), 4, 2, true));
    CH.store(ch, Ordering::Relaxed);
    let ids = [spawn(batch_receiver), spawn(batch_receiver)];
    task::yield_now(); // both park
    assert_eq!(channel::data(ch).waiters.count(WaitKind::Recv), 2);
    assert!(matches!(channel::append(ch, 1, true), SendResult::Sent));
    assert!(matches!(channel::append(ch, 2, true), SendResult::Sent)); // fills the batch: published
    for id in ids {
        task::await_task(id, false);
    }
    let mut got = [GOT[0].load(Ordering::Relaxed), GOT[1].load(Ordering::Relaxed)];
    got.sort_unstable();
    assert_eq!(got, [1, 2]);
}

extern "C" fn main_task(_: *mut TaskCtx, _: u64, _: *const u64) -> u64 {
    phase_follows_contents();
    send_commits_to_a_parked_selection();
    cancel_after_handoff_keeps_the_value();
    publish_wakes_a_receiver_per_message();
    0
}

#[test]
fn channel_state_machine_contract() {
    let start = entry(main_task);
    assert!(matches!(task::run_main(start), task::Exit::Returned(0)));
}
