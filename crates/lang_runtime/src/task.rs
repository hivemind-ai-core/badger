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
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU8, AtomicU64, AtomicUsize, Ordering};
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
    /// Waiting for a wakeup, suspended without the runtime lock (it
    /// resumes without it): see [`park_free`].
    ParkFree,
    /// An explicit yield or exhausted budget without the runtime lock.
    YieldFree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum State {
    Ready,
    Running,
    Parked,
    Cleaning,
    Done,
    /// On its way to parking without the runtime lock: registered to wait
    /// and still on its stack. A wake records [`Task::pending_wake`]; the
    /// worker it suspends to then queues it (or marks it parked).
    Parking,
}

/// A task's state, readable from any worker (channel fast paths check a
/// waiter's state without the runtime lock; transitions of a parked task
/// happen under the scheduler queue lock, see [`sched`]).
struct StateCell(AtomicU8);

impl StateCell {
    fn new(s: State) -> Self {
        StateCell(AtomicU8::new(s as u8))
    }

    fn get(&self) -> State {
        match self.0.load(Ordering::Acquire) {
            0 => State::Ready,
            1 => State::Running,
            2 => State::Parked,
            3 => State::Cleaning,
            4 => State::Done,
            _ => State::Parking,
        }
    }

    fn set(&self, s: State) {
        self.0.store(s as u8, Ordering::Release);
    }
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
    state: StateCell,
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
    /// A split result's payload (see `abi::lang_rt_take_aux`), traced while
    /// it waits to be taken.
    pub aux: u64,
    pub aux_is_ptr: bool,
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
    /// The call site recorded by a direct call of a function with
    /// preconditions (taken by the callee on entry).
    pub contract_site: u64,
    /// Errors that scopes being closed are returning (innermost last): a
    /// defect raised meanwhile (a failing cleanup) keeps them as notes, so
    /// the original error is not lost.
    pub returning: Vec<String>,
    /// Bumped each time the task registers to wait on a channel: a
    /// registration left behind by an abandoned wait (cancellation) has an
    /// older number, so it can no longer deliver to or wake a later wait.
    pub wait_gen: AtomicU64,
    /// Its cleanup outlived a grace period (that was reported); later
    /// waits for its settling do not wait again.
    pub grace_expired: bool,
    /// The address range of its coroutine stack (`limit..base`), for
    /// memory measurement.
    pub stack: (usize, usize),
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
    /// A wake that arrived while the task was [`State::Parking`] (written
    /// and read under the scheduler queue lock).
    pending_wake: Option<Wake>,
    /// It suspended without the runtime lock, so it resumes without it.
    free_resume: bool,
    /// What it waits for while parked without the runtime lock (see
    /// [`FREE_WAITS`]; 0: nothing). Atomic: inspection reads it under the
    /// lock while the task runs.
    free_wait: AtomicU8,
}

/// Wait reasons of parks without the runtime lock, by `free_wait` number.
const FREE_WAITS: [&str; 3] = ["", "channel receive", "channel send"];

/// What a task waits for (inspection).
fn wait_reason(t: &Task) -> &'static str {
    if !t.wait.is_empty() { t.wait } else { FREE_WAITS[t.free_wait.load(Ordering::Acquire) as usize] }
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

/// The generation of the current task's next wait (stored with its
/// registration, checked by [`waits`]; every park ends it).
pub fn begin_wait() -> u64 {
    current().expect("wait outside a task").wait_gen.load(Ordering::Acquire)
}

/// End the current wait: registrations made for it no longer apply.
fn end_wait() {
    if let Some(t) = current() {
        t.wait_gen.fetch_add(1, Ordering::AcqRel);
    }
}

/// Whether task `id` is still in the wait numbered `wait`: parked in it
/// (or publishing its staged batches on the way), not cancelled.
pub fn waits(id: TaskId, wait: u64) -> bool {
    task(id).is_some_and(|t| {
        t.ctx.flags & FLAG_CANCEL == 0
            && t.wait_gen.load(Ordering::Acquire) == wait
            && (matches!(t.state.get(), State::Parked | State::Parking) || t.publishing)
    })
}

/// Whether a task is parked (waiting to be woken).
pub fn is_parked(id: TaskId) -> bool {
    task(id).is_some_and(|t| t.state.get() == State::Parked)
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

/// [`set_current`] for a task whose record is known (no table lookup:
/// callers without the runtime lock).
#[inline(never)]
fn set_current_task(id: TaskId, p: *mut Task) {
    CURRENT.with(|c| c.set(id));
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

// ------------------------------------------------- lock attribution

/// Why the runtime lock was taken: a runtime function generated code calls
/// (interned names, from `FIRST_ENTRY`) or one of the internal reasons.
pub mod reason {
    pub const OTHER: u16 = 0;
    pub const CHANNEL_SLOW: u16 = 1;
    pub const BATCH_SLOW: u16 = 2;
    pub const CALLBACK_RETURN: u16 = 3;
    pub const SAFE_END: u16 = 4;
    pub const GC: u16 = 5;
    pub const DEFECT: u16 = 6;
    pub const SCHEDULER: u16 = 7;
    pub const STARTUP: u16 = 8;
    pub const CONSTANT: u16 = 9;
    pub const POLL: u16 = 10;
    pub const FIRST_ENTRY: u16 = 16;
    pub const NAMES: [&str; 11] = [
        "other",
        "channel slow path",
        "batch slow path",
        "return from compiled callback",
        "end of foreign call",
        "collection",
        "defect",
        "scheduler (park, wake, idle)",
        "worker startup",
        "shared constant",
        "yield (poll or task.yield_now)",
    ];
}

const REASONS: usize = 512;

/// Per worker, per reason: acquisitions and sampled hold nanoseconds (every
/// 64th acquisition is timed; the total is scaled).
#[repr(align(128))]
struct ReasonStats {
    count: [AtomicU64; REASONS],
    hold_ns: [AtomicU64; REASONS],
}

static BY_REASON: [ReasonStats; MAX_WORKERS] = [const {
    ReasonStats {
        count: [const { AtomicU64::new(0) }; REASONS],
        hold_ns: [const { AtomicU64::new(0) }; REASONS],
    }
}; MAX_WORKERS];

static ENTRY_NAMES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// The attribution id of runtime function `name` (generated code passes it
/// to `lang_rt_enter`).
pub fn lock_reason_id(name: &str) -> u16 {
    let mut v = ENTRY_NAMES.lock().unwrap();
    let i = match v.iter().position(|n| n == name) {
        Some(i) => i,
        None => {
            v.push(name.to_string());
            v.len() - 1
        }
    };
    (reason::FIRST_ENTRY as usize + i).min(REASONS - 1) as u16
}

fn reason_name(r: usize) -> String {
    if r < reason::NAMES.len() {
        return reason::NAMES[r].to_string();
    }
    let i = r.saturating_sub(reason::FIRST_ENTRY as usize);
    ENTRY_NAMES.lock().unwrap().get(i).cloned().unwrap_or_else(|| format!("reason {r}"))
}

thread_local! {
    /// The reason of the lock this thread holds, and when its hold started
    /// (sampled acquisitions only).
    static HOLD: Cell<(u16, Option<Instant>)> = const { Cell::new((0, None)) };
}

fn note_acquired(r: u16) {
    let w = &BY_REASON[worker_index()];
    let n = w.count[r as usize].fetch_add(1, Ordering::Relaxed);
    let started = n.is_multiple_of(64).then(Instant::now);
    HOLD.with(|h| h.set((r, started)));
}

fn note_released() {
    let (r, started) = HOLD.with(|h| h.replace((0, None)));
    if let Some(t) = started {
        BY_REASON[worker_index()].hold_ns[r as usize]
            .fetch_add(t.elapsed().as_nanos() as u64 * 64, Ordering::Relaxed);
    }
}

fn print_reasons() {
    let mut rows: Vec<(u64, u64, usize)> = (0..REASONS)
        .map(|r| {
            let c = BY_REASON.iter().map(|w| w.count[r].load(Ordering::Relaxed)).sum();
            let h = BY_REASON.iter().map(|w| w.hold_ns[r].load(Ordering::Relaxed)).sum();
            (c, h, r)
        })
        .filter(|x| x.0 > 0)
        .collect();
    rows.sort_by_key(|a| std::cmp::Reverse(a.0));
    for (c, h, r) in rows.iter().take(15) {
        eprintln!("[runtime stats]   {c:>9} x {:<40} ~{:.3} ms held", reason_name(*r), *h as f64 / 1e6);
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
    print_reasons();
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

fn lock_for(r: u16) {
    lock_inner();
    if stats_on() {
        note_acquired(r);
    }
}

fn lock_inner() {
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
    if stats_on() {
        note_released();
    }
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
pub extern "C" fn lang_rt_enter(_ctx: *mut TaskCtx, why: u64) {
    IN_COMPILED.fetch_sub(1, Ordering::SeqCst);
    chaos_jitter();
    lock_for(why as u16);
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
    lock_for(reason::SAFE_END);
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
    enter_runtime_for(reason::OTHER)
}

/// [`enter_runtime`], attributed to `why` (see [`reason`]).
pub fn enter_runtime_for(why: u16) {
    if multi() {
        lang_rt_enter(std::ptr::null_mut(), why as u64);
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
        lang_rt_enter(std::ptr::null_mut(), reason::CALLBACK_RETURN as u64);
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
        if t.aux_is_ptr {
            f(t.aux);
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
        state: StateCell::new(State::Ready),
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
        aux: 0,
        aux_is_ptr: false,
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
        contract_site: 0,
        returning: vec![],
        wait_gen: AtomicU64::new(0),
        grace_expired: false,
        stack: (0, 0),
        early_wake: None,
        select_offer: vec![],
        select_done: None,
        publishing: false,
        stages: vec![],
        pending_wake: None,
        free_resume: false,
        free_wait: AtomicU8::new(0),
    });
    r.tasks.insert(id, t);
    // the coroutine (and its stack) is made when the task first runs, so a
    // burst of spawns reuses the stacks of tasks that finished meanwhile
    task(id).unwrap().result_is_ptr = result_is_ptr;
    push_ready(id);
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
    if let Some(t) = task(id) {
        t.stack = (stack.limit().get(), stack.base().get());
    }
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
    // (the wait ends exactly when `check_interrupts` abandons it)
    let t = current().unwrap();
    if t.state.get() != State::Cleaning && t.ctx.flags & (FLAG_CANCEL | FLAG_FAIL) != 0 {
        end_wait();
    }
    check_interrupts();
    if let Some(w) = current().unwrap().early_wake.take() {
        end_wait();
        check_interrupts();
        return w;
    }
    {
        let t = current().unwrap();
        t.state.set(State::Parked);
        t.wake = Wake::Normal;
    }
    suspend(Signal::Park);
    end_wait();
    check_interrupts();
    current().unwrap().wake
}

/// Park without honoring cancellation (cleanup and settle waits).
pub fn park_shielded() -> Wake {
    publish_current();
    if let Some(w) = current().unwrap().early_wake.take() {
        end_wait();
        return w;
    }
    {
        let t = current().unwrap();
        t.state.set(State::Parked);
        t.wake = Wake::Normal;
    }
    suspend(Signal::Park);
    end_wait();
    current().unwrap().wake
}

/// Whether the running task may park or yield without the runtime lock:
/// several workers, uncontrolled scheduling, called without the lock, and
/// nothing to do as it schedules out (no staged batches to publish, no
/// cleanup running).
pub fn can_suspend_free() -> bool {
    multi()
        && !sched_controlled()
        && !holds_lock()
        && current()
            .is_some_and(|t| t.on_deschedule.is_empty() && !t.in_cleanup && t.state.get() == State::Running)
}

fn flags_of(t: &Task) -> u64 {
    unsafe { (*(&raw const t.ctx.flags).cast::<AtomicU64>()).load(Ordering::Acquire) }
}

/// Without the runtime lock, holding what the running task registers with
/// (so no waker takes the registration before the task is parking): mark
/// it parking, waiting for `why` (an index into [`FREE_WAITS`]). `false`
/// (cancellation or a failure is pending): it must not park; the locked
/// path decides instead. The flags are read under the scheduler queue
/// lock, which a canceller's wake takes after setting them.
pub fn begin_park_free(why: u8) -> bool {
    let p = current().expect("park outside a task") as *mut Task;
    sched(|| {
        let t = unsafe { &mut *p };
        if flags_of(t) & (FLAG_CANCEL | FLAG_FAIL) != 0 {
            return false;
        }
        t.free_wait.store(why, Ordering::Release);
        t.wake = Wake::Normal;
        t.pending_wake = None;
        t.free_resume = true;
        t.state.set(State::Parking);
        true
    })
}

/// Park the task [`begin_park_free`] marked (after releasing what it
/// registered with), without the runtime lock; returns once woken.
pub fn park_free() -> Wake {
    chaos_jitter();
    suspend(Signal::ParkFree);
    end_wait();
    let t = current().unwrap();
    t.free_wait.store(0, Ordering::Release);
    t.wake
}

/// A yield (a poll's slow path, `task.yield_now`) without the runtime lock:
/// requeue behind the other ready tasks when nothing needs the lock, else
/// the locked [`yield_now`].
pub fn yield_free() {
    let t = current().unwrap();
    if !can_suspend_free()
        || flags_of(t) & (FLAG_CANCEL | FLAG_FAIL) != 0
        || STOP.load(Ordering::SeqCst)
        || events_due()
    {
        // (a collection waiting for this worker: the lock is a safe point)
        enter_runtime_for(reason::POLL);
        yield_now();
        leave_runtime();
        return;
    }
    t.ctx.budget = task_budget();
    if !has_ready() {
        return;
    }
    t.free_resume = true;
    chaos_jitter();
    suspend(Signal::YieldFree);
    if flags_of(current().unwrap()) & (FLAG_CANCEL | FLAG_FAIL) != 0 {
        enter_runtime_for(reason::POLL);
        check_interrupts();
        leave_runtime();
    }
}

/// A worker without the runtime lock is about to run compiled code: count
/// it among [`IN_COMPILED`], waiting out a collection stopping the world.
fn gc_enter() {
    loop {
        IN_COMPILED.fetch_add(1, Ordering::SeqCst);
        if !STOP.load(Ordering::SeqCst) {
            return;
        }
        IN_COMPILED.fetch_sub(1, Ordering::SeqCst);
        while STOP.load(Ordering::SeqCst) {
            std::hint::spin_loop();
            std::thread::yield_now();
        }
    }
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
    if t.state.get() == State::Cleaning {
        return;
    }
    if t.ctx.flags & FLAG_FAIL != 0 {
        abandon(None);
    }
    if t.ctx.flags & FLAG_CANCEL != 0 {
        abandon(Some(Exit::Cancelled));
    }
}

/// Queue a woken task. A task woken by a running task (several workers,
/// uncontrolled scheduling) runs next on the waker's worker, so a request
/// and its reply stay on one worker without waking another; the task it
/// displaces joins the shared queue.
/// Whether a wake from here may use this worker's next slot (several
/// workers, woken by a running task, uncontrolled scheduling); under
/// chaos, a random half of wakes do not.
fn wake_slot() -> bool {
    let chaotic_skip = chaos().is_some() && chaos_next() & 1 == 0;
    multi() && current_raw() != 0 && !chaotic_skip && !sched_controlled()
}

/// Queue a ready task (into this worker's next slot when `slot`); returns
/// whether idle workers should be told. Takes the scheduler queue lock.
fn enqueue(e: Entry, slot: bool) -> bool {
    sched(|| enqueue_locked(e, slot))
}

fn enqueue_locked(e: Entry, slot: bool) -> bool {
    if slot {
        let w = worker_index();
        let prev = std::mem::replace(&mut queue().next[w], e);
        queue().next_at[w] = now_ns();
        if prev.0 != 0 {
            queue().ready.push_back(prev);
            return true;
        }
        return false;
    }
    queue().ready.push_back(e);
    true
}

/// Queue a ready task in the shared queue and tell idle workers (runtime
/// lock held: the task is looked up).
fn push_ready(id: TaskId) {
    let p = task(id).map_or(0, |t| t as *mut Task as usize);
    if enqueue((id, p), false) {
        notify_ready();
    }
}

/// The scheduler queue lock: the ready queue, the workers' next slots, and
/// a parked task's transition to ready (with a delivered value). Taken
/// inside the runtime lock, or by a channel fast path holding its channel;
/// nothing is waited for while holding it, and no other lock is taken
/// under it.
static SCHED_Q: AtomicBool = AtomicBool::new(false);

/// A ready task and its record (so a worker without the runtime lock can
/// resume it without looking it up).
type Entry = (TaskId, usize);

/// What [`SCHED_Q`] protects (kept apart from [`Runtime`], which the
/// runtime lock's holder borrows).
struct ReadyQueue {
    ready: VecDeque<Entry>,
    /// Several workers: a task woken by a running task, kept to run next on
    /// the waker's worker (0 = none); idle workers steal from these.
    next: [Entry; MAX_WORKERS],
    /// When each next slot was filled ([`now_ns`]): another worker may take
    /// it only after [`STEAL_AFTER_NS`], so a woken partner usually runs on
    /// the worker whose caches hold the data it was woken for.
    next_at: [u64; MAX_WORKERS],
}

const STEAL_AFTER_NS: u64 = 50_000;

/// Monotonic nanoseconds since the runtime started.
fn now_ns() -> u64 {
    epoch().elapsed().as_nanos() as u64
}

fn epoch() -> Instant {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    *START.get_or_init(Instant::now)
}

/// The earliest timer's deadline ([`now_ns`]; `u64::MAX`: none), so a
/// worker without the runtime lock knows when to take it and harvest.
static NEXT_TIMER: AtomicU64 = AtomicU64::new(u64::MAX);
/// External events were posted and not yet harvested.
static EVENTS: AtomicBool = AtomicBool::new(false);

/// Record the earliest timer (runtime lock held, after changing timers).
fn note_timers() {
    let at =
        rt().timers.peek().map_or(u64::MAX, |t| t.at.saturating_duration_since(epoch()).as_nanos() as u64);
    NEXT_TIMER.store(at, Ordering::SeqCst);
}

/// Whether harvesting would find work: a due timer or posted events.
fn events_due() -> bool {
    EVENTS.load(Ordering::SeqCst) || NEXT_TIMER.load(Ordering::SeqCst) <= now_ns()
}

struct QueueCell(UnsafeCell<ReadyQueue>);
unsafe impl Sync for QueueCell {}
static QUEUE: QueueCell = QueueCell(UnsafeCell::new(ReadyQueue {
    ready: VecDeque::new(),
    next: [(0, 0); MAX_WORKERS],
    next_at: [0; MAX_WORKERS],
}));

/// The ready queue (only inside [`sched`]).
#[allow(clippy::mut_from_ref)]
fn queue() -> &'static mut ReadyQueue {
    unsafe { &mut *QUEUE.0.get() }
}

fn sched<R>(f: impl FnOnce() -> R) -> R {
    if !multi() {
        return f();
    }
    while SCHED_Q.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
        std::hint::spin_loop();
    }
    let r = f();
    SCHED_Q.store(false, Ordering::Release);
    r
}

/// Whether any task waits to run (the shared queue or a worker's next slot).
fn has_ready() -> bool {
    sched(|| {
        let q = queue();
        !q.ready.is_empty() || q.next.iter().any(|t| t.0 != 0)
    })
}

/// This worker's next task, else the shared queue, else another worker's
/// next task.
fn take_ready() -> Option<Entry> {
    sched(|| {
        let w = worker_index();
        let mine = std::mem::take(&mut queue().next[w]);
        if mine.0 != 0 {
            return Some(mine);
        }
        if let Some(t) = chaos_pop() {
            return Some(t);
        }
        // another worker's next task, once it has waited there a while
        let now = now_ns();
        let q = queue();
        (0..MAX_WORKERS)
            .find(|&i| q.next[i].0 != 0 && now.saturating_sub(q.next_at[i]) >= STEAL_AFTER_NS)
            .map(|i| std::mem::take(&mut q.next[i]))
    })
}

/// Whether this worker has something to run (its next slot, the shared
/// queue, or another worker's slot it may take), and otherwise how soon
/// another worker's slot becomes takable.
fn work_here() -> (bool, Option<Duration>) {
    sched(|| {
        let q = queue();
        let w = worker_index();
        if q.next[w].0 != 0 || !q.ready.is_empty() {
            return (true, None);
        }
        let now = now_ns();
        let mut soonest: Option<u64> = None;
        for i in 0..MAX_WORKERS {
            if q.next[i].0 != 0 {
                let age = now.saturating_sub(q.next_at[i]);
                if age >= STEAL_AFTER_NS {
                    return (true, None);
                }
                let left = STEAL_AFTER_NS - age;
                soonest = Some(soonest.map_or(left, |s: u64| s.min(left)));
            }
        }
        (false, soonest.map(Duration::from_nanos))
    })
}

pub fn wake(id: TaskId, reason: Wake) {
    wake_delivering(id, reason, None)
}

/// [`wake`] a task that is parked or parking (never recorded as an early
/// wake of a running task).
fn wake_waiting(id: TaskId, reason: Wake) {
    let Some(t) = task(id) else { return };
    let t = t as *mut Task;
    let slot = wake_slot();
    let notify = sched(|| {
        let t = unsafe { &mut *t };
        match t.state.get() {
            State::Parked => {
                t.state.set(if t.in_cleanup { State::Cleaning } else { State::Ready });
                t.wake = reason;
                enqueue_locked((t.id, t as *mut Task as usize), slot)
            }
            State::Parking => {
                t.pending_wake = Some(reason);
                false
            }
            _ => false,
        }
    });
    if notify {
        notify_ready();
    }
}

/// Wake `id` (if parked), first delivering `(value, is_ptr, status)`.
fn wake_delivering(id: TaskId, reason: Wake, delivery: Option<(u64, bool, u64)>) {
    let Some(t) = task(id) else { return };
    let t = t as *mut Task;
    let slot = wake_slot();
    let notify = sched(|| {
        let t = unsafe { &mut *t };
        if let Some((value, is_ptr, status)) = delivery {
            t.delivered = value;
            t.delivered_is_ptr = is_ptr;
            t.status = status;
        }
        if t.state.get() == State::Parked {
            t.state.set(if t.in_cleanup { State::Cleaning } else { State::Ready });
            t.wake = reason;
            enqueue_locked((t.id, t as *mut Task as usize), slot)
        } else if t.state.get() == State::Parking {
            // its worker queues it once it has left its stack
            t.pending_wake = Some(reason);
            false
        } else {
            if t.publishing {
                // the running task woke itself on its way to parking
                t.early_wake = Some(reason);
            }
            false
        }
    });
    if notify {
        notify_ready();
    }
}

/// The running task's record (stable while the task exists: channel
/// registrations keep it so a fast path need not look the task up).
pub fn current_ptr() -> usize {
    CURRENT_TASK.with(|c| c.get()) as usize
}

/// Without the runtime lock (a channel fast path holding the channel the
/// task registered with, which keeps the task alive): deliver `value` to
/// task `t` if it is still parked in wait `wait`, and make it ready.
/// Returns whether it was delivered.
pub fn deliver(t: usize, wait: u64, value: u64, is_ptr: bool, status: u64) -> bool {
    let t = t as *mut Task;
    let slot = wake_slot();
    let woke = sched(|| {
        let t = unsafe { &mut *t };
        let flags = unsafe { (*(&raw const t.ctx.flags).cast::<AtomicU64>()).load(Ordering::Acquire) };
        let state = t.state.get();
        if flags & FLAG_CANCEL != 0
            || t.wait_gen.load(Ordering::Acquire) != wait
            || !matches!(state, State::Parked | State::Parking)
        {
            return None;
        }
        t.delivered = value;
        t.delivered_is_ptr = is_ptr;
        t.status = status;
        if state == State::Parking {
            // its worker queues it once it has left its stack
            t.pending_wake = Some(Wake::Normal);
            return Some(false);
        }
        t.state.set(if t.in_cleanup { State::Cleaning } else { State::Ready });
        t.wake = Wake::Normal;
        Some(enqueue_locked((t.id, t as *mut Task as usize), slot))
    });
    match woke {
        Some(notify) => {
            if notify {
                notify_ready();
            }
            true
        }
        None => false,
    }
}

/// Let idle workers see newly ready tasks.
fn notify_ready() {
    if multi() && IDLE.load(Ordering::SeqCst) > 0 {
        READY_SEQ.fetch_add(1, Ordering::SeqCst);
        // wake one idle worker, unless a woken one is still on its way: it
        // wakes the next if it finds more ready work (no thundering herd
        // on the runtime lock)
        if WAKING.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            return;
        }
        // the idle worker may have left meanwhile: nobody would clear it
        if IDLE.load(Ordering::SeqCst) == 0 {
            WAKING.store(false, Ordering::SeqCst);
            return;
        }
        stat(|w| &w.notifies, 1);
        let q = rt().external.clone();
        let _g = q.queue.lock().unwrap();
        q.cond.notify_one();
    }
}

/// A worker was woken for ready work and has not yet looked for it.
static WAKING: AtomicBool = AtomicBool::new(false);

/// Wake a parked task, delivering a value (`is_ptr`: managed pointer).
pub fn wake_with(id: TaskId, value: u64, is_ptr: bool, status: u64) {
    wake_delivering(id, Wake::Normal, Some((value, is_ptr, status)))
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
    // keep running unless another task is ready or scheduler work (an
    // inspection request, a REPL evaluation, a reload) waits for a step
    // boundary, which a lone busy task would otherwise never reach
    if !has_ready() && rt().scheduler_work.is_empty() {
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
    if t.state.get() == State::Done || t.state.get() == State::Cleaning {
        return;
    }
    if t.ctx.flags & FLAG_CANCEL == 0 {
        t.cancel_reason = reason;
    }
    t.ctx.flags |= FLAG_CANCEL;
    // tasks waiting for it now wait under the cleanup grace period
    let joiners = t.joiners.clone();
    for j in joiners {
        if task(j).is_some_and(|x| x.state.get() == State::Parked && x.wait == "join") {
            wake(j, Wake::Normal);
        }
    }
    // a task parking without the runtime lock checks the flags under the
    // scheduler queue lock, which this wake takes (it sees the request, or
    // the wake sees it parking)
    wake_waiting(id, Wake::Cancelled);
}

/// Make another task fail at its next safe point (child defect propagation).
pub fn fail_pending(id: TaskId, info: PanicInfo) {
    let Some(t) = task(id) else { return };
    if t.state.get() == State::Done || t.state.get() == State::Cleaning || t.pending_exit.is_some() {
        return;
    }
    t.ctx.flags |= FLAG_FAIL;
    t.pending_exit = Some(Exit::Failed(info));
    wake_waiting(id, Wake::Cancelled);
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
/// The call site a direct call recorded for a function with
/// preconditions, cleared (the function reads it on entry).
pub fn take_contract_site() -> String {
    let Some(t) = current() else { return String::new() };
    let l = std::mem::take(&mut t.contract_site);
    if l == 0 { String::new() } else { crate::string::str_of(l).to_string() }
}

pub fn set_contract_site(loc: u64) {
    if let Some(t) = current() {
        t.contract_site = loc;
    }
}

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
        enter_runtime_for(reason::DEFECT);
    }
    let t = current().expect("panic outside of a task");
    if t.state.get() == State::Cleaning {
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
    let notes = t.returning.iter().rev().map(|e| format!("while returning the error: {e}")).collect();
    abandon(Some(Exit::Failed(PanicInfo { message, location, notes })))
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
    note_timers();
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
        EVENTS.store(true, Ordering::SeqCst);
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
    EVENTS.store(true, Ordering::SeqCst);
    q.cond.notify_one();
}

pub fn exit_of(id: TaskId) -> Option<Exit> {
    task(id).and_then(|t| if t.state.get() == State::Done { t.exit.clone() } else { None })
}

pub fn is_done(id: TaskId) -> bool {
    task(id).is_some_and(|t| t.state.get() == State::Done)
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
        let pending: Vec<TaskId> = ids
            .iter()
            .copied()
            .filter(|id| task(*id).is_some_and(|t| t.state.get() != State::Done && !t.grace_expired))
            .collect();
        if pending.is_empty() {
            return;
        }
        let now = Instant::now();
        if now >= deadline {
            for id in &pending {
                if let Some(t) = task(*id) {
                    t.grace_expired = true;
                }
            }
            let names: Vec<String> = pending
                .iter()
                .map(|id| match task(*id) {
                    Some(t) if !wait_reason(t).is_empty() => {
                        format!("task {id}, {:?} on {}", t.state.get(), wait_reason(t))
                    }
                    Some(t) => format!("task {id}, {:?}", t.state.get()),
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
        note_timers();
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
                || t.state.get() == State::Cleaning
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

/// Whether scheduling is controlled (kept current by every change to
/// [`SCHED`], so the wake path does not lock it to find out).
static CONTROLLED: AtomicBool = AtomicBool::new(false);

fn sched_controlled() -> bool {
    CONTROLLED.load(Ordering::Acquire)
}

/// Record whether `c` controls scheduling (call after changing it).
pub fn refresh_controlled(c: &SchedControl) {
    CONTROLLED.store(c.seed.is_some() || c.record || !c.replay.is_empty(), Ordering::Release);
}

/// Pick the next ready task: FIFO unless scheduling is controlled.
fn next_ready() -> Option<Entry> {
    let n = sched(|| queue().ready.len()) as u64;
    if n == 0 {
        return None;
    }
    let mut c = SCHED.lock().unwrap();
    if c.seed.is_none() && c.replay.is_empty() && !c.record {
        drop(c);
        return sched(chaos_pop);
    }
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
    refresh_controlled(&c);
    drop(c);
    sched(|| queue().ready.remove(idx as usize))
}

/// The next ready task: FIFO, or a random one under chaos.
fn chaos_pop() -> Option<Entry> {
    let r = queue();
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
    crate::profile::start_from_env();
    if multi() {
        return run_main_multi(entry);
    }
    let mut main = spawn_main(entry);
    rt().main = Some(main);
    loop {
        if let Some(id) = next_ready() {
            run_one(id.0);
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
                    t.state.get(),
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
                && t.state.get() == State::Parked
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
    lock_for(reason::STARTUP);
    let main = spawn_main(entry);
    rt().main = Some(main);
    let n = workers().min(MAX_WORKERS);
    for w in 1..n {
        let h = std::thread::Builder::new()
            .name(format!("lang-worker-{w}"))
            .spawn(move || {
                WORKER.with(|c| c.set(w));
                crate::profile::start_thread();
                lock_for(reason::STARTUP);
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
            take_ready()
        };
        if let Some(id) = pick {
            // more work than this worker can take: wake another (tasks in
            // workers' next slots are taken by idle workers after a while)
            if sched(|| !queue().ready.is_empty()) {
                notify_ready();
            }
            if is_free(id) && !sched_controlled() {
                run_free(id, true);
            } else {
                run_one(id.0);
            }
            run_scheduler_work();
            continue;
        }
        harvest_events();
        run_scheduler_work();
        // (a worker standing aside for controlled scheduling waits instead)
        let aside = worker_index() != 0 && sched_controlled();
        let (work, steal_in) = work_here();
        if work && !aside {
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
            && !r.tasks.values().any(|t| t.state.get() == State::Running);
        if quiet && auto_advance() {
            continue;
        }
        if quiet {
            // every task is parked and nothing can wake them
            let msg = "deadlock: all tasks are blocked".to_string();
            if task(main).is_some_and(|t| t.state.get() == State::Parked) {
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
            .min(Duration::from_millis(20))
            .min(steal_in.unwrap_or(Duration::MAX));
        IDLE.fetch_add(1, Ordering::SeqCst);
        unlock();
        {
            let g = q.queue.lock().unwrap();
            if g.is_empty() && READY_SEQ.load(Ordering::SeqCst) == seq && !SHUTDOWN.load(Ordering::SeqCst) {
                stat(|w| &w.idle_waits, 1);
                let _ = q.cond.wait_timeout(g, wait).unwrap();
            }
        }
        WAKING.store(false, Ordering::SeqCst);
        lock_for(reason::SCHEDULER);
        IDLE.fetch_sub(1, Ordering::SeqCst);
    }
}

fn run_one(id: TaskId) {
    let Some(t) = task(id) else { return };
    if t.state.get() == State::Done {
        return;
    }
    if t.free_resume {
        run_free((id, t as *mut Task as usize), false);
        return;
    }
    // a cancelled task that never started does not run
    if t.ctx.flags & FLAG_CANCEL != 0
        && t.co.as_ref().is_none_or(|c| !c.started())
        && t.state.get() == State::Ready
    {
        t.pending_exit = Some(Exit::Cancelled);
        if let Some(c) = t.co.as_mut() {
            unsafe { c.force_reset() };
        }
        start_cleanup(id);
        return;
    }
    if t.co.is_none() {
        let co = make_coroutine(id, Body::Entry);
        task(id).unwrap().co = Some(co);
    }
    let t = task(id).unwrap();
    let was_cleaning = t.state.get() == State::Cleaning;
    t.state.set(if was_cleaning { State::Cleaning } else { State::Running });
    t.ctx.budget = task_budget();
    set_current(id);
    stat(|w| &w.runs, 1);
    let p = t as *mut Task as usize;
    let co = t.co.as_mut().unwrap() as *mut Co;
    let res = unsafe { (*co).resume(()) };
    set_current(0);
    // a task resumed under the lock releases it as it returns to compiled
    // code, and may then suspend without it (the table is not looked at
    // without the lock)
    if suspended_free((id, p), &res) {
        lock_for(reason::SCHEDULER);
    } else {
        after_resume(id, res);
    }
}

/// After a task suspended without the runtime lock (`res`), the worker
/// (not holding the lock) parks or requeues it; returns whether it did.
fn suspended_free(e: Entry, res: &CoroutineResult<Signal, ()>) -> bool {
    let p = e.1 as *mut Task;
    let notify = match res {
        CoroutineResult::Yield(Signal::ParkFree) => {
            IN_COMPILED.fetch_sub(1, Ordering::SeqCst);
            sched(|| {
                let t = unsafe { &mut *p };
                match t.pending_wake.take() {
                    Some(w) => {
                        t.wake = w;
                        t.state.set(State::Ready);
                        enqueue_locked(e, false)
                    }
                    None => {
                        t.state.set(State::Parked);
                        false
                    }
                }
            })
        }
        CoroutineResult::Yield(Signal::YieldFree) => {
            IN_COMPILED.fetch_sub(1, Ordering::SeqCst);
            sched(|| {
                unsafe { &*p }.state.set(State::Ready);
                enqueue_locked(e, false)
            })
        }
        _ => return false,
    };
    if notify {
        notify_ready();
    }
    true
}

/// Whether a ready task suspended without the runtime lock.
fn is_free(e: Entry) -> bool {
    unsafe { (*(e.1 as *const Task)).free_resume }
}

/// Run a task that suspended without the runtime lock (the lock is held on
/// entry and on return): release the lock and resume it. With `cycle`, the
/// tasks this worker takes next run without the lock too, while each of
/// them suspended without it and nothing needs the lock (due timers,
/// posted events, shutdown).
fn run_free(mut e: Entry, cycle: bool) {
    unlock();
    loop {
        let p = e.1 as *mut Task;
        let t = unsafe { &mut *p };
        gc_enter();
        t.free_resume = false;
        t.state.set(State::Running);
        t.ctx.budget = task_budget();
        set_current_task(e.0, p);
        stat(|w| &w.runs, 1);
        let co = t.co.as_mut().unwrap() as *mut Co;
        let res = unsafe { (*co).resume(()) };
        set_current_task(0, std::ptr::null_mut());
        if !suspended_free(e, &res) {
            // it suspended (or finished) holding the runtime lock
            after_resume(e.0, res);
            return;
        }
        if !cycle || SHUTDOWN.load(Ordering::SeqCst) || events_due() {
            break;
        }
        chaos_jitter();
        let Some(n) = take_ready() else { break };
        if sched(|| !queue().ready.is_empty()) {
            notify_ready();
        }
        if !is_free(n) {
            lock_for(reason::SCHEDULER);
            run_one(n.0);
            return;
        }
        e = n;
    }
    lock_for(reason::SCHEDULER);
}

/// The scheduler's part after a task suspended or finished holding the
/// runtime lock.
fn after_resume(id: TaskId, res: CoroutineResult<Signal, ()>) {
    debug_assert!(holds_lock(), "a task suspended without the runtime lock");
    let t = task(id).unwrap();
    match res {
        CoroutineResult::Yield(Signal::Yield) => {
            if t.state.get() == State::Running {
                t.state.set(State::Ready);
            }
            push_ready(id);
        }
        CoroutineResult::Yield(Signal::Park) => {}
        CoroutineResult::Yield(Signal::ParkFree | Signal::YieldFree) => {
            unreachable!("handled by suspended_free")
        }
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
            if t.state.get() == State::Cleaning {
                // a cleanup failed: continue with the remaining cleanups
                start_cleanup(id);
            } else {
                start_cleanup(id);
            }
        }
        CoroutineResult::Return(()) => {
            if t.state.get() == State::Cleaning {
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
    t.state.set(State::Cleaning);
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
    push_ready(id);
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
    t.state.set(State::Done);
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
        Ok(mut g) => {
            EVENTS.store(false, Ordering::SeqCst);
            g.drain(..).collect()
        }
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
    note_timers();
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
    refresh_controlled(&c);
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
    note_timers();
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
/// Bytes of `lo..hi` backed by physical memory (pages touched so far).
fn resident(lo: usize, hi: usize) -> usize {
    let page = 4096usize;
    let (lo, hi) = (lo & !(page - 1), (hi + page - 1) & !(page - 1));
    if hi <= lo {
        return 0;
    }
    let mut v = vec![0u8; (hi - lo) / page];
    let r = unsafe { libc::mincore(lo as *mut libc::c_void, hi - lo, v.as_mut_ptr()) };
    if r != 0 {
        return 0;
    }
    v.iter().filter(|b| **b & 1 != 0).count() * page
}

/// A task's memory: its stack (address space reserved and pages in use),
/// the closure it runs with its captures, the values the runtime holds for
/// it, and the channels it is registered with.
fn memory_json(t: &Task) -> String {
    let (lo, hi) = t.stack;
    let captures =
        if t.entry != 0 { crate::gc::HEADER + unsafe { crate::gc::obj(t.entry).payload_size() } } else { 0 };
    format!(
        "{{\"stack_reserved\":{},\"stack_resident\":{},\"captures\":{captures},\"roots\":{},\"channels\":{}}}",
        hi - lo,
        resident(lo, hi),
        (t.roots.len() + t.temp_roots.len() + t.entry_args.len()) * 8,
        t.chan_regs.len()
    )
}

pub fn snapshot_json() -> String {
    let mut ts: Vec<&Box<Task>> = rt().tasks.values().collect();
    ts.sort_by_key(|t| t.id);
    let items: Vec<String> = ts
        .iter()
        .map(|t| {
            let state = match t.state.get() {
                State::Ready => "ready",
                State::Running => "running",
                State::Parked => "parked",
                State::Cleaning => "cleaning",
                State::Done => "done",
                State::Parking => "parked",
            };
            let stages: Vec<String> = t.stages.iter().map(|s| crate::intrinsics_io::json_quote(s)).collect();
            format!(
                "{{\"id\":{},\"name\":{},\"state\":\"{state}\",\"wait\":{},\"scope\":{},\"cleanups\":{},\"code_version\":{},\"stage\":{},\"stages\":[{}],\"memory\":{}}}",
                t.id,
                crate::intrinsics_io::json_quote(&t.name),
                if wait_reason(t).is_empty() {
                    "null".to_string()
                } else {
                    crate::intrinsics_io::json_quote(wait_reason(t))
                },
                t.scope.map(|s| s.to_string()).unwrap_or("null".into()),
                t.cleanups.len(),
                t.code_version,
                t.stages.last().map(|s| crate::intrinsics_io::json_quote(s)).unwrap_or("null".into()),
                stages.join(","),
                memory_json(t)
            )
        })
        .collect();
    format!("[{}]", items.join(","))
}
