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
    objects: Vec<*mut Header>,
    allocated: usize,
    threshold: usize,
    live: usize,
    globals: Vec<*mut u64>,
    temp_roots: Vec<u64>,
    pinned: Vec<u64>,
    pub collections: usize,
    pub disabled: usize,
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
    objects: Vec::new(),
    allocated: 0,
    threshold: 16 << 20,
    live: 0,
    globals: Vec::new(),
    temp_roots: Vec::new(),
    pinned: Vec::new(),
    collections: 0,
    disabled: 0,
    pause_total: 0,
    pause_max: 0,
    quarantine: VecDeque::new(),
    quarantine_bytes: 0,
    freed: FxHashMap::with_hasher(FxBuildHasher),
    immortals: FxHashSet::with_hasher(FxBuildHasher),
}));

/// The heap belongs to the single worker thread.
#[allow(clippy::mut_from_ref)]
pub fn heap() -> &'static mut Heap {
    unsafe { &mut *HEAP.0.get() }
}

fn layout(payload: usize) -> Layout {
    Layout::from_size_align(HEADER + payload.max(8), 8).unwrap()
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
pub fn alloc(desc: &'static TypeDesc, payload: usize) -> u64 {
    let h = heap();
    let chaos = crate::task::chaos().is_some() && crate::task::chaos_next().is_multiple_of(512);
    if (h.allocated > h.threshold || stress() || chaos) && h.disabled == 0 {
        collect();
    }
    let h = heap();
    unsafe {
        let p = alloc_zeroed(layout(payload)) as *mut Header;
        if p.is_null() {
            eprintln!("fatal: out of memory");
            std::process::abort();
        }
        (*p).desc = desc;
        (*p).meta = (payload as u64) << 32;
        h.objects.push(p);
        h.allocated += HEADER + payload;
        p as u64
    }
}

/// Count memory owned outside the heap by a just-allocated object (map
/// tries, buffers) toward the next collection.
pub fn add_pressure(bytes: usize) {
    heap().allocated += bytes;
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
        heap().disabled += 1;
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
        heap().disabled -= 1;
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
        if checking { h.objects.iter().map(|p| *p as usize).collect() } else { FxHashSet::default() };
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
    // trace
    while let Some(w) = stack.pop() {
        let mut children = Vec::new();
        trace(w, &mut |c| children.push(c));
        for c in children {
            check(c, &|| format!("a `{}` object", unsafe { obj(w) }.desc().name));
            mark(c, &mut stack);
        }
    }
    // sweep
    let mut live = 0usize;
    let mut kept = Vec::with_capacity(h.objects.len());
    for &p in &h.objects {
        unsafe {
            if (*p).meta & MARK != 0 {
                (*p).meta &= !MARK;
                live += HEADER + (*p).payload_size();
                kept.push(p);
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
                    dealloc(p as *mut u8, layout((*p).payload_size()));
                }
            }
        }
    }
    while h.quarantine_bytes > QUARANTINE_BYTES {
        let Some((p, _)) = h.quarantine.pop_front() else { break };
        unsafe {
            let size = (*p).payload_size();
            h.quarantine_bytes -= HEADER + size;
            h.freed.remove(&(p as usize));
            dealloc(p as *mut u8, layout(size));
        }
    }
    h.objects = kept;
    h.live = live;
    h.allocated = live;
    h.threshold = (live * 2).max(16 << 20);
}

fn finalize(p: *mut Header) {
    let desc = unsafe { (*p).desc() };
    match &desc.kind {
        Kind::List { .. } => crate::list::finalize(p),
        Kind::Map { .. } | Kind::Set { .. } => crate::map::finalize(p),
        Kind::Opaque { finalize: Some(f), .. } => f(p),
        _ => {}
    }
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
    (h.objects.len(), h.allocated, h.collections)
}

/// Total and longest collection pause, in nanoseconds.
pub fn pauses() -> (u64, u64) {
    let h = heap();
    (h.pause_total, h.pause_max)
}
