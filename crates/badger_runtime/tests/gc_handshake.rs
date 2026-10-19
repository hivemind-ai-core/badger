//! The stop-the-world handshake (`gc_handshake`), driven from plain threads
//! standing in for workers: the collector does not trace while a worker is
//! online, a worker does not go online while the world is stopped, and a
//! nested stop keeps it stopped (`spec/tla/GcHandshake.tla`).
//!
//! Slot 0 is the collector's (always offline); the handshake state is
//! global, so the tests run one at a time.
use badger_runtime::gc_handshake;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

static SERIAL: Mutex<()> = Mutex::new(());
const WORKERS: usize = 4;

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

/// Run `f` on a thread and pause: its flag is still clear if `f` blocks.
fn blocks(f: impl FnOnce() + Send + 'static) -> (Arc<AtomicBool>, std::thread::JoinHandle<()>) {
    let done = Arc::new(AtomicBool::new(false));
    let d = done.clone();
    let h = std::thread::spawn(move || {
        f();
        d.store(true, Ordering::SeqCst);
    });
    std::thread::sleep(Duration::from_millis(50));
    (done, h)
}

#[test]
fn a_stop_waits_for_an_online_worker() {
    let _g = serial();
    gc_handshake::online(1);
    let (done, h) = blocks(|| gc_handshake::stop_the_world(WORKERS));
    assert!(!done.load(Ordering::SeqCst), "the stop ended with a worker online");
    gc_handshake::offline(1);
    h.join().unwrap();
    assert!(gc_handshake::stopping());
    gc_handshake::resume_the_world();
    assert!(!gc_handshake::stopping());
}

#[test]
fn going_online_waits_out_a_stop() {
    let _g = serial();
    gc_handshake::stop_the_world(WORKERS);
    let (done, h) = blocks(|| {
        gc_handshake::online(2);
        gc_handshake::offline(2);
    });
    assert!(!done.load(Ordering::SeqCst), "a worker went online during a stop");
    gc_handshake::resume_the_world();
    h.join().unwrap();
}

#[test]
fn a_nested_stop_keeps_the_world_stopped() {
    let _g = serial();
    gc_handshake::stop_the_world(WORKERS);
    gc_handshake::stop_the_world(WORKERS);
    gc_handshake::resume_the_world();
    assert!(gc_handshake::stopping());
    let (done, h) = blocks(|| {
        gc_handshake::online(3);
        gc_handshake::offline(3);
    });
    assert!(!done.load(Ordering::SeqCst), "the inner resume restarted the world");
    gc_handshake::resume_the_world();
    h.join().unwrap();
}

/// Workers go online without the lock and with it (`badger_rt_leave`), and
/// publish a returned pointer before going offline; a collector stopping
/// the world repeatedly never sees a worker online, nor a held pointer it
/// was not given.
#[test]
fn no_worker_runs_while_the_world_is_stopped() {
    let _g = serial();
    static MUTATING: [AtomicBool; WORKERS] = [const { AtomicBool::new(false) }; WORKERS];
    static HELD: [AtomicU64; WORKERS] = [const { AtomicU64::new(0) }; WORKERS];
    static RETURNED: [AtomicU64; WORKERS] = [const { AtomicU64::new(0) }; WORKERS];
    let lock = Arc::new(Mutex::new(()));
    let stop = Arc::new(AtomicBool::new(false));
    let workers: Vec<_> = (1..WORKERS)
        .map(|w| {
            let (lock, stop) = (lock.clone(), stop.clone());
            std::thread::spawn(move || {
                let mut i = 0u64;
                while !stop.load(Ordering::Relaxed) {
                    i += 1;
                    if i.is_multiple_of(3) {
                        let _l = lock.lock().unwrap();
                        gc_handshake::online_locked(w);
                    } else {
                        gc_handshake::online(w);
                    }
                    MUTATING[w].store(true, Ordering::SeqCst);
                    HELD[w].store(i, Ordering::SeqCst);
                    std::hint::spin_loop();
                    RETURNED[w].store(i, Ordering::SeqCst);
                    MUTATING[w].store(false, Ordering::SeqCst);
                    gc_handshake::offline(w);
                    let _l = lock.lock().unwrap();
                    RETURNED[w].store(0, Ordering::SeqCst);
                    HELD[w].store(0, Ordering::SeqCst);
                }
            })
        })
        .collect();
    for _ in 0..50000 {
        let _l = lock.lock().unwrap();
        gc_handshake::stop_the_world(WORKERS);
        for (w, ((mutating, held), returned)) in MUTATING.iter().zip(&HELD).zip(&RETURNED).enumerate().skip(1)
        {
            assert!(!mutating.load(Ordering::SeqCst), "worker {w} online during the stop");
            let held = held.load(Ordering::SeqCst);
            assert!(held == 0 || returned.load(Ordering::SeqCst) == held, "worker {w}'s root unpublished");
        }
        std::thread::yield_now();
        for (w, mutating) in MUTATING.iter().enumerate().skip(1) {
            assert!(!mutating.load(Ordering::SeqCst), "worker {w} went online during the stop");
        }
        gc_handshake::resume_the_world();
    }
    stop.store(true, Ordering::Relaxed);
    for h in workers {
        h.join().unwrap();
    }
}
