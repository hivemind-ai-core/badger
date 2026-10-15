//! Paged lists (`List<T>`) and their affine builders (`ListBuilder<T>`).
//!
//! A list object's payload holds a pointer to [`ListData`]: a page directory
//! of managed page objects. Pages have a fixed capacity derived from the
//! element size and a byte budget; only the final page may be partial.
//! Appending never moves existing elements (pages are stable), and freezing a
//! builder transfers the directory without copying elements.

use crate::desc::{Kind, Repr, TypeDesc};
use crate::gc::{self, Header, obj};

const PAGE_BYTES: usize = 4096;

pub struct ListData {
    pub len: usize,
    pub page_cap: usize,
    pub elem: &'static TypeDesc,
    pub repr: Repr,
    pub pages: Vec<u64>,
}

fn page_desc() -> &'static TypeDesc {
    static D: std::sync::OnceLock<&'static TypeDesc> = std::sync::OnceLock::new();
    D.get_or_init(|| {
        TypeDesc::new("Page", Kind::Opaque { trace: Some(trace_page), finalize: None, args: vec![] }).leak()
    })
}

/// Page payload: `elem desc ptr`, `len`, then element storage.
const PAGE_HDR: usize = 16;

fn trace_page(o: *mut Header, f: &mut dyn FnMut(u64)) {
    unsafe {
        let p = (*o).payload();
        let elem = &*(*(p as *const *const TypeDesc));
        if !elem.is_ptr() {
            return;
        }
        let n = *(p.add(8) as *const u64) as usize;
        for i in 0..n {
            f(*(p.add(PAGE_HDR + i * 8) as *const u64));
        }
    }
}

pub fn data<'a>(w: u64) -> &'a mut ListData {
    unsafe {
        let p = obj(w).payload() as *mut *mut ListData;
        if (*p).is_null() {
            crate::task::panic_current("use of a consumed list builder".into(), String::new());
        }
        &mut **p
    }
}

pub fn trace(o: *mut Header, f: &mut dyn FnMut(u64)) {
    unsafe {
        let p = (*o).payload() as *mut *mut ListData;
        if !(*p).is_null() {
            for &pg in &(**p).pages {
                f(pg);
            }
        }
    }
}

pub fn finalize(o: *mut Header) {
    unsafe {
        let p = (*o).payload() as *mut *mut ListData;
        if !(*p).is_null() {
            drop(Box::from_raw(*p));
            *p = std::ptr::null_mut();
        }
    }
}

/// Create an empty list (or builder) object with descriptor `desc`
/// (`Kind::List { elem }`).
pub fn new(desc: &'static TypeDesc) -> u64 {
    let elem = match &desc.kind {
        Kind::List { elem } => *elem,
        _ => crate::desc::builtin::unit(),
    };
    let repr = elem.repr();
    let size = repr.size() as usize;
    let page_cap = (PAGE_BYTES / size.max(1)).clamp(16, 4096);
    let w = gc::alloc(desc, 8);
    let d = Box::new(ListData { len: 0, page_cap, elem, repr, pages: vec![] });
    unsafe { *(obj(w).payload() as *mut *mut ListData) = Box::into_raw(d) };
    w
}

pub fn len(w: u64) -> usize {
    data(w).len
}

pub fn get(w: u64, i: usize) -> u64 {
    let d = data(w);
    let (pg, off) = (i / d.page_cap, i % d.page_cap);
    let page = d.pages[pg];
    unsafe {
        let p = obj(page).payload().add(PAGE_HDR);
        d.repr.load(p.add(off * d.repr.size() as usize))
    }
}

/// Read slot `slot` of a page object (for stable element references).
pub fn page_read(page: u64, slot: usize, repr: Repr) -> u64 {
    unsafe { repr.load(obj(page).payload().add(PAGE_HDR + slot * repr.size() as usize)) }
}

/// Address of element `i` (stable while the list is alive).
pub fn addr(w: u64, i: usize) -> *mut u8 {
    let d = data(w);
    let (pg, off) = (i / d.page_cap, i % d.page_cap);
    unsafe { obj(d.pages[pg]).payload().add(PAGE_HDR + off * d.repr.size() as usize) }
}

/// Append a word. The list object and value must be rooted by the caller.
pub fn push(w: u64, v: u64) {
    let (need_page, cap, size, elem) = {
        let d = data(w);
        (
            d.len % d.page_cap == 0 && d.len / d.page_cap == d.pages.len(),
            d.page_cap,
            d.repr.size() as usize,
            d.elem,
        )
    };
    if need_page {
        let _r = gc::Roots::new();
        _r.add(w);
        if elem.is_ptr() {
            _r.add(v);
        }
        // rooted until the list holds it
        let page = _r.add(gc::alloc(page_desc(), PAGE_HDR + cap * size.max(8)));
        unsafe {
            let p = obj(page).payload();
            *(p as *mut *const TypeDesc) = elem;
        }
        data(w).pages.push(page);
    }
    let d = data(w);
    let (pg, off) = (d.len / d.page_cap, d.len % d.page_cap);
    unsafe {
        let p = obj(d.pages[pg]).payload();
        d.repr.store(p.add(PAGE_HDR + off * size), v);
        // page length (for tracing)
        *(p.add(8) as *mut u64) = off as u64 + 1;
    }
    d.len += 1;
}

/// Freeze a builder into a list, transferring its page directory.
pub fn finish(builder: u64, list_desc: &'static TypeDesc) -> u64 {
    let _r = gc::Roots::new();
    _r.add(builder);
    let out = gc::alloc(list_desc, 8);
    unsafe {
        let src = obj(builder).payload() as *mut *mut ListData;
        if (*src).is_null() {
            crate::task::panic_current("list builder used after finish".into(), String::new());
        }
        *(obj(out).payload() as *mut *mut ListData) = *src;
        *src = std::ptr::null_mut();
    }
    out
}

pub fn from_words(desc: &'static TypeDesc, words: &[u64]) -> u64 {
    let _r = gc::Roots::new();
    for &w in words {
        _r.add(w);
    }
    let l = _r.add(new(desc));
    for &w in words {
        push(l, w);
    }
    l
}

pub fn to_vec(w: u64) -> Vec<u64> {
    (0..len(w)).map(|i| get(w, i)).collect()
}

/// Is `w` a list with a live (unconsumed) directory?
pub fn is_live(w: u64) -> bool {
    unsafe { !(*(obj(w).payload() as *const *mut ListData)).is_null() }
}

/// Take slot `used` of a shared page if no version has written it yet
/// (the page's written-slot count is `used`): an atomic claim, so two
/// appends to one list version never write the same slot.
fn claim_slot(page: u64, used: usize) -> bool {
    let count = unsafe { &*(obj(page).payload().add(8) as *const std::sync::atomic::AtomicU64) };
    count
        .compare_exchange(
            used as u64,
            used as u64 + 1,
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Relaxed,
        )
        .is_ok()
}

/// Persistent append: a new list sharing every page of `w`. The partial
/// tail page is shared too when no other version has written past `w`'s
/// end in it; otherwise it is copied first. O(pages), not O(elements).
pub fn append(w: u64, v: u64, desc: &'static TypeDesc) -> u64 {
    let _r = gc::Roots::new();
    _r.add(w);
    if data(w).elem.is_ptr() {
        _r.add(v);
    }
    let out = _r.add(new(desc));
    {
        let (src_pages, src_len) = (data(w).pages.clone(), data(w).len);
        let d = data(out);
        d.pages = src_pages;
        d.len = src_len;
    }
    let (len, cap, size) = {
        let d = data(out);
        (d.len, d.page_cap, d.repr.size() as usize)
    };
    let used = len % cap;
    if used != 0 {
        let last = *data(out).pages.last().unwrap();
        // claim the next slot of the shared tail page: exactly one version
        // extends it, even when workers append to one list concurrently
        if claim_slot(last, used) {
            let d = data(out);
            unsafe {
                d.repr.store(obj(last).payload().add(PAGE_HDR + used * size), v);
            }
            d.len += 1;
            return out;
        }
        {
            // another version extended this page: copy our prefix of it
            let elem = data(out).elem;
            let copy = _r.add(gc::alloc(page_desc(), PAGE_HDR + cap * size.max(8)));
            unsafe {
                let dst = obj(copy).payload();
                let src = obj(last).payload();
                *(dst as *mut *const TypeDesc) = elem;
                std::ptr::copy_nonoverlapping(src.add(PAGE_HDR), dst.add(PAGE_HDR), used * size);
                *(dst.add(8) as *mut u64) = used as u64;
            }
            *data(out).pages.last_mut().unwrap() = copy;
        }
    }
    push(out, v);
    out
}
