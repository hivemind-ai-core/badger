//! Task scopes: structured ownership of child tasks.
//!
//! A scope owns every task spawned in it. Closing a scope normally waits for
//! all children; closing it with an error cancels and settles them. A child
//! defect fails the scope: siblings are cancelled and the failure propagates
//! to the owning task. If the owner itself is abandoned, the scope's
//! registered cleanup cancels and settles the children.

use std::cell::UnsafeCell;

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

struct Scopes(UnsafeCell<Option<(u64, FxHashMap<u64, ScopeRec>)>>);
unsafe impl Sync for Scopes {}
static SCOPES: Scopes = Scopes(UnsafeCell::new(None));

fn scopes() -> &'static mut (u64, FxHashMap<u64, ScopeRec>) {
    unsafe { (*SCOPES.0.get()).get_or_insert_with(|| (1, FxHashMap::default())) }
}

pub fn get(id: u64) -> Option<&'static mut ScopeRec> {
    scopes().1.get_mut(&id)
}

/// Open a scope owned by the current task.
pub fn open() -> u64 {
    let s = scopes();
    let id = s.0;
    s.0 += 1;
    let owner = task::current_id();
    // if the owner is abandoned its cleanups run last-in first: settle the
    // children, then the defers (newest first), then release the record
    let release = task::push_cleanup(Cleanup::Native(Box::new(move |_| release(id))));
    let cleanup = task::push_cleanup(Cleanup::Native(Box::new(move |_| settle(id, true))));
    s.1.insert(
        id,
        ScopeRec {
            owner,
            children: vec![],
            failure: None,
            cleanup,
            release,
            captured: vec![],
            defers: vec![],
        },
    );
    id
}

/// Forget a closed scope: its record and its finished children. A child
/// still running (it outlived the cleanup grace period, and escalation
/// keeps what it uses) keeps its entry.
fn release(id: u64) {
    if let Some(s) = scopes().1.remove(&id) {
        for c in &s.children {
            if task::is_done(*c) {
                task::remove_task(*c);
            }
        }
    }
}

pub fn spawn(scope: u64, closure: u64, captured: bool, result_is_ptr: bool) -> TaskId {
    let name = format!("child of task {}", task::current_id());
    let id = task::spawn_typed(closure, Some(scope), name, result_is_ptr);
    if let Some(s) = get(scope) {
        s.children.push(id);
        if captured {
            s.captured.push(id);
        }
    }
    id
}

/// Called by the scheduler when a child finishes.
pub fn child_finished(scope: u64, child: TaskId, exit: &Exit) {
    let Some(s) = get(scope) else { return };
    if let Exit::Failed(info) = exit {
        if s.captured.contains(&child) || s.failure.is_some() {
            return;
        }
        let info = PanicInfo {
            message: format!("child task failed: {}", info.message),
            location: info.location.clone(),
            notes: info.notes.clone(),
        };
        s.failure = Some(info.clone());
        let siblings: Vec<TaskId> = s.children.iter().copied().filter(|c| *c != child).collect();
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
    let children = s.children.clone();
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
    let children = s.children.clone();
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
