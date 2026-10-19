//! Admission gates (`std.admission`): at most `capacity` admitted calls,
//! at most `queue` more waiting (FIFO), each until its deadline. A released
//! permit passes directly to the longest waiter, so a waiter that times out
//! or is cancelled never strands one.

use std::collections::{HashSet, VecDeque};
use std::sync::Mutex;

use crate::gc;
use crate::task::{self, TaskId, Wake};

struct Gate {
    capacity: usize,
    queue: usize,
    available: usize,
    waiting: VecDeque<(TaskId, u64)>,
    /// Waiters handed a permit that have not resumed yet.
    granted: HashSet<u64>,
    admitted: u64,
    rejected_full: u64,
    rejected_deadline: u64,
}

static GATES: Mutex<Vec<Gate>> = Mutex::new(Vec::new());
static NEXT_TICKET: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub fn new(capacity: usize, queue: usize) -> usize {
    let mut g = GATES.lock().unwrap();
    g.push(Gate {
        capacity,
        queue,
        available: capacity,
        waiting: VecDeque::new(),
        granted: HashSet::new(),
        admitted: 0,
        rejected_full: 0,
        rejected_deadline: 0,
    });
    g.len() - 1
}

/// Admission outcomes.
pub const ADMITTED: u64 = 0;
pub const FULL: u64 = 1;
pub const DEADLINE: u64 = 2;

/// Take a permit, waiting at most `wait` (`None`: without limit). Called
/// from a task, holding the runtime lock.
pub fn acquire(id: usize, wait: Option<std::time::Duration>) -> u64 {
    let ticket = {
        let mut gs = GATES.lock().unwrap();
        let g = &mut gs[id];
        // an expired call does no work, even when a permit is free
        if wait == Some(std::time::Duration::ZERO) {
            g.rejected_deadline += 1;
            return DEADLINE;
        }
        if g.available > 0 && g.waiting.is_empty() {
            g.available -= 1;
            g.admitted += 1;
            return ADMITTED;
        }
        if g.waiting.len() >= g.queue {
            g.rejected_full += 1;
            return FULL;
        }
        let ticket = NEXT_TICKET.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // The gate's own queue is native memory outside the managed heap.
        // Gates live for the process and never shrink their queue, so the
        // charge is released only when the process ends.
        let was = g.waiting.capacity();
        g.waiting.push_back((task::current_id(), ticket));
        let grew = g.waiting.capacity() - was;
        if grew > 0 {
            gc::external_alloc(grew * std::mem::size_of::<(TaskId, u64)>());
        }
        ticket
    };
    // a cancelled waiter leaves the queue, returning a permit handed to it
    let cleanup = task::push_cleanup(task::Cleanup::Native(Box::new(move |_| leave(id, ticket))));
    let deadline = wait.map(|w| task::monotonic_nanos().saturating_add(w.as_nanos() as i64));
    loop {
        let remaining =
            deadline.map(|d| std::time::Duration::from_nanos((d - task::monotonic_nanos()).max(0) as u64));
        let w = match remaining {
            Some(r) if r.is_zero() => Wake::Timeout,
            _ => task::park_until(remaining, "admission"),
        };
        let mut gs = GATES.lock().unwrap();
        let g = &mut gs[id];
        if g.granted.remove(&ticket) {
            g.admitted += 1;
            drop(gs);
            task::remove_cleanup(cleanup);
            return ADMITTED;
        }
        if matches!(w, Wake::Timeout) {
            g.waiting.retain(|(_, t)| *t != ticket);
            g.rejected_deadline += 1;
            drop(gs);
            task::remove_cleanup(cleanup);
            return DEADLINE;
        }
    }
}

/// A waiter that will not resume (cancelled): out of the queue, and a
/// permit already handed to it goes back.
fn leave(id: usize, ticket: u64) {
    let granted = {
        let mut gs = GATES.lock().unwrap();
        let g = &mut gs[id];
        g.waiting.retain(|(_, t)| *t != ticket);
        g.granted.remove(&ticket)
    };
    if granted {
        release(id);
    }
}

/// Return a permit: to the longest waiter, else to the gate.
pub fn release(id: usize) {
    let next = {
        let mut gs = GATES.lock().unwrap();
        let g = &mut gs[id];
        match g.waiting.pop_front() {
            Some((task, ticket)) => {
                g.granted.insert(ticket);
                Some(task)
            }
            None => {
                g.available = (g.available + 1).min(g.capacity);
                None
            }
        }
    };
    if let Some(t) = next {
        task::wake(t, Wake::Normal);
    }
}

/// `(executing, waiting, admitted, rejected_full, rejected_deadline)`.
pub fn stats(id: usize) -> (u64, u64, u64, u64, u64) {
    let gs = GATES.lock().unwrap();
    let g = &gs[id];
    (
        (g.capacity - g.available) as u64,
        g.waiting.len() as u64,
        g.admitted,
        g.rejected_full,
        g.rejected_deadline,
    )
}
