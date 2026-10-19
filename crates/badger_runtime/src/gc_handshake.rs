//! The stop-the-world handshake as a quiescent-state grace period (QSBR,
//! after Desnoyers et al., "User-Level Implementations of Read-Copy
//! Update", 2012). The contract is `spec/tla/GcHandshake.tla`.
//!
//! A worker is *online* while it may run compiled code without the runtime
//! lock (it may read and write the heap and its roots), and publishes the
//! epoch it observed when it went online; it is *offline* (in a quiescent
//! state) otherwise. A collector, holding the runtime lock, advances the
//! epoch and waits for the grace period: every worker has acknowledged the
//! new epoch by being offline. A worker running compiled code keeps the
//! epoch it observed, so the wait ends only when it reaches a safe point.
//! The world stays stopped until the collector advances the epoch again.
//!
//! Every access to the epoch and the slots is sequentially consistent: a
//! worker going online without the lock publishes its slot and then
//! re-reads the epoch, the collector advances the epoch and then reads the
//! slots, so one of them sees the other.
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::task::MAX_WORKERS;

/// The global epoch: even while workers may run compiled code, odd from a
/// collector's request until its resume. Only the runtime lock holder
/// advances it.
static EPOCH: AtomicU64 = AtomicU64::new(0);

/// Stops the runtime lock holder has in progress: a nested stop (the
/// collector inspecting the heap) does not begin another grace period, and
/// only the outermost resume ends it.
static DEPTH: AtomicUsize = AtomicUsize::new(0);

/// A worker's slot while it is offline. Greater than every epoch, so an
/// offline worker has acknowledged any of them.
const OFFLINE: u64 = u64::MAX;

/// Each worker's observed epoch: the even epoch it observed when it went
/// online, or [`OFFLINE`]. Only its own worker thread writes a slot.
static OBSERVED: [Slot; MAX_WORKERS] = [const { Slot(AtomicU64::new(OFFLINE)) }; MAX_WORKERS];

/// One worker's slot, on its own cache line: going online and offline
/// never writes a line another worker writes.
#[repr(align(128))]
struct Slot(AtomicU64);

/// Worker `w` goes online without the runtime lock: it publishes the epoch
/// it observes, then re-reads it, and waits out a stop that began in
/// between (it goes offline while it waits).
pub fn online(w: usize) {
    let slot = &OBSERVED[w].0;
    loop {
        let e = EPOCH.load(Ordering::SeqCst);
        if e & 1 == 0 {
            let was = slot.swap(e, Ordering::SeqCst);
            debug_assert_eq!(was, OFFLINE, "worker {w} went online twice");
            if EPOCH.load(Ordering::SeqCst) == e {
                return;
            }
            slot.store(OFFLINE, Ordering::SeqCst);
        }
        while EPOCH.load(Ordering::SeqCst) & 1 == 1 {
            std::hint::spin_loop();
            std::thread::yield_now();
        }
    }
}

/// Worker `w` goes online holding the runtime lock, which excludes a
/// collection: it must publish its slot before it releases the lock.
pub fn online_locked(w: usize) {
    let e = EPOCH.load(Ordering::SeqCst);
    debug_assert_eq!(e & 1, 0, "worker {w} went online during a stop");
    let was = OBSERVED[w].0.swap(e, Ordering::SeqCst);
    debug_assert_eq!(was, OFFLINE, "worker {w} went online twice");
}

/// Worker `w` reaches a quiescent state: it acknowledges every epoch until
/// it goes online again. A pointer compiled code returned must be
/// published as a root before this.
pub fn offline(w: usize) {
    let was = OBSERVED[w].0.swap(OFFLINE, Ordering::SeqCst);
    debug_assert_ne!(was, OFFLINE, "worker {w} went offline twice");
}

/// Whether a collector has stopped the world, or is waiting for it.
pub fn stopping() -> bool {
    EPOCH.load(Ordering::SeqCst) & 1 == 1
}

/// Stop the world (runtime lock held, the caller offline): advance the
/// epoch and wait until each of the first `workers` slots acknowledged it.
/// A slot read as acknowledged stays so for the stop: outside the lock a
/// worker goes online only in [`online`], which backs out.
pub fn stop_the_world(workers: usize) {
    if DEPTH.fetch_add(1, Ordering::Relaxed) > 0 {
        return;
    }
    let e = EPOCH.load(Ordering::Relaxed) + 1;
    EPOCH.store(e, Ordering::SeqCst);
    for slot in &OBSERVED[..workers.min(MAX_WORKERS)] {
        while slot.0.load(Ordering::SeqCst) < e {
            std::hint::spin_loop();
            std::thread::yield_now();
        }
    }
}

/// End the stop [`stop_the_world`] began (the outermost one advances the
/// epoch: the world runs again).
pub fn resume_the_world() {
    let depth = DEPTH.fetch_sub(1, Ordering::Relaxed);
    debug_assert!(depth > 0, "resume without a stop");
    if depth == 1 {
        EPOCH.store(EPOCH.load(Ordering::Relaxed) + 1, Ordering::SeqCst);
    }
}
