//! Immutable UTF-8 strings, byte strings, and interned symbols.
//!
//! Layout: payload = `len: u64` followed by `len` bytes.

use std::sync::Mutex;

use rustc_hash::FxHashMap;

use crate::desc::builtin;
use crate::gc::{self, obj};

pub fn alloc_bytes(desc: &'static crate::desc::TypeDesc, b: &[u8]) -> u64 {
    let w = gc::alloc(desc, 8 + b.len());
    unsafe {
        let p = obj(w).payload();
        *(p as *mut u64) = b.len() as u64;
        std::ptr::copy_nonoverlapping(b.as_ptr(), p.add(8), b.len());
    }
    w
}

pub fn new_string(s: &str) -> u64 {
    alloc_bytes(builtin::string(), s.as_bytes())
}

pub fn new_bytes(b: &[u8]) -> u64 {
    alloc_bytes(builtin::bytes(), b)
}

pub fn immortal_string(s: &str) -> u64 {
    let w = gc::alloc_immortal(builtin::string(), 8 + s.len());
    unsafe {
        let p = obj(w).payload();
        *(p as *mut u64) = s.len() as u64;
        std::ptr::copy_nonoverlapping(s.as_ptr(), p.add(8), s.len());
    }
    w
}

pub fn bytes_of<'a>(w: u64) -> &'a [u8] {
    if w == 0 {
        return &[];
    }
    unsafe {
        let p = obj(w).payload();
        let n = *(p as *const u64) as usize;
        std::slice::from_raw_parts(p.add(8), n)
    }
}

pub fn str_of<'a>(w: u64) -> &'a str {
    unsafe { std::str::from_utf8_unchecked(bytes_of(w)) }
}

static SYMBOLS: Mutex<Option<FxHashMap<String, u64>>> = Mutex::new(None);

/// Interned symbol: an immortal string object, unique per name.
pub fn symbol(name: &str) -> u64 {
    let mut g = SYMBOLS.lock().unwrap();
    let m = g.get_or_insert_with(FxHashMap::default);
    if let Some(&w) = m.get(name) {
        return w;
    }
    let w = gc::alloc_immortal(builtin::symbol(), 8 + name.len());
    unsafe {
        let p = obj(w).payload();
        *(p as *mut u64) = name.len() as u64;
        std::ptr::copy_nonoverlapping(name.as_ptr(), p.add(8), name.len());
    }
    m.insert(name.to_string(), w);
    w
}
