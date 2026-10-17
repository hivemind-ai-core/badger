//! Persistent maps and sets with value semantics. Tables are hash tries
//! ([`crate::hamt`]) of managed nodes: an update shares structure with the
//! original, so insert/remove are O(log n) and old versions stay valid.
//! Keys use descriptor-driven equality and hashing.
//!
//! A map object's payload is the trie root (0 when empty) and the length.

use std::hash::Hasher;

use crate::desc::{Kind, TypeDesc};
use crate::gc::{self, Header, obj};
use crate::hamt::{self, Entry};

fn descs(m: u64) -> (&'static TypeDesc, Option<&'static TypeDesc>) {
    match &unsafe { obj(m).desc() }.kind {
        Kind::Map { key, val } => (key, Some(val)),
        Kind::Set { elem } => (elem, None),
        _ => (crate::desc::builtin::unit(), None),
    }
}

fn root(m: u64) -> u64 {
    unsafe { *(obj(m).payload() as *const u64) }
}

pub fn len(m: u64) -> usize {
    unsafe { *(obj(m).payload().add(8) as *const u64) as usize }
}

fn set(m: u64, root: u64, len: usize) {
    unsafe {
        let p = obj(m).payload() as *mut u64;
        *p = root;
        *p.add(1) = len as u64;
    }
}

fn hash(key: &TypeDesc, k: u64) -> u64 {
    let mut h = rustc_hash::FxHasher::default();
    crate::value::hash(key, k, &mut h);
    h.finish()
}

fn flags(key: &TypeDesc, val: Option<&TypeDesc>) -> u32 {
    let mut f = 0;
    if key.is_ptr() {
        f |= hamt::KEY_PTR;
    }
    if val.is_some_and(|v| v.is_ptr()) {
        f |= hamt::VAL_PTR;
    }
    f
}

pub fn trace(o: *mut Header, f: &mut dyn FnMut(u64)) {
    let r = unsafe { *((*o).payload() as *const u64) };
    if r != 0 {
        f(r);
    }
}

/// A cursor over map or set `m`'s entries (see [`hamt::cursor`]).
pub fn cursor(m: u64) -> u64 {
    let (kd, vd) = descs(m);
    let r = gc::Roots::new();
    r.add(m);
    hamt::cursor(root(m), flags(kd, vd))
}

/// New empty map/set for descriptor `desc` (`Kind::Map` or `Kind::Set`).
pub fn new(desc: &'static TypeDesc) -> u64 {
    gc::alloc(desc, 16)
}

/// The root and length after inserting `k` -> `v` into map `m` (rooted, as
/// are `k` and `v` when managed).
fn inserted(m: u64, k: u64, v: u64) -> (u64, usize) {
    let (kd, vd) = descs(m);
    let e = Entry { hash: hash(kd, k), key: k, val: v };
    let eq = |a, b| crate::value::eq(kd, a, b);
    let (r, added) = hamt::insert(root(m), e, flags(kd, vd), &eq);
    (r, len(m) + added as usize)
}

fn root_args(r: &gc::Roots, m: u64, k: u64, v: u64) {
    let (kd, vd) = descs(m);
    r.add(m);
    if kd.is_ptr() {
        r.add(k);
    }
    if vd.is_some_and(|d| d.is_ptr()) {
        r.add(v);
    }
}

/// Persistent insert: returns a new map.
pub fn insert(m: u64, k: u64, v: u64) -> u64 {
    let r = gc::Roots::new();
    root_args(&r, m, k, v);
    let (t, n) = inserted(m, k, v);
    r.add(t);
    let out = gc::alloc(unsafe { obj(m).desc() }, 16);
    set(out, t, n);
    out
}

/// Insert into a fresh (unshared) map in place (builders/literals).
pub fn insert_mut(m: u64, k: u64, v: u64) {
    let r = gc::Roots::new();
    root_args(&r, m, k, v);
    let (t, n) = inserted(m, k, v);
    set(m, t, n);
    // (a builder's map may predate a collection)
    gc::barrier(m);
}

pub fn remove(m: u64, k: u64) -> u64 {
    let r = gc::Roots::new();
    root_args(&r, m, k, 0);
    let (kd, _) = descs(m);
    let eq = |a, b| crate::value::eq(kd, a, b);
    let Some(t) = hamt::remove(root(m), hash(kd, k), k, &eq) else {
        // absent: an equal new version
        let out = gc::alloc(unsafe { obj(m).desc() }, 16);
        set(out, root(m), len(m));
        return out;
    };
    r.add(t);
    let out = gc::alloc(unsafe { obj(m).desc() }, 16);
    set(out, t, len(m) - 1);
    out
}

pub fn get(m: u64, k: u64) -> Option<u64> {
    let (kd, _) = descs(m);
    hamt::get(root(m), hash(kd, k), k, &|a, b| crate::value::eq(kd, a, b))
}

pub fn entries(m: u64) -> Vec<(u64, u64)> {
    let mut out = Vec::with_capacity(len(m));
    hamt::for_each(root(m), &mut |e| out.push((e.key, e.val)));
    out
}

pub fn eq(a: u64, b: u64) -> bool {
    if len(a) != len(b) {
        return false;
    }
    let (kd, vd) = descs(a);
    let rb = root(b);
    let key_eq = |x, y| crate::value::eq(kd, x, y);
    let mut same = true;
    hamt::for_each(root(a), &mut |e| {
        if same {
            same = match hamt::get(rb, e.hash, e.key, &key_eq) {
                Some(v2) => vd.is_none_or(|vd| crate::value::eq(vd, e.val, v2)),
                None => false,
            };
        }
    });
    same
}
