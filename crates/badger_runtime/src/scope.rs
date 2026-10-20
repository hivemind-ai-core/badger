//! Task scopes: structured ownership of child tasks.
//!
//! A scope owns every task spawned in it. Closing a scope normally waits for
//! all children; closing it with an error cancels and settles them. A child
//! defect fails the scope: siblings are cancelled and the failure propagates
//! to the owning task. If the owner itself is abandoned, the scope's
//! registered cleanup cancels and settles the children.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, Ordering};

use rustc_hash::FxHashMap;

use crate::task::{self, Cleanup, Exit, PanicInfo, TaskId};

pub struct ScopeRec {
    pub owner: TaskId,
    pub children: Vec<TaskId>,
    pub failure: Option<PanicInfo>,
    /// The owner's cleanup settling the children (kept above the defers).
    pub cleanup: u64,
    /// The owner's cleanup releasing the record (below the defers).
    pub release: u64,
    /// Children whose defects are contained (`capture`).
    pub captured: Vec<TaskId>,
    /// Cleanup handles registered with `scope.defer` (run in reverse at
    /// close; on a defect they run as the owner's cleanups).
    pub defers: Vec<u64>,
}

/// Scope records (boxed: a record stays in place while its scope is open)
/// under their own lock: owners spawn into them without the runtime lock.
/// A record's `children` and `captured` grow without the runtime lock (by
/// its owner); other tasks read them under this lock.
struct Scopes(UnsafeCell<Option<(u64, FxHashMap<u64, Box<ScopeRec>>)>>);
unsafe impl Sync for Scopes {}
static SCOPES: Scopes = Scopes(UnsafeCell::new(None));
static SCOPES_LOCK: AtomicBool = AtomicBool::new(false);

fn scopes<R>(f: impl FnOnce(&mut (u64, FxHashMap<u64, Box<ScopeRec>>)) -> R) -> R {
    let locking = task::multi();
    if locking {
        while SCOPES_LOCK.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            std::hint::spin_loop();
        }
    }
    let r = f(unsafe { (*SCOPES.0.get()).get_or_insert_with(|| (1, FxHashMap::default())) });
    if locking {
        SCOPES_LOCK.store(false, Ordering::Release);
    }
    r
}

pub fn get(id: u64) -> Option<&'static mut ScopeRec> {
    scopes(|s| s.1.get_mut(&id).map(|b| unsafe { &mut *(b.as_mut() as *mut ScopeRec) }))
}

/// Open a scope owned by the current task.
pub fn open() -> u64 {
    let id = scopes(|s| {
        s.0 += 1;
        s.0 - 1
    });
    let owner = task::current_id();
    // if the owner is abandoned its cleanups run last-in first: settle the
    // children, then the defers (newest first), then release the record
    let release = task::push_cleanup(Cleanup::Native(Box::new(move |_| release(id))));
    let cleanup = task::push_cleanup(Cleanup::Native(Box::new(move |_| settle(id, true))));
    let rec = Box::new(ScopeRec {
        owner,
        children: vec![],
        failure: None,
        cleanup,
        release,
        captured: vec![],
        defers: vec![],
    });
    scopes(|s| s.1.insert(id, rec));
    id
}

/// Forget a closed scope: its record and its finished children. A child
/// still running (it outlived the cleanup grace period, and escalation
/// keeps what it uses) keeps its entry.
fn release(id: u64) {
    if let Some(s) = scopes(|s| s.1.remove(&id)) {
        for c in &s.children {
            if task::is_done(*c) {
                task::remove_task(*c);
            }
        }
    }
}

pub fn spawn(scope: u64, closure: u64, captured: bool, result_is_ptr: bool) -> TaskId {
    spawn_with_args(scope, closure, captured, result_is_ptr, vec![])
}

/// [`spawn`] with entry arguments (see [`task::spawn_with_args`]).
pub fn spawn_with_args(
    scope: u64,
    closure: u64,
    captured: bool,
    result_is_ptr: bool,
    args: Vec<(u64, bool)>,
) -> TaskId {
    let name = format!("child of task {}", task::current_id());
    // recorded before it can run: its failure must find it captured
    task::spawn_with_args(closure, Some(scope), name, result_is_ptr, args, |id| {
        scopes(|ss| {
            if let Some(s) = ss.1.get_mut(&scope) {
                s.children.push(id);
                if captured {
                    s.captured.push(id);
                }
            }
        })
    })
}

/// Called by the scheduler when a child finishes.
pub fn child_finished(scope: u64, child: TaskId, exit: &Exit) {
    let Some(s) = get(scope) else { return };
    if let Exit::Failed(info) = exit {
        // (its owner may be spawning into it meanwhile)
        let (contained, siblings) = scopes(|_| {
            (
                s.captured.contains(&child),
                s.children.iter().copied().filter(|c| *c != child).collect::<Vec<_>>(),
            )
        });
        // a captured child's failure is its joiner's to see, unless the
        // owner is being cancelled and will not join it (its cleanup
        // failed as it stopped, say): then it fails the scope, not lost
        let joined = contained && !task::stopping(s.owner);
        if joined || s.failure.is_some() {
            return;
        }
        let info = PanicInfo {
            message: format!("child task failed: {}", info.message),
            location: info.location.clone(),
            notes: info.notes.clone(),
        };
        s.failure = Some(info.clone());
        let owner = s.owner;
        for c in siblings {
            task::cancel_with(c, task::CancelReason::SiblingFailed);
        }
        task::fail_pending(owner, info);
    }
}

/// Cancel (optionally) and wait for every child of the scope.
pub fn settle(id: u64, cancel: bool) {
    let Some(s) = get(id) else { return };
    let children = scopes(|_| s.children.clone());
    if cancel {
        for &c in &children {
            task::cancel_with(c, task::CancelReason::ScopeExited);
        }
        task::await_cancelled(&children);
    }
    for &c in &children {
        task::await_task(c, true);
    }
}

/// Close a scope after its body returned. `cancel` is true when the body
/// returned an error. Propagates a recorded child failure as a defect.
pub fn close(id: u64, cancel: bool) {
    let Some(s) = get(id) else { return };
    let handle = s.cleanup;
    let children = scopes(|_| s.children.clone());
    if cancel {
        for &c in &children {
            task::cancel_with(c, task::CancelReason::ScopeExited);
        }
        task::await_cancelled(&children);
    }
    for &c in &children {
        // waiting honors cancellation of the owner
        task::await_task(c, cancel);
    }
    task::remove_cleanup(handle);
    // deferred cleanups, after every child has settled; one that fails
    // abandons the owner, whose cleanups then run the remaining defers and
    // the release
    let defers = std::mem::take(&mut get(id).unwrap().defers);
    for h in defers.into_iter().rev() {
        if let Some(Cleanup::Closure(w)) = task::remove_cleanup(h) {
            let ctx = task::ctx_of(task::current_id());
            task::current().unwrap().roots.push(w);
            unsafe { crate::call_closure(ctx, w, &[]) };
            task::current().unwrap().roots.pop();
        }
    }
    // a child a defer spawned (the scope is open while they run) settles
    // too before the scope is released: no child outlives its scope
    let late: Vec<TaskId> = get(id)
        .map(|s| scopes(|_| s.children.iter().copied().filter(|c| !children.contains(c)).collect()))
        .unwrap_or_default();
    if cancel {
        for &c in &late {
            task::cancel_with(c, task::CancelReason::ScopeExited);
        }
        task::await_cancelled(&late);
    }
    for &c in &late {
        task::await_task(c, cancel);
    }
    let failure = get(id).and_then(|s| s.failure.take());
    if let Some(r) = get(id).map(|s| s.release) {
        task::remove_cleanup(r);
    }
    release(id);
    if let Some(f) = failure {
        task::panic_current(f.message, f.location);
    }
}

/// Register `f` (a `fn() -> Unit` closure) to run when the scope closes.
pub fn defer(id: u64, f: u64) {
    let h = task::push_cleanup(Cleanup::Closure(f));
    if let Some(s) = get(id) {
        s.defers.push(h);
        // the children settle before any defer runs, also on abandonment
        if task::remove_cleanup(s.cleanup).is_some() {
            s.cleanup = task::push_cleanup(Cleanup::Native(Box::new(move |_| settle(id, true))));
        }
    }
}

pub fn owner(id: u64) -> TaskId {
    get(id).map(|s| s.owner).unwrap_or(0)
}
