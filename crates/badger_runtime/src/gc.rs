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
//! `BADGER_GC_VERIFY=1` checks the heap at every collection: each root and
//! traced pointer must refer to a live object, and freed objects are
//! poisoned and quarantined (not reused) so that a later use aborts with a
//! report instead of corrupting memory. Combined with `BADGER_GC_STRESS=1`
//! it turns missing roots into immediate, attributed failures.

use std::alloc::{Layout, alloc_zeroed, dealloc};
use std::cell::UnsafeCell;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};

use crate::desc::{Kind, Repr, TypeDesc};

#[cfg(feature = "gc-profile")]
pub(crate) mod allocations;
#[cfg(feature = "gc-profile")]
pub mod diagnostics;
#[cfg(any(feature = "gc-profile", test))]
mod sweep;

#[repr(C)]
pub struct Header {
    pub desc: *const TypeDesc,
    /// Bits 0..3: GC flags; 3..32: client metadata; 32..64: payload size.
    pub meta: u64,
}

pub const HEADER: usize = std::mem::size_of::<Header>();
/// Header meta bits. With generational collection a survivor keeps its
/// mark: marked objects are the old generation, which a minor collection
/// neither traces through nor sweeps; an old object written with a pointer
/// is remembered (see [`barrier`]) so its children are traced.
pub const MARK: u64 = 1;
pub const IMMORTAL: u64 = 2;
pub const REMEMBERED: u64 = 4;
/// Reserved for object-specific scalar metadata. GC flag updates must preserve
/// these bits; allocation/reuse starts them at zero. HAMT ownership uses them.
pub(crate) const CLIENT_META: u64 = 0xffff_fff8;

impl Header {
    pub fn desc(&self) -> &'static TypeDesc {
        unsafe { &*self.desc }
    }

    /// The object's payload. Objects are reached through pointer words
    /// (`u64`), which expose their allocation's provenance; the payload
    /// pointer takes that provenance from the address, as a pointer derived
    /// from `&self` may only touch the header itself.
    pub fn payload(&self) -> *mut u8 {
        std::ptr::with_exposed_provenance_mut((self as *const Header).addr() + HEADER)
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
    minor_collections: usize,
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
    minor_collections: 0,
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
static EXTERNAL_LIVE: AtomicUsize = AtomicUsize::new(0);
static EXTERNAL_PRESSURE: AtomicUsize = AtomicUsize::new(0);
static MINOR_COLLECTION: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub const MAX_CONSECUTIVE_MINORS: usize = 8;
pub(crate) fn is_minor_collection() -> bool {
    MINOR_COLLECTION.load(Ordering::Relaxed)
}
static POOL_RESERVED: AtomicUsize = AtomicUsize::new(0);
pub fn pool_reserved_bytes() -> usize {
    POOL_RESERVED.load(Ordering::Relaxed)
}

fn layout(payload: usize) -> Layout {
    Layout::from_size_align(HEADER + payload.max(8), 8).unwrap()
}

/// Small objects (header and payload up to `SMALL_MAX` bytes) live in
/// 16-byte cells carved from each worker's blocks (see [`Pool`]); larger
/// objects use the system allocator.
// Dense HAMT roots occupy 288 bytes; pool their frequent persistent path copies.
const SMALL_MAX: usize = 512;
/// The allocation census's 16-byte size classes (diagnostic builds can
/// compare larger pooled cells without changing their binary).
#[cfg(feature = "gc-profile")]
const CLASSES: usize = 1024 / 16;
fn small_max() -> usize {
    #[cfg(feature = "gc-profile")]
    return diagnostics::pool_max(SMALL_MAX);
    #[cfg(not(feature = "gc-profile"))]
    SMALL_MAX
}
const CHUNK: usize = 256 << 10;
/// Chunks after a worker's first: 2 MiB, aligned and advised as a huge
/// page, so a program that allocates much takes one page fault per chunk
/// instead of 512 (a small program keeps its small first chunk).
const BIG_CHUNK: usize = 2 << 20;
fn refill_bytes() -> usize {
    #[cfg(feature = "gc-profile")]
    return diagnostics::pool_refill(BIG_CHUNK);
    #[cfg(not(feature = "gc-profile"))]
    BIG_CHUNK
}

/// Chunks are divided into aligned blocks of `BLOCK` bytes, and blocks into
/// lines of `LINE` bytes. A block starts with its line bitmap (one bit per
/// line, set while a live or quarantined cell overlaps the line), which
/// occupies its first `HEADER_LINES` lines; their bits are always set. A cell
/// never crosses a block boundary, so its block is its address rounded down.
const BLOCK: usize = 32 << 10;
const LINE: usize = 32;
const LINES: usize = BLOCK / LINE;
type LineMap = [u64; LINES / 64];
const HEADER_LINES: usize = std::mem::size_of::<LineMap>() / LINE;
const HEADER_BITS: u64 = (1 << HEADER_LINES) - 1;
/// Cells above this size that miss the current hole take the overflow
/// region instead, so the rest of the hole is kept for smaller cells.
const MEDIUM: usize = 4 * LINE;

#[derive(Clone, Copy)]
struct Chunk {
    base: *mut u8,
    bytes: usize,
    align: usize,
    /// Blocks handed out, from the start; the blocks after them are still
    /// zero and have no line bitmap yet.
    used: usize,
}

impl Chunk {
    fn blocks(&self) -> usize {
        self.bytes / BLOCK
    }

    fn block(&self, i: usize) -> *mut u8 {
        unsafe { self.base.add(i * BLOCK) }
    }
}

/// The line bitmap of the block holding `p`.
fn line_map<'a>(p: *mut u8) -> &'a mut LineMap {
    unsafe { &mut *(p.map_addr(|a| a & !(BLOCK - 1)) as *mut LineMap) }
}

/// The free lines from line `from` on: the next run of clear bits, as a
/// line range.
fn next_hole(map: &LineMap, from: usize) -> Option<(usize, usize)> {
    let mut start = from;
    while start < LINES {
        let free = !map[start / 64] >> (start % 64);
        if free != 0 {
            start += free.trailing_zeros() as usize;
            break;
        }
        start = (start / 64 + 1) * 64;
    }
    if start >= LINES {
        return None;
    }
    let mut end = start;
    while end < LINES {
        let used = map[end / 64] >> (end % 64);
        if used != 0 {
            end += used.trailing_zeros() as usize;
            break;
        }
        end = (end / 64 + 1) * 64;
    }
    Some((start, end.min(LINES)))
}

fn block_empty(map: &LineMap) -> bool {
    map[0] == HEADER_BITS && map[1..].iter().all(|&w| w == 0)
}

/// Mark the lines overlapped by the small cell at `p` live.
fn mark_lines(p: *mut Header, payload: usize) {
    let size = cell_size(payload);
    if size > small_max() {
        return;
    }
    let offset = p.addr() & (BLOCK - 1);
    let map = line_map(p as *mut u8);
    for line in offset / LINE..=(offset + size - 1) / LINE {
        map[line / 64] |= 1 << (line % 64);
    }
}

/// A worker's small-cell allocator, Immix-style: cells are bump-allocated
/// in holes (runs of free lines) of the worker's blocks, and a dead cell is
/// reclaimed by its lines staying clear at the next collection, so
/// partly live blocks are reused in place and nothing moves. The pool is
/// touched only by its worker's allocations, and by the collector while
/// the world is stopped, which marks the lines of the survivors of its
/// worker's object list and then rebuilds the block lists
/// ([`Pool::rebuild`]).
struct Pool {
    /// The hole being allocated from: the next cell and the bytes left.
    cursor: *mut u8,
    left: usize,
    /// The hole held dead cells: its cells are zeroed as they are handed out.
    recycled: bool,
    /// The overflow region (a block with no live line) for medium cells.
    over: *mut u8,
    over_left: usize,
    over_recycled: bool,
    /// The partly live block being searched for holes, and the line to
    /// search from.
    scan: *mut u8,
    scan_line: usize,
    /// Blocks with live and free lines, and blocks with no live line, as
    /// the last collection left them (taken from the back).
    recyclable: Vec<*mut u8>,
    empty: Vec<*mut u8>,
    chunks: Vec<Chunk>,
    /// Whether the last cell came from dead cells' lines (the census).
    #[cfg(feature = "gc-profile")]
    reused: bool,
}

struct Pools([UnsafeCell<Pool>; crate::task::MAX_WORKERS]);
unsafe impl Sync for Pools {}
static POOLS: Pools = Pools(
    [const {
        UnsafeCell::new(Pool {
            cursor: std::ptr::null_mut(),
            left: 0,
            recycled: false,
            over: std::ptr::null_mut(),
            over_left: 0,
            over_recycled: false,
            scan: std::ptr::null_mut(),
            scan_line: 0,
            recyclable: Vec::new(),
            empty: Vec::new(),
            chunks: Vec::new(),
            #[cfg(feature = "gc-profile")]
            reused: false,
        })
    }; crate::task::MAX_WORKERS],
);

#[allow(clippy::mut_from_ref)]
fn pool(w: usize) -> &'static mut Pool {
    unsafe { &mut *POOLS.0[w].get() }
}

impl Pool {
    /// The next hole of at least `size` bytes in the partly live blocks
    /// (smaller holes on the way are left until the next collection).
    fn next_hole(&mut self, size: usize) -> bool {
        loop {
            if self.scan.is_null() {
                let Some(block) = self.recyclable.pop() else { return false };
                self.scan = block;
                self.scan_line = HEADER_LINES;
            }
            match next_hole(line_map(self.scan), self.scan_line) {
                Some((start, end)) => {
                    self.scan_line = end;
                    if (end - start) * LINE >= size {
                        self.cursor = unsafe { self.scan.add(start * LINE) };
                        self.left = (end - start) * LINE;
                        self.recycled = true;
                        return true;
                    }
                }
                None => self.scan = std::ptr::null_mut(),
            }
        }
    }

    /// Bump-allocate `size` bytes from the overflow region or the hole,
    /// which has room for them.
    #[inline]
    fn bump(&mut self, overflow: bool, size: usize) -> *mut Header {
        let (cursor, left, recycled) = if overflow {
            (&mut self.over, &mut self.over_left, self.over_recycled)
        } else {
            (&mut self.cursor, &mut self.left, self.recycled)
        };
        let p = *cursor;
        *cursor = unsafe { p.add(size) };
        *left -= size;
        if recycled {
            unsafe { std::ptr::write_bytes(p, 0, size) };
        }
        #[cfg(feature = "gc-profile")]
        {
            self.reused = recycled;
        }
        p as *mut Header
    }

    /// A block with no live line, and whether it held dead cells: an empty
    /// block, else a fresh one (from a new chunk when none is left and
    /// `grow`).
    fn take_block(&mut self, grow: bool) -> Option<(*mut u8, bool)> {
        if let Some(block) = self.empty.pop() {
            return Some((block, true));
        }
        if !self.chunks.last().is_some_and(|c| c.used < c.blocks()) {
            if !grow {
                return None;
            }
            let bytes = if self.chunks.is_empty() { CHUNK } else { refill_bytes() };
            let align = if bytes == BIG_CHUNK { BIG_CHUNK } else { BLOCK };
            let base = unsafe { alloc_zeroed(Layout::from_size_align(bytes, align).unwrap()) };
            if base.is_null() {
                return None;
            }
            // a hint only (Miri does not model it)
            if bytes == BIG_CHUNK && !cfg!(miri) {
                unsafe { libc::madvise(base as *mut libc::c_void, bytes, libc::MADV_HUGEPAGE) };
            }
            self.chunks.push(Chunk { base, bytes, align, used: 0 });
            POOL_RESERVED.fetch_add(bytes, Ordering::Relaxed);
            #[cfg(feature = "gc-profile")]
            allocations::record_pool_event(allocations::PoolEvent::Refill, bytes);
        }
        let chunk = self.chunks.last_mut().unwrap();
        let block = chunk.block(chunk.used);
        chunk.used += 1;
        line_map(block)[0] = HEADER_BITS;
        Some((block, false))
    }

    /// After a collection's sweep (when the lines of every live or
    /// quarantined cell are marked): with `trim`, whole chunks with no live
    /// line are released; every block not yet handed out stays fresh, the
    /// others are sorted by their line bitmaps, and allocation starts over.
    fn rebuild(&mut self, trim: bool) {
        if trim {
            self.chunks.retain(|chunk| {
                let empty = (0..chunk.used).all(|i| block_empty(line_map(chunk.block(i))));
                if empty {
                    unsafe { dealloc(chunk.base, Layout::from_size_align(chunk.bytes, chunk.align).unwrap()) };
                    POOL_RESERVED.fetch_sub(chunk.bytes, Ordering::Relaxed);
                    #[cfg(feature = "gc-profile")]
                    allocations::record_pool_event(allocations::PoolEvent::Trim, chunk.bytes);
                }
                !empty
            });
        }
        self.cursor = std::ptr::null_mut();
        self.left = 0;
        self.over = std::ptr::null_mut();
        self.over_left = 0;
        self.scan = std::ptr::null_mut();
        self.recyclable.clear();
        self.empty.clear();
        for chunk in self.chunks.iter().rev() {
            for i in (0..chunk.used).rev() {
                let block = chunk.block(i);
                let map = line_map(block);
                if block_empty(map) {
                    self.empty.push(block);
                } else if map.iter().any(|&w| w != u64::MAX) {
                    self.recyclable.push(block);
                }
            }
        }
    }

    /// Before a full collection's sweep: no line is live.
    fn clear_lines(&mut self) {
        for chunk in &self.chunks {
            for i in 0..chunk.used {
                let map = line_map(chunk.block(i));
                map.fill(0);
                map[0] = HEADER_BITS;
            }
        }
    }
}

fn cell_size(payload: usize) -> usize {
    (HEADER + payload.max(8) + 15) & !15
}

/// A zeroed cell for an object with `payload` bytes, owned by worker `w`.
#[inline]
fn alloc_cell(w: usize, payload: usize) -> *mut Header {
    let size = cell_size(payload);
    if size > small_max() {
        return unsafe { alloc_zeroed(layout(payload)) as *mut Header };
    }
    let pool = pool(w);
    if pool.left >= size {
        return pool.bump(false, size);
    }
    alloc_cell_slow(pool, size)
}

/// A cell that misses the current hole. A medium cell takes the overflow
/// region while the hole still has room for smaller cells and a block with
/// no live line is at hand; otherwise the next hole that fits is taken, or
/// a block with no live line (from a new chunk if need be).
#[cold]
fn alloc_cell_slow(pool: &mut Pool, size: usize) -> *mut Header {
    if size > MEDIUM && pool.left >= MEDIUM {
        if pool.over_left < size
            && let Some((block, recycled)) = pool.take_block(false)
        {
            pool.over = unsafe { block.add(HEADER_LINES * LINE) };
            pool.over_left = BLOCK - HEADER_LINES * LINE;
            pool.over_recycled = recycled;
        }
        if pool.over_left >= size {
            return pool.bump(true, size);
        }
    }
    if !pool.next_hole(size) {
        let Some((block, recycled)) = pool.take_block(true) else { return std::ptr::null_mut() };
        pool.cursor = unsafe { block.add(HEADER_LINES * LINE) };
        pool.left = BLOCK - HEADER_LINES * LINE;
        pool.recycled = recycled;
    }
    pool.bump(false, size)
}

/// Release the cell of a dead object with `payload` bytes (only while the
/// world is stopped). A small cell needs nothing: its lines are not marked
/// by the sweep, so they are free once its block is rebuilt.
fn free_cell(p: *mut Header, payload: usize) {
    if cell_size(payload) > small_max() {
        unsafe { dealloc(p as *mut u8, layout(payload)) };
    }
}

/// `BADGER_GC_STRESS=1`: collect before every allocation (a debugging aid
/// that turns missing roots into deterministic failures).
fn stress() -> bool {
    static STRESS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *STRESS.get_or_init(|| std::env::var_os("BADGER_GC_STRESS").is_some_and(|v| v == "1"))
}

/// `BADGER_GC_VERIFY=1`: verify the heap at every collection and quarantine
/// freed objects (see the module documentation).
pub fn verify() -> bool {
    static VERIFY: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *VERIFY.get_or_init(|| std::env::var_os("BADGER_GC_VERIFY").is_some_and(|v| v == "1"))
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

#[cfg_attr(feature = "gc-profile", track_caller)]
pub fn alloc(desc: &'static TypeDesc, payload: usize) -> u64 {
    debug_assert!(!FINALIZING.load(Ordering::Relaxed), "a finalizer allocated");
    let chaos = crate::task::chaos().is_some() && crate::task::chaos_next().is_multiple_of(512);
    let due = || {
        ALLOCATED.load(Ordering::Relaxed).saturating_add(EXTERNAL_PRESSURE.load(Ordering::Relaxed))
            > THRESHOLD.load(Ordering::Relaxed)
    };
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
        #[cfg(feature = "gc-profile")]
        let source = allocations::cell_source(w, payload);
        (*p).desc = desc;
        (*p).meta = (payload as u64) << 32;
        let list = objects(w);
        list.push(p);
        #[cfg(feature = "gc-profile")]
        allocations::record(w, desc, payload, list.len(), std::panic::Location::caller(), source);
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
    EXTERNAL_PRESSURE.fetch_add(bytes, Ordering::Relaxed);
}
pub fn external_alloc(bytes: usize) {
    EXTERNAL_LIVE.fetch_add(bytes, Ordering::Relaxed);
    add_pressure(bytes);
}
pub fn external_free(bytes: usize) {
    EXTERNAL_LIVE.fetch_sub(bytes, Ordering::Relaxed);
}
pub fn memory_bytes() -> (usize, usize) {
    (heap().live, EXTERNAL_LIVE.load(Ordering::Relaxed))
}

/// Resident set size (Linux `/proc/self/statm`: pages × page size), the
/// collector's resident-memory input. 0 where unavailable.
pub fn resident_bytes() -> usize {
    let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    let Ok(text) = std::fs::read_to_string("/proc/self/statm") else { return 0 };
    text.split_whitespace()
        .nth(1)
        .and_then(|p| p.parse::<usize>().ok())
        .map(|pages| pages.saturating_mul(page.max(1) as usize))
        .unwrap_or(0)
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
/// The current task's temporary roots (or the heap's, outside tasks), as a
/// raw pointer taken without making a reference to the heap, so guards
/// holding it stay valid while other code borrows the heap.
fn temp_roots_ptr() -> *mut Vec<u64> {
    match crate::task::current_temp_roots() {
        Some(v) => v,
        None => unsafe { &raw mut (*HEAP.0.get()).temp_roots },
    }
}

fn temp_roots() -> &'static mut Vec<u64> {
    unsafe { &mut *temp_roots_ptr() }
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
        let v = temp_roots_ptr();
        Roots(v, unsafe { (*v).len() })
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

/// The heap budget (`[runtime] heap_max_bytes`; 0: none).
static HEAP_MAX: AtomicUsize = AtomicUsize::new(0);
/// The next full collection runs for the budget, not the schedule.
static BUDGET_FULL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_heap_max(bytes: usize) {
    HEAP_MAX.store(bytes, Ordering::Relaxed);
}

/// An allocation of `payload` bytes by compiled code, under the heap
/// budget (see [`budget_check`]).
#[cfg_attr(feature = "gc-profile", track_caller)]
pub fn alloc_checked(desc: &'static TypeDesc, payload: usize) -> u64 {
    budget_check(payload);
    alloc(desc, payload)
}

/// The heap budget, checked where compiled code asks the runtime for
/// memory (before an operation changes anything; its arguments are rooted):
/// when live data and new allocation would pass the budget, a full
/// collection first; when live data still does, the allocating task fails
/// (a defect it or its supervisor can contain), rather than the process.
pub fn budget_check(payload: usize) {
    let max = HEAP_MAX.load(Ordering::Relaxed);
    if max > 0 && crate::task::current_id() != 0 {
        let owed: usize = DEBT.iter().map(|d| d.0.load(Ordering::Relaxed)).sum();
        // (generational: `ALLOCATED` counts since the last collection; else
        // it includes the live data)
        let since = ALLOCATED.load(Ordering::Relaxed).saturating_add(owed).saturating_add(payload);
        let used = if generational() {
            heap_bytes().saturating_add(since)
        } else {
            since.saturating_add(EXTERNAL_LIVE.load(Ordering::Relaxed))
        };
        if used > max {
            BUDGET_FULL.store(true, Ordering::Relaxed);
            let locked = crate::task::holds_lock();
            if !locked {
                crate::task::enter_runtime_for(crate::task::reason::GC);
            }
            collect();
            let live = heap_bytes();
            if !locked {
                crate::task::leave_runtime();
            }
            if live.saturating_add(payload) > max {
                crate::task::panic_current(
                    format!("heap budget exceeded: {live} bytes live, budget {max} (`heap_max_bytes`)"),
                    String::new(),
                );
            }
        }
    }
}

/// Bytes of live data at the last collection.
fn heap_bytes() -> usize {
    (if generational() { OLD_BYTES.load(Ordering::Relaxed) } else { heap().live })
        .saturating_add(EXTERNAL_LIVE.load(Ordering::Relaxed))
}

/// `BADGER_GC_GENERATIONAL=0`: every collection is a full one.
fn generational() -> bool {
    static G: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *G.get_or_init(|| std::env::var_os("BADGER_GC_GENERATIONAL").is_none_or(|v| v != "0"))
}

/// Old objects written with a pointer since the last collection, per
/// worker (only that worker pushes; the collector reads them all while
/// the world is stopped).
struct Remembered([UnsafeCell<Vec<u64>>; crate::task::MAX_WORKERS]);
unsafe impl Sync for Remembered {}
static REMEMBER: Remembered = Remembered([const { UnsafeCell::new(Vec::new()) }; crate::task::MAX_WORKERS]);

/// Where each worker's objects allocated since the last collection start
/// in its object list (the young generation).
static YOUNG_START: [AtomicUsize; crate::task::MAX_WORKERS] =
    [const { AtomicUsize::new(0) }; crate::task::MAX_WORKERS];

/// Bytes in the old generation, and the size at which the next collection
/// is a full one.
static OLD_BYTES: AtomicUsize = AtomicUsize::new(0);
static MAJOR_AT: AtomicUsize = AtomicUsize::new(64 << 20);

/// The write barrier: call after storing a managed pointer into object
/// `o` (any store into an object that may predate the last collection;
/// generated code and runtime containers do). An old object is
/// remembered, once, so the next minor collection traces its children.
#[inline]
pub fn barrier(o: u64) {
    if o == 0 {
        return;
    }
    let meta = unsafe { &*((o as *const u8).add(8) as *const std::sync::atomic::AtomicU64) };
    if meta.load(Ordering::Relaxed) & (MARK | IMMORTAL | REMEMBERED) != MARK {
        return;
    }
    if meta.fetch_or(REMEMBERED, Ordering::Relaxed) & REMEMBERED == 0 {
        unsafe { (*REMEMBER.0[crate::task::worker_index()].get()).push(o) };
    }
}

pub fn collect() {
    let started = std::time::Instant::now();
    // several workers: the grace period (no worker runs compiled code
    // until the resume)
    #[cfg(feature = "gc-profile")]
    let mut profile = diagnostics::Collection::new();
    crate::task::stop_the_world();
    #[cfg(feature = "gc-profile")]
    profile.phase("stop_world");
    collect_inner(
        #[cfg(feature = "gc-profile")]
        &mut profile,
    );
    crate::task::resume_the_world();
    #[cfg(feature = "gc-profile")]
    {
        profile.phase("resume_world");
        profile.finish();
    }
    let h = heap();
    let ns = started.elapsed().as_nanos() as u64;
    h.pause_total += ns;
    h.pause_max = h.pause_max.max(ns);
}

/// The collector's scratch space: the mark stack and the remembered
/// buffer. Both are empty when a collection ends, so their allocation is
/// kept for the next one instead of being made again. A nested collection
/// (a finalizer that allocates) takes fresh buffers instead of the shared
/// ones.
struct Scratch {
    in_use: AtomicBool,
    stack: UnsafeCell<Vec<u64>>,
    remembered: UnsafeCell<Vec<u64>>,
}
unsafe impl Sync for Scratch {}
static SCRATCH: Scratch = Scratch {
    in_use: AtomicBool::new(false),
    stack: UnsafeCell::new(Vec::new()),
    remembered: UnsafeCell::new(Vec::new()),
};

impl Scratch {
    /// Take the reusable buffers, or fresh ones when they are in use.
    fn take(&self) -> (Vec<u64>, Vec<u64>) {
        if self.in_use.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            return (Vec::new(), Vec::new());
        }
        (
            std::mem::take(unsafe { &mut *self.stack.get() }),
            std::mem::take(unsafe { &mut *self.remembered.get() }),
        )
    }

    /// Keep the buffers for the next collection.
    fn give(&self, stack: Vec<u64>, remembered: Vec<u64>) {
        unsafe {
            *self.stack.get() = stack;
            *self.remembered.get() = remembered;
        }
        self.in_use.store(false, Ordering::Release);
    }
}

fn collect_inner(#[cfg(feature = "gc-profile")] profile: &mut diagnostics::Collection) {
    #[cfg(feature = "gc-profile")]
    {
        profile.snapshot("objects_before", all_objects().count());
        profile.snapshot("external_before", EXTERNAL_LIVE.load(Ordering::Relaxed));
        profile.snapshot("resident_before", resident_bytes());
        profile.snapshot("pool_before", pool_reserved_bytes());
        profile.snapshot("pressure_managed", ALLOCATED.load(Ordering::Relaxed));
        profile.snapshot("pressure_external", EXTERNAL_PRESSURE.load(Ordering::Relaxed));
    }
    let h = heap();
    h.collections += 1;
    #[cfg(feature = "gc-profile")]
    profile.snapshot("collection", h.collections);
    EXTERNAL_PRESSURE.store(0, Ordering::Relaxed);
    let sticky = generational();
    // (collection stress: a full collection every 16th, so dead old objects
    // do not pile up between the many minor ones)
    let budget_full = BUDGET_FULL.swap(false, Ordering::Relaxed);
    let max_minors = MAX_CONSECUTIVE_MINORS;
    #[cfg(feature = "gc-profile")]
    let max_minors = diagnostics::max_minors(max_minors);
    let minor = sticky
        && h.minor_collections < max_minors
        && OLD_BYTES.load(Ordering::Relaxed) < MAJOR_AT.load(Ordering::Relaxed)
        && !(stress() && h.collections.is_multiple_of(16))
        && !budget_full;
    h.minor_collections = if minor { h.minor_collections + 1 } else { 0 };
    MINOR_COLLECTION.store(minor, Ordering::Relaxed);
    let (mut stack, mut remembered) = SCRATCH.take();
    // The buffers are reused: they still hold the previous collection's
    // entries, and those objects may be gone.
    stack.clear();
    remembered.clear();
    let checking = verify();
    if sticky && !minor {
        // a full collection: every object is unmarked (young) again
        for p in all_objects() {
            unsafe { (*p).meta &= !(MARK | REMEMBERED) };
        }
    }
    // remembered old objects: their children are traced (a full collection
    // reaches them anyway). The workers' buffers keep their capacity and
    // the collector's buffer is reused (it is empty at the end of a
    // collection).
    for r in &REMEMBER.0 {
        remembered.extend(unsafe { &mut *r.get() }.drain(..));
    }
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
    #[cfg(feature = "gc-profile")]
    let (candidates, marked, leaves) =
        (std::cell::Cell::new(0), std::cell::Cell::new(0), std::cell::Cell::new(0));
    #[cfg(feature = "gc-profile")]
    let (mut traced, mut remembered_traced) = (0, 0);
    let mark = |w: u64, stack: &mut Vec<u64>| {
        if w == 0 {
            return;
        }
        #[cfg(feature = "gc-profile")]
        candidates.set(candidates.get() + 1);
        let o = w as *mut Header;
        unsafe {
            // immortal objects (literals, static closures) never reference
            // mortal objects and are never traced
            if (*o).meta & (MARK | IMMORTAL) != 0 {
                return;
            }
            (*o).meta |= MARK;
            #[cfg(feature = "gc-profile")]
            marked.set(marked.get() + 1);
            // nothing inside to trace (scalar records, strings, bytes)
            if (*o).desc().is_leaf() {
                #[cfg(feature = "gc-profile")]
                leaves.set(leaves.get() + 1);
                return;
            }
        }
        stack.push(w);
    };
    #[cfg(feature = "gc-profile")]
    {
        profile.snapshot("minor", minor as usize);
        profile.phase("prepare");
    }
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
    #[cfg(feature = "gc-profile")]
    {
        profile.snapshot("remembered", remembered.len());
        profile.phase("roots");
    }
    for &o in &remembered {
        unsafe { (*(o as *mut Header)).meta &= !REMEMBERED };
        if minor {
            #[cfg(feature = "gc-profile")]
            {
                traced += 1;
                remembered_traced += 1;
            }
            trace(o, &mut |c| {
                check(c, &|| format!("a remembered `{}` object", unsafe { obj(o) }.desc().name));
                mark(c, &mut stack);
            });
        }
    }
    // trace: children are marked as they are visited (no per-object
    // scratch storage during the pause)
    while let Some(w) = stack.pop() {
        #[cfg(feature = "gc-profile")]
        {
            traced += 1;
        }
        let mut pending = std::mem::take(&mut stack);
        trace(w, &mut |c| {
            check(c, &|| format!("a `{}` object", unsafe { obj(w) }.desc().name));
            mark(c, &mut pending);
        });
        stack = pending;
    }
    #[cfg(feature = "gc-profile")]
    {
        profile.snapshot("mark_candidates", candidates.get());
        profile.snapshot("marked_objects", marked.get());
        profile.snapshot("marked_leaf_objects", leaves.get());
        profile.snapshot("traced_objects", traced);
        profile.snapshot("remembered_traced_objects", remembered_traced);
        profile.phase("trace");
    }
    if !minor {
        crate::list::clear_unmarked_directory_pages();
    }
    #[cfg(feature = "gc-profile")]
    profile.phase("weak_pages");
    // (under collection stress, a sample: a missed barrier also shows as a
    // use of a poisoned freed object)
    if minor && checking && (!stress() || h.collections.is_multiple_of(64)) {
        verify_minor(h);
    }
    #[cfg(feature = "gc-profile")]
    profile.phase("verify");
    // sweep each worker's list in place: a minor collection only its young
    // objects; survivors keep their marks (old) when collection is
    // generational
    let from =
        |w: usize| if minor { YOUNG_START[w].load(Ordering::Relaxed).min(objects(w).len()) } else { 0 };
    let mut live = 0usize;
    let lists: Vec<usize> = (0..crate::task::MAX_WORKERS).filter(|&w| objects(w).len() > from(w)).collect();
    let total: usize = lists.iter().map(|&w| objects(w).len() - from(w)).sum();
    #[cfg(feature = "gc-profile")]
    profile.snapshot("swept", total);
    #[cfg(feature = "gc-profile")]
    {
        profile.snapshot_counts(
            "sweep_work_per_worker",
            (0..crate::task::MAX_WORKERS).map(|w| objects(w).len() - from(w)),
        );
        let helpers = if !checking && lists.len() > 1 && total >= PARALLEL_SWEEP { &lists[1..] } else { &[] };
        profile.snapshot("sweep_helpers", helpers.len());
        profile.snapshot_counts("sweep_work_per_helper", helpers.iter().map(|&w| objects(w).len() - from(w)));
        // Observe marks before either serial or parallel sweep clears them.
        // The profiler compacts its side metadata in the same order as sweep.
        for &w in &lists {
            allocations::before_sweep(w, objects(w), from(w), minor);
        }
        profile.phase("allocation_census");
    }
    if !minor {
        // a full collection marks every survivor's lines again
        for w in 0..crate::task::MAX_WORKERS {
            pool(w).clear_lines();
        }
    }
    #[cfg(feature = "gc-profile")]
    profile.phase("clear_lines");
    if !checking && lists.len() > 1 && total >= PARALLEL_SWEEP {
        let starts: Vec<usize> = lists.iter().map(|&w| from(w)).collect();
        live = sweep_parallel(
            &lists,
            &starts,
            sticky,
            #[cfg(feature = "gc-profile")]
            profile,
        );
    } else {
        for wl in lists {
            let start = from(wl);
            sweep_from(
                objects(wl),
                start,
                sticky,
                |p| unsafe {
                    live += HEADER + (*p).payload_size();
                },
                |p| unsafe {
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
                        free_cell(p, (*p).payload_size());
                    }
                },
            );
        }
    }
    #[cfg(feature = "gc-profile")]
    profile.phase("sweep_finalize");
    for (w, start) in YOUNG_START.iter().enumerate() {
        start.store(objects(w).len(), Ordering::Relaxed);
    }
    while h.quarantine_bytes > QUARANTINE_BYTES {
        let Some((p, _)) = h.quarantine.pop_front() else { break };
        unsafe {
            let size = (*p).payload_size();
            h.quarantine_bytes -= HEADER + size;
            h.freed.remove(&(p as usize));
            free_cell(p, size);
        }
    }
    // quarantined cells keep their lines (some were released just now,
    // and their lines stay marked until the next full collection)
    for &(p, _) in &h.quarantine {
        mark_lines(p, unsafe { (*p).payload_size() });
    }
    for (wl, c) in COUNTS.iter().enumerate() {
        c.0.store(objects(wl).len(), Ordering::Relaxed);
    }
    #[cfg(feature = "gc-profile")]
    profile.phase("accounting");
    #[cfg(not(feature = "gc-profile"))]
    let trim = true;
    #[cfg(feature = "gc-profile")]
    let trim = diagnostics::trim();
    if !minor && trim {
        if h.temp_roots.capacity() > h.temp_roots.len().max(4096) * 4 {
            h.temp_roots.shrink_to(h.temp_roots.len().max(4096));
        }
        for w in 0..crate::task::MAX_WORKERS {
            let list = objects(w);
            if list.capacity() > list.len().max(4096) * 4 {
                list.shrink_to(list.len().max(4096));
            }
        }
    }
    #[cfg(feature = "gc-profile")]
    profile.phase("trim");
    for w in 0..crate::task::MAX_WORKERS {
        pool(w).rebuild(!minor && trim);
    }
    #[cfg(feature = "gc-profile")]
    profile.phase("blocks");
    // every worker is stopped: their unpublished debt is part of `live`
    for d in &DEBT {
        d.0.store(0, Ordering::Relaxed);
    }
    if sticky {
        // `live`: the survivors swept (all of them in a full collection,
        // the young ones in a minor one); old objects that died since are
        // found by the next full collection, when the old generation has
        // doubled
        let old = if minor { OLD_BYTES.load(Ordering::Relaxed) + live } else { live };
        OLD_BYTES.store(old, Ordering::Relaxed);
        if !minor {
            MAJOR_AT.store((old * 2).max(64 << 20), Ordering::Relaxed);
        }
        h.live = old;
        // the young generation's budget: a minor collection after this much
        // allocation (its cost is the young survivors, not the heap)
        ALLOCATED.store(0, Ordering::Relaxed);
        let young_bytes = (old / 2).clamp(16 << 20, 256 << 20);
        #[cfg(feature = "gc-profile")]
        let young_bytes = diagnostics::young_bytes(young_bytes);
        THRESHOLD.store(young_bytes, Ordering::Relaxed);
    } else {
        h.live = live;
        ALLOCATED.store(live, Ordering::Relaxed);
        THRESHOLD.store((live * 2).max(16 << 20), Ordering::Relaxed);
    }
    MINOR_COLLECTION.store(false, Ordering::Relaxed);
    #[cfg(feature = "gc-profile")]
    {
        profile.snapshot("objects_after", all_objects().count());
        profile.snapshot("external_after", EXTERNAL_LIVE.load(Ordering::Relaxed));
        profile.snapshot("resident_after", resident_bytes());
        profile.snapshot("pool_after", pool_reserved_bytes());
        profile.snapshot("managed_live", h.live);
        profile.phase("finish");
    }
    SCRATCH.give(stack, remembered);
}

#[cfg(any(target_arch = "x86_64", feature = "gc-profile"))]
const SWEEP_PREFETCH: usize = 32;

/// Sweep `list[start..]` in place: marked objects survive (`keep`; their
/// marks stay when `sticky`, and their lines are marked live in the
/// list's own pool), the others are handed to `free`.
fn sweep_from(
    list: &mut Vec<*mut Header>,
    start: usize,
    sticky: bool,
    mut keep: impl FnMut(*mut Header),
    mut free: impl FnMut(*mut Header),
) {
    #[cfg(target_arch = "x86_64")]
    let ahead = {
        #[cfg(feature = "gc-profile")]
        {
            diagnostics::sweep_prefetch(SWEEP_PREFETCH)
        }
        #[cfg(not(feature = "gc-profile"))]
        {
            SWEEP_PREFETCH
        }
    };
    let mut j = start;
    for i in start..list.len() {
        // The world is stopped and this job owns the list: future entries
        // have not been compacted or freed. Hint only, with no extra reads
        // of object contents or changes to finalization/reclamation order.
        #[cfg(target_arch = "x86_64")]
        if ahead != 0
            && let Some(&next) = list.get(i + ahead)
        {
            unsafe { std::arch::x86_64::_mm_prefetch(next.cast(), std::arch::x86_64::_MM_HINT_T0) };
        }
        let p = list[i];
        unsafe {
            if (*p).meta & MARK != 0 {
                if !sticky {
                    (*p).meta &= !MARK;
                }
                mark_lines(p, (*p).payload_size());
                keep(p);
                list[j] = p;
                j += 1;
            } else {
                free(p);
            }
        }
    }
    list.truncate(j);
}

/// Verification after a minor collection's marking: every object reachable
/// from the roots must be marked (old, or young and found); an unmarked
/// one means an old object was written without the write barrier.
fn verify_minor(h: &Heap) {
    let mut seen: FxHashSet<u64> = FxHashSet::default();
    let mut stack: Vec<(u64, u64)> = vec![];
    let root = |w: u64, stack: &mut Vec<(u64, u64)>| {
        if w != 0 && w.is_multiple_of(8) && w >= 4096 {
            stack.push((w, 0));
        }
    };
    for &g in &h.globals {
        root(unsafe { *g }, &mut stack);
    }
    for &r in h.temp_roots.iter().chain(&h.pinned) {
        root(r, &mut stack);
    }
    crate::task::for_each_root(&mut |w| root(w, &mut stack));
    while let Some((w, parent)) = stack.pop() {
        if !seen.insert(w) {
            continue;
        }
        let o = w as *mut Header;
        let meta = unsafe { (*o).meta };
        if meta & IMMORTAL != 0 {
            continue;
        }
        if meta & MARK == 0 {
            let by = if parent == 0 {
                "a root".to_string()
            } else {
                format!("a `{}` object at {parent:#x}", unsafe { obj(parent) }.desc().name)
            };
            eprintln!(
                "fatal: heap verification: a reachable `{}` object {w:#x} was not marked by a minor collection: {by} holds it (a write without the barrier)",
                unsafe { obj(w) }.desc().name
            );
            eprintln!("{}", std::backtrace::Backtrace::force_capture());
            std::process::abort();
        }
        trace(w, &mut |c| {
            if c != 0 {
                stack.push((c, w));
            }
        });
    }
}

/// Heaps with at least this many objects (spread over several workers'
/// lists) are swept in parallel.
const PARALLEL_SWEEP: usize = 32 << 10;
#[cfg(feature = "gc-profile")]
const REUSE_SWEEP_HELPERS: bool = false;

#[cfg(any(feature = "gc-profile", test))]
type SweptList = (usize, Vec<*mut Header>);

// Only sweep jobs transfer these pointers: the world is stopped, each job owns
// a different worker's list/pool, and every result is received before resuming.
#[cfg(any(feature = "gc-profile", test))]
struct SweepResult(SweptList);
#[cfg(any(feature = "gc-profile", test))]
unsafe impl std::marker::Send for SweepResult {}

#[cfg(any(feature = "gc-profile", test))]
fn sweep_list(wl: usize, start: usize, sticky: bool) -> SweptList {
    let mut live = 0usize;
    let mut fin = vec![];
    sweep_from(
        objects(wl),
        start,
        sticky,
        |p| live += HEADER + unsafe { (*p).payload_size() },
        |p| unsafe {
            if let Kind::Opaque { finalize: Some(_), .. } = &(*p).desc().kind {
                fin.push(p);
            } else {
                free_cell(p, (*p).payload_size());
            }
        },
    );
    (live, fin)
}

/// Close the helper cache before profiler/process shutdown. Later collections
/// use scoped helpers, so a concurrent late collection cannot reopen the cache.
#[cfg(feature = "gc-profile")]
pub(crate) fn shutdown_sweep_helpers() {
    sweep::shutdown();
}

/// Sweep the given workers' lists on one thread each (the world is
/// stopped, so the other workers are not using their lists or pools; each
/// thread frees only into its list's own pool). Dead objects with a
/// finalizer are finalized and freed afterwards on this thread, in list
/// order: finalizers touch shared runtime state. Returns live bytes; diagnostics
/// additionally record native helper creation.
fn sweep_parallel(
    lists: &[usize],
    starts: &[usize],
    sticky: bool,
    #[cfg(feature = "gc-profile")] profile: &mut diagnostics::Collection,
) -> usize {
    #[cfg(feature = "gc-profile")]
    {
        if diagnostics::reuse_sweep_helpers(REUSE_SWEEP_HELPERS)
            && let Some((results, created)) = sweep::run(lists, starts, sticky)
        {
            profile.snapshot("sweep_helpers_created", created);
            return finish_sweep(lists, results);
        }
        profile.snapshot("sweep_helpers_created", lists.len() - 1);
    }
    struct Send<T>(T);
    unsafe impl<T> std::marker::Send for Send<T> {}
    let sweep = |wl: usize, start: usize| -> (usize, Vec<*mut Header>) {
        let mut live = 0usize;
        let mut fin = vec![];
        sweep_from(
            objects(wl),
            start,
            sticky,
            |p| live += HEADER + unsafe { (*p).payload_size() },
            |p| unsafe {
                if let Kind::Opaque { finalize: Some(_), .. } = &(*p).desc().kind {
                    fin.push(p);
                } else {
                    free_cell(p, (*p).payload_size());
                }
            },
        );
        (live, fin)
    };
    let results: Vec<(usize, Vec<*mut Header>)> = std::thread::scope(|sc| {
        let handles: Vec<_> = lists[1..]
            .iter()
            .zip(&starts[1..])
            .map(|(&wl, &st)| {
                sc.spawn(move || {
                    #[cfg(feature = "gc-profile")]
                    crate::profile::start_thread();
                    Send(sweep(wl, st))
                })
            })
            .collect();
        let mut out = vec![sweep(lists[0], starts[0])];
        out.extend(handles.into_iter().map(|h| h.join().expect("sweep thread").0));
        out
    });
    let mut live = 0;
    for (l, fin) in results {
        live += l;
        for p in fin {
            finalize(p);
            free_cell(p, unsafe { (*p).payload_size() });
        }
    }
    live
}

#[cfg(any(feature = "gc-profile", test))]
fn finish_sweep(lists: &[usize], results: Vec<SweptList>) -> usize {
    let mut live = 0;
    for (_, (l, fin)) in lists.iter().zip(results) {
        live += l;
        for p in fin {
            finalize(p);
            free_cell(p, unsafe { (*p).payload_size() });
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

/// Visit every managed object of descriptor `desc` (runtime lock held:
/// stops the world while it looks; inspection only).
pub fn for_each_object_of(desc: &'static TypeDesc, f: &mut dyn FnMut(u64)) {
    crate::task::stop_the_world();
    for_each_object_of_stopped(desc, f);
    crate::task::resume_the_world();
}

/// Collector-internal inspection: the caller already stopped the world.
/// Never resume it here; the collector still has marking/sweeping to do.
pub(crate) fn for_each_object_of_stopped(desc: &'static TypeDesc, f: &mut dyn FnMut(u64)) {
    for p in all_objects() {
        if std::ptr::eq(unsafe { (*p).desc }, desc) {
            f(p as u64);
        }
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
    (
        objects,
        ALLOCATED.load(Ordering::Relaxed) + owed + EXTERNAL_PRESSURE.load(Ordering::Relaxed),
        h.collections,
    )
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

#[cfg(test)]
mod pool_tests {
    use super::*;

    #[test]
    fn cells_reuse_dead_lines_and_trim_respects_quarantine() {
        // An isolated test process avoids racing other runtime unit tests that
        // use the global heap. Its only test is this allocator exercise.
        if std::env::var_os("BADGER_POOL_TEST_CHILD").is_none() {
            let limits: &[usize] =
                if cfg!(feature = "gc-profile") { &[256, 512, 1024] } else { &[SMALL_MAX] };
            let refills: &[usize] =
                if cfg!(feature = "gc-profile") { &[256, 512, 2048] } else { &[BIG_CHUNK >> 10] };
            for (limit, refill) in
                limits.iter().flat_map(|limit| refills.iter().map(move |refill| (limit, refill)))
            {
                for verify in ["0", "1"] {
                    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
                    for (key, _) in std::env::vars_os() {
                        if key.to_string_lossy().starts_with("BADGER_") {
                            command.env_remove(key);
                        }
                    }
                    let output = command
                        .args([
                            "--exact",
                            "gc::pool_tests::cells_reuse_dead_lines_and_trim_respects_quarantine",
                        ])
                        .env("BADGER_POOL_TEST_CHILD", "1")
                        .env("BADGER_GC_POOL_MAX", limit.to_string())
                        .env("BADGER_GC_POOL_REFILL_KIB", refill.to_string())
                        .env("BADGER_GC_VERIFY", verify)
                        .output()
                        .unwrap();
                    assert!(
                        output.status.success(),
                        "pool{limit} refill{refill} verify{verify}: {} {}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
            }
            return;
        }
        let w = 0;
        let size_of = |payload: usize| {
            if cell_size(payload) <= small_max() { cell_size(payload) } else { layout(payload).size() }
        };
        // Every cell is zeroed, and the pooled ones lie in one block each.
        let mut cells = vec![];
        for payload in 0..=1040 {
            let bytes = size_of(payload);
            let p = alloc_cell(w, payload);
            assert!(!p.is_null());
            unsafe {
                assert!(std::slice::from_raw_parts(p as *const u8, bytes).iter().all(|&b| b == 0));
                std::ptr::write_bytes(p as *mut u8, 0xAB, bytes);
            }
            if cell_size(payload) <= small_max() {
                let offset = p.addr() & (BLOCK - 1);
                assert!(offset >= HEADER_LINES * LINE && offset + bytes <= BLOCK);
            }
            cells.push((p, payload));
        }
        // A collection that keeps every third cell: the others' lines are
        // reused in place, zeroed, and never overlap a survivor.
        let reserved = pool_reserved_bytes();
        let mut kept = vec![];
        for (i, &(p, payload)) in cells.iter().enumerate() {
            if i % 3 == 0 {
                mark_lines(p, payload);
                kept.push((p, payload));
            } else {
                free_cell(p, payload);
            }
        }
        pool(w).rebuild(false);
        assert!(!pool(w).recyclable.is_empty());
        let mut again = vec![];
        for payload in (0..=small_max() - HEADER).step_by(8).cycle().take(300) {
            let p = alloc_cell(w, payload);
            let bytes = cell_size(payload);
            unsafe {
                assert!(std::slice::from_raw_parts(p as *const u8, bytes).iter().all(|&b| b == 0));
            }
            for &(q, kept_payload) in &kept {
                let (a, b) = (p.addr(), q.addr());
                assert!(a + bytes <= b || b + size_of(kept_payload) <= a, "{a:#x} overlaps a survivor at {b:#x}");
            }
            again.push((p, payload));
        }
        assert_eq!(pool_reserved_bytes(), reserved, "dead lines are reused before new chunks");
        for &(p, payload) in &kept {
            assert!(unsafe { std::slice::from_raw_parts(p as *const u8, size_of(payload)) }.iter().all(|&b| b == 0xAB));
        }
        for (p, payload) in again.into_iter().chain(kept) {
            free_cell(p, payload);
        }
        pool(w).rebuild(false);
        // Cross a chunk boundary, then retain just one quarantined cell from
        // the first chunk: only the chunks with no live line are released.
        let payload = small_max() - HEADER;
        let cells: Vec<_> = (0..2 * CHUNK / small_max() + 8).map(|_| alloc_cell(w, payload)).collect();
        assert!(pool(w).chunks.len() >= 2);
        assert_eq!(pool(w).chunks[1].bytes, refill_bytes());
        let held = cells[0];
        assert!(held.addr() >= pool(w).chunks[0].base.addr() && held.addr() < pool(w).chunks[0].base.addr() + CHUNK);
        // This raw allocator fixture has not published a managed pointer
        // word, which normally exposes the provenance used by payload().
        let _ = held.expose_provenance();
        for &p in &cells[1..] {
            free_cell(p, payload);
        }
        unsafe {
            (*held).desc = crate::desc::builtin::unit();
            (*held).meta = (payload as u64) << 32;
            std::ptr::write_bytes((*held).payload(), 0xCD, payload);
        }
        pool(w).clear_lines();
        mark_lines(held, payload);
        pool(w).rebuild(true);
        assert_eq!(pool(w).chunks.len(), 1);
        unsafe {
            assert!(std::slice::from_raw_parts((*held).payload(), payload).iter().all(|&b| b == 0xCD));
        }
        free_cell(held, payload);
        pool(w).clear_lines();
        pool(w).rebuild(true);
        let p = pool(w);
        assert!(p.chunks.is_empty() && p.recyclable.is_empty() && p.empty.is_empty());
        assert!(p.cursor.is_null() && p.over.is_null() && p.scan.is_null());
        assert_eq!(p.left + p.over_left, 0);
        assert_eq!(pool_reserved_bytes(), 0);

        // A pool belongs to its worker; large cells need no pool.
        let p = alloc_cell(7, 8);
        assert!(!p.is_null() && pool(7).chunks.len() == 1 && pool(0).chunks.is_empty());
        free_cell(p, 8);
        let large = small_max() + 1;
        let p = alloc_cell(7, large);
        assert!(!p.is_null());
        free_cell(p, large);
        pool(7).clear_lines();
        pool(7).rebuild(true);
        assert!(pool(7).chunks.is_empty());
        assert_eq!(pool_reserved_bytes(), 0);
    }

    #[test]
    fn holes_are_runs_of_clear_lines() {
        let mut map: LineMap = [0; LINES / 64];
        map[0] = HEADER_BITS;
        assert_eq!(next_hole(&map, HEADER_LINES), Some((HEADER_LINES, LINES)));
        map[1] = 1 << 3 | 1 << 4;
        map[15] = 1 << 63;
        assert_eq!(next_hole(&map, HEADER_LINES), Some((HEADER_LINES, 67)));
        assert_eq!(next_hole(&map, 67), Some((69, LINES - 1)));
        assert_eq!(next_hole(&map, LINES - 1), None);
        assert!(!block_empty(&map));
        map.fill(u64::MAX);
        assert_eq!(next_hole(&map, 0), None);
    }
}
