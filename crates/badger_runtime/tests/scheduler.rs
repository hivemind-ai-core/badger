//! Multi-worker scheduling: per-worker ready queues, the next slot, and
//! work stealing. Every spawned task must run to completion, and with
//! several workers the tasks must spread over them (the point of the
//! per-worker queues).
//!
//! An entry function runs as compiled code does: outside the runtime lock,
//! so each runtime call is bracketed with [`task::enter_runtime`] and
//! [`task::leave_runtime`].
use badger_runtime::{Adapter, Kind, TaskCtx, TypeDesc, gc, task};
use std::sync::atomic::{AtomicU64, Ordering};

/// How many children each worker ran.
static RAN: [AtomicU64; task::MAX_WORKERS] = [const { AtomicU64::new(0) }; task::MAX_WORKERS];

fn entry(adapter: Adapter) -> u64 {
    let desc =
        TypeDesc::new("scheduler-closure", Kind::Opaque { trace: None, finalize: None, args: vec![] })
            .leak();
    let w = gc::alloc_immortal(desc, 16);
    unsafe {
        *(gc::obj(w).payload().add(8) as *mut usize) = adapter as usize;
    }
    w
}

extern "C" fn child(_: *mut TaskCtx, _: u64, _: *const u64) -> u64 {
    // yield repeatedly: a ready task left on a worker's queue is work for
    // its thieves
    for _ in 0..8 {
        task::enter_runtime();
        task::yield_now();
        task::leave_runtime();
    }
    task::enter_runtime();
    let w = task::worker_index();
    task::leave_runtime();
    RAN[w].fetch_add(1, Ordering::Relaxed);
    (w + 1) as u64
}

extern "C" fn main_task(_: *mut TaskCtx, _: u64, _: *const u64) -> u64 {
    task::enter_runtime();
    let start = entry(child);
    let ids: Vec<task::TaskId> =
        (0..64).map(|_| task::spawn_typed(start, None, "child".into(), false)).collect();
    task::leave_runtime();
    let mut mask = 0u64;
    for &id in &ids {
        task::enter_runtime();
        task::await_task(id, false);
        let exit = task::exit_of(id);
        task::leave_runtime();
        let Some(task::Exit::Returned(worker)) = exit else {
            panic!("child {id} did not return");
        };
        assert!(
            worker >= 1 && worker as usize <= task::workers(),
            "child ran on worker {}",
            worker - 1
        );
        mask |= 1 << (worker - 1);
    }
    mask.count_ones() as u64
}

/// Every task runs, and the workers share the load.
#[test]
fn tasks_spread_over_workers() {
    task::set_workers(4);
    match task::run_main(entry(main_task)) {
        task::Exit::Returned(used) => {
            let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
            if cores >= 2 {
                assert!(used >= 2, "64 tasks ran on {used} worker(s)");
            }
        }
        other => panic!("main failed: {other:?}"),
    }
    assert_eq!(RAN.iter().map(|c| c.load(Ordering::Relaxed)).sum::<u64>(), 64);
}
