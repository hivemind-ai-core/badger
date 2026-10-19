//! Persistent maps and sets with value semantics. Tables are hash tries
//! ([`crate::hamt`]) of managed nodes: an update shares structure with the
//! original, so insert/remove are O(log n) and old versions stay valid.
//! Keys use descriptor-driven equality and hashing.
//!
//! A map object's payload is the trie root (0 when empty) and the length.

use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

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

/// Key hashes, fixed per build: a key hashes the same in every process, so
/// a map's shape and iteration order never depend on a random seed.
static HASHER: ahash::RandomState = ahash::RandomState::with_seeds(
    0x243f_6a88_85a3_08d3,
    0x1319_8a2e_0370_7344,
    0xa409_3822_299f_31d0,
    0x082e_fa98_ec4e_6c89,
);

fn hash(key: &TypeDesc, k: u64) -> u64 {
    let mut h = HASHER.build_hasher();
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
    freeze(m);
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
    freeze(m);
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
    let (kd, vd) = descs(m);
    let e = Entry { hash: hash(kd, k), key: k, val: v };
    let eq = |a, b| crate::value::eq(kd, a, b);
    // Builders/literals start empty and expose no snapshot before finishing:
    // every trie node, not just the map wrapper, is exclusively owned.
    let (t, added) = hamt::insert_mut(root(m), e, flags(kd, vd), &eq);
    set(m, t, len(m) + added as usize);
    // (a builder's map may predate a collection)
    gc::barrier(m);
}

// Owned wrappers add one scalar word; ordinary maps retain their 16-byte payload.
fn owner(m: u64) -> u64 {
    if unsafe { obj(m).payload_size() } == 24 {
        unsafe { *(obj(m).payload().add(16) as *const u64) }
    } else {
        0
    }
}

/// End the current private epoch before exposing a map or sharing its trie.
/// Descendant tags remain valid but can never match a future active epoch.
pub fn freeze(m: u64) {
    // Published owned wrappers already hold zero. Do not write them again:
    // multiple tasks may concurrently read the same frozen version.
    if owner(m) != 0 {
        unsafe { *(obj(m).payload().add(16) as *mut u64) = 0 };
    }
}

static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);

fn next_owner(counter: &AtomicU64) -> u64 {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| (n <= hamt::MAX_OWNER).then(|| n + 1))
        .unwrap_or(0)
}

/// Compiler-proved scalar-map update. Reuse the wrapper only when its old
/// aliases are dead; otherwise freeze it and start a fresh ownership epoch.
/// Every publication must call freeze, including an unknown call or cursor.
pub fn insert_owned(m: u64, k: u64, v: u64, old_alias_live: bool) -> u64 {
    let (kd, vd) = descs(m);
    if flags(kd, vd) != 0 {
        return insert(m, k, v);
    }
    let roots = gc::Roots::new();
    roots.add(m);
    let active = owner(m);
    let (out, token) = if active != 0 && !old_alias_live {
        (m, active)
    } else {
        // In particular, a local fork must invalidate the old active token:
        // either alias can be updated later, and they now share descendants.
        freeze(m);
        let token = next_owner(&NEXT_OWNER);
        if token == 0 {
            return insert(m, k, v);
        }
        let out = gc::alloc(unsafe { obj(m).desc() }, 24);
        set(out, root(m), len(m));
        unsafe { *(obj(out).payload().add(16) as *mut u64) = token };
        (out, token)
    };
    roots.add(out);
    let e = Entry { hash: hash(kd, k), key: k, val: v };
    let (t, added) =
        hamt::insert_owned(root(out), e, flags(kd, vd), &|a, b| crate::value::eq(kd, a, b), token);
    set(out, t, len(out) + added as usize);
    gc::barrier(out);
    out
}

pub fn remove(m: u64, k: u64) -> u64 {
    freeze(m);
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

/// Visit every entry in key order without materialising a list of them.
pub(crate) fn for_each(m: u64, f: &mut dyn FnMut(u64, u64)) {
    hamt::for_each(root(m), &mut |e| f(e.key, e.val));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn owned_epochs_preserve_versions_and_gc_edges() {
        // Unit-test heap is global. Exercise it only in isolated child processes.
        if std::env::var_os("BADGER_OWNED_MAP_TEST_CHILD").is_none() {
            for gen_mode in ["0", "1"] {
                for verify in ["0", "1"] {
                    for stress in ["0", "1"] {
                        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
                        for (key, _) in std::env::vars_os() {
                            if key.to_string_lossy().starts_with("BADGER_") {
                                command.env_remove(key);
                            }
                        }
                        let output = command
                            .args(["--exact", "map::tests::owned_epochs_preserve_versions_and_gc_edges"])
                            .env("BADGER_OWNED_MAP_TEST_CHILD", "1")
                            .env("BADGER_GC_GENERATIONAL", gen_mode)
                            .env("BADGER_GC_VERIFY", verify)
                            .env("BADGER_GC_STRESS", stress)
                            .output()
                            .unwrap();
                        assert!(
                            output.status.success(),
                            "gen={gen_mode} verify={verify} stress={stress}: {}{}",
                            String::from_utf8_lossy(&output.stdout),
                            String::from_utf8_lossy(&output.stderr)
                        );
                    }
                }
            }
            return;
        }
        // The last token is issued once; exhaustion never wraps or reissues it.
        let counter = AtomicU64::new(hamt::MAX_OWNER - 1);
        assert_eq!(next_owner(&counter), hamt::MAX_OWNER - 1);
        assert_eq!(next_owner(&counter), hamt::MAX_OWNER);
        assert_eq!(next_owner(&counter), 0);
        assert_eq!(next_owner(&counter), 0);

        let scalar = crate::desc::builtin::int();
        let desc = TypeDesc::new("Map", Kind::Map { key: scalar, val: scalar }).leak();
        for interval in [1, 2, 64, 256] {
            let roots = gc::Roots::new();
            let mut m = roots.add(new(desc));
            let mut model = HashMap::new();
            let mut versions = vec![];
            for step in 0..512u64 {
                {
                    let temp = gc::Roots::new();
                    temp.add(m);
                    let previous = m;
                    let active = owner(m);
                    let k = step.wrapping_mul(173) % 257;
                    m = temp.add(insert_owned(m, k, step, false));
                    if active != 0 {
                        assert_eq!(m, previous);
                    }
                    model.insert(k, step);
                    if step % 31 == 0 {
                        gc::collect();
                    }
                }
                if (step + 1).is_multiple_of(interval) {
                    freeze(m);
                    assert_eq!(owner(m), 0);
                    roots.add(m);
                    versions.push((m, model.clone()));
                }
            }
            roots.add(m);
            // Fork while an old alias is live, then update BOTH sides. Neither
            // branch may retain a token that can edit a shared descendant.
            let old = m;
            let fork = roots.add(insert_owned(old, 0, 9001, true));
            assert_eq!(owner(old), 0);
            let old_changed = roots.add(insert_owned(old, 1, 9002, false));
            assert_eq!(get(old, 0), model.get(&0).copied());
            assert_eq!(get(old, 1), model.get(&1).copied());
            assert_eq!(get(fork, 0), Some(9001));
            assert_eq!(get(fork, 1), model.get(&1).copied());
            assert_eq!(get(old_changed, 0), model.get(&0).copied());
            assert_eq!(get(old_changed, 1), Some(9002));
            // Ordinary sharing APIs close the epoch as a runtime backstop.
            let persistent = roots.add(insert(fork, 2, 9003));
            assert_eq!(owner(fork), 0);
            let fork2 = roots.add(insert_owned(fork, 2, 9004, false));
            assert_eq!(get(persistent, 2), Some(9003));
            assert_eq!(get(fork2, 2), Some(9004));
            let removed = roots.add(remove(fork2, 2));
            assert_eq!(owner(fork2), 0);
            assert_eq!(get(removed, 2), None);
            let _cursor = roots.add(cursor(old_changed));
            assert_eq!(owner(old_changed), 0);
            gc::collect();
            for (version, reference) in versions {
                assert_eq!(len(version), reference.len());
                for k in 0..257 {
                    assert_eq!(
                        get(version, k),
                        reference.get(&k).copied(),
                        "snapshot interval {interval}, key {k}"
                    );
                }
            }
        }
        // Test the complete helper's exhausted-token fallback, not just the
        // allocator. This process is isolated and has no other mutators.
        NEXT_OWNER.store(hamt::MAX_OWNER, Ordering::Relaxed);
        let roots = gc::Roots::new();
        let empty = roots.add(new(desc));
        let last = roots.add(insert_owned(empty, 7, 11, false));
        assert_eq!(owner(last), hamt::MAX_OWNER);
        for _ in 0..20 {
            gc::collect();
            assert_eq!(owner(last), hamt::MAX_OWNER);
            assert_eq!(unsafe { obj(root(last)).meta } & gc::CLIENT_META, gc::CLIENT_META);
        }
        let next = roots.add(insert_owned(last, 7, 12, true));
        assert_eq!(owner(last), 0);
        assert_eq!(unsafe { obj(next).payload_size() }, 16);
        assert_eq!(get(last, 7), Some(11));
        assert_eq!(get(next, 7), Some(12));
        let again = roots.add(insert_owned(next, 7, 13, false));
        gc::collect();
        assert_eq!(get(next, 7), Some(12));
        assert_eq!(get(again, 7), Some(13));
    }
}
