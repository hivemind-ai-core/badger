//! Cancelled senders blocked on a full channel leave it: their waiter
//! registrations (and the values they offered) do not outlive them.
//! Adapted from the runtime audit reproducer (single worker, word ABI).
//! Each round cancels eight blocked senders, removes their tasks, then collects.
//! No sender registration may remain after cleanup.
use badger_runtime::{Adapter, Kind, TaskCtx, TypeDesc, channel, gc, string, task};
use std::sync::atomic::{AtomicU64, Ordering};

static CH: AtomicU64 = AtomicU64::new(0);

fn entry(adapter: Adapter) -> u64 {
    let desc =
        TypeDesc::new("audit-closure", Kind::Opaque { trace: None, finalize: None, args: vec![] }).leak();
    let w = gc::alloc_immortal(desc, 16);
    unsafe {
        *(gc::obj(w).payload().add(8) as *mut usize) = adapter as usize;
    }
    w
}

extern "C" fn producer(_: *mut TaskCtx, _: u64, _: *const u64) -> u64 {
    let value = string::new_string(&format!("payload-{}", task::current_id()));
    channel::send(CH.load(Ordering::Relaxed), value, true);
    0
}

extern "C" fn main_task(_: *mut TaskCtx, _: u64, _: *const u64) -> u64 {
    let roots = gc::Roots::new();
    let ch = roots.add(channel::new(badger_runtime::desc::builtin::string(), 1, 1, false));
    CH.store(ch, Ordering::Relaxed);
    let value = string::new_string("full");
    channel::send(ch, value, false);
    let start = entry(producer);
    for round in 1..=2 {
        let ids: Vec<_> = (0..8).map(|_| task::spawn_typed(start, None, "sender".into(), false)).collect();
        task::yield_now(); // all senders run and park before main resumes
        for &id in &ids {
            task::cancel(id);
        }
        task::await_cancelled(&ids);
        for id in ids {
            task::remove_task(id);
        }
        gc::collect();
        let stale = channel::data(ch).waiters.count(channel::WaitKind::Send);
        assert_eq!(stale, 0, "round {round}: {stale} cancelled senders still registered");
    }
    0
}

#[test]
fn cancelled_senders_leave_the_channel() {
    let start = entry(main_task);
    assert!(matches!(task::run_main(start), task::Exit::Returned(0)));
}
