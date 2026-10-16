//! Precise, nonmoving mark-sweep heap.
//!
//! Every managed object starts with a [`Header`] (descriptor pointer plus a
//! metadata word); the payload follows at [`HEADER`] bytes. Objects never
//! move, so raw pointers held in registers stay valid while the object is
//! reachable from a root. Roots are:
//!
//! - every task's shadow stack (frames pushed by compiled code),
//! - registered global slots (module-level values),
//! - runtime-held references (task entries, queued messages via their owning
//!   objects, temporary roots of runtime code).
//!
//! Collection happens only inside allocation, which is a safe point.
//!
//! `LANG_GC_VERIFY=1` checks the heap at every collection: each root and
//! traced pointer must refer to a live object, and freed objects are
//! poisoned and quarantined (not reused) so that a later use aborts with a
//! report instead of corrupting memory. Combined with `LANG_GC_STRESS=1`
//! it turns missing roots into immediate, attributed failures.

use std::alloc::{Layout, alloc_zeroed, dealloc};
use std::cell::UnsafeCell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};

use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};

use crate::desc::{Kind, Repr, TypeDesc};

#[repr(C)]
pub struct Header {
    pub desc: *const TypeDesc,
    /// bit 0: mark, bit 1: immortal, bits 32..64: payload size.
    pub meta: u64,
}

pub const HEADER: usize = std::mem::size_of::<Header>();
const MARK: u64 = 1;
const IMMORTAL: u64 = 2;

impl Header {
    pub fn desc(&self) -> &'static TypeDesc {
        unsafe { &*self.desc }
    }

    pub fn payload(&self) -> *mut u8 {
        unsafe { (self as *const Header as *mut u8).add(HEADER) }
    }

    pub fn payload_size(&self) -> usize {
        (self.meta >> 32) as usize
    }

    pub fn is_immortal(&self) -> bool {
        self.meta & IMMORTAL != 0
    }
}

/// View a pointer word as an object header.
///
/// # Safety
/// `w` must be a live managed pointer.
pub unsafe fn obj<'a>(w: u64) -> &'a Header {
    let o = unsafe { &*(w as *const Header) };
    if verify() && std::ptr::eq(o.desc, poison()) {
        freed_use(w, "the runtime");
    }
    o
}

pub struct Heap {
    live: usize,
    globals: Vec<*mut u64>,
    temp_roots: Vec<u64>,
    pinned: Vec<u64>,
    pub collections: usize,
    /// Stop-the-world pause times (nanoseconds).
    pub pause_total: u64,
    pub pause_max: u64,
    /// Verification mode: freed objects awaiting release (with their
    /// original descriptors), their total size, and immortal objects.
    quarantine: VecDeque<(*mut Header, &'static TypeDesc)>,
    quarantine_bytes: usize,
    freed: FxHashMap<usize, &'static TypeDesc>,
    immortals: FxHashSet<usize>,
}

struct HeapCell(UnsafeCell<Heap>);
unsafe impl Sync for HeapCell {}

static HEAP: HeapCell = HeapCell(UnsafeCell::new(Heap {
    live: 0,
    globals: Vec::new(),
    temp_roots: Vec::new(),
    pinned: Vec::new(),
    collections: 0,
    pause_total: 0,
    pause_max: 0,
    quarantine: VecDeque::new(),
    quarantine_bytes: 0,
    freed: FxHashMap::with_hasher(FxBuildHasher),
    immortals: FxHashSet::with_hasher(FxBuildHasher),
}));

/// The heap's shared state (roots, statistics, verification), used under
/// the runtime lock.
#[allow(clippy::mut_from_ref)]
pub fn heap() -> &'static mut Heap {
    unsafe { &mut *HEAP.0.get() }
}

/// Objects allocated by each worker. A worker appends to its own list
/// without the runtime lock; the collector reads and sweeps every list
/// only while the world is stopped (no worker allocating).
struct Objects([UnsafeCell<Vec<*mut Header>>; crate::task::MAX_WORKERS]);
unsafe impl Sync for Objects {}
static OBJECTS: Objects = Objects([const { UnsafeCell::new(Vec::new()) }; crate::task::MAX_WORKERS]);

#[allow(clippy::mut_from_ref)]
fn objects(w: usize) -> &'static mut Vec<*mut Header> {
    unsafe { &mut *OBJECTS.0[w].get() }
}

/// Bytes allocated since the last collection, the collection threshold,
/// and collection suppression (`NoGc`) depth.
static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static THRESHOLD: AtomicUsize = AtomicUsize::new(16 << 20);
static DISABLED: AtomicUsize = AtomicUsize::new(0);

fn layout(payload: usize) -> Layout {
    Layout::from_size_align(HEADER + payload.max(8), 8).unwrap()
}

/// Small objects (header and payload up to `SMALL_MAX` bytes) live in
/// 16-byte size classes carved from 256 KiB chunks; each worker keeps free
/// lists per class, filled by the sweep of its own object list, and
/// allocates from them without the lock (like its object list). Small
/// cells are reused, not returned to the system. Larger objects use the
/// system allocator.
const SMALL_MAX: usize = 256;
const CLASSES: usize = SMALL_MAX / 16;
const CHUNK: usize = 256 << 10;

struct Pool {
    free: [*mut u8; CLASSES],
    bump: *mut u8,
    left: usize,
}

struct Pools([UnsafeCell<Pool>; crate::task::MAX_WORKERS]);
unsafe impl Sync for Pools {}
static POOLS: Pools = Pools(
    [const { UnsafeCell::new(Pool { free: [std::ptr::null_mut(); CLASSES], bump: std::ptr::null_mut(), left: 0 }) };
        crate::task::MAX_WORKERS],
);

fn cell_size(payload: usize) -> usize {
    (HEADER + payload.max(8) + 15) & !15
}

/// A zeroed cell for an object with `payload` bytes, owned by worker `w`.
fn alloc_cell(w: usize, payload: usize) -> *mut Header {
    let size = cell_size(payload);
    if size > SMALL_MAX {
        return unsafe { alloc_zeroed(layout(payload)) as *mut Header };
    }
    let pool = unsafe { &mut *POOLS.0[w].get() };
    let class = size / 16 - 1;
    let p = pool.free[class];
    if !p.is_null() {
        unsafe {
            pool.free[class] = *(p as *mut *mut u8);
            std::ptr::write_bytes(p, 0, size);
        }
        return p as *mut Header;
    }
    if pool.left < size {
        let chunk = unsafe { alloc_zeroed(Layout::from_size_align(CHUNK, 16).unwrap()) };
        if chunk.is_null() {
            return chunk as *mut Header;
        }
        pool.bump = chunk;
        pool.left = CHUNK;
    }
    let p = pool.bump;
    pool.bump = unsafe { p.add(size) };
    pool.left -= size;
    p as *mut Header
}

/// Return the cell of a dead object with `payload` bytes to worker `w`'s
/// pool (only while the world is stopped, by the sweep of `w`'s own object
/// list: a pool is otherwise touched only by its worker's allocations).
fn free_cell(w: usize, p: *mut Header, payload: usize) {
    let size = cell_size(payload);
    if size > SMALL_MAX {
        return unsafe { dealloc(p as *mut u8, layout(payload)) };
    }
    let pool = unsafe { &mut *POOLS.0[w].get() };
    let class = size / 16 - 1;
    unsafe { *(p as *mut *mut u8) = pool.free[class] };
    pool.free[class] = p as *mut u8;
}

/// `LANG_GC_STRESS=1`: collect before every allocation (a debugging aid
/// that turns missing roots into deterministic failures).
fn stress() -> bool {
    static STRESS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *STRESS.get_or_init(|| std::env::var_os("LANG_GC_STRESS").is_some_and(|v| v == "1"))
}

/// `LANG_GC_VERIFY=1`: verify the heap at every collection and quarantine
/// freed objects (see the module documentation).
pub fn verify() -> bool {
    static VERIFY: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *VERIFY.get_or_init(|| std::env::var_os("LANG_GC_VERIFY").is_some_and(|v| v == "1"))
}

/// The descriptor of a freed (quarantined) object.
fn poison() -> &'static TypeDesc {
    static D: std::sync::OnceLock<&'static TypeDesc> = std::sync::OnceLock::new();
    D.get_or_init(|| {
        TypeDesc::new("<freed object>", Kind::Opaque { trace: None, finalize: None, args: vec![] }).leak()
    })
}

/// Quarantined objects are released once they exceed this many bytes.
const QUARANTINE_BYTES: usize = 256 << 20;

/// A freed object was used: report what it was and abort.
fn freed_use(w: u64, by: &str) -> ! {
    let was = heap().freed.get(&(w as usize)).map(|d| d.name.as_str()).unwrap_or("?");
    eprintln!("fatal: heap verification: {by} used freed object {w:#x} (a `{was}`)");
    eprintln!("{}", std::backtrace::Backtrace::force_capture());
    std::process::abort();
}

/// Allocate a zeroed object with `payload` bytes. May collect first.
struct Finalized;

impl Drop for Finalized {
    fn drop(&mut self) {
        FINALIZING.store(false, Ordering::Relaxed);
    }
}

pub fn alloc(desc: &'static TypeDesc, payload: usize) -> u64 {
    debug_assert!(!FINALIZING.load(Ordering::Relaxed), "a finalizer allocated");
    let chaos = crate::task::chaos().is_some() && crate::task::chaos_next().is_multiple_of(512);
    let due = || ALLOCATED.load(Ordering::Relaxed) > THRESHOLD.load(Ordering::Relaxed);
    if (due() || stress() || chaos) && DISABLED.load(Ordering::Relaxed) == 0 {
        debt_flush();
        if crate::task::holds_lock() {
            collect();
        } else {
            // generated code allocating without the runtime lock: take it
            // (this is a safe point: its pointers are rooted) and collect
            // unless another worker just did
            crate::task::enter_runtime_for(crate::task::reason::GC);
            if due() || stress() || chaos {
                collect();
            }
            crate::task::leave_runtime();
        }
    }
    unsafe {
        let w = crate::task::worker_index();
        let p = alloc_cell(w, payload);
        if p.is_null() {
            eprintln!("fatal: out of memory");
            std::process::abort();
        }
        (*p).desc = desc;
        (*p).meta = (payload as u64) << 32;
        let list = objects(w);
        list.push(p);
        COUNTS[w].0.store(list.len(), Ordering::Relaxed);
        // collection pressure is published in blocks: every worker adding
        // to one shared counter per allocation would contend on it
        let d = &DEBT[w].0;
        let owed = d.load(Ordering::Relaxed) + HEADER + payload;
        if owed >= DEBT_BLOCK {
            d.store(0, Ordering::Relaxed);
            ALLOCATED.fetch_add(owed, Ordering::Relaxed);
        } else {
            d.store(owed, Ordering::Relaxed);
        }
        p as u64
    }
}

/// Bytes a worker allocated but has not yet added to `ALLOCATED` (only that
/// worker writes its slot, except the collector while the world is stopped).
/// The collection threshold may be overshot by at most one block per worker.
static DEBT: [Count; crate::task::MAX_WORKERS] =
    [const { Count(std::sync::atomic::AtomicUsize::new(0)) }; crate::task::MAX_WORKERS];
const DEBT_BLOCK: usize = 64 << 10;

/// Publish this worker's allocation debt (before deciding to collect).
fn debt_flush() {
    let d = &DEBT[crate::task::worker_index()].0;
    let owed = d.swap(0, Ordering::Relaxed);
    if owed > 0 {
        ALLOCATED.fetch_add(owed, Ordering::Relaxed);
    }
}

/// Count memory owned outside the heap by a just-allocated object (map
/// tries, buffers) toward the next collection.
pub fn add_pressure(bytes: usize) {
    ALLOCATED.fetch_add(bytes, Ordering::Relaxed);
}

/// Allocate an object that is never collected (literals, static closures).
pub fn alloc_immortal(desc: &'static TypeDesc, payload: usize) -> u64 {
    unsafe {
        let p = alloc_zeroed(layout(payload)) as *mut Header;
        (*p).desc = desc;
        (*p).meta = ((payload as u64) << 32) | IMMORTAL;
        if verify() {
            heap().immortals.insert(p as usize);
        }
        p as u64
    }
}

/// Register a global slot (module-level value) as a root.
pub fn register_global(slot: *mut u64) {
    heap().globals.push(slot);
}

/// The temporary roots of the running task (each task has its own: tasks
/// interleave at suspension points, so one shared stack would let one
/// task's guard drop another's roots), or the heap's own outside tasks.
fn temp_roots() -> &'static mut Vec<u64> {
    match crate::task::current_temp_roots() {
        Some(v) => unsafe { &mut *v },
        None => &mut heap().temp_roots,
    }
}

/// Keep `w` alive until the matching [`pop_root`]. Runtime code uses this
/// while it allocates several objects.
pub fn push_root(w: u64) {
    temp_roots().push(w);
}

/// Keep `w` alive until [`unpin`] (runtime registries).
pub fn pin(w: u64) {
    heap().pinned.push(w);
}

pub fn unpin(w: u64) {
    let p = &mut heap().pinned;
    if let Some(i) = p.iter().rposition(|x| *x == w) {
        p.remove(i);
    }
}

pub fn pop_root() {
    temp_roots().pop();
}

/// RAII guard over temporary roots (of the task that created it).
pub struct Roots(*mut Vec<u64>, usize);

impl Roots {
    pub fn new() -> Roots {
        let v = temp_roots();
        Roots(v as *mut Vec<u64>, v.len())
    }

    pub fn add(&self, w: u64) -> u64 {
        unsafe { (*self.0).push(w) };
        w
    }
}

impl Default for Roots {
    fn default() -> Self {
        Roots::new()
    }
}

impl Drop for Roots {
    fn drop(&mut self) {
        unsafe { (*self.0).truncate(self.1) };
    }
}

/// Suppress collection (runtime sections holding unrooted pointers).
pub struct NoGc;

impl NoGc {
    pub fn new() -> NoGc {
        DISABLED.fetch_add(1, Ordering::Relaxed);
        NoGc
    }
}

impl Default for NoGc {
    fn default() -> Self {
        NoGc::new()
    }
}

impl Drop for NoGc {
    fn drop(&mut self) {
        DISABLED.fetch_sub(1, Ordering::Relaxed);
    }
}

pub fn collect() {
    let started = std::time::Instant::now();
    // several workers: wait until none executes compiled code
    crate::task::stop_the_world();
    collect_inner();
    crate::task::resume_the_world();
    let h = heap();
    let ns = started.elapsed().as_nanos() as u64;
    h.pause_total += ns;
    h.pause_max = h.pause_max.max(ns);
}

fn collect_inner() {
    let h = heap();
    h.collections += 1;
    let mut stack: Vec<u64> = Vec::new();
    let checking = verify();
    let live: FxHashSet<usize> =
        if checking { all_objects().map(|p| p as usize).collect() } else { FxHashSet::default() };
    // a referenced word must be a live or immortal object
    let check = |w: u64, by: &dyn Fn() -> String| {
        if !checking || w == 0 || live.contains(&(w as usize)) || h.immortals.contains(&(w as usize)) {
            return;
        }
        if h.freed.contains_key(&(w as usize)) {
            freed_use(w, &by());
        }
        eprintln!("fatal: heap verification: {} holds {w:#x}, which is not a heap object", by());
        std::process::abort();
    };
    let mark = |w: u64, stack: &mut Vec<u64>| {
        if w == 0 {
            return;
        }
        let o = w as *mut Header;
        unsafe {
            // immortal objects (literals, static closures) never reference
            // mortal objects and are never traced
            if (*o).meta & (MARK | IMMORTAL) != 0 {
                return;
            }
            (*o).meta |= MARK;
        }
        stack.push(w);
    };
    // roots
    for &g in &h.globals {
        let w = unsafe { *g };
        check(w, &|| "a module-level value".into());
        mark(w, &mut stack);
    }
    for &r in &h.temp_roots {
        // runtime helpers root only pointer-typed words; this guard keeps a
        // stray scalar from being dereferenced
        if r % 8 == 0 && r >= 4096 {
            check(r, &|| "a temporary runtime root".into());
            mark(r, &mut stack);
        }
    }
    for &r in &h.pinned {
        check(r, &|| "a pinned runtime root".into());
        mark(r, &mut stack);
    }
    crate::task::for_each_root(&mut |w| {
        check(w, &|| "a task root (stack frame, entry, result, or held value)".into());
        mark(w, &mut stack)
    });
    // trace: children are marked as they are visited (no per-object
    // scratch storage during the pause)
    while let Some(w) = stack.pop() {
        let mut pending = std::mem::take(&mut stack);
        trace(w, &mut |c| {
            check(c, &|| format!("a `{}` object", unsafe { obj(w) }.desc().name));
            mark(c, &mut pending);
        });
        stack = pending;
    }
    // sweep each worker's list in place
    let mut live = 0usize;
    let lists: Vec<usize> = (0..crate::task::MAX_WORKERS).filter(|&w| !objects(w).is_empty()).collect();
    let total: usize = lists.iter().map(|&w| objects(w).len()).sum();
    if !checking && lists.len() > 1 && total >= PARALLEL_SWEEP {
        live = sweep_parallel(&lists);
    } else {
        for wl in lists {
            objects(wl).retain(|&p| unsafe {
                if (*p).meta & MARK != 0 {
                    (*p).meta &= !MARK;
                    live += HEADER + (*p).payload_size();
                    true
                } else {
                    finalize(p);
                    if checking {
                        // poison and keep: a later use is reported, not reused
                        let size = (*p).payload_size();
                        let was = (*p).desc();
                        std::ptr::write_bytes((*p).payload(), 0xA5, size.max(8));
                        (*p).desc = poison();
                        h.freed.insert(p as usize, was);
                        h.quarantine.push_back((p, was));
                        h.quarantine_bytes += HEADER + size;
                    } else {
                        free_cell(wl, p, (*p).payload_size());
                    }
                    false
                }
            });
        }
    }
    while h.quarantine_bytes > QUARANTINE_BYTES {
        let Some((p, _)) = h.quarantine.pop_front() else { break };
        unsafe {
            let size = (*p).payload_size();
            h.quarantine_bytes -= HEADER + size;
            h.freed.remove(&(p as usize));
            free_cell(0, p, size);
        }
    }
    for (wl, c) in COUNTS.iter().enumerate() {
        c.0.store(objects(wl).len(), Ordering::Relaxed);
    }
    h.live = live;
    // every worker is stopped: their unpublished debt is part of `live`
    for d in &DEBT {
        d.0.store(0, Ordering::Relaxed);
    }
    ALLOCATED.store(live, Ordering::Relaxed);
    THRESHOLD.store((live * 2).max(16 << 20), Ordering::Relaxed);
}

/// Heaps with at least this many objects (spread over several workers'
/// lists) are swept in parallel.
const PARALLEL_SWEEP: usize = 32 << 10;

/// Sweep the given workers' lists on one thread each (the world is
/// stopped, so the other workers are not using their lists or pools; each
/// thread frees only into its list's own pool). Dead objects with a
/// finalizer are finalized and freed afterwards on this thread, in list
/// order: finalizers touch shared runtime state. Returns the live bytes.
fn sweep_parallel(lists: &[usize]) -> usize {
    struct Send<T>(T);
    unsafe impl<T> std::marker::Send for Send<T> {}
    let sweep = |wl: usize| -> (usize, Vec<*mut Header>) {
        let mut live = 0usize;
        let mut fin = vec![];
        objects(wl).retain(|&p| unsafe {
            if (*p).meta & MARK != 0 {
                (*p).meta &= !MARK;
                live += HEADER + (*p).payload_size();
                true
            } else {
                if let Kind::Opaque { finalize: Some(_), .. } = &(*p).desc().kind {
                    fin.push(p);
                } else {
                    free_cell(wl, p, (*p).payload_size());
                }
                false
            }
        });
        (live, fin)
    };
    let results: Vec<(usize, Vec<*mut Header>)> = std::thread::scope(|sc| {
        let handles: Vec<_> = lists[1..].iter().map(|&wl| sc.spawn(move || Send(sweep(wl)))).collect();
        let mut out = vec![sweep(lists[0])];
        out.extend(handles.into_iter().map(|h| h.join().expect("sweep thread").0));
        out
    });
    let mut live = 0;
    for (&wl, (l, fin)) in lists.iter().zip(results) {
        live += l;
        for p in fin {
            finalize(p);
            free_cell(wl, p, unsafe { (*p).payload_size() });
        }
    }
    live
}

/// Finalizers run inside a collection: they release native resources and
/// must not allocate (checked in debug builds).
static FINALIZING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn finalize(p: *mut Header) {
    let desc = unsafe { (*p).desc() };
    FINALIZING.store(true, Ordering::Relaxed);
    let _done = Finalized;
    if let Kind::Opaque { finalize: Some(f), .. } = &desc.kind {
        f(p)
    }
}

/// Every managed object (while the world is stopped or under the lock in
/// single-worker mode).
fn all_objects() -> impl Iterator<Item = *mut Header> {
    (0..crate::task::MAX_WORKERS).flat_map(|w| objects(w).iter().copied())
}

/// Visit the managed pointers inside object `w`.
pub fn trace(w: u64, f: &mut dyn FnMut(u64)) {
    let o = unsafe { obj(w) };
    let desc = o.desc();
    let base = o.payload();
    let fields = |fs: &[crate::desc::Field], base: *mut u8, f: &mut dyn FnMut(u64)| {
        for fl in fs {
            if fl.repr == Repr::Ptr {
                f(unsafe { Repr::Ptr.load(base.add(fl.offset as usize)) });
            }
        }
    };
    match &desc.kind {
        Kind::Record { fields: fs, .. } => fields(fs, base, f),
        Kind::Closure { captures, .. } => fields(captures, base, f),
        Kind::Enum { variants, .. } => {
            let tag = unsafe { *(base as *const u32) };
            if let Some(v) = variants.get(tag as usize) {
                fields(&v.fields, base, f);
            }
        }
        Kind::List { .. } => crate::list::trace(o as *const Header as *mut Header, f),
        Kind::Map { .. } | Kind::Set { .. } => crate::map::trace(o as *const Header as *mut Header, f),
        Kind::Opaque { trace: Some(t), .. } => t(o as *const Header as *mut Header, f),
        _ => {}
    }
}

pub fn stats() -> (usize, usize, usize) {
    let h = heap();
    let objects = COUNTS.iter().map(|c| c.0.load(Ordering::Relaxed)).sum();
    let owed: usize = DEBT.iter().map(|d| d.0.load(Ordering::Relaxed)).sum();
    (objects, ALLOCATED.load(Ordering::Relaxed) + owed, h.collections)
}

/// Each worker's object count, published by the worker itself (others
/// must not read a list it is appending to).
#[repr(align(64))]
struct Count(std::sync::atomic::AtomicUsize);
static COUNTS: [Count; crate::task::MAX_WORKERS] =
    [const { Count(std::sync::atomic::AtomicUsize::new(0)) }; crate::task::MAX_WORKERS];

/// Total and longest collection pause, in nanoseconds.
pub fn pauses() -> (u64, u64) {
    let h = heap();
    (h.pause_total, h.pause_max)
}
