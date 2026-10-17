//! Bounded native sweep helpers. The collector holds the runtime lock and has
//! stopped all allocating workers before calling `run`; it receives every
//! result before finalizing objects or resuming workers. Idle helpers hold no
//! managed pointers. Each helper only touches the job's own object list/pool.
use std::sync::{Mutex, mpsc};
use std::thread::{self, JoinHandle};

use super::{SweepResult, SweptList, sweep_list};

struct Job {
    worker: usize,
    start: usize,
    sticky: bool,
}

struct Helper {
    jobs: mpsc::SyncSender<Job>,
    results: mpsc::Receiver<SweepResult>,
    thread: JoinHandle<()>,
}

impl Helper {
    fn new(worker: usize) -> Self {
        // Only one job can be outstanding per helper; collections are serial.
        let (jobs, receive) = mpsc::sync_channel::<Job>(1);
        let (send, results) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name(format!("badger-gc-{worker}"))
            .spawn(move || {
                #[cfg(feature = "gc-profile")]
                crate::profile::start_thread();
                while let Ok(job) = receive.recv() {
                    debug_assert_eq!(job.worker, worker);
                    let result = SweepResult(sweep_list(job.worker, job.start, job.sticky));
                    if send.send(result).is_err() {
                        break;
                    }
                }
            })
            .expect("GC sweep helper");
        Self { jobs, results, thread }
    }
}

struct Pool {
    helpers: Vec<Option<Helper>>,
}

impl Pool {
    fn new() -> Self {
        Self { helpers: (0..crate::task::MAX_WORKERS).map(|_| None).collect() }
    }

    fn run(&mut self, lists: &[usize], starts: &[usize], sticky: bool) -> (Vec<SweptList>, usize) {
        let mut created = 0;
        for (&worker, &start) in lists[1..].iter().zip(&starts[1..]) {
            let helper = self.helpers[worker].get_or_insert_with(|| {
                created += 1;
                Helper::new(worker)
            });
            helper.jobs.send(Job { worker, start, sticky }).expect("GC sweep helper stopped");
        }
        let mut results = Vec::with_capacity(lists.len());
        results.push(sweep_list(lists[0], starts[0], sticky));
        // Receive in list order, then finalize only after ALL jobs finish.
        for &worker in &lists[1..] {
            results.push(
                self.helpers[worker].as_ref().unwrap().results.recv().expect("GC sweep helper failed").0,
            );
        }
        (results, created)
    }

    fn shutdown(self) {
        for helper in self.helpers.into_iter().flatten() {
            drop(helper.jobs);
            helper.thread.join().expect("GC sweep helper failed");
        }
    }
}

struct Manager {
    closed: bool,
    pool: Option<Pool>,
}

static MANAGER: Mutex<Manager> = Mutex::new(Manager { closed: false, pool: None });

pub(super) fn run(lists: &[usize], starts: &[usize], sticky: bool) -> Option<(Vec<SweptList>, usize)> {
    debug_assert!(!lists.is_empty() && lists.len() == starts.len());
    // No helper acquires this mutex or the runtime lock. It also excludes
    // shutdown until every outstanding result has been received.
    let mut manager = MANAGER.lock().unwrap();
    if manager.closed {
        return None;
    }
    Some(manager.pool.get_or_insert_with(Pool::new).run(lists, starts, sticky))
}

pub(super) fn shutdown() {
    let pool = {
        let mut manager = MANAGER.lock().unwrap();
        manager.closed = true;
        manager.pool.take()
    };
    if let Some(pool) = pool {
        pool.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desc::{Kind, TypeDesc};
    use crate::gc::{self, HEADER, MARK};

    static FINALIZED: Mutex<Vec<(thread::ThreadId, u64)>> = Mutex::new(Vec::new());

    fn record(p: *mut gc::Header) {
        FINALIZED.lock().unwrap().push((thread::current().id(), unsafe { *(*p).payload().cast::<u64>() }));
    }

    #[test]
    fn helpers_reuse_join_and_preserve_prefix_marks_and_finalizer_order() {
        // Raw object lists/pools belong to this isolated child, with no task
        // workers or other runtime unit tests accessing the global heap.
        if std::env::var_os("BADGER_SWEEP_TEST_CHILD").is_none() {
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            for (key, _) in std::env::vars_os() {
                if key.to_string_lossy().starts_with("BADGER_") {
                    command.env_remove(key);
                }
            }
            let output = command
                .args([
                    "--exact",
                    "gc::sweep::tests::helpers_reuse_join_and_preserve_prefix_marks_and_finalizer_order",
                ])
                .env("BADGER_SWEEP_TEST_CHILD", "1")
                .env("BADGER_GC_SWEEP_REUSE", "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let leaf = TypeDesc::new("sweep-leaf", Kind::Int { bits: 64, signed: false }).leak();
        let opaque = TypeDesc::new(
            "sweep-finalizer",
            Kind::Opaque { trace: None, finalize: Some(record), args: vec![] },
        )
        .leak();
        let mut expected = vec![];
        let coordinator = thread::current().id();
        let lists = [0, 3, 7];
        for w in lists {
            assert!(gc::objects(w).is_empty());
            for i in 0..(50 + w * 17) {
                let p = gc::alloc_cell(w, 8);
                assert!(!p.is_null());
                let value = ((w as u64) << 32) | i as u64;
                unsafe {
                    (*p).desc = if i % 3 == 1 { opaque } else { leaf };
                    (*p).meta = (8 << 32) | if i % 3 == 0 { MARK } else { 0 };
                    *(*p).payload().cast::<u64>() = value;
                }
                gc::objects(w).push(p);
                if i >= 5 && i % 3 == 1 {
                    expected.push((coordinator, value));
                }
            }
        }
        let (results, created) = run(&lists, &[5, 5, 5], true).unwrap();
        assert_eq!(created, 2);
        let bytes = gc::finish_sweep(&lists, results);
        let mut survivors = 0;
        for w in lists {
            let young = (5..(50 + w * 17)).filter(|i| i % 3 == 0).count();
            survivors += young;
            assert_eq!(gc::objects(w).len(), 5 + young);
            for &p in &gc::objects(w)[5..] {
                assert_ne!(unsafe { (*p).meta } & MARK, 0);
            }
        }
        assert_eq!(bytes, survivors * (HEADER + 8));
        assert_eq!(*FINALIZED.lock().unwrap(), expected);

        // A full sweep includes the old prefix. Keep one marked leaf per list;
        // sticky=false must clear its mark. Other old objects are reclaimed.
        for w in lists {
            for (i, &p) in gc::objects(w).iter().enumerate() {
                unsafe {
                    (*p).meta = (8 << 32) | if i == 0 { MARK } else { 0 };
                }
            }
            for i in [1, 4] {
                expected.push((coordinator, ((w as u64) << 32) | i));
            }
        }
        let (results, created) = run(&lists, &[0, 0, 0], false).unwrap();
        assert_eq!(created, 0, "native threads must be reused");
        assert_eq!(gc::finish_sweep(&lists, results), 3 * (HEADER + 8));
        assert_eq!(*FINALIZED.lock().unwrap(), expected);
        for w in lists {
            assert_eq!(gc::objects(w).len(), 1);
            assert_eq!(unsafe { (*gc::objects(w)[0]).meta } & MARK, 0);
        }
        let (results, created) = run(&lists, &[0, 0, 0], false).unwrap();
        assert_eq!(created, 0);
        assert_eq!(gc::finish_sweep(&lists, results), 0);
        assert!(lists.iter().all(|&w| gc::objects(w).is_empty()));
        shutdown();
        assert!(run(&lists, &[0, 0, 0], false).is_none(), "shutdown cannot reopen the cache");
        assert_eq!(
            gc::sweep_parallel(&lists, &[0, 0, 0], false),
            (0, 2),
            "closed cache falls back to joined scoped threads"
        );
        shutdown(); // idempotent, all helper threads already joined
    }
}
