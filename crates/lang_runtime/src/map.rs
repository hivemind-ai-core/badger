//! Persistent maps and sets with value semantics. Tables are hash array
//! mapped tries (`imbl`): an update shares structure with the original, so
//! insert/remove are O(log n) and old versions stay valid. Keys use
//! descriptor-driven equality and hashing.

use std::hash::{Hash, Hasher};

type Table = imbl::GenericHashMap<Key, u64, rustc_hash::FxBuildHasher, imbl::shared_ptr::DefaultSharedPtr>;

use crate::desc::{Kind, TypeDesc};
use crate::gc::{self, Header, obj};

#[derive(Clone, Copy)]
pub struct Key {
    pub w: u64,
    pub d: &'static TypeDesc,
}

impl Hash for Key {
    fn hash<H: Hasher>(&self, h: &mut H) {
        crate::value::hash(self.d, self.w, h);
    }
}

impl PartialEq for Key {
    fn eq(&self, o: &Key) -> bool {
        crate::value::eq(self.d, self.w, o.w)
    }
}

impl Eq for Key {}

pub struct MapData {
    pub key: &'static TypeDesc,
    pub val: Option<&'static TypeDesc>,
    pub table: Table,
}

pub fn data<'a>(w: u64) -> &'a mut MapData {
    unsafe { &mut **(obj(w).payload() as *mut *mut MapData) }
}

pub fn trace(o: *mut Header, f: &mut dyn FnMut(u64)) {
    unsafe {
        let p = *((*o).payload() as *mut *mut MapData);
        if p.is_null() {
            return;
        }
        let d = &*p;
        let kp = d.key.is_ptr();
        let vp = d.val.is_some_and(|v| v.is_ptr());
        for (k, v) in &d.table {
            if kp {
                f(k.w);
            }
            if vp {
                f(*v);
            }
        }
    }
}

pub fn finalize(o: *mut Header) {
    unsafe {
        let p = (*o).payload() as *mut *mut MapData;
        if !(*p).is_null() {
            drop(Box::from_raw(*p));
            *p = std::ptr::null_mut();
        }
    }
}

/// New empty map/set for descriptor `desc` (`Kind::Map` or `Kind::Set`).
pub fn new(desc: &'static TypeDesc) -> u64 {
    let (key, val) = match &desc.kind {
        Kind::Map { key, val } => (*key, Some(*val)),
        Kind::Set { elem } => (*elem, None),
        _ => (crate::desc::builtin::unit(), None),
    };
    with_table(desc, MapData { key, val, table: Table::default() })
}

fn with_table(desc: &'static TypeDesc, d: MapData) -> u64 {
    let w = gc::alloc(desc, 8);
    // a new version owns roughly one copied trie path
    gc::add_pressure(512);
    unsafe { *(obj(w).payload() as *mut *mut MapData) = Box::into_raw(Box::new(d)) };
    w
}

/// Persistent insert: returns a new map.
pub fn insert(m: u64, k: u64, v: u64) -> u64 {
    let _r = gc::Roots::new();
    _r.add(m);
    let src = data(m);
    if src.key.is_ptr() {
        _r.add(k);
    }
    if src.val.is_some_and(|d| d.is_ptr()) {
        _r.add(v);
    }
    let mut table = src.table.clone();
    table.insert(Key { w: k, d: src.key }, v);
    let desc = unsafe { obj(m).desc() };
    with_table(desc, MapData { key: src.key, val: src.val, table })
}

/// Insert into a fresh (unshared) map in place (builders/literals).
pub fn insert_mut(m: u64, k: u64, v: u64) {
    let d = data(m);
    d.table.insert(Key { w: k, d: d.key }, v);
}

pub fn remove(m: u64, k: u64) -> u64 {
    let _r = gc::Roots::new();
    _r.add(m);
    let src = data(m);
    let mut table = src.table.clone();
    table.remove(&Key { w: k, d: src.key });
    let desc = unsafe { obj(m).desc() };
    with_table(desc, MapData { key: src.key, val: src.val, table })
}

pub fn get(m: u64, k: u64) -> Option<u64> {
    let d = data(m);
    d.table.get(&Key { w: k, d: d.key }).copied()
}

pub fn len(m: u64) -> usize {
    data(m).table.len()
}

pub fn entries(m: u64) -> Vec<(u64, u64)> {
    data(m).table.iter().map(|(k, v)| (k.w, *v)).collect()
}

pub fn eq(a: u64, b: u64) -> bool {
    let (da, db) = (data(a), data(b));
    if da.table.len() != db.table.len() {
        return false;
    }
    da.table.iter().all(|(k, v)| match db.table.get(k) {
        Some(v2) => match da.val {
            Some(vd) => crate::value::eq(vd, *v, *v2),
            None => true,
        },
        None => false,
    })
}
