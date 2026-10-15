//! Stackful tasks, the scheduler (one worker, or several with
//! `[runtime] workers`), cancellation, defects, scope cleanup, timers, and
//! blocking-pool integration.
//!
//! With several workers, each OS thread runs tasks from a shared ready queue
//! and tasks migrate between workers at suspension points. Runtime code runs
//! under one runtime lock; compiled code runs in parallel without it.
//! Compiled code brackets every runtime call with [`lang_rt_enter`] and
//! [`lang_rt_leave`]; the runtime releases the lock around calls back into
//! compiled code. Collections stop the world: they wait until no worker is
//! executing compiled code (each reaches its next runtime call or poll).
//!
//! Every language task runs on its own native stack (a `corosensei`
//! coroutine). Compiled code keeps managed pointers in shadow-stack frames
//! linked from the task's [`TaskCtx`], so suspended stacks are scanned
//! precisely. A task defect (panic, checked overflow, bounds failure) or a
//! cancellation abandons the task's stack without unwinding through compiled
//! frames; the task's registered cleanups then run on a fresh stack.

use std::cell::{Cell, UnsafeCell};
use std::collections::{BinaryHeap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use corosensei::stack::DefaultStack;
use corosensei::{Coroutine, CoroutineResult, Yielder};
use rustc_hash::FxHashMap;

use crate::desc::TypeDesc;

pub type TaskId = u32;

/// Per-task context passed to all compiled code. Field offsets are part of
/// the code generator's ABI.
#[repr(C)]
pub struct TaskCtx {
    /// Top shadow-stack frame (offset 0).
    pub shadow: *mut Frame,
    /// Remaining scheduling budget; compiled polls decrement it (offset 8).
    pub budget: i64,
    /// Bit 0: cancellation requested. Bit 1: failure pending (offset 16).
    pub flags: u64,
    /// Owning task (offset 24).
    pub task: u64,
    /// Lowest stack address compiled code may grow to before reporting a
    /// stack overflow defect (offset 32; 0 = unchecked).
    pub stack_limit: u64,
}

pub const CTX_STACK_LIMIT: i32 = 32;

/// Headroom kept below the limit for the runtime's own overflow handling.
const STACK_RESERVE: usize = 64 << 10;

pub const CTX_SHADOW: i32 = 0;
pub const CTX_BUDGET: i32 = 8;
pub const BUDGET: i64 = 10_000;
const FLAG_CANCEL: u64 = 1;
const FLAG_FAIL: u64 = 2;

/// Shadow-stack frame: `prev`, `len`, then `len` pointer slots.
#[repr(C)]
pub struct Frame {
    pub prev: *mut Frame,
    pub len: u64,
}

#[derive(Debug, Clone)]
pub struct PanicInfo {
    pub message: String,
    pub location: String,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Exit {
    Returned(u64),
    Failed(PanicInfo),
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    /// Budget exhausted or explicit yield: requeue.
    Yield,
    /// Waiting for a wakeup.
    Park,
    /// The task's stack is abandoned (defect or cancellation).
    Abandon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Ready,
    Running,
    Parked,
    Cleaning,
    Done,
}

/// Why a parked task was woken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wake {
    Normal,
    Timeout,
    Cancelled,
}

pub enum Cleanup {
    /// Language closure `fn() -> Unit` (or `-> Result<Unit, E>`).
    Closure(u64),
    Native(Box<dyn FnOnce(*mut TaskCtx)>),
}

type Co = Coroutine<(), Signal, (), DefaultStack>;

pub struct Task {
    pub id: TaskId,
    pub ctx: Box<TaskCtx>,
    co: Option<Co>,
    yielder: *const Yielder<(), Signal>,
    state: State,
    pub exit: Option<Exit>,
    /// Closure run by this task (rooted until the task is removed).
    pub entry: u64,
    /// Arguments passed to the entry closure (managed pointers are rooted).
    pub entry_args: Vec<(u64, bool)>,
    pub joiners: Vec<TaskId>,
    pub cleanups: Vec<(u64, Cleanup)>,
    /// Scoped iterators with a registered closer: (iterator, cleanup handle).
    pub iter_closers: Vec<(u64, u64)>,
    pub scope: Option<u64>,
    /// Managed values held by the runtime on behalf of this task.
    pub roots: Vec<u64>,
    /// Temporary roots of runtime code running for this task (`gc::Roots`).
    pub temp_roots: Vec<u64>,
    /// Channels this task is registered with (waiting, watching, leasing).
    pub chan_regs: Vec<u64>,
    /// Value delivered by the waker (e.g. a received message).
    pub delivered: u64,
    pub delivered_is_ptr: bool,
    /// Status word set by the waker (non-pointer).
    pub status: u64,
    /// Whether this task's result word is a managed pointer.
    pub result_is_ptr: bool,
    pub wake: Wake,
    /// Primary failure while cleanups run.
    pending_exit: Option<Exit>,
    /// Running registered cleanups on a fresh stack.
    in_cleanup: bool,
    /// Hooks run when the task schedules out (batch publication).
    pub on_deschedule: Vec<u64>,
    /// What a parked task waits for (inspection).
    pub wait: &'static str,
    /// Why cancellation was requested (see [`CancelReason`]).
    pub cancel_reason: CancelReason,
    /// Sequence number of the timer this task currently sleeps on; timers
    /// of abandoned sleeps must not wake later waits.
    sleep_seq: u64,
    pub name: String,
    /// Live code version current when the task started.
    pub code_version: u32,
    /// Location of the latest call to a test helper (a static string).
    pub caller_loc: u64,
    /// A wakeup that arrived while the task was still running (e.g. from
    /// publishing its own staged batch on the way to parking): the next
    /// park returns at once instead of losing it.
    pub early_wake: Option<Wake>,
    /// A parked selection's cases, so a rendezvous partner can commit one
    /// of them on its behalf (`(channel, index, is_send, value)`), and the
    /// committed case once one is.
    pub select_offer: Vec<(u64, usize, bool, u64)>,
    pub select_done: Option<usize>,
    /// Publishing staged batches on the way to parking.
    pub publishing: bool,
    /// Pipeline stages the task is inside, outermost first.
    pub stages: Vec<String>,
}

struct Timer {
    at: Instant,
    seq: u64,
    task: TaskId,
}

impl PartialEq for Timer {
    fn eq(&self, o: &Self) -> bool {
        self.at == o.at && self.seq == o.seq
    }
}
impl Eq for Timer {}
impl PartialOrd for Timer {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Timer {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        o.at.cmp(&self.at).then(o.seq.cmp(&self.seq))
    }
}

/// Event delivered from another OS thread (blocking pool, sockets, signals).
pub enum External {
    /// Wake `task`, delivering a boxed result to it.
    Complete { task: TaskId, value: Box<dyn std::any::Any + Send> },
    /// Run a callback on the worker (e.g. admit an HTTP request).
    Callback(Box<dyn FnOnce() + Send>),
    /// Run a callback on a scheduler stack between task steps, never on a
    /// task's stack (compilation for live reload and the remote REPL).
    Scheduler(Box<dyn FnOnce() + Send>),
}

pub struct ExternalQueue {
    pub queue: Mutex<VecDeque<External>>,
    pub cond: Condvar,
}

pub struct Runtime {
    tasks: FxHashMap<TaskId, Box<Task>>,
    ready: VecDeque<TaskId>,
    next_id: TaskId,
    timers: BinaryHeap<Timer>,
    /// Sleeps under a controlled clock: (deadline in controlled nanos,
    /// timer sequence, task).
    virtual_timers: Vec<(i64, u64, TaskId)>,
    timer_seq: u64,
    pub external: Arc<ExternalQueue>,
    /// Outstanding operations that will produce an external event.
    pub outstanding: usize,
    /// Long-lived sources of external events (servers, signal waiters).
    pub listeners: usize,
    completions: FxHashMap<TaskId, Box<dyn std::any::Any + Send>>,
    pub main: Option<TaskId>,
    pub stack_size: usize,
    /// Called when a task schedules out: publish its staged batches.
    pub deschedule_hook: Option<fn(TaskId, &[u64])>,
    /// Called at safe points: publish staging older than its latency target.
    pub safepoint_hook: Option<fn(TaskId, &[u64])>,
    /// Scheduler callbacks received while a task was running.
    scheduler_work: Vec<Box<dyn FnOnce() + Send>>,
    /// Entry of a new program version replacing main once it has wound down
    /// (controlled reconstruction).
    restart: Option<u64>,
}

struct RtCell(UnsafeCell<Option<Runtime>>);
unsafe impl Sync for RtCell {}
static RT: RtCell = RtCell(UnsafeCell::new(None));

#[allow(clippy::mut_from_ref)]
pub fn rt() -> &'static mut Runtime {
    unsafe {
        let r = &mut *RT.0.get();
        if r.is_none() {
            *r = Some(Runtime {
                tasks: FxHashMap::default(),
                ready: VecDeque::new(),
                next_id: 1,
                timers: BinaryHeap::new(),
                virtual_timers: vec![],
                timer_seq: 0,
                external: Arc::new(ExternalQueue {
                    queue: Mutex::new(VecDeque::new()),
                    cond: Condvar::new(),
                }),
                outstanding: 0,
                listeners: 0,
                completions: FxHashMap::default(),
                main: None,
                stack_size: STACK_SIZE.load(Ordering::SeqCst),
                deschedule_hook: None,
                safepoint_hook: None,
                scheduler_work: vec![],
                restart: None,
            });
        }
        r.as_mut().unwrap()
    }
}

/// Whether a task is parked (waiting to be woken).
pub fn is_parked(id: TaskId) -> bool {
    task(id).is_some_and(|t| t.state == State::Parked)
}

pub fn task(id: TaskId) -> Option<&'static mut Task> {
    rt().tasks.get_mut(&id).map(|b| unsafe { &mut *(b.as_mut() as *mut Task) })
}

// ---------------------------------------------------------------- workers

thread_local! {
    /// Task running on this worker thread (0 = none) and the worker's index.
    static CURRENT: Cell<TaskId> = const { Cell::new(0) };
    /// The running task's (boxed, address-stable) entry.
    static CURRENT_TASK: Cell<*mut Task> = const { Cell::new(std::ptr::null_mut()) };
    static WORKER: Cell<usize> = const { Cell::new(0) };
}

// Tasks migrate between threads at suspension points, so thread-local state
// is only read through these out-of-line accessors (never cached across a
// suspension inside one frame).
#[inline(never)]
fn current_raw() -> TaskId {
    CURRENT.with(|c| c.get())
}

#[inline(never)]
fn set_current(id: TaskId) {
    CURRENT.with(|c| c.set(id));
    let p = if id == 0 {
        std::ptr::null_mut()
    } else {
        task(id).map_or(std::ptr::null_mut(), |t| t as *mut Task)
    };
    CURRENT_TASK.with(|c| c.set(p));
}

/// Index of the worker thread running the caller (0 with one worker).
#[inline(never)]
pub fn worker_index() -> usize {
    WORKER.with(|w| w.get())
}

/// Number of worker threads (1 unless configured).
static WORKERS: AtomicUsize = AtomicUsize::new(1);

pub fn set_workers(n: usize) {
    WORKERS.store(n.max(1), Ordering::SeqCst);
}

pub fn workers() -> usize {
    WORKERS.load(Ordering::Relaxed)
}

pub fn multi() -> bool {
    workers() > 1
}

/// The runtime lock (multi-worker mode). Not bound to a thread: a task that
/// parks under the lock may be resumed, still logically holding it, by
/// another worker. Uncontended acquisition is one compare-and-swap;
/// contended waiters spin briefly, then park.
static LOCKED: AtomicBool = AtomicBool::new(false);
static WAITERS: AtomicUsize = AtomicUsize::new(0);
static PARK: Mutex<()> = Mutex::new(());
static PARK_CV: Condvar = Condvar::new();

/// The worker holding the runtime lock (index + 1; 0 when free).
static OWNER: AtomicUsize = AtomicUsize::new(0);

// ---------------------------------------------------------------- statistics

/// `LANG_RUNTIME_STATS=1`: per-worker counters of lock acquisition and
/// scheduling, summed and printed on stderr when the program ends.
pub fn stats_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("LANG_RUNTIME_STATS").is_some_and(|v| v == "1"))
}

#[repr(align(128))]
pub struct WorkerStats {
    /// Lock acquired at once / after spinning / after parking.
    pub lock_fast: AtomicU64,
    pub lock_spin: AtomicU64,
    pub lock_park: AtomicU64,
    /// Nanoseconds spent parked waiting for the lock.
    pub lock_park_ns: AtomicU64,
    /// Task resumptions, idle waits, and broadcasts waking idle workers.
    pub runs: AtomicU64,
    pub idle_waits: AtomicU64,
    pub notifies: AtomicU64,
}

pub static STATS: [WorkerStats; MAX_WORKERS] = [const {
    WorkerStats {
        lock_fast: AtomicU64::new(0),
        lock_spin: AtomicU64::new(0),
        lock_park: AtomicU64::new(0),
        lock_park_ns: AtomicU64::new(0),
        runs: AtomicU64::new(0),
        idle_waits: AtomicU64::new(0),
        notifies: AtomicU64::new(0),
    }
}; MAX_WORKERS];

/// Count an event for this worker (when statistics are on).
pub fn stat(f: impl Fn(&WorkerStats) -> &AtomicU64, n: u64) {
    if stats_on() {
        f(&STATS[worker_index()]).fetch_add(n, Ordering::Relaxed);
    }
}

pub fn print_stats() {
    if !stats_on() {
        return;
    }
    let sum = |f: &dyn Fn(&WorkerStats) -> &AtomicU64| -> u64 {
        STATS.iter().map(|w| f(w).load(Ordering::Relaxed)).sum()
    };
    eprintln!(
        "[runtime stats] lock: {} fast, {} after spinning, {} after parking ({:.3} ms parked); tasks run: {}; idle waits: {}; idle wakeups: {}",
        sum(&|w| &w.lock_fast),
        sum(&|w| &w.lock_spin),
        sum(&|w| &w.lock_park),
        sum(&|w| &w.lock_park_ns) as f64 / 1e6,
        sum(&|w| &w.runs),
        sum(&|w| &w.idle_waits),
        sum(&|w| &w.notifies),
    );
}

fn try_lock() -> bool {
    let ok = LOCKED.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok();
    if ok {
        OWNER.store(worker_index() + 1, Ordering::Relaxed);
    }
    ok
}

/// Whether this thread may use runtime state freely: one worker, or this
/// worker holds the runtime lock (generated code calling an unlocked
/// runtime entry, such as allocation, does not).
pub fn holds_lock() -> bool {
    !multi() || OWNER.load(Ordering::Relaxed) == worker_index() + 1
}

fn lock() {
    if try_lock() {
        stat(|w| &w.lock_fast, 1);
        return;
    }
    for _ in 0..200 {
        std::hint::spin_loop();
        if !LOCKED.load(Ordering::Relaxed) && try_lock() {
            stat(|w| &w.lock_spin, 1);
            return;
        }
    }
    let started = stats_on().then(Instant::now);
    let mut g = PARK.lock().unwrap();
    WAITERS.fetch_add(1, Ordering::SeqCst);
    while !try_lock() {
        g = PARK_CV.wait_timeout(g, Duration::from_millis(1)).unwrap().0;
    }
    WAITERS.fetch_sub(1, Ordering::SeqCst);
    if let Some(t) = started {
        stat(|w| &w.lock_park, 1);
        stat(|w| &w.lock_park_ns, t.elapsed().as_nanos() as u64);
    }
}

fn unlock() {
    crate::channel::release_claims();
    OWNER.store(0, Ordering::Relaxed);
    LOCKED.store(false, Ordering::Release);
    if WAITERS.load(Ordering::SeqCst) > 0 {
        let _g = PARK.lock().unwrap();
        PARK_CV.notify_one();
    }
}

/// Workers currently executing compiled code (outside the runtime lock).
static IN_COMPILED: AtomicIsize = AtomicIsize::new(0);
/// Set while a collection waits for every worker to reach a safe point.
static STOP: AtomicBool = AtomicBool::new(false);
/// Most worker threads (per-worker tables are sized by it).
pub const MAX_WORKERS: usize = 64;
/// A managed pointer compiled code just returned to runtime code, per
/// worker: rooted while the worker waits for the runtime lock, when a
/// collection on another worker may run.
static RETURNED: [AtomicU64; MAX_WORKERS] = [const { AtomicU64::new(0) }; MAX_WORKERS];
/// Bumped on every wake so idle workers do not miss new work.
static READY_SEQ: AtomicU64 = AtomicU64::new(0);
static IDLE: AtomicUsize = AtomicUsize::new(0);
static SHUTDOWN: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------- chaos

/// `LANG_CHAOS=<seed>`: perturb scheduling to expose races. Tasks get
/// random budgets (preemption at arbitrary polls), workers pick random
/// ready tasks, threads yield or briefly sleep around runtime entry and
/// exit, and collections run at random allocations. With one worker the
/// perturbation is reproducible from the seed.
pub fn chaos() -> Option<u64> {
    static CHAOS: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
    *CHAOS.get_or_init(|| std::env::var("LANG_CHAOS").ok().and_then(|v| v.parse().ok()))
}

thread_local! {
    static CHAOS_STATE: Cell<u64> = const { Cell::new(0) };
}

/// The next chaos random number (splitmix64 per thread, seeded from the
/// chaos seed and the worker index).
pub fn chaos_next() -> u64 {
    let seed = chaos().unwrap_or(0);
    CHAOS_STATE.with(|c| {
        let mut x = c.get();
        if x == 0 {
            x = seed ^ (worker_index() as u64 + 1).wrapping_mul(0xD1B5_4A32_D192_ED03) | 1;
        }
        x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        c.set(x);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    })
}

/// A task's budget until its next yield: random under chaos (often tiny).
fn task_budget() -> i64 {
    if chaos().is_none() {
        return BUDGET;
    }
    let r = chaos_next();
    if r & 1 == 0 { 1 + ((r >> 8) % 64) as i64 } else { 1 + ((r >> 8) % BUDGET as u64) as i64 }
}

/// Under chaos with several workers: sometimes give up the CPU, rarely
/// sleep briefly, to widen windows between threads.
fn chaos_jitter() {
    if chaos().is_none() || !multi() {
        return;
    }
    let r = chaos_next();
    if r.is_multiple_of(8) {
        std::thread::yield_now();
    } else if r.is_multiple_of(97) {
        std::thread::sleep(Duration::from_micros((r >> 16) % 200));
    }
}

/// Compiled code is about to call into the runtime (a safe point).
pub extern "C" fn lang_rt_enter(_ctx: *mut TaskCtx) {
    IN_COMPILED.fetch_sub(1, Ordering::SeqCst);
    chaos_jitter();
    lock();
}

/// Compiled code continues after a runtime call.
pub extern "C" fn lang_rt_leave(_ctx: *mut TaskCtx) {
    IN_COMPILED.fetch_add(1, Ordering::SeqCst);
    unlock();
    chaos_jitter();
}

/// A foreign call from compiled code: it cannot touch managed memory, so it
/// does not hold up a collection.
pub extern "C" fn lang_rt_safe_begin(_ctx: *mut TaskCtx) {
    IN_COMPILED.fetch_sub(1, Ordering::SeqCst);
}

pub extern "C" fn lang_rt_safe_end(_ctx: *mut TaskCtx) {
    // increments only under the lock, so a collection in progress (which
    // holds the lock) finishes first
    lock();
    IN_COMPILED.fetch_add(1, Ordering::SeqCst);
    unlock();
}

/// Runtime code calls compiled code (closures, display hooks).
pub fn leave_runtime() {
    if multi() {
        lang_rt_leave(std::ptr::null_mut());
    }
}

pub fn enter_runtime() {
    if multi() {
        lang_rt_enter(std::ptr::null_mut());
    }
}

/// [`enter_runtime`] after compiled code returned the managed pointer `w`:
/// it stays rooted until the runtime lock is held (the caller must root it
/// before allocating or releasing the lock).
pub fn enter_runtime_holding(w: u64) {
    if multi() {
        // the task may have moved to another worker while it ran
        let slot = &RETURNED[worker_index()];
        slot.store(w, Ordering::SeqCst);
        lang_rt_enter(std::ptr::null_mut());
        slot.store(0, Ordering::SeqCst);
    }
}

/// Stop the world for a collection (called under the runtime lock): every
/// other worker is idle, waiting for the lock, or reaches a safe point
/// soon: compiled code calls into the runtime (or takes a poll's slow path)
/// within one task budget of polls. Another thread's budget is never
/// written: the running task owns it.
pub fn stop_the_world() {
    if !multi() {
        return;
    }
    STOP.store(true, Ordering::SeqCst);
    while IN_COMPILED.load(Ordering::SeqCst) > 0 {
        std::hint::spin_loop();
        std::thread::yield_now();
    }
}

pub fn resume_the_world() {
    STOP.store(false, Ordering::SeqCst);
}

/// The running task (no table lookup: a task's entry stays in place while
/// it runs).
#[inline(never)]
pub fn current() -> Option<&'static mut Task> {
    let p = CURRENT_TASK.with(|c| c.get());
    if p.is_null() { None } else { Some(unsafe { &mut *p }) }
}

/// The running task's temporary-root stack (see `gc::Roots`).
pub fn current_temp_roots() -> Option<*mut Vec<u64>> {
    if current_raw() == 0 {
        return None;
    }
    current().map(|t| &mut t.temp_roots as *mut Vec<u64>)
}

pub fn current_id() -> TaskId {
    current_raw()
}

/// Visit every task-held root: shadow stacks, entries, results, runtime roots.
pub fn for_each_root(f: &mut dyn FnMut(u64)) {
    for slot in &RETURNED {
        let w = slot.load(Ordering::SeqCst);
        if w != 0 {
            f(w);
        }
    }
    let Some(r) = (unsafe { (*RT.0.get()).as_ref() }) else { return };
    for t in r.tasks.values() {
        f(t.entry);
        for (w, p) in &t.entry_args {
            if *p {
                f(*w);
            }
        }
        if t.delivered_is_ptr {
            f(t.delivered);
        }
        if t.result_is_ptr {
            if let Some(Exit::Returned(w)) = &t.exit {
                f(*w);
            }
            if let Some(Exit::Returned(w)) = &t.pending_exit {
                f(*w);
            }
        }
        for &w in &t.roots {
            f(w);
        }
        // runtime helpers root only pointer-typed words; the guard keeps a
        // stray scalar from being dereferenced
        for &w in &t.temp_roots {
            if w % 8 == 0 && w >= 4096 {
                f(w);
            }
        }
        for &w in &t.chan_regs {
            f(w);
        }
        for (_, c) in &t.cleanups {
            if let Cleanup::Closure(w) = c {
                f(*w);
            }
        }
        for &w in &t.on_deschedule {
            f(w);
        }
        let mut fr = t.ctx.shadow;
        while !fr.is_null() {
            unsafe {
                let slots = (fr as *mut u64).add(2);
                for i in 0..(*fr).len as usize {
                    f(*slots.add(i));
                }
                fr = (*fr).prev;
            }
        }
    }
}

/// Spawn a task that runs closure `entry` (`fn() -> T`); its result word is
/// stored in its exit.
pub fn spawn(entry: u64, scope: Option<u64>, name: String) -> TaskId {
    spawn_typed(entry, scope, name, true)
}

pub fn spawn_typed(entry: u64, scope: Option<u64>, name: String, result_is_ptr: bool) -> TaskId {
    let r = rt();
    let id = r.next_id;
    r.next_id += 1;
    let ctx = Box::new(TaskCtx {
        shadow: std::ptr::null_mut(),
        budget: task_budget(),
        flags: 0,
        task: id as u64,
        stack_limit: 0,
    });
    let t = Box::new(Task {
        id,
        ctx,
        co: None,
        yielder: std::ptr::null(),
        state: State::Ready,
        exit: None,
        entry,
        entry_args: vec![],
        joiners: vec![],
        cleanups: vec![],
        iter_closers: vec![],
        scope,
        roots: vec![],
        temp_roots: vec![],
        chan_regs: vec![],
        delivered: 0,
        delivered_is_ptr: false,
        wait: "",
        cancel_reason: CancelReason::Requested,
        sleep_seq: 0,
        status: 0,
        result_is_ptr: true,
        wake: Wake::Normal,
        pending_exit: None,
        in_cleanup: false,
        on_deschedule: vec![],
        name,
        code_version: crate::live::version(),
        caller_loc: 0,
        early_wake: None,
        select_offer: vec![],
        select_done: None,
        publishing: false,
        stages: vec![],
    });
    r.tasks.insert(id, t);
    let co = make_coroutine(id, Body::Entry);
    task(id).unwrap().co = Some(co);
    task(id).unwrap().result_is_ptr = result_is_ptr;
    r.ready.push_back(id);
    notify_ready();
    id
}

enum Body {
    Entry,
    Cleanup,
}

/// Stacks of finished tasks, reused by later spawns (mapping a fresh stack
/// dominates the cost of a short task).
static mut STACK_POOL: Vec<DefaultStack> = Vec::new();
const STACK_POOL_MAX: usize = 256;

fn make_coroutine(id: TaskId, body: Body) -> Co {
    #[allow(static_mut_refs)]
    let pooled = unsafe { STACK_POOL.pop() };
    let stack = match pooled {
        Some(s) => s,
        None => DefaultStack::new(rt().stack_size).expect("task stack"),
    };
    use corosensei::stack::Stack;
    let limit = stack.limit().get() as u64 + STACK_RESERVE as u64;
    Coroutine::with_stack(stack, move |y: &Yielder<(), Signal>, ()| {
        let t = task(id).unwrap();
        t.yielder = y as *const _;
        t.ctx.stack_limit = limit;
        match body {
            Body::Entry => {
                let ctx: *mut TaskCtx = &mut *t.ctx;
                let args: Vec<u64> = t.entry_args.iter().map(|a| a.0).collect();
                let r = unsafe { crate::call_closure_returning(ctx, t.entry, &args, t.result_is_ptr) };
                let t = task(id).unwrap();
                t.exit = Some(Exit::Returned(r));
            }
            Body::Cleanup => run_cleanups(id),
        }
    })
}

fn run_cleanups(id: TaskId) {
    loop {
        let t = task(id).unwrap();
        let Some((_, c)) = t.cleanups.pop() else { break };
        let ctx: *mut TaskCtx = &mut *t.ctx;
        match c {
            Cleanup::Closure(w) => {
                t.roots.push(w);
                unsafe { crate::call_closure(ctx, w, &[]) };
                let t = task(id).unwrap();
                t.roots.pop();
            }
            Cleanup::Native(f) => f(ctx),
        }
    }
}

/// Suspend the current task with `sig`. Returns when resumed.
fn suspend(sig: Signal) {
    let t = current().expect("suspend outside a task");
    let y = t.yielder;
    unsafe { (*y).suspend(sig) };
}

/// Park the current task until [`wake`]. Cancellation (and a pending child
/// failure) interrupts the wait unless the task is running cleanups.
/// Park, recording what the task waits for (shown by inspection).
pub fn park_for(reason: &'static str) -> Wake {
    if let Some(t) = current() {
        t.wait = reason;
    }
    let w = park();
    if let Some(t) = current() {
        t.wait = "";
    }
    w
}

pub fn park() -> Wake {
    publish_current();
    // cancellation requested while the task was running (on another
    // worker) woke nothing: honor it before waiting, or nothing will
    check_interrupts();
    if let Some(w) = current().unwrap().early_wake.take() {
        check_interrupts();
        return w;
    }
    {
        let t = current().unwrap();
        t.state = State::Parked;
        t.wake = Wake::Normal;
    }
    suspend(Signal::Park);
    check_interrupts();
    current().unwrap().wake
}

/// Park without honoring cancellation (cleanup and settle waits).
pub fn park_shielded() -> Wake {
    publish_current();
    if let Some(w) = current().unwrap().early_wake.take() {
        return w;
    }
    {
        let t = current().unwrap();
        t.state = State::Parked;
        t.wake = Wake::Normal;
    }
    suspend(Signal::Park);
    current().unwrap().wake
}

fn publish_current() {
    let Some(t) = current() else { return };
    if !t.on_deschedule.is_empty()
        && let Some(h) = rt().deschedule_hook
    {
        let objs = t.on_deschedule.clone();
        t.publishing = true;
        h(t.id, &objs);
        if let Some(t) = current() {
            t.publishing = false;
        }
    }
}

/// Abandon the current task if cancellation or a child failure is pending.
pub fn check_interrupts() {
    let t = current().unwrap();
    if t.state == State::Cleaning {
        return;
    }
    if t.ctx.flags & FLAG_FAIL != 0 {
        abandon(None);
    }
    if t.ctx.flags & FLAG_CANCEL != 0 {
        abandon(Some(Exit::Cancelled));
    }
}

pub fn wake(id: TaskId, reason: Wake) {
    if let Some(t) = task(id) {
        if t.state == State::Parked {
            t.state = if t.in_cleanup { State::Cleaning } else { State::Ready };
            t.wake = reason;
            rt().ready.push_back(id);
            notify_ready();
        } else if t.publishing {
            // the running task woke itself on its way to parking
            t.early_wake = Some(reason);
        }
    }
}

/// Let idle workers see newly ready tasks.
fn notify_ready() {
    if multi() && IDLE.load(Ordering::SeqCst) > 0 {
        stat(|w| &w.notifies, 1);
        READY_SEQ.fetch_add(1, Ordering::SeqCst);
        let q = rt().external.clone();
        let _g = q.queue.lock().unwrap();
        q.cond.notify_all();
    }
}

/// Wake a parked task, delivering a value (`is_ptr`: managed pointer).
pub fn wake_with(id: TaskId, value: u64, is_ptr: bool, status: u64) {
    if let Some(t) = task(id) {
        t.delivered = value;
        t.delivered_is_ptr = is_ptr;
        t.status = status;
    }
    wake(id, Wake::Normal);
}

/// Take (and clear) the value delivered to the current task.
pub fn take_delivered() -> (u64, u64) {
    let t = current().unwrap();
    let r = (t.delivered, t.status);
    t.delivered = 0;
    t.delivered_is_ptr = false;
    t.status = 0;
    r
}

pub fn yield_now() {
    let t = current().unwrap();
    t.ctx.budget = task_budget();
    // the flush-latency target is checked at every safe point
    if !t.on_deschedule.is_empty()
        && let Some(h) = rt().safepoint_hook
    {
        let (id, objs) = (t.id, t.on_deschedule.clone());
        h(id, &objs);
    }
    // make due timers and completed external work runnable (fairness)
    harvest_events();
    if rt().ready.is_empty() {
        check_interrupts();
        return;
    }
    publish_current();
    suspend(Signal::Yield);
    check_interrupts();
}

/// Why a task was cancelled (the order matches `task.CancelReason`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelReason {
    Requested,
    Timeout,
    ScopeExited,
    SiblingFailed,
    SiblingError,
}

impl CancelReason {
    pub fn name(self) -> &'static str {
        match self {
            CancelReason::Requested => "requested",
            CancelReason::Timeout => "timeout",
            CancelReason::ScopeExited => "scope_exited",
            CancelReason::SiblingFailed => "sibling_failed",
            CancelReason::SiblingError => "sibling_error",
        }
    }
}

/// Request cancellation of a task. Not-yet-started tasks never run.
pub fn cancel(id: TaskId) {
    cancel_with(id, CancelReason::Requested)
}

pub fn cancel_with(id: TaskId, reason: CancelReason) {
    let Some(t) = task(id) else { return };
    if t.state == State::Done || t.state == State::Cleaning {
        return;
    }
    if t.ctx.flags & FLAG_CANCEL == 0 {
        t.cancel_reason = reason;
    }
    t.ctx.flags |= FLAG_CANCEL;
    // tasks waiting for it now wait under the cleanup grace period
    let joiners = t.joiners.clone();
    for j in joiners {
        if task(j).is_some_and(|x| x.state == State::Parked && x.wait == "join") {
            wake(j, Wake::Normal);
        }
    }
    if t.state == State::Parked {
        wake(id, Wake::Cancelled);
    }
}

/// Make another task fail at its next safe point (child defect propagation).
pub fn fail_pending(id: TaskId, info: PanicInfo) {
    let Some(t) = task(id) else { return };
    if t.state == State::Done || t.state == State::Cleaning || t.pending_exit.is_some() {
        return;
    }
    t.ctx.flags |= FLAG_FAIL;
    t.pending_exit = Some(Exit::Failed(info));
    if t.state == State::Parked {
        wake(id, Wake::Cancelled);
    }
}

/// Attach a note (a cleanup failure) to the current task's primary defect.
pub fn attach_note(note: String) {
    if let Some(t) = current()
        && let Some(Exit::Failed(p)) = &mut t.pending_exit
    {
        p.notes.push(note);
    }
}

/// Abandon the current task's stack (defect or cancellation). Never returns.
pub fn abandon(exit: Option<Exit>) -> ! {
    let t = current().unwrap();
    if let Some(e) = exit
        && t.pending_exit.is_none()
    {
        t.pending_exit = Some(e);
    }
    suspend(Signal::Abandon);
    unreachable!("abandoned task resumed");
}

/// The user-code location recorded by the last `lang_rt_caller_loc` (empty
/// when none was recorded).
pub fn caller_location() -> String {
    current()
        .map(|t| t.caller_loc)
        .filter(|l| *l != 0)
        .map(|l| crate::string::str_of(l).to_string())
        .unwrap_or_default()
}

pub fn panic_current(message: String, location: String) -> ! {
    // a defect raised by runtime code running without the lock (an unlocked
    // entry called by generated code) takes it, as a locked call would
    if !holds_lock() {
        enter_runtime();
    }
    let t = current().expect("panic outside of a task");
    if t.state == State::Cleaning {
        // a failing cleanup: attach to the primary failure and continue
        if let Some(Exit::Failed(p)) = &mut t.pending_exit
            && p.message != message
        {
            let at = if location.is_empty() { String::new() } else { format!(" at {location}") };
            p.notes.push(format!("cleanup failed: {message}{at}"));
        }
        suspend(Signal::Abandon);
        unreachable!();
    }
    abandon(Some(Exit::Failed(PanicInfo { message, location, notes: vec![] })))
}

/// Register a cleanup for the current task; returns a handle for removal.
pub fn push_cleanup(c: Cleanup) -> u64 {
    static mut NEXT: u64 = 1;
    let h = unsafe {
        NEXT += 1;
        NEXT
    };
    current().unwrap().cleanups.push((h, c));
    h
}

/// Register `close` to run when iterator `it` terminates (exhausted,
/// abandoned early by its consumer, or at task exit).
pub fn iter_on_close(it: u64, close: u64) {
    let h = push_cleanup(Cleanup::Closure(close));
    let t = current().unwrap();
    t.roots.push(it);
    t.iter_closers.push((it, h));
}

pub fn iter_has_close(it: u64) -> bool {
    current().is_some_and(|t| t.iter_closers.iter().any(|(w, _)| *w == it))
}

/// Run and forget the closer of `it`, if any (idempotent).
pub fn iter_close(it: u64) {
    let Some(t) = current() else { return };
    let Some(i) = t.iter_closers.iter().rposition(|(w, _)| *w == it) else { return };
    let (_, h) = t.iter_closers.remove(i);
    if let Some(i) = t.roots.iter().rposition(|w| *w == it) {
        t.roots.remove(i);
    }
    if let Some(Cleanup::Closure(w)) = remove_cleanup(h) {
        let t = current().unwrap();
        let ctx: *mut TaskCtx = &mut *t.ctx;
        t.roots.push(w);
        unsafe { crate::call_closure(ctx, w, &[]) };
        let t = current().unwrap();
        if let Some(i) = t.roots.iter().rposition(|x| *x == w) {
            t.roots.remove(i);
        }
    }
}

pub fn remove_cleanup(h: u64) -> Option<Cleanup> {
    let t = current().unwrap();
    let i = t.cleanups.iter().position(|(x, _)| *x == h)?;
    Some(t.cleanups.remove(i).1)
}

pub fn sleep(d: Duration) -> Wake {
    let r = rt();
    let id = current_id();
    // a controlled clock: the sleep ends when the clock is advanced past
    // its deadline (explicitly, or automatically once nothing else can run)
    let manual = *crate::intrinsics_os::MANUAL_CLOCK.lock().unwrap();
    if let Some(now) = manual {
        r.timer_seq += 1;
        r.virtual_timers.push((now + d.as_nanos() as i64, r.timer_seq, id));
        if let Some(t) = current() {
            t.sleep_seq = r.timer_seq;
        }
        let w = park_for("sleep (controlled clock)");
        if let Some(t) = current() {
            t.sleep_seq = 0;
        }
        return w;
    }
    r.timer_seq += 1;
    r.timers.push(Timer { at: Instant::now() + d, seq: r.timer_seq, task: id });
    if let Some(t) = current() {
        t.sleep_seq = r.timer_seq;
    }
    let w = park_for("sleep");
    if let Some(t) = current() {
        t.sleep_seq = 0;
    }
    w
}

/// Run `f` on a blocking-pool thread, parking the current task meanwhile.
pub fn run_blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    run_on(&BLOCKING_POOL, "blocking call", f)
}

/// Run `f` on the one dedicated pinned thread (foreign thread affinity),
/// parking the current task meanwhile.
pub fn run_pinned<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    run_on(&PINNED_POOL, "pinned foreign call", f)
}

type Job = Box<dyn FnOnce() + Send>;

/// A bounded pool of OS threads for work that may block.
pub struct ThreadPool {
    jobs: Mutex<VecDeque<Job>>,
    cond: Condvar,
    threads: AtomicUsize,
    idle: AtomicUsize,
    max: AtomicUsize,
    name: &'static str,
}

impl ThreadPool {
    const fn new(max: usize, name: &'static str) -> ThreadPool {
        ThreadPool {
            jobs: Mutex::new(VecDeque::new()),
            cond: Condvar::new(),
            threads: AtomicUsize::new(0),
            idle: AtomicUsize::new(0),
            max: AtomicUsize::new(max),
            name,
        }
    }

    fn submit(&'static self, job: Job) {
        let mut g = self.jobs.lock().unwrap();
        g.push_back(job);
        // `idle` counts threads waiting for jobs, updated under this lock:
        // a thread already woken for an earlier job still counts until it
        // takes one, so every unclaimed job beyond the waiting threads gets
        // a new thread (otherwise it would wait behind a job that blocks)
        if g.len() > self.idle.load(Ordering::SeqCst)
            && self.threads.load(Ordering::SeqCst) < self.max.load(Ordering::SeqCst)
        {
            self.threads.fetch_add(1, Ordering::SeqCst);
            let _ = std::thread::Builder::new().name(self.name.into()).spawn(move || self.work());
        }
        drop(g);
        self.cond.notify_one();
    }

    fn work(&'static self) {
        loop {
            let job = {
                let mut g = self.jobs.lock().unwrap();
                loop {
                    if let Some(j) = g.pop_front() {
                        break j;
                    }
                    self.idle.fetch_add(1, Ordering::SeqCst);
                    g = self.cond.wait(g).unwrap();
                    self.idle.fetch_sub(1, Ordering::SeqCst);
                }
            };
            job();
        }
    }
}

/// Blocking I/O and `extern "c blocking"` calls (`[runtime] blocking_threads`).
pub static BLOCKING_POOL: ThreadPool = ThreadPool::new(16, "lang-blocking");
/// `extern "c pinned"` calls: always the same OS thread.
pub static PINNED_POOL: ThreadPool = ThreadPool::new(1, "lang-pinned");

pub fn set_blocking_threads(n: usize) {
    BLOCKING_POOL.max.store(n.max(1), Ordering::SeqCst);
}

fn run_on<T: Send + 'static>(
    pool: &'static ThreadPool,
    wait: &'static str,
    f: impl FnOnce() -> T + Send + 'static,
) -> T {
    let id = current_id();
    let q = rt().external.clone();
    rt().outstanding += 1;
    pool.submit(Box::new(move || {
        let v = f();
        let mut g = q.queue.lock().unwrap();
        g.push_back(External::Complete { task: id, value: Box::new(v) });
        q.cond.notify_one();
    }));
    if let Some(t) = current() {
        t.wait = wait;
    }
    loop {
        park_shielded();
        if let Some(v) = rt().completions.remove(&id) {
            if let Some(t) = current() {
                t.wait = "";
            }
            return *v.downcast::<T>().expect("blocking result type");
        }
    }
}

/// Post an event from any thread.
/// The scheduler's external event queue (for threads that post callbacks).
pub fn external_queue() -> Arc<ExternalQueue> {
    rt().external.clone()
}

/// Keep the scheduler waiting for external events even when every task is
/// parked (a watcher or listener will post work later).
pub fn add_listener() {
    rt().listeners += 1;
}

pub fn post(q: &Arc<ExternalQueue>, e: External) {
    let mut g = q.queue.lock().unwrap();
    g.push_back(e);
    q.cond.notify_one();
}

pub fn exit_of(id: TaskId) -> Option<Exit> {
    task(id).and_then(|t| if t.state == State::Done { t.exit.clone() } else { None })
}

pub fn is_done(id: TaskId) -> bool {
    task(id).is_some_and(|t| t.state == State::Done)
}

/// Task stack size (`[runtime] task_stack_bytes`, default 8 MiB of address
/// space; pages are committed as they are touched).
static STACK_SIZE: AtomicUsize = AtomicUsize::new(8 << 20);

pub fn set_stack_size(bytes: usize) {
    STACK_SIZE.store(bytes.max(256 << 10), Ordering::SeqCst);
    rt().stack_size = bytes.max(256 << 10);
}

/// Compiled code ran past its task's stack limit.
pub fn stack_overflow() -> ! {
    panic_current("stack overflow: recursion too deep for the task stack".into(), String::new())
}

/// Grace period for cancelled tasks to finish their cleanup
/// (`[runtime] cleanup_grace_ms`, default 5s).
static GRACE_MS: AtomicU64 = AtomicU64::new(5000);

pub fn set_cleanup_grace(ms: u64) {
    GRACE_MS.store(ms, Ordering::SeqCst);
}

/// Wait for cancelled tasks to settle within the cleanup grace period. An
/// uncooperative task escalates: the waiting task fails with a defect while
/// the task keeps its resources (nothing it may still use is freed).
pub fn await_cancelled(ids: &[TaskId]) {
    let deadline = Instant::now() + Duration::from_millis(GRACE_MS.load(Ordering::SeqCst));
    loop {
        let pending: Vec<TaskId> =
            ids.iter().copied().filter(|id| task(*id).is_some_and(|t| t.state != State::Done)).collect();
        if pending.is_empty() {
            return;
        }
        let now = Instant::now();
        if now >= deadline {
            let names: Vec<String> = pending
                .iter()
                .map(|id| match task(*id) {
                    Some(t) if !t.wait.is_empty() => format!("task {id}, {:?} on {}", t.state, t.wait),
                    Some(t) => format!("task {id}, {:?}", t.state),
                    None => format!("task {id}"),
                })
                .collect();
            panic_current(
                format!(
                    "cleanup exceeded the grace period of {}ms ({} still running)",
                    GRACE_MS.load(Ordering::SeqCst),
                    names.join(", ")
                ),
                String::new(),
            );
        }
        let me = current_id();
        for id in &pending {
            if let Some(t) = task(*id)
                && !t.joiners.contains(&me)
            {
                t.joiners.push(me);
            }
        }
        // wake at the deadline as well
        let r = rt();
        r.timer_seq += 1;
        r.timers.push(Timer { at: deadline, seq: r.timer_seq, task: me });
        if let Some(t) = current() {
            t.sleep_seq = r.timer_seq;
        }
        park_shielded();
        if let Some(t) = current() {
            t.sleep_seq = 0;
        }
    }
}

/// Park until task `id` finishes (shielded variant for settle waits).
pub fn await_task(id: TaskId, shielded: bool) {
    loop {
        if is_done(id) || task(id).is_none() {
            return;
        }
        // waiting on a cancelled task is bounded by the cleanup grace period
        if task(id).is_some_and(|t| {
            t.ctx.flags & FLAG_CANCEL != 0
                || t.state == State::Cleaning
                    && t.pending_exit.as_ref().is_some_and(|e| matches!(e, Exit::Cancelled))
        }) {
            await_cancelled(&[id]);
            return;
        }
        let me = current_id();
        task(id).unwrap().joiners.push(me);
        if shielded {
            park_shielded()
        } else {
            park_for("join")
        };
    }
}

pub fn remove_task(id: TaskId) {
    rt().tasks.remove(&id);
}

/// Controlled reconstruction: cancel the running program (its scopes cancel
/// their tasks and cleanups halt resources in reverse order), then start
/// `entry` as the new main task. Live values are rebuilt by the new version,
/// never reinterpreted.
pub fn reconstruct(entry: u64) {
    rt().restart = Some(entry);
    if let Some(m) = rt().main {
        cancel(m);
    }
}

/// When main has finished and a reconstruction is pending, start the new
/// main; returns whether one was started.
fn restart_main() -> bool {
    let Some(entry) = rt().restart.take() else { return false };
    let main = spawn_main(entry);
    rt().main = Some(main);
    true
}

/// Controlled scheduling (single worker): a seeded choice among ready
/// tasks, a recording of the choices made, or a replay of recorded choices.
/// With a controlled clock and seeded randomness this makes interleavings
/// reproducible.
pub struct SchedControl {
    pub seed: Option<u64>,
    pub record: bool,
    pub decisions: Vec<u32>,
    pub replay: VecDeque<u32>,
}

pub static SCHED: Mutex<SchedControl> =
    Mutex::new(SchedControl { seed: None, record: false, decisions: Vec::new(), replay: VecDeque::new() });

fn sched_controlled() -> bool {
    let c = SCHED.lock().unwrap();
    c.seed.is_some() || c.record || !c.replay.is_empty()
}

/// Pick the next ready task: FIFO unless scheduling is controlled.
fn next_ready() -> Option<TaskId> {
    let r = rt();
    if r.ready.is_empty() {
        return None;
    }
    let mut c = SCHED.lock().unwrap();
    if c.seed.is_none() && c.replay.is_empty() && !c.record {
        drop(c);
        return chaos_pop();
    }
    let n = r.ready.len() as u64;
    let idx = if let Some(i) = c.replay.pop_front() {
        (i as u64).min(n - 1)
    } else if let Some(seed) = c.seed.as_mut() {
        // splitmix64
        *seed = seed.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = *seed;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        (z ^ (z >> 31)) % n
    } else {
        0
    };
    if c.record {
        c.decisions.push(idx as u32);
    }
    drop(c);
    r.ready.remove(idx as usize)
}

/// The next ready task: FIFO, or a random one under chaos.
fn chaos_pop() -> Option<TaskId> {
    let r = rt();
    if chaos().is_some() && r.ready.len() > 1 {
        let i = (chaos_next() % r.ready.len() as u64) as usize;
        return r.ready.remove(i);
    }
    r.ready.pop_front()
}

/// Arguments for the next main task (a pinned `List<String>`), if its
/// entry function takes them.
static MAIN_ARGS: Mutex<Option<u64>> = Mutex::new(None);

pub fn set_main_args(list: Option<u64>) {
    *MAIN_ARGS.lock().unwrap() = list;
}

fn spawn_main(entry: u64) -> TaskId {
    let id = spawn(entry, None, "main".into());
    if let Some(l) = *MAIN_ARGS.lock().unwrap()
        && let Some(t) = task(id)
    {
        t.entry_args = vec![(l, true)];
    }
    id
}

/// Run the scheduler until the main task finishes. Returns its exit.
pub fn run_main(entry: u64) -> Exit {
    if multi() {
        return run_main_multi(entry);
    }
    let mut main = spawn_main(entry);
    rt().main = Some(main);
    loop {
        if let Some(id) = next_ready() {
            run_one(id);
            run_scheduler_work();
            if is_done(main) {
                if restart_main() {
                    main = rt().main.unwrap();
                    continue;
                }
                break;
            }
            continue;
        }
        if std::env::var_os("LANG_DEBUG_SCHED").is_some() {
            for t in rt().tasks.values() {
                eprintln!(
                    "[sched] task {} {:?} cleanups={} name={}",
                    t.id,
                    t.state,
                    t.cleanups.len(),
                    t.name
                );
            }
            eprintln!("[sched] listeners={} outstanding={}", rt().listeners, rt().outstanding);
        }
        if !wait_events() {
            // every task is parked and nothing can wake them
            let msg = "deadlock: all tasks are blocked".to_string();
            if let Some(t) = task(main)
                && t.state == State::Parked
            {
                fail_pending(main, PanicInfo { message: msg, location: String::new(), notes: vec![] });
                continue;
            }
            eprintln!("fatal: {msg}");
            return Exit::Failed(PanicInfo {
                message: "deadlock".into(),
                location: String::new(),
                notes: vec![],
            });
        }
    }
    task(main).unwrap().exit.clone().unwrap()
}

/// Several workers: this thread is worker 0; the others are spawned here.
/// Worker threads of the previous `run_main` (tests run one after another).
static WORKER_THREADS: Mutex<Vec<std::thread::JoinHandle<()>>> = Mutex::new(Vec::new());

fn run_main_multi(entry: u64) -> Exit {
    // the previous run's workers finish before a new run starts
    for h in std::mem::take(&mut *WORKER_THREADS.lock().unwrap()) {
        let _ = h.join();
    }
    SHUTDOWN.store(false, Ordering::SeqCst);
    lock();
    let main = spawn_main(entry);
    rt().main = Some(main);
    let n = workers().min(MAX_WORKERS);
    for w in 1..n {
        let h = std::thread::Builder::new()
            .name(format!("lang-worker-{w}"))
            .spawn(move || {
                WORKER.with(|c| c.set(w));
                lock();
                worker_loop(n);
                unlock();
            })
            .expect("worker thread");
        WORKER_THREADS.lock().unwrap().push(h);
    }
    let _ = main;
    let exit = worker_loop(n);
    // whichever worker saw main finish, its exit is recorded on the task
    let exit =
        exit.or_else(|| rt().main.and_then(task).and_then(|t| t.exit.clone())).unwrap_or(Exit::Cancelled);
    unlock();
    exit
}

/// Run ready tasks until the main task finishes (runtime lock held, except
/// while waiting idle). Worker 0 returns the main task's exit.
fn worker_loop(n: usize) -> Option<Exit> {
    loop {
        if SHUTDOWN.load(Ordering::SeqCst) {
            return None;
        }
        let main = rt().main.unwrap();
        if is_done(main) && restart_main() {
            continue;
        }
        if is_done(main) {
            SHUTDOWN.store(true, Ordering::SeqCst);
            let q = rt().external.clone();
            let _g = q.queue.lock().unwrap();
            q.cond.notify_all();
            return task(main).and_then(|t| t.exit.clone());
        }
        // controlled scheduling (tests) runs tasks on worker 0 only, in its
        // chosen order, so interleavings are reproducible
        let pick = if sched_controlled() {
            if worker_index() == 0 { next_ready() } else { None }
        } else {
            chaos_pop()
        };
        if let Some(id) = pick {
            run_one(id);
            run_scheduler_work();
            continue;
        }
        harvest_events();
        run_scheduler_work();
        // (a worker standing aside for controlled scheduling waits instead)
        let aside = worker_index() != 0 && sched_controlled();
        if !rt().ready.is_empty() && !aside {
            continue;
        }
        // (a worker standing aside for controlled scheduling waits instead)
        let aside = worker_index() != 0 && sched_controlled();
        if !rt().ready.is_empty() && !aside {
            continue;
        }
        let r = rt();
        let q = r.external.clone();
        let next_timer = r.timers.peek().map(|t| t.at);
        let quiet = next_timer.is_none()
            && r.outstanding == 0
            && r.listeners == 0
            && q.queue.lock().unwrap().is_empty()
            && IDLE.load(Ordering::SeqCst) == n - 1
            && !r.tasks.values().any(|t| t.state == State::Running);
        if quiet && auto_advance() {
            continue;
        }
        if quiet {
            // every task is parked and nothing can wake them
            let msg = "deadlock: all tasks are blocked".to_string();
            if task(main).is_some_and(|t| t.state == State::Parked) {
                fail_pending(main, PanicInfo { message: msg, location: String::new(), notes: vec![] });
                continue;
            }
            eprintln!("fatal: {msg}");
            SHUTDOWN.store(true, Ordering::SeqCst);
            return Some(Exit::Failed(PanicInfo {
                message: "deadlock".into(),
                location: String::new(),
                notes: vec![],
            }));
        }
        // idle: wait for work without the runtime lock
        let seq = READY_SEQ.load(Ordering::SeqCst);
        let wait = next_timer
            .map(|at| at.saturating_duration_since(Instant::now()))
            .unwrap_or(Duration::from_millis(20))
            .min(Duration::from_millis(20));
        IDLE.fetch_add(1, Ordering::SeqCst);
        unlock();
        {
            let g = q.queue.lock().unwrap();
            if g.is_empty() && READY_SEQ.load(Ordering::SeqCst) == seq && !SHUTDOWN.load(Ordering::SeqCst) {
                stat(|w| &w.idle_waits, 1);
                let _ = q.cond.wait_timeout(g, wait).unwrap();
            }
        }
        lock();
        IDLE.fetch_sub(1, Ordering::SeqCst);
    }
}

fn run_one(id: TaskId) {
    let Some(t) = task(id) else { return };
    if t.state == State::Done {
        return;
    }
    // a cancelled task that never started does not run
    if t.ctx.flags & FLAG_CANCEL != 0
        && t.co.as_ref().is_some_and(|c| !c.started())
        && t.state == State::Ready
    {
        t.pending_exit = Some(Exit::Cancelled);
        unsafe { t.co.as_mut().unwrap().force_reset() };
        start_cleanup(id);
        return;
    }
    let was_cleaning = t.state == State::Cleaning;
    t.state = if was_cleaning { State::Cleaning } else { State::Running };
    t.ctx.budget = task_budget();
    set_current(id);
    stat(|w| &w.runs, 1);
    let co = t.co.as_mut().unwrap() as *mut Co;
    let res = unsafe { (*co).resume(()) };
    set_current(0);
    let t = task(id).unwrap();
    match res {
        CoroutineResult::Yield(Signal::Yield) => {
            if t.state == State::Running {
                t.state = State::Ready;
            }
            rt().ready.push_back(id);
            notify_ready();
        }
        CoroutineResult::Yield(Signal::Park) => {}
        CoroutineResult::Yield(Signal::Abandon) => {
            // staged messages were accepted by the runtime: publish them
            if !t.on_deschedule.is_empty()
                && let Some(h) = rt().deschedule_hook
            {
                let objs = t.on_deschedule.clone();
                h(id, &objs);
            }
            unsafe { t.co.as_mut().unwrap().force_reset() };
            t.ctx.shadow = std::ptr::null_mut();
            if t.state == State::Cleaning {
                // a cleanup failed: continue with the remaining cleanups
                start_cleanup(id);
            } else {
                start_cleanup(id);
            }
        }
        CoroutineResult::Return(()) => {
            if t.state == State::Cleaning {
                finish(id);
            } else {
                // normal completion: run any still-registered cleanups
                if t.cleanups.is_empty() {
                    t.pending_exit = t.exit.take();
                    finish(id);
                } else {
                    t.pending_exit = t.exit.take();
                    start_cleanup(id);
                }
            }
        }
    }
}

fn start_cleanup(id: TaskId) {
    let t = task(id).unwrap();
    t.state = State::Cleaning;
    t.in_cleanup = true;
    t.ctx.flags &= !(FLAG_CANCEL | FLAG_FAIL);
    t.ctx.shadow = std::ptr::null_mut();
    if t.cleanups.is_empty() {
        finish(id);
        return;
    }
    let co = make_coroutine(id, Body::Cleanup);
    let old = task(id).unwrap().co.replace(co);
    drop_reset(old);
    rt().ready.push_back(id);
    notify_ready();
}

fn drop_reset(co: Option<Co>) {
    if let Some(mut c) = co {
        if c.done() {
            #[allow(static_mut_refs)]
            unsafe {
                if STACK_POOL.len() < STACK_POOL_MAX {
                    STACK_POOL.push(c.into_stack());
                    return;
                }
            }
        } else if c.started() {
            unsafe { c.force_reset() };
        }
        drop(c);
    }
}

fn finish(id: TaskId) {
    if let Some(t) = task(id)
        && !t.on_deschedule.is_empty()
        && let Some(h) = rt().deschedule_hook
    {
        let objs = std::mem::take(&mut t.on_deschedule);
        h(id, &objs);
    }
    crate::channel::task_finished(id);
    let t = task(id).unwrap();
    t.state = State::Done;
    t.exit = t.pending_exit.take().or(t.exit.take()).or(Some(Exit::Cancelled));
    t.ctx.shadow = std::ptr::null_mut();
    let joiners = std::mem::take(&mut t.joiners);
    let scope = t.scope;
    let exit = t.exit.clone();
    let old = t.co.take();
    drop_reset(old);
    for j in joiners {
        wake(j, Wake::Normal);
    }
    if let (Some(s), Some(e)) = (scope, exit) {
        crate::scope::child_finished(s, id, &e);
    }
}

/// Non-blocking: wake tasks whose timers expired and deliver external events.
pub fn harvest_events() {
    let q = rt().external.clone();
    let events: Vec<External> = match q.queue.try_lock() {
        Ok(mut g) => g.drain(..).collect(),
        Err(_) => vec![],
    };
    for e in events {
        match e {
            External::Complete { task: id, value } => {
                rt().outstanding -= 1;
                rt().completions.insert(id, value);
                wake(id, Wake::Normal);
            }
            External::Callback(f) => f(),
            External::Scheduler(f) => rt().scheduler_work.push(f),
        }
    }
    let now = Instant::now();
    while let Some(t) = rt().timers.peek() {
        if t.at > now {
            break;
        }
        let t = rt().timers.pop().unwrap();
        if task(t.task).is_some_and(|x| x.sleep_seq == t.seq) {
            wake(t.task, Wake::Timeout);
        }
    }
}

/// Run scheduler callbacks (outside any task).
fn run_scheduler_work() {
    while let Some(f) = rt().scheduler_work.pop() {
        f();
    }
}

/// Isolate tests: each starts on the real clock with no controlled sleeps.
pub fn reset_test_state() {
    *crate::intrinsics_os::MANUAL_CLOCK.lock().unwrap() = None;
    rt().virtual_timers.clear();
    let mut c = SCHED.lock().unwrap();
    c.seed = std::env::var("LANG_SCHED_SEED").ok().and_then(|v| v.parse().ok());
    c.record = false;
    c.decisions.clear();
    c.replay.clear();
}

/// The controlled clock moved to `now`: wake sleeps that are due.
pub fn clock_advanced(now: i64) {
    let r = rt();
    let due: Vec<(i64, u64, TaskId)> =
        r.virtual_timers.iter().copied().filter(|(at, _, _)| *at <= now).collect();
    r.virtual_timers.retain(|(at, _, _)| *at > now);
    for (_, seq, id) in due {
        if task(id).is_some_and(|x| x.sleep_seq == seq) {
            wake(id, Wake::Timeout);
        }
    }
}

/// Nothing else can happen: move a controlled clock to the earliest
/// pending sleep. Returns false if there is none.
fn auto_advance() -> bool {
    let r = rt();
    r.virtual_timers.retain(|(_, seq, id)| task(*id).is_some_and(|x| x.sleep_seq == *seq));
    let Some(at) = r.virtual_timers.iter().map(|v| v.0).min() else { return false };
    {
        let mut g = crate::intrinsics_os::MANUAL_CLOCK.lock().unwrap();
        // (on the real clock again, the sleeps are simply woken)
        if let Some(t) = g.as_mut() {
            *t = (*t).max(at);
        }
    }
    let now = crate::intrinsics_os::MANUAL_CLOCK.lock().unwrap().unwrap_or(i64::MAX);
    clock_advanced(now);
    true
}

/// Wait for timers or external events. Returns false if nothing can happen.
fn wait_events() -> bool {
    let r = rt();
    let q = r.external.clone();
    let next_timer = r.timers.peek().map(|t| t.at);
    if next_timer.is_none() && r.outstanding == 0 && r.listeners == 0 && q.queue.lock().unwrap().is_empty() {
        return auto_advance();
    }
    {
        let mut g = q.queue.lock().unwrap();
        if g.is_empty() {
            match next_timer {
                Some(at) => {
                    let now = Instant::now();
                    if at > now {
                        g = q.cond.wait_timeout(g, at - now).unwrap().0;
                    }
                }
                None => {
                    g = q.cond.wait(g).unwrap();
                }
            }
        }
        let events: Vec<External> = g.drain(..).collect();
        drop(g);
        for e in events {
            match e {
                External::Complete { task: id, value } => {
                    rt().outstanding -= 1;
                    rt().completions.insert(id, value);
                    wake(id, Wake::Normal);
                }
                External::Callback(f) => f(),
                External::Scheduler(f) => f(),
            }
        }
    }
    let now = Instant::now();
    while let Some(t) = rt().timers.peek() {
        if t.at > now {
            break;
        }
        let t = rt().timers.pop().unwrap();
        if task(t.task).is_some_and(|x| x.sleep_seq == t.seq) {
            wake(t.task, Wake::Timeout);
        }
    }
    true
}

pub fn task_name(id: TaskId) -> String {
    task(id).map(|t| t.name.clone()).unwrap_or_default()
}

pub fn ctx_of(id: TaskId) -> *mut TaskCtx {
    task(id).map(|t| &mut *t.ctx as *mut TaskCtx).unwrap_or(std::ptr::null_mut())
}

pub fn desc_name(d: &TypeDesc) -> &str {
    &d.name
}

pub fn now() -> Instant {
    Instant::now()
}

pub fn set_listener(delta: isize) {
    let r = rt();
    r.listeners = (r.listeners as isize + delta).max(0) as usize;
}

/// JSON snapshot of all tasks (for `inspect.tasks()`).
pub fn snapshot_json() -> String {
    let mut ts: Vec<&Box<Task>> = rt().tasks.values().collect();
    ts.sort_by_key(|t| t.id);
    let items: Vec<String> = ts
        .iter()
        .map(|t| {
            let state = match t.state {
                State::Ready => "ready",
                State::Running => "running",
                State::Parked => "parked",
                State::Cleaning => "cleaning",
                State::Done => "done",
            };
            let stages: Vec<String> = t.stages.iter().map(|s| crate::intrinsics_io::json_quote(s)).collect();
            format!(
                "{{\"id\":{},\"name\":{},\"state\":\"{state}\",\"wait\":{},\"scope\":{},\"cleanups\":{},\"code_version\":{},\"stage\":{},\"stages\":[{}]}}",
                t.id,
                crate::intrinsics_io::json_quote(&t.name),
                if t.wait.is_empty() { "null".to_string() } else { crate::intrinsics_io::json_quote(t.wait) },
                t.scope.map(|s| s.to_string()).unwrap_or("null".into()),
                t.cleanups.len(),
                t.code_version,
                t.stages.last().map(|s| crate::intrinsics_io::json_quote(s)).unwrap_or("null".into()),
                stages.join(",")
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}
