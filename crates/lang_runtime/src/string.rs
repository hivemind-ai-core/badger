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

/// Byte boundaries for a checked Unicode-scalar slice. Only scan up to
/// `end`; a small prefix does not build an index of the whole input.
pub fn char_range(s: &str, start: i64, end: i64) -> Option<std::ops::Range<usize>> {
    if start < 0 || end < start {
        return None;
    }
    let (start, end) = (start as usize, end as usize);
    if end <= s.len() && s.as_bytes()[..end].is_ascii() {
        return Some(start..end);
    }
    let mut chars = s.char_indices();
    let from = if start == 0 {
        0
    } else {
        let (i, c) = chars.nth(start - 1)?;
        i + c.len_utf8()
    };
    let to = if end == start {
        from
    } else {
        let (i, c) = chars.nth(end - start - 1)?;
        i + c.len_utf8()
    };
    Some(from..to)
}

#[cfg(test)]
mod tests {
    #[test]
    fn scalar_slice_matches_character_reference() {
        for input in ["", "abc", "éλ🦀z", "a\u{301}b", "\r\n", "aéz"] {
            let chars: Vec<_> = input.chars().collect();
            for a in -1..=chars.len() as i64 + 1 {
                for b in -1..=chars.len() as i64 + 1 {
                    let expected = if a >= 0 && b >= a && b as usize <= chars.len() {
                        Some(chars[a as usize..b as usize].iter().collect::<String>())
                    } else {
                        None
                    };
                    let actual = super::char_range(input, a, b).map(|r| input[r].to_string());
                    assert_eq!(actual, expected, "{input:?} [{a}, {b})");
                }
            }
        }
    }
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
