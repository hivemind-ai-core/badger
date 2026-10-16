//! Persistent hash tries (CHAMP layout) for maps and sets.
//!
//! Nodes are managed objects: an update copies the nodes on its path and
//! shares the rest, old versions die when the collector finds them
//! unreachable, and no reference counts are touched. A node's payload is
//!
//! ```text
//! datamap u32 | nodemap u32 | flags u32 | entry count u32
//! entries: (hash, key, value) words, in bit order
//! children: node pointers, in bit order
//! ```
//!
//! Bit `i` of `datamap` (`nodemap`) says hash chunk `i` at this depth holds
//! an inline entry (a child node). Once the 64 hash bits are used up, a
//! node is a collision bucket: no bitmaps, entries searched linearly.
//! Removal pulls a lone entry back into its parent, so a trie's shape
//! depends only on its content (apart from the order inside collision
//! buckets). `flags` say whether keys/values are managed pointers, for
//! tracing.
//!
//! Callers keep the trie being read or updated reachable (rooted), and root
//! keys and values passed in; allocation of new nodes may collect.

use crate::desc::{Kind, TypeDesc};
use crate::gc::{self, Header, obj};

const BITS: u32 = 5;
const HDR: usize = 16;
pub const KEY_PTR: u32 = 1;
pub const VAL_PTR: u32 = 2;

#[derive(Clone, Copy)]
pub struct Entry {
    pub hash: u64,
    pub key: u64,
    pub val: u64,
}

fn node_desc() -> &'static TypeDesc {
    static D: std::sync::OnceLock<&'static TypeDesc> = std::sync::OnceLock::new();
    D.get_or_init(|| {
        TypeDesc::new("MapNode", Kind::Opaque { trace: Some(trace_node), finalize: None, args: vec![] })
            .leak()
    })
}

fn trace_node(o: *mut Header, f: &mut dyn FnMut(u64)) {
    let n = Node(unsafe { (*o).payload() });
    let fl = n.flags();
    if fl & (KEY_PTR | VAL_PTR) != 0 {
        for i in 0..n.entries() {
            let e = n.entry(i);
            if fl & KEY_PTR != 0 {
                f(e.key);
            }
            if fl & VAL_PTR != 0 {
                f(e.val);
            }
        }
    }
    for i in 0..n.children() {
        f(n.child(i));
    }
}

#[derive(Clone, Copy)]
struct Node(*mut u8);

impl Node {
    fn of(w: u64) -> Node {
        Node(unsafe { obj(w).payload() })
    }
    fn word(self, off: usize) -> u64 {
        unsafe { *(self.0.add(off) as *const u64) }
    }
    fn u32_at(self, off: usize) -> u32 {
        unsafe { *(self.0.add(off) as *const u32) }
    }
    fn datamap(self) -> u32 {
        self.u32_at(0)
    }
    fn nodemap(self) -> u32 {
        self.u32_at(4)
    }
    fn flags(self) -> u32 {
        self.u32_at(8)
    }
    fn entries(self) -> usize {
        self.u32_at(12) as usize
    }
    fn children(self) -> usize {
        self.nodemap().count_ones() as usize
    }
    fn entry(self, i: usize) -> Entry {
        let o = HDR + i * 24;
        Entry { hash: self.word(o), key: self.word(o + 8), val: self.word(o + 16) }
    }
    fn child(self, i: usize) -> u64 {
        self.word(HDR + self.entries() * 24 + i * 8)
    }
}

fn bit(hash: u64, shift: u32) -> u32 {
    1 << ((hash >> shift) & 31)
}

fn index(map: u32, b: u32) -> usize {
    (map & (b - 1)).count_ones() as usize
}

fn collision(shift: u32) -> bool {
    shift >= 64
}

/// Allocate a node and fill it. Child pointers must be reachable or rooted.
fn alloc_node(datamap: u32, nodemap: u32, flags: u32, entries: &[Entry], children: &[u64]) -> u64 {
    let w = gc::alloc(node_desc(), HDR + entries.len() * 24 + children.len() * 8);
    unsafe {
        let p = obj(w).payload();
        *(p as *mut u32) = datamap;
        *(p.add(4) as *mut u32) = nodemap;
        *(p.add(8) as *mut u32) = flags;
        *(p.add(12) as *mut u32) = entries.len() as u32;
        let mut q = p.add(HDR) as *mut u64;
        for e in entries {
            *q = e.hash;
            *q.add(1) = e.key;
            *q.add(2) = e.val;
            q = q.add(3);
        }
        std::ptr::copy_nonoverlapping(children.as_ptr(), q, children.len());
    }
    w
}

/// One edit of a sequence: at `at`, drop `del` old items and insert `ins`.
#[derive(Clone, Copy)]
struct Edit<T> {
    at: usize,
    del: usize,
    ins: Option<T>,
}

impl<T> Edit<T> {
    fn none() -> Self {
        Edit { at: 0, del: 0, ins: None }
    }
}

/// A copy of node `w` with new bitmaps and one edit to its entries and one
/// to its children.
fn splice(w: u64, datamap: u32, nodemap: u32, e: Edit<Entry>, c: Edit<u64>) -> u64 {
    let n = Node::of(w);
    let (ne, nc) = (n.entries(), n.children());
    let new_ne = ne - e.del + e.ins.is_some() as usize;
    let new_nc = nc - c.del + c.ins.is_some() as usize;
    let out = gc::alloc(node_desc(), HDR + new_ne * 24 + new_nc * 8);
    unsafe {
        let p = obj(out).payload();
        *(p as *mut u32) = datamap;
        *(p.add(4) as *mut u32) = nodemap;
        *(p.add(8) as *mut u32) = n.flags();
        *(p.add(12) as *mut u32) = new_ne as u32;
        let src = n.0.add(HDR) as *const u64;
        let dst = p.add(HDR) as *mut u64;
        // entries: [0, at) | ins | [at + del, ne)
        std::ptr::copy_nonoverlapping(src, dst, e.at * 3);
        let mut k = e.at * 3;
        if let Some(x) = e.ins {
            *dst.add(k) = x.hash;
            *dst.add(k + 1) = x.key;
            *dst.add(k + 2) = x.val;
            k += 3;
        }
        let rest = (ne - e.at - e.del) * 3;
        std::ptr::copy_nonoverlapping(src.add((e.at + e.del) * 3), dst.add(k), rest);
        // children likewise
        let src = src.add(ne * 3);
        let dst = dst.add(new_ne * 3);
        std::ptr::copy_nonoverlapping(src, dst, c.at);
        let mut k = c.at;
        if let Some(x) = c.ins {
            *dst.add(k) = x;
            k += 1;
        }
        std::ptr::copy_nonoverlapping(src.add(c.at + c.del), dst.add(k), nc - c.at - c.del);
    }
    out
}

/// A node holding two entries whose hashes agree below `shift`.
fn pair(a: Entry, b: Entry, shift: u32, flags: u32) -> u64 {
    if collision(shift) {
        return alloc_node(0, 0, flags, &[a, b], &[]);
    }
    let (ba, bb) = (bit(a.hash, shift), bit(b.hash, shift));
    if ba == bb {
        let r = gc::Roots::new();
        let c = r.add(pair(a, b, shift + BITS, flags));
        alloc_node(0, ba, flags, &[], &[c])
    } else if ba < bb {
        alloc_node(ba | bb, 0, flags, &[a, b], &[])
    } else {
        alloc_node(ba | bb, 0, flags, &[b, a], &[])
    }
}

pub fn get(root: u64, hash: u64, key: u64, eq: &dyn Fn(u64, u64) -> bool) -> Option<u64> {
    if root == 0 {
        return None;
    }
    let mut n = Node::of(root);
    let mut shift = 0;
    loop {
        if collision(shift) {
            return (0..n.entries())
                .map(|i| n.entry(i))
                .find(|e| e.hash == hash && eq(e.key, key))
                .map(|e| e.val);
        }
        let b = bit(hash, shift);
        let (dm, nm) = (n.datamap(), n.nodemap());
        if dm & b != 0 {
            let e = n.entry(index(dm, b));
            return (e.hash == hash && eq(e.key, key)).then_some(e.val);
        }
        if nm & b == 0 {
            return None;
        }
        n = Node::of(n.child(index(nm, b)));
        shift += BITS;
    }
}

/// Insert or replace: the new root, and whether the key was new.
pub fn insert(root: u64, e: Entry, flags: u32, eq: &dyn Fn(u64, u64) -> bool) -> (u64, bool) {
    if root == 0 {
        return (alloc_node(bit(e.hash, 0), 0, flags, &[e], &[]), true);
    }
    insert_at(root, e, 0, flags, eq)
}

fn insert_at(w: u64, e: Entry, shift: u32, flags: u32, eq: &dyn Fn(u64, u64) -> bool) -> (u64, bool) {
    let n = Node::of(w);
    if collision(shift) {
        for i in 0..n.entries() {
            let x = n.entry(i);
            if x.hash == e.hash && eq(x.key, e.key) {
                return (splice(w, 0, 0, Edit { at: i, del: 1, ins: Some(e) }, Edit::none()), false);
            }
        }
        return (splice(w, 0, 0, Edit { at: n.entries(), del: 0, ins: Some(e) }, Edit::none()), true);
    }
    let b = bit(e.hash, shift);
    let (dm, nm) = (n.datamap(), n.nodemap());
    if dm & b != 0 {
        let i = index(dm, b);
        let x = n.entry(i);
        if x.hash == e.hash && eq(x.key, e.key) {
            return (splice(w, dm, nm, Edit { at: i, del: 1, ins: Some(e) }, Edit::none()), false);
        }
        // the resident entry and the new one move into a child
        let r = gc::Roots::new();
        let c = r.add(pair(x, e, shift + BITS, flags));
        let j = index(nm, b);
        let out = splice(
            w,
            dm & !b,
            nm | b,
            Edit { at: i, del: 1, ins: None },
            Edit { at: j, del: 0, ins: Some(c) },
        );
        return (out, true);
    }
    if nm & b != 0 {
        let j = index(nm, b);
        let (c, added) = insert_at(n.child(j), e, shift + BITS, flags, eq);
        let r = gc::Roots::new();
        r.add(c);
        return (splice(w, dm, nm, Edit::none(), Edit { at: j, del: 1, ins: Some(c) }), added);
    }
    (splice(w, dm | b, nm, Edit { at: index(dm, b), del: 0, ins: Some(e) }, Edit::none()), true)
}

enum Removed {
    Node(u64),
    /// the node would hold just this entry (its parent takes it inline)
    Lone(Entry),
}

/// Remove a key: the new root (0 when empty), or `None` if it is absent
/// (the trie is unchanged).
pub fn remove(root: u64, hash: u64, key: u64, eq: &dyn Fn(u64, u64) -> bool) -> Option<u64> {
    get(root, hash, key, eq)?;
    Some(match remove_at(root, hash, key, 0, eq) {
        Removed::Node(w) => w,
        Removed::Lone(e) => {
            let flags = Node::of(root).flags();
            alloc_node(bit(e.hash, 0), 0, flags, &[e], &[])
        }
    })
}

/// Remove a key known to be present below `w`.
fn remove_at(w: u64, hash: u64, key: u64, shift: u32, eq: &dyn Fn(u64, u64) -> bool) -> Removed {
    let n = Node::of(w);
    let (ne, nc) = (n.entries(), n.children());
    if collision(shift) {
        let i = (0..ne).find(|&i| n.entry(i).hash == hash && eq(n.entry(i).key, key)).unwrap();
        if ne == 2 {
            return Removed::Lone(n.entry(1 - i));
        }
        return Removed::Node(splice(w, 0, 0, Edit { at: i, del: 1, ins: None }, Edit::none()));
    }
    let b = bit(hash, shift);
    let (dm, nm) = (n.datamap(), n.nodemap());
    if dm & b != 0 {
        let i = index(dm, b);
        if shift > 0 && ne == 2 && nc == 0 {
            return Removed::Lone(n.entry(1 - i));
        }
        if ne == 1 && nc == 0 {
            // the root's last entry
            return Removed::Node(0);
        }
        return Removed::Node(splice(w, dm & !b, nm, Edit { at: i, del: 1, ins: None }, Edit::none()));
    }
    let j = index(nm, b);
    match remove_at(n.child(j), hash, key, shift + BITS, eq) {
        Removed::Node(c) => {
            let r = gc::Roots::new();
            r.add(c);
            Removed::Node(splice(w, dm, nm, Edit::none(), Edit { at: j, del: 1, ins: Some(c) }))
        }
        Removed::Lone(e) => {
            if shift > 0 && ne == 0 && nc == 1 {
                return Removed::Lone(e);
            }
            let i = index(dm, b);
            Removed::Node(splice(
                w,
                dm | b,
                nm & !b,
                Edit { at: i, del: 0, ins: Some(e) },
                Edit { at: j, del: 1, ins: None },
            ))
        }
    }
}

/// A walk over a trie's entries in [`for_each`]'s order, one step at a
/// time (`for` loops over maps and sets: no list of entries is built). A
/// cursor object's payload is
///
/// ```text
/// key word | value word | flags u32 | depth u32 | (node, position) * MAX_DEPTH
/// ```
///
/// where a position counts a node's entries, then its children. The
/// current key and value sit at fixed offsets ([`CURSOR_KEY`],
/// [`CURSOR_VAL`]) for generated code to load after [`cursor_next`].
pub const CURSOR_KEY: i32 = 0;
pub const CURSOR_VAL: i32 = 8;
/// Trie levels: 64 hash bits in 5-bit chunks (13), then a collision bucket.
const MAX_DEPTH: usize = 14;
const CURSOR_STACK: usize = 24;

fn cursor_desc() -> &'static TypeDesc {
    static D: std::sync::OnceLock<&'static TypeDesc> = std::sync::OnceLock::new();
    D.get_or_init(|| {
        TypeDesc::new("MapCursor", Kind::Opaque { trace: Some(trace_cursor), finalize: None, args: vec![] })
            .leak()
    })
}

fn trace_cursor(o: *mut Header, f: &mut dyn FnMut(u64)) {
    let p = unsafe { (*o).payload() };
    let w = |off: usize| unsafe { *(p.add(off) as *const u64) };
    let flags = unsafe { *(p.add(16) as *const u32) };
    let depth = unsafe { *(p.add(20) as *const u32) } as usize;
    if flags & KEY_PTR != 0 {
        f(w(CURSOR_KEY as usize));
    }
    if flags & VAL_PTR != 0 {
        f(w(CURSOR_VAL as usize));
    }
    for i in 0..depth {
        f(w(CURSOR_STACK + i * 16));
    }
}

/// A cursor at the start of the trie `root` (0: empty) whose entries'
/// keys and values are managed pointers as `flags` say. The caller keeps
/// the trie reachable until the cursor exists (it then keeps the nodes it
/// will visit).
pub fn cursor(root: u64, flags: u32) -> u64 {
    let r = gc::Roots::new();
    if root != 0 {
        r.add(root);
    }
    let c = gc::alloc(cursor_desc(), CURSOR_STACK + MAX_DEPTH * 16);
    unsafe {
        let p = obj(c).payload();
        *(p.add(16) as *mut u32) = flags;
        if root != 0 {
            *(p.add(20) as *mut u32) = 1;
            *(p.add(CURSOR_STACK) as *mut u64) = root;
        }
    }
    c
}

/// Advance cursor `c` to the next entry: 1 when there is one (its key and
/// value are then at [`CURSOR_KEY`] and [`CURSOR_VAL`]), 0 at the end.
/// Allocates nothing.
pub fn cursor_next(c: u64) -> u64 {
    unsafe {
        let p = obj(c).payload();
        let depth = &mut *(p.add(20) as *mut u32);
        loop {
            if *depth == 0 {
                *(p as *mut u64) = 0;
                *(p.add(8) as *mut u64) = 0;
                return 0;
            }
            let top = CURSOR_STACK + (*depth as usize - 1) * 16;
            let node = *(p.add(top) as *const u64);
            let pos = &mut *(p.add(top + 8) as *mut u64);
            let n = Node::of(node);
            let (ne, nc) = (n.entries() as u64, n.children() as u64);
            if *pos < ne {
                let e = n.entry(*pos as usize);
                *pos += 1;
                *(p as *mut u64) = e.key;
                *(p.add(8) as *mut u64) = e.val;
                return 1;
            }
            if *pos < ne + nc {
                let child = n.child((*pos - ne) as usize);
                *pos += 1;
                let next = CURSOR_STACK + *depth as usize * 16;
                debug_assert!((*depth as usize) < MAX_DEPTH);
                *(p.add(next) as *mut u64) = child;
                *(p.add(next + 8) as *mut u64) = 0;
                *depth += 1;
                continue;
            }
            *depth -= 1;
        }
    }
}

/// Visit every entry (no allocation may happen during the walk).
pub fn for_each(root: u64, f: &mut dyn FnMut(Entry)) {
    if root == 0 {
        return;
    }
    let n = Node::of(root);
    for i in 0..n.entries() {
        f(n.entry(i));
    }
    for i in 0..n.children() {
        for_each(n.child(i), f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(k: u64) -> u64 {
        // few distinct hashes: exercises splits down to collision buckets
        if k.is_multiple_of(7) { 42 } else { k.wrapping_mul(0x9E37_79B9_7F4A_7C15) }
    }

    #[test]
    fn matches_a_model_across_versions() {
        use std::collections::HashMap;
        let eq = |a: u64, b: u64| a == b;
        let _off = gc::NoGc::new();
        let mut t = 0u64;
        let mut len = 0usize;
        let mut model = HashMap::new();
        let mut versions = vec![];
        let mut x: u64 = 1;
        for step in 0..20000u64 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let k = x % 3000;
            if x.is_multiple_of(3) {
                let r = remove(t, h(k), k, &eq);
                assert_eq!(r.is_some(), model.remove(&k).is_some());
                if let Some(r) = r {
                    t = r;
                    len -= 1;
                }
            } else {
                let (r, added) = insert(t, Entry { hash: h(k), key: k, val: step }, 0, &eq);
                assert_eq!(added, model.insert(k, step).is_none());
                t = r;
                len += added as usize;
            }
            if step.is_multiple_of(1000) {
                versions.push((t, len, model.clone()));
            }
        }
        versions.push((t, len, model));
        for (t, len, m) in &versions {
            assert_eq!(*len, m.len());
            let mut n = 0;
            for_each(*t, &mut |e| {
                n += 1;
                assert_eq!(m.get(&e.key), Some(&e.val));
            });
            assert_eq!(n, m.len());
            for k in 0..3000 {
                assert_eq!(get(*t, h(k), k, &eq), m.get(&k).copied());
            }
        }
    }
}
