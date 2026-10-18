//! Queue-only experiment: no fibers, GC, select, cancellation or close.
//! The SPSC prototype is deliberately restricted to one writer per index.
//! This is measurement scaffolding, not a proposed production implementation.
use std::cell::UnsafeCell;
use std::hint::{black_box, spin_loop};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::time::Instant;

#[repr(align(128))]
struct Padded<T>(T);

struct Ring {
    slots: Box<[UnsafeCell<u64>]>,
    head: Padded<AtomicUsize>,
    tail: Padded<AtomicUsize>,
}

// Each endpoint exclusively owns its index. A Release tail publishes initialized
// slots to the consumer's Acquire load; Release head publishes completed reads
// before the producer may overwrite those slots. No slots are accessed outside
// these published regions. Counters do not wrap within the measured run.
unsafe impl Sync for Ring {}

impl Ring {
    fn new(cap: usize) -> Self {
        Self {
            slots: (0..cap).map(|_| UnsafeCell::new(0)).collect(),
            head: Padded(AtomicUsize::new(0)),
            tail: Padded(AtomicUsize::new(0)),
        }
    }
}

struct QueueState {
    slots: Vec<u64>,
    head: usize,
    tail: usize,
}

struct Locked {
    held: AtomicBool,
    state: UnsafeCell<QueueState>,
}

// All state accesses are inside the Acquire/Release lock.
unsafe impl Sync for Locked {}

impl Locked {
    fn new(cap: usize) -> Self {
        Self {
            held: AtomicBool::new(false),
            state: UnsafeCell::new(QueueState { slots: vec![0; cap], head: 0, tail: 0 }),
        }
    }

    fn push(&self, start: usize, want: usize) -> usize {
        if self.held.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            return 0;
        }
        let s = unsafe { &mut *self.state.get() };
        let n = want.min(s.slots.len() - (s.tail - s.head));
        let mask = s.slots.len() - 1;
        for j in 0..n {
            s.slots[(s.tail + j) & mask] = (start + j) as u64;
        }
        s.tail += n;
        self.held.store(false, Ordering::Release);
        n
    }

    fn pop(&self, want: usize) -> (usize, u64) {
        if self.held.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            return (0, 0);
        }
        let s = unsafe { &mut *self.state.get() };
        let n = want.min(s.tail - s.head);
        let mask = s.slots.len() - 1;
        let mut sum = 0;
        for j in 0..n {
            let v = s.slots[(s.head + j) & mask];
            assert_eq!(v, (s.head + j) as u64);
            sum += v;
        }
        s.head += n;
        self.held.store(false, Ordering::Release);
        (n, sum)
    }
}

#[cfg(target_os = "linux")]
fn pin(cpu: usize) {
    unsafe extern "C" {
        fn sched_setaffinity(pid: i32, size: usize, mask: *const usize) -> i32;
    }
    let mut mask = [0usize; 16];
    mask[cpu / usize::BITS as usize] |= 1 << (cpu % usize::BITS as usize);
    assert_eq!(unsafe { sched_setaffinity(0, std::mem::size_of_val(&mask), mask.as_ptr()) }, 0);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args[1].as_str();
    let n: usize = args[2].parse().unwrap();
    let cap: usize = args[3].parse().unwrap();
    let batch: usize = args[4].parse().unwrap();
    assert!(cap.is_power_of_two() && batch > 0);
    pin(1);
    let barrier = Arc::new(Barrier::new(3));
    let ring = Arc::new(Ring::new(cap));
    let lock = Arc::new(Locked::new(cap));
    let mpmc = Arc::new(crossbeam_queue::ArrayQueue::new(cap));
    let started = Arc::new(std::sync::OnceLock::new());
    std::thread::scope(|scope| {
        let b = barrier.clone();
        let r = ring.clone();
        let l = lock.clone();
        let q = mpmc.clone();
        let clock = started.clone();
        scope.spawn(move || {
            pin(1);
            b.wait();
            let _ = clock.set(Instant::now());
            let mut tail = 0;
            let mut cached_head = 0;
            while tail < n {
                let count = match mode {
                    "lock" => l.push(tail, batch.min(n - tail)),
                    "mpmc" => usize::from(q.push(tail as u64).is_ok()),
                    "spsc" => {
                        let want = batch.min(n - tail);
                        if cap - (tail - cached_head) < want {
                            cached_head = r.head.0.load(Ordering::Acquire);
                        }
                        let count = want.min(cap - (tail - cached_head));
                        for j in 0..count {
                            unsafe { *r.slots[(tail + j) & (cap - 1)].get() = (tail + j) as u64 };
                        }
                        if count > 0 {
                            r.tail.0.store(tail + count, Ordering::Release);
                        }
                        count
                    }
                    _ => panic!("unknown mode"),
                };
                tail += count;
                if count == 0 {
                    spin_loop();
                }
            }
        });
        let b = barrier.clone();
        let r = ring.clone();
        let l = lock.clone();
        let q = mpmc.clone();
        let clock = started.clone();
        let consumer = scope.spawn(move || {
            pin(2);
            b.wait();
            let _ = clock.set(Instant::now());
            let mut head = 0;
            let mut cached_tail = 0;
            let mut sum = 0;
            while head < n {
                let (count, part) = match mode {
                    "lock" => l.pop(batch.min(n - head)),
                    "mpmc" => match q.pop() {
                        Some(v) => {
                            assert_eq!(v, head as u64);
                            (1, v)
                        }
                        None => (0, 0),
                    },
                    "spsc" => {
                        if cached_tail == head {
                            cached_tail = r.tail.0.load(Ordering::Acquire);
                        }
                        let count = batch.min(cached_tail - head);
                        let mut part = 0;
                        for j in 0..count {
                            let v = unsafe { *r.slots[(head + j) & (cap - 1)].get() };
                            assert_eq!(v, (head + j) as u64);
                            part += v;
                        }
                        if count > 0 {
                            r.head.0.store(head + count, Ordering::Release);
                        }
                        (count, part)
                    }
                    _ => unreachable!(),
                };
                head += count;
                sum += part;
                if count == 0 {
                    spin_loop();
                }
            }
            let elapsed = clock.get().unwrap().elapsed();
            assert_eq!(black_box(sum), (n as u64) * (n as u64 - 1) / 2);
            println!("{{\"mode\":\"{mode}\",\"batch\":{batch},\"capacity\":{cap},\"items\":{n},\"ns_per_message\":{:.3}}}", elapsed.as_nanos() as f64 / n as f64);
        });
        barrier.wait();
        consumer.join().unwrap();
    });
}
