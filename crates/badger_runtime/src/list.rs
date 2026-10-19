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

/// Elements per page for an element size: a power of two for every element
/// size (1, 2, 4, 8 bytes), which generated code relies on to index pages
/// with shifts and masks.
pub const fn page_cap_for(size: usize) -> usize {
    let c = PAGE_BYTES / if size == 0 { 1 } else { size };
    if c < 16 {
        16
    } else if c > 4096 {
        4096
    } else {
        c
    }
}

/// Payload offsets that generated code reads directly: a list's length and
/// page directory, and the first page pointer in a directory.
pub const LEN_OFFSET: i32 = 0;
pub const DIR_OFFSET: i32 = 8;
pub const DIR_PAGES: i32 = DIR_HDR as i32;
/// A list's capacity word (its top bit marks a consumed builder), and a
/// page's count of written slots and first element: generated code appends
/// to a builder's tail page directly.
pub const CAP_OFFSET: i32 = 24;
pub const PAGE_COUNT: i32 = 8;
pub const PAGE_ELEMS: i32 = PAGE_HDR as i32;

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
pub const PAGE_HDR: usize = 16;

fn trace_page(o: *mut Header, f: &mut dyn FnMut(u64)) {
    unsafe {
        let p = (*o).payload();
        let elem = &*(*(p as *const *const TypeDesc));
        if !elem.is_ptr() {
            return;
        }
        // (a frozen small page keeps its count under the flag)
        let n = ((*(p.add(8) as *const AtomicU64)).load(Ordering::Acquire) & !FROZEN) as usize;
        for i in 0..n {
            f(*(p.add(PAGE_HDR + i * 8) as *const u64));
        }
    }
}

/// Directory payload: claimed-slot count, capacity, then page pointers.
const DIR_HDR: usize = 16;

fn trace_dir(o: *mut Header, f: &mut dyn FnMut(u64)) {
    // In a full collection directories contain weak page slots: each
    // live list marks only its visible pages below. Minor collections
    // still scan remembered old directories to find newly installed pages.
    if !gc::is_minor_collection() {
        return;
    }
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
        gc::barrier(self.0 as u64 - gc::HEADER as u64);
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
    let p = unsafe { (*o).payload() as *const u64 };
    let d = unsafe { *p.add(1) };
    if d != 0 {
        f(d);
        let len = unsafe { *p as usize };
        let cap = unsafe { (*p.add(3) & !CONSUMED) as usize };
        for i in 0..len.div_ceil(cap) {
            let page = unsafe { *dir_slot(d, i) };
            debug_assert_ne!(page, 0, "a visible list page must be live");
            f(page);
        }
    }
}

/// Before full sweep, erase weak slots for unmarked pages. Their claimed
/// count stays unchanged, so an append branches/copies instead of reading
/// an old version's invisible page. All mutators are stopped.
pub(crate) fn clear_unmarked_directory_pages() {
    gc::for_each_object_of_stopped(dir_desc(), &mut |dir| {
        let n = dir_used(dir).load(Ordering::Relaxed) as usize;
        for i in 0..n {
            let slot = dir_slot(dir, i);
            let page = unsafe { *slot };
            if page != 0 && unsafe { obj(page).meta } & (gc::MARK | gc::IMMORTAL) == 0 {
                unsafe { *slot = 0 };
            }
        }
    });
}

/// Create an empty list (or builder) object with descriptor `desc`
/// (`Kind::List { elem }`).
pub fn new(desc: &'static TypeDesc) -> u64 {
    let elem = match &desc.kind {
        Kind::List { elem } => *elem,
        _ => crate::desc::builtin::unit(),
    };
    let size = elem.repr().size() as usize;
    let page_cap = page_cap_for(size);
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

/// Contiguous element storage of page `p`, for bulk copies.
pub fn page_elems(w: u64, p: usize) -> *const u8 {
    unsafe { obj(hdr(w).page(p)).payload().add(PAGE_HDR) }
}

/// Copy every element of `l` into `dst`, one bulk run per page. Element
/// width is `l`'s representation width; `dst` must have room for `len` of
/// them. Runs stay within a page, so pointer elements copy as full words.
pub fn copy_into(l: u64, dst: *mut u8) {
    let h = hdr(l);
    let (cap, n, size) = (h.cap(), h.len(), h.repr().size() as usize);
    let mut i = 0;
    while i < n {
        let end = (i + cap).min(n);
        unsafe {
            std::ptr::copy_nonoverlapping(
                obj(h.page(i / cap)).payload().add(PAGE_HDR),
                dst.add(i * size),
                (end - i) * size,
            );
        }
        i = end;
    }
}

/// A frozen array holding `l`'s elements (a contiguous copy of its pages).
pub fn to_array(l: u64, elem: &'static TypeDesc) -> u64 {
    let r = gc::Roots::new();
    r.add(l);
    let n = len(l);
    let w = r.add(crate::buffer::new(elem, n));
    {
        let d = crate::buffer::data(w);
        copy_into(l, d.items.as_mut_ptr());
        d.items.set_len(n);
    }
    crate::buffer::freeze(w)
}

/// Address of element `i` (stable while the list is alive); out of range
/// is a task defect.
pub fn addr(w: u64, i: usize) -> *mut u8 {
    if i < len(w) {
        stabilize(w, i);
    }
    let h = hdr(w);
    if i >= h.len() {
        crate::task::panic_current(
            format!("index out of bounds: the length is {} but the index is {i}", h.len()),
            crate::task::caller_location(),
        );
    }
    let cap = h.cap();
    unsafe { obj(h.page(i / cap)).payload().add(PAGE_HDR + (i % cap) * h.repr().size() as usize) }
}

/// Slots of a list's first page when it is made: small lists take a small
/// page (from the collector's small-object pools), which grows by copying,
/// fourfold up to the full page capacity, as the list grows. Later pages
/// are full. Indexing is unaffected (page `i / cap`, slot `i % cap`): the
/// first page is grown before an index can pass its end.
const FIRST_PAGE_SLOTS: usize = 16;

/// The slots a page object holds (its physical capacity: the page
/// capacity, or less for a small first page).
fn slots(page: u64, size: usize) -> usize {
    (unsafe { obj(page) }.payload_size() - PAGE_HDR) / size.max(1)
}

/// Set in a small first page's written-slot count when it is replaced by a
/// full page (see [`stabilize`]): appends can no longer claim its slots.
const FROZEN: u64 = 1 << 62;

/// Before an element's address or stable reference is handed out: if it is
/// in a small first page, replace that page (in its directory slot) with a
/// full-size copy, which never moves, so the address stays valid while the
/// list grows. The small page is frozen first, so an append racing with
/// this (on a version sharing it) fails its claim and copies privately.
/// `w` is rooted.
pub fn stabilize(w: u64, i: usize) {
    let h = hdr(w);
    let (cap, elem) = (h.cap(), h.elem());
    let size = elem.repr().size() as usize;
    if i / cap != 0 || h.dir() == 0 {
        return;
    }
    let dir = h.dir();
    let small = unsafe { *dir_slot(dir, 0) };
    if slots(small, size) >= cap {
        return;
    }
    let count = unsafe { &*(obj(small).payload().add(8) as *const AtomicU64) };
    let mut c = count.load(Ordering::Acquire);
    loop {
        if c & FROZEN != 0 {
            // another promotion is under way: wait for its full page
            while unsafe { std::ptr::read_volatile(dir_slot(dir, 0)) } == small {
                std::hint::spin_loop();
            }
            return;
        }
        match count.compare_exchange(c, c | FROZEN, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => break,
            Err(now) => c = now,
        }
    }
    let r = gc::Roots::new();
    r.add(w);
    r.add(small);
    let full = r.add(new_page(elem, cap));
    unsafe {
        let dst = obj(full).payload();
        let src = obj(small).payload();
        std::ptr::copy_nonoverlapping(src.add(PAGE_HDR), dst.add(PAGE_HDR), c as usize * size);
        (*(dst.add(8) as *const AtomicU64)).store(c, Ordering::Release);
        (*(dir_slot(dir, 0) as *const AtomicU64)).store(full, Ordering::Release);
    }
    gc::barrier(dir);
}

fn new_page(elem: &'static TypeDesc, cap: usize) -> u64 {
    let size = elem.repr().size() as usize;
    let page = gc::alloc(page_desc(), PAGE_HDR + cap * size.max(1));
    unsafe { *(obj(page).payload() as *mut *const TypeDesc) = elem };
    page
}

/// Take slot `used` of a shared page or directory whose claimed-slot count
/// is at `count`, if no version has taken it yet: exactly one version
/// extends there, even when workers append to one list concurrently.
fn claim(count: &AtomicU64, used: usize) -> bool {
    count.compare_exchange(used as u64, used as u64 + 1, Ordering::AcqRel, Ordering::Relaxed).is_ok()
}

/// Claim `n` free slots at once, for a bulk append.
fn claim_n(count: &AtomicU64, used: usize, n: usize) -> bool {
    count.compare_exchange(used as u64, used as u64 + n as u64, Ordering::AcqRel, Ordering::Relaxed).is_ok()
}

/// Make page `page` slot `p` of list `w`'s directory, which holds `p`
/// pages for `w`: claimed in place when free, otherwise in a copy of the
/// directory's first `p` slots. `w` and `page` must be rooted.
fn put_page(w: u64, p: usize, page: u64) {
    let dir = hdr(w).dir();
    if dir != 0 && p < dir_cap(dir) && claim(dir_used(dir), p) {
        unsafe { *dir_slot(dir, p) = page };
        gc::barrier(dir);
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
        let slots = if len == 0 { cap.min(FIRST_PAGE_SLOTS) } else { cap };
        let page = r.add(new_page(elem, slots));
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
    if off >= slots(page, size) {
        // a full small first page: a private copy with room
        copy_tail(w);
        return push(w, v);
    }
    // a fresh list's tail page is its own, but claim it like `append` so
    // a version sharing it cannot have written there
    let count = unsafe { &*(obj(page).payload().add(8) as *const AtomicU64) };
    if claim(count, off) {
        unsafe { elem.repr().store(obj(page).payload().add(PAGE_HDR + off * size), v) };
        if elem.is_ptr() {
            gc::barrier(page);
        }
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
    // a first page grows fourfold (later pages are full-size)
    let slots = if p == 0 { (used * 4).clamp(FIRST_PAGE_SLOTS.min(cap), cap) } else { cap };
    let copy = r.add(new_page(elem, slots));
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

/// Append `n` packed elements from `src` (contiguous, at `dst`'s element
/// width) to `dst`, filling whole runs into destination pages instead of
/// one `push` per element. `dst` is a fresh list or builder and must not be
/// shared with another version; `dst` must be rooted by the caller and
/// `src` must stay valid (it does: pages never move).
pub fn push_raw(dst: u64, src: *const u8, n: usize) {
    if n == 0 {
        return;
    }
    let r = gc::Roots::new();
    r.add(dst);
    let mut done = 0;
    while done < n {
        let h = hdr(dst);
        let (len, cap, elem) = (h.len(), h.cap(), h.elem());
        let size = elem.repr().size() as usize;
        let off = len % cap;
        let page = if off == 0 {
            // A bulk append knows how many elements remain, so a first page
            // can be the exact size the run needs instead of growing by
            // copying (which `push` must do, one element at a time).
            let slots = if len == 0 { cap.min(FIRST_PAGE_SLOTS.max(n - done)) } else { cap };
            let page = r.add(new_page(elem, slots));
            unsafe { (*(obj(page).payload().add(8) as *const AtomicU64)).store(0, Ordering::Release) };
            put_page(dst, len / cap, page);
            page
        } else {
            hdr(dst).page(len / cap)
        };
        let free = slots(page, size) - off;
        if free == 0 {
            copy_tail(dst);
            continue;
        }
        let run = free.min(n - done);
        let count = unsafe { &*(obj(page).payload().add(8) as *const AtomicU64) };
        if !claim_n(count, off, run) {
            copy_tail(dst);
            continue;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(
                src.add(done * size),
                obj(page).payload().add(PAGE_HDR + off * size),
                run * size,
            );
        }
        if elem.is_ptr() {
            gc::barrier(page);
        }
        hdr(dst).set_len(len + run);
        done += run;
    }
}

/// Append the elements of `src[start..end]` to `dst`, one bulk run per
/// source page. `dst` is a fresh list or builder; `dst` and `src` must be
/// rooted by the caller.
pub fn push_run(dst: u64, src: u64, start: usize, end: usize) {
    if start >= end {
        return;
    }
    let h = hdr(src);
    let (cap, size) = (h.cap(), h.repr().size() as usize);
    let mut i = start;
    while i < end {
        let (p, off) = (i / cap, i % cap);
        let run = (cap - off).min(end - i);
        push_raw(dst, unsafe { obj(h.page(p)).payload().add(PAGE_HDR + off * size) }, run);
        i += run;
    }
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
    // only managed elements are roots (a scalar word is not an object)
    if matches!(&desc.kind, Kind::List { elem } if elem.is_ptr()) {
        for &w in words {
            r.add(w);
        }
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
