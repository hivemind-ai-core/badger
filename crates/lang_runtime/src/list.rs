//! Paged lists (`List<T>`) and their affine builders (`ListBuilder<T>`).
//!
//! Everything is a managed object; nothing needs a finalizer. A list
//! object's payload is
//!
//! ```text
//! len | page directory (0 when empty) | element descriptor | page capacity
//! ```
//!
//! The directory holds page pointers; pages have a fixed capacity derived
//! from the element size and a byte budget, and only a version's final page
//! may be partial. Versions made by persistent append share the directory
//! and pages: each records its own length, and a directory slot or page
//! slot past every version's end is claimed atomically by the one version
//! that extends there (any other copies first). Appending never moves
//! existing elements (pages are stable), and freezing a builder copies its
//! four header words.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::desc::{Kind, Repr, TypeDesc};
use crate::gc::{self, Header, obj};

const PAGE_BYTES: usize = 4096;

/// Set in the capacity word of a builder that `finish` consumed.
const CONSUMED: u64 = 1 << 63;

fn page_desc() -> &'static TypeDesc {
    static D: std::sync::OnceLock<&'static TypeDesc> = std::sync::OnceLock::new();
    D.get_or_init(|| {
        TypeDesc::new("Page", Kind::Opaque { trace: Some(trace_page), finalize: None, args: vec![] }).leak()
    })
}

fn dir_desc() -> &'static TypeDesc {
    static D: std::sync::OnceLock<&'static TypeDesc> = std::sync::OnceLock::new();
    D.get_or_init(|| {
        TypeDesc::new("PageDirectory", Kind::Opaque { trace: Some(trace_dir), finalize: None, args: vec![] })
            .leak()
    })
}

/// Page payload: `elem desc ptr`, written-slot count, then element storage.
const PAGE_HDR: usize = 16;

fn trace_page(o: *mut Header, f: &mut dyn FnMut(u64)) {
    unsafe {
        let p = (*o).payload();
        let elem = &*(*(p as *const *const TypeDesc));
        if !elem.is_ptr() {
            return;
        }
        let n = (*(p.add(8) as *const AtomicU64)).load(Ordering::Acquire) as usize;
        for i in 0..n {
            f(*(p.add(PAGE_HDR + i * 8) as *const u64));
        }
    }
}

/// Directory payload: claimed-slot count, capacity, then page pointers.
const DIR_HDR: usize = 16;

fn trace_dir(o: *mut Header, f: &mut dyn FnMut(u64)) {
    unsafe {
        let p = (*o).payload();
        let n = (*(p as *const AtomicU64)).load(Ordering::Acquire) as usize;
        for i in 0..n {
            let pg = *(p.add(DIR_HDR + i * 8) as *const u64);
            if pg != 0 {
                f(pg);
            }
        }
    }
}

fn dir_used(dir: u64) -> &'static AtomicU64 {
    unsafe { &*(obj(dir).payload() as *const AtomicU64) }
}

fn dir_cap(dir: u64) -> usize {
    unsafe { *(obj(dir).payload().add(8) as *const u64) as usize }
}

fn dir_slot(dir: u64, i: usize) -> *mut u64 {
    unsafe { obj(dir).payload().add(DIR_HDR + i * 8) as *mut u64 }
}

/// A list object's header words.
struct Hdr(*mut u64);

impl Hdr {
    fn len(&self) -> usize {
        unsafe { *self.0 as usize }
    }
    fn set_len(&self, n: usize) {
        unsafe { *self.0 = n as u64 }
    }
    fn dir(&self) -> u64 {
        unsafe { *self.0.add(1) }
    }
    fn set_dir(&self, d: u64) {
        unsafe { *self.0.add(1) = d }
    }
    fn elem(&self) -> &'static TypeDesc {
        unsafe { &*(*self.0.add(2) as *const TypeDesc) }
    }
    fn cap(&self) -> usize {
        unsafe { (*self.0.add(3) & !CONSUMED) as usize }
    }
    fn repr(&self) -> Repr {
        self.elem().repr()
    }
    fn page(&self, p: usize) -> u64 {
        unsafe { *dir_slot(self.dir(), p) }
    }
}

fn hdr(w: u64) -> Hdr {
    let p = unsafe { obj(w).payload() as *mut u64 };
    if unsafe { *p.add(3) } & CONSUMED != 0 {
        crate::task::panic_current("use of a consumed list builder".into(), String::new());
    }
    Hdr(p)
}

pub fn trace(o: *mut Header, f: &mut dyn FnMut(u64)) {
    let d = unsafe { *((*o).payload() as *const u64).add(1) };
    if d != 0 {
        f(d);
    }
}

/// Create an empty list (or builder) object with descriptor `desc`
/// (`Kind::List { elem }`).
pub fn new(desc: &'static TypeDesc) -> u64 {
    let elem = match &desc.kind {
        Kind::List { elem } => *elem,
        _ => crate::desc::builtin::unit(),
    };
    let size = elem.repr().size() as usize;
    let page_cap = (PAGE_BYTES / size.max(1)).clamp(16, 4096);
    let w = gc::alloc(desc, 32);
    unsafe {
        let p = obj(w).payload() as *mut u64;
        *p.add(2) = elem as *const TypeDesc as u64;
        *p.add(3) = page_cap as u64;
    }
    w
}

pub fn len(w: u64) -> usize {
    hdr(w).len()
}

/// Elements per page.
pub fn page_cap(w: u64) -> usize {
    hdr(w).cap()
}

/// Number of pages holding this list's elements.
pub fn page_count(w: u64) -> usize {
    let h = hdr(w);
    h.len().div_ceil(h.cap())
}

/// Page object number `p` (below `page_count`).
pub fn page(w: u64, p: usize) -> u64 {
    hdr(w).page(p)
}

pub fn get(w: u64, i: usize) -> u64 {
    let h = hdr(w);
    debug_assert!(i < h.len());
    let cap = h.cap();
    let repr = h.repr();
    unsafe { repr.load(obj(h.page(i / cap)).payload().add(PAGE_HDR + (i % cap) * repr.size() as usize)) }
}

/// Read slot `slot` of a page object (for stable element references).
pub fn page_read(page: u64, slot: usize, repr: Repr) -> u64 {
    unsafe { repr.load(obj(page).payload().add(PAGE_HDR + slot * repr.size() as usize)) }
}

/// Address of element `i` (stable while the list is alive).
pub fn addr(w: u64, i: usize) -> *mut u8 {
    let h = hdr(w);
    let cap = h.cap();
    unsafe { obj(h.page(i / cap)).payload().add(PAGE_HDR + (i % cap) * h.repr().size() as usize) }
}

fn new_page(elem: &'static TypeDesc, cap: usize) -> u64 {
    let size = elem.repr().size() as usize;
    let page = gc::alloc(page_desc(), PAGE_HDR + cap * size.max(8));
    unsafe { *(obj(page).payload() as *mut *const TypeDesc) = elem };
    page
}

/// Take slot `used` of a shared page or directory whose claimed-slot count
/// is at `count`, if no version has taken it yet: exactly one version
/// extends there, even when workers append to one list concurrently.
fn claim(count: &AtomicU64, used: usize) -> bool {
    count.compare_exchange(used as u64, used as u64 + 1, Ordering::AcqRel, Ordering::Relaxed).is_ok()
}

/// Make page `page` slot `p` of list `w`'s directory, which holds `p`
/// pages for `w`: claimed in place when free, otherwise in a copy of the
/// directory's first `p` slots. `w` and `page` must be rooted.
fn put_page(w: u64, p: usize, page: u64) {
    let dir = hdr(w).dir();
    if dir != 0 && p < dir_cap(dir) && claim(dir_used(dir), p) {
        unsafe { *dir_slot(dir, p) = page };
        return;
    }
    let cap = (p * 2).max(4);
    let nd = gc::alloc(dir_desc(), DIR_HDR + cap * 8);
    let dir = hdr(w).dir();
    unsafe {
        let q = obj(nd).payload() as *mut u64;
        *q.add(1) = cap as u64;
        if p > 0 {
            std::ptr::copy_nonoverlapping(dir_slot(dir, 0), dir_slot(nd, 0), p);
        }
        *dir_slot(nd, p) = page;
        dir_used(nd).store(p as u64 + 1, Ordering::Release);
    }
    hdr(w).set_dir(nd);
}

/// Append a word in place (builders and fresh lists). The list object and
/// value must be rooted by the caller.
pub fn push(w: u64, v: u64) {
    let h = hdr(w);
    let (len, cap, elem) = (h.len(), h.cap(), h.elem());
    let size = elem.repr().size() as usize;
    let off = len % cap;
    if off == 0 {
        let r = gc::Roots::new();
        r.add(w);
        if elem.is_ptr() {
            r.add(v);
        }
        let page = r.add(new_page(elem, cap));
        unsafe {
            let p = obj(page).payload();
            elem.repr().store(p.add(PAGE_HDR), v);
            (*(p.add(8) as *const AtomicU64)).store(1, Ordering::Release);
        }
        put_page(w, len / cap, page);
        hdr(w).set_len(len + 1);
        return;
    }
    let page = h.page(len / cap);
    // a fresh list's tail page is its own, but claim it like `append` so
    // a version sharing it cannot have written there
    let count = unsafe { &*(obj(page).payload().add(8) as *const AtomicU64) };
    if claim(count, off) {
        unsafe { elem.repr().store(obj(page).payload().add(PAGE_HDR + off * size), v) };
        h.set_len(len + 1);
        return;
    }
    copy_tail(w);
    push(w, v);
}

/// Replace list `w`'s shared partial tail page with a private copy of its
/// prefix (another version extended the shared page). `w` is rooted.
fn copy_tail(w: u64) {
    let r = gc::Roots::new();
    r.add(w);
    let h = hdr(w);
    let (len, cap, elem) = (h.len(), h.cap(), h.elem());
    let size = elem.repr().size() as usize;
    let (p, used) = (len / cap, len % cap);
    let copy = r.add(new_page(elem, cap));
    let h = hdr(w);
    unsafe {
        let dst = obj(copy).payload();
        let src = obj(h.page(p)).payload();
        std::ptr::copy_nonoverlapping(src.add(PAGE_HDR), dst.add(PAGE_HDR), used * size);
        (*(dst.add(8) as *const AtomicU64)).store(used as u64, Ordering::Release);
    }
    // the directory slot for the tail is taken: a private directory
    // holding the first `p` pages and the copy
    let cap_dir = (p * 2).max(4);
    let nd = gc::alloc(dir_desc(), DIR_HDR + cap_dir * 8);
    let h = hdr(w);
    unsafe {
        *(obj(nd).payload() as *mut u64).add(1) = cap_dir as u64;
        std::ptr::copy_nonoverlapping(dir_slot(h.dir(), 0), dir_slot(nd, 0), p);
        *dir_slot(nd, p) = copy;
        dir_used(nd).store(p as u64 + 1, Ordering::Release);
    }
    h.set_dir(nd);
}

/// Freeze a builder into a list, taking over its directory.
pub fn finish(builder: u64, list_desc: &'static TypeDesc) -> u64 {
    let r = gc::Roots::new();
    r.add(builder);
    hdr(builder);
    let out = gc::alloc(list_desc, 32);
    unsafe {
        let src = obj(builder).payload() as *mut u64;
        let dst = obj(out).payload() as *mut u64;
        std::ptr::copy_nonoverlapping(src, dst, 4);
        *src.add(1) = 0;
        *src.add(3) |= CONSUMED;
    }
    out
}

pub fn from_words(desc: &'static TypeDesc, words: &[u64]) -> u64 {
    let r = gc::Roots::new();
    for &w in words {
        r.add(w);
    }
    let l = r.add(new(desc));
    for &w in words {
        push(l, w);
    }
    l
}

pub fn to_vec(w: u64) -> Vec<u64> {
    (0..len(w)).map(|i| get(w, i)).collect()
}

/// Is `w` a list that is not a consumed builder?
pub fn is_live(w: u64) -> bool {
    unsafe { *(obj(w).payload() as *const u64).add(3) & CONSUMED == 0 }
}

/// Persistent append: a new list sharing every page of `w`. The partial
/// tail page (and the directory) are shared too when no other version has
/// written past `w`'s end in them; otherwise they are copied first.
/// O(1) except when copying, which is O(pages).
pub fn append(w: u64, v: u64, desc: &'static TypeDesc) -> u64 {
    let r = gc::Roots::new();
    r.add(w);
    let h = hdr(w);
    if h.elem().is_ptr() {
        r.add(v);
    }
    let out = r.add(gc::alloc(desc, 32));
    unsafe {
        std::ptr::copy_nonoverlapping(obj(w).payload() as *const u64, obj(out).payload() as *mut u64, 4);
    }
    push(out, v);
    out
}
