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

use std::alloc::{Layout, alloc_zeroed, dealloc};
use std::cell::UnsafeCell;

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
    unsafe { &*(w as *const Header) }
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

/// Allocate a zeroed object with `payload` bytes. May collect first.
pub fn alloc(desc: &'static TypeDesc, payload: usize) -> u64 {
    let h = heap();
    if (h.allocated > h.threshold || stress()) && h.disabled == 0 {
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
        p as u64
    }
}

/// Register a global slot (module-level value) as a root.
pub fn register_global(slot: *mut u64) {
    heap().globals.push(slot);
}

/// Keep `w` alive until the matching [`pop_root`]. Runtime code uses this
/// while it allocates several objects.
pub fn push_root(w: u64) {
    heap().temp_roots.push(w);
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
    heap().temp_roots.pop();
}

/// RAII guard over temporary roots.
pub struct Roots(usize);

impl Roots {
    pub fn new() -> Roots {
        Roots(heap().temp_roots.len())
    }

    pub fn add(&self, w: u64) -> u64 {
        heap().temp_roots.push(w);
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
        heap().temp_roots.truncate(self.0);
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
        mark(unsafe { *g }, &mut stack);
    }
    for &r in &h.temp_roots {
        // runtime helpers root only pointer-typed words; this guard keeps a
        // stray scalar from being dereferenced
        if r % 8 == 0 && r >= 4096 {
            mark(r, &mut stack);
        }
    }
    for &r in &h.pinned {
        mark(r, &mut stack);
    }
    crate::task::for_each_root(&mut |w| mark(w, &mut stack));
    // trace
    while let Some(w) = stack.pop() {
        let mut children = Vec::new();
        trace(w, &mut |c| children.push(c));
        for c in children {
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
                dealloc(p as *mut u8, layout((*p).payload_size()));
            }
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
