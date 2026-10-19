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
//! A root holding at most `FLAT_MAX` (8) entries is *flat*: a node with both
//! bitmaps zero whose entries are scanned linearly (a collision bucket is
//! the only other node without bitmaps, and it is never a root). Its
//! entries are in the order a trie holding them would visit, so iteration
//! does not depend on the representation: inserting a ninth entry promotes
//! the root to a trie, and removing down to `FLAT_MAX` flattens it again.
//! The representation is thus a function of the length alone.
//!
//! Callers keep the trie being read or updated reachable (rooted), and root
//! keys and values passed in; allocation of new nodes may collect.
//! Fresh builders use a separate insertion path: all their nodes are private
//! until the builder is finished, so edits that fit an allocation reuse it.

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
        self.u32_at(8) & (KEY_PTR | VAL_PTR)
    }
    fn owner(self) -> u64 {
        let header = std::ptr::with_exposed_provenance::<Header>(self.0.addr() - gc::HEADER);
        let hi = unsafe { (*header).meta } & gc::CLIENT_META;
        ((hi >> 3) << 30) | (self.u32_at(8) as u64 >> 2)
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
    /// A root node in the flat representation (see the module docs).
    fn flat(self) -> bool {
        self.datamap() == 0 && self.nodemap() == 0
    }
}

/// Filler for entry buffers.
const BLANK: Entry = Entry { hash: 0, key: 0, val: 0 };

/// The most entries a flat root holds.
pub(crate) const FLAT_MAX: usize = 8;

fn chunk(hash: u64, shift: u32) -> u32 {
    ((hash >> shift) & 31) as u32
}

/// Put `es` (entries agreeing on their hashes below `shift`) in the order
/// a trie node holding them visits them: entries alone in their hash chunk
/// first, then the entries of each shared chunk, chunks ascending, each
/// group in its own child's order. Stable, so entries with equal full
/// hashes keep their collision-bucket order. At most [`FLAT_MAX`] + 1
/// entries.
fn trie_order(es: &mut [Entry], shift: u32) {
    if es.len() < 2 || collision(shift) {
        return;
    }
    for i in 1..es.len() {
        let mut j = i;
        while j > 0 && chunk(es[j - 1].hash, shift) > chunk(es[j].hash, shift) {
            es.swap(j - 1, j);
            j -= 1;
        }
    }
    let run = |es: &[Entry], i: usize| {
        let c = chunk(es[i].hash, shift);
        es[i..].iter().take_while(|e| chunk(e.hash, shift) == c).count()
    };
    let mut buf = [BLANK; FLAT_MAX + 1];
    let (mut k, mut groups) = (0, 0);
    for alone in [true, false] {
        groups = k;
        let mut i = 0;
        while i < es.len() {
            let r = run(es, i);
            if (r == 1) == alone {
                buf[k..k + r].copy_from_slice(&es[i..i + r]);
                k += r;
            }
            i += r;
        }
    }
    es.copy_from_slice(&buf[..es.len()]);
    let mut i = groups;
    while i < es.len() {
        let r = run(es, i);
        trie_order(&mut es[i..i + r], shift + BITS);
        i += r;
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

// Distinct monomorphizations keep owner words, token tests and metadata reads
// out of the existing persistent/private paths. Builder and Persistent are zero
// sized: only Epoch carries a runtime argument through the recursive insertion.
trait Mutation: Copy {
    fn token(self) -> u64;
    fn can_edit(self, node: Node) -> bool;
    /// A bulk builder owns every node outright; its nodes get spare
    /// capacity so a growing node is not recopied per insertion.
    fn builder(self) -> bool;
}

#[derive(Clone, Copy)]
struct Persistent;
impl Mutation for Persistent {
    #[inline(always)]
    fn token(self) -> u64 {
        0
    }
    #[inline(always)]
    fn can_edit(self, _node: Node) -> bool {
        false
    }
    #[inline(always)]
    fn builder(self) -> bool {
        false
    }
}

#[derive(Clone, Copy)]
struct Builder;
impl Mutation for Builder {
    #[inline(always)]
    fn token(self) -> u64 {
        0
    }
    #[inline(always)]
    fn can_edit(self, _node: Node) -> bool {
        true
    }
    #[inline(always)]
    fn builder(self) -> bool {
        true
    }
}

#[derive(Clone, Copy)]
struct Epoch(u64);
impl Mutation for Epoch {
    #[inline(always)]
    fn token(self) -> u64 {
        self.0
    }
    #[inline(always)]
    fn can_edit(self, node: Node) -> bool {
        node.owner() == self.0
    }
    #[inline(always)]
    fn builder(self) -> bool {
        false
    }
}

/// Stamp only a newly allocated node, never a shared one. Low owner bits live
/// above pointer flags; high bits use GC-reserved client metadata. This keeps
/// entry counts, offsets, and ordinary node size unchanged.
fn stamp_owner(w: u64, owner: u64) {
    debug_assert!(owner <= MAX_OWNER);
    unsafe {
        let h = w as *mut Header;
        let p = (*h).payload().add(8) as *mut u32;
        *p = (*p & (KEY_PTR | VAL_PTR)) | ((owner as u32) << 2);
        (*h).meta = ((*h).meta & !gc::CLIENT_META) | ((owner >> 30) << 3);
    }
}

pub(crate) const MAX_OWNER: u64 = (1 << 59) - 1;

/// Allocate a node and fill it. Child pointers must be reachable or rooted.
fn alloc_node(datamap: u32, nodemap: u32, flags: u32, entries: &[Entry], children: &[u64]) -> u64 {
    let size = HDR + entries.len() * 24 + children.len() * 8;
    alloc_node_sized(datamap, nodemap, flags, entries, children, size)
}

fn alloc_node_sized(
    datamap: u32,
    nodemap: u32,
    flags: u32,
    entries: &[Entry],
    children: &[u64],
    size: usize,
) -> u64 {
    let w = gc::alloc(node_desc(), size);
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

fn alloc_owned<M: Mutation>(
    datamap: u32,
    nodemap: u32,
    flags: u32,
    entries: &[Entry],
    children: &[u64],
    mode: M,
) -> u64 {
    let needed = HDR + entries.len() * 24 + children.len() * 8;
    let slack = {
        static SLACK: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *SLACK.get_or_init(|| std::env::var("BADGER_HAMT_SLACK").map(|v| v != "0").unwrap_or(true))
    };
    let size = if mode.builder() && slack { (needed + 3 * 24 + 3 * 8).div_ceil(16) * 16 } else { needed };
    let w = alloc_node_sized(datamap, nodemap, flags, entries, children, size);
    if mode.token() != 0 {
        stamp_owner(w, mode.token());
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
    splice_owned(w, datamap, nodemap, e, c, Persistent)
}

fn splice_owned<M: Mutation>(
    w: u64,
    datamap: u32,
    nodemap: u32,
    e: Edit<Entry>,
    c: Edit<u64>,
    mode: M,
) -> u64 {
    let n = Node::of(w);
    let (ne, nc) = (n.entries(), n.children());
    let new_ne = ne - e.del + e.ins.is_some() as usize;
    let new_nc = nc - c.del + c.ins.is_some() as usize;
    // An editable copy can be inserted into again. Reserve a little room so
    // a growing node is not reallocated and recopied for every entry.
    let needed = HDR + new_ne * 24 + new_nc * 8;
    let slack = {
        static SLACK: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *SLACK.get_or_init(|| std::env::var("BADGER_HAMT_SLACK").map(|v| v != "0").unwrap_or(true))
    };
    let size = if mode.builder() && slack { (needed + 3 * 24 + 3 * 8).div_ceil(16) * 16 } else { needed };
    let out = gc::alloc(node_desc(), size);
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
    if mode.token() != 0 {
        stamp_owner(out, mode.token());
    }
    out
}

/// Builder or epoch-owned edit. Every node reachable from `w` must be exclusively owned
/// by the unfinished builder for Builder. Epoch only edits nodes bearing its
/// token. Persistent always copies.
/// Keep the allocation's original payload size: shrinking the logical node can
/// leave spare capacity, but must not change how the collector frees its cell.
fn edit<M: Mutation>(w: u64, datamap: u32, nodemap: u32, e: Edit<Entry>, c: Edit<u64>, mode: M) -> u64 {
    let n = Node::of(w);
    if !mode.can_edit(n) {
        return splice_owned(w, datamap, nodemap, e, c, mode);
    }
    let (ne, nc) = (n.entries(), n.children());
    let new_ne = ne - e.del + e.ins.is_some() as usize;
    let new_nc = nc - c.del + c.ins.is_some() as usize;
    if HDR + new_ne * 24 + new_nc * 8 > unsafe { obj(w).payload_size() } {
        return splice_owned(w, datamap, nodemap, e, c, mode);
    }
    // Insertion never deletes a child without replacing it. Consequently
    // relocating the old children fits whenever the final layout fits.
    debug_assert!(c.del <= c.ins.is_some() as usize);
    unsafe {
        let entries = n.0.add(HDR) as *mut u64;
        let children = entries.add(new_ne * 3);
        // Growing entries would overwrite the old child array, so move it
        // first. Shrinking entries must finish before moving children left.
        if new_ne > ne {
            std::ptr::copy(entries.add(ne * 3), children, nc);
        }
        if e.del != e.ins.is_some() as usize {
            std::ptr::copy(
                entries.add((e.at + e.del) * 3),
                entries.add((e.at + e.ins.is_some() as usize) * 3),
                (ne - e.at - e.del) * 3,
            );
        }
        if let Some(x) = e.ins {
            let p = entries.add(e.at * 3);
            *p = x.hash;
            *p.add(1) = x.key;
            *p.add(2) = x.val;
        }
        if new_ne < ne {
            std::ptr::copy(entries.add(ne * 3), children, nc);
        }
        if c.del != c.ins.is_some() as usize {
            std::ptr::copy(
                children.add(c.at + c.del),
                children.add(c.at + c.ins.is_some() as usize),
                nc - c.at - c.del,
            );
        }
        if let Some(child) = c.ins {
            *children.add(c.at) = child;
        }
        *(n.0 as *mut u32) = datamap;
        *(n.0.add(4) as *mut u32) = nodemap;
        *(n.0.add(12) as *mut u32) = new_ne as u32;
    }
    gc::barrier(w);
    w
}

/// A node holding two entries whose hashes agree below `shift`.
fn pair<M: Mutation>(a: Entry, b: Entry, shift: u32, flags: u32, mode: M) -> u64 {
    if collision(shift) {
        return alloc_owned(0, 0, flags, &[a, b], &[], mode);
    }
    let (ba, bb) = (bit(a.hash, shift), bit(b.hash, shift));
    if ba == bb {
        let r = gc::Roots::new();
        let c = r.add(pair(a, b, shift + BITS, flags, mode));
        alloc_owned(0, ba, flags, &[], &[c], mode)
    } else if ba < bb {
        alloc_owned(ba | bb, 0, flags, &[a, b], &[], mode)
    } else {
        alloc_owned(ba | bb, 0, flags, &[b, a], &[], mode)
    }
}

pub fn get(root: u64, hash: u64, key: u64, eq: &dyn Fn(u64, u64) -> bool) -> Option<u64> {
    if root == 0 {
        return None;
    }
    let mut n = Node::of(root);
    if n.flat() {
        return (0..n.entries())
            .map(|i| n.entry(i))
            .find(|e| e.hash == hash && eq(e.key, key))
            .map(|e| e.val);
    }
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
    insert_with(root, e, flags, eq, Persistent)
}

/// Insert into a fresh builder. The root and every descendant must be private:
/// never call this on a finished map or on a trie sharing persistent nodes.
pub(crate) fn insert_mut(root: u64, e: Entry, flags: u32, eq: &dyn Fn(u64, u64) -> bool) -> (u64, bool) {
    insert_with(root, e, flags, eq, Builder)
}

/// Reuse only nodes owned by this active, globally unique nonzero epoch.
/// A caller must freeze its old wrapper before sharing any of these nodes.
pub(crate) fn insert_owned(
    root: u64,
    e: Entry,
    flags: u32,
    eq: &dyn Fn(u64, u64) -> bool,
    owner: u64,
) -> (u64, bool) {
    assert!(owner != 0 && owner <= MAX_OWNER);
    insert_with(root, e, flags, eq, Epoch(owner))
}

fn insert_with<M: Mutation>(
    root: u64,
    e: Entry,
    flags: u32,
    eq: &dyn Fn(u64, u64) -> bool,
    mode: M,
) -> (u64, bool) {
    if root == 0 {
        return (alloc_owned(0, 0, flags, &[e], &[], mode), true);
    }
    if Node::of(root).flat() {
        return insert_flat(root, e, flags, eq, mode);
    }
    insert_at(root, e, 0, flags, eq, mode)
}

/// Insert into flat root `w`, promoting it to a trie at [`FLAT_MAX`] + 1
/// entries.
fn insert_flat<M: Mutation>(
    w: u64,
    e: Entry,
    flags: u32,
    eq: &dyn Fn(u64, u64) -> bool,
    mode: M,
) -> (u64, bool) {
    let n = Node::of(w);
    let ne = n.entries();
    if let Some(i) = (0..ne).find(|&i| n.entry(i).hash == e.hash && eq(n.entry(i).key, e.key)) {
        return (edit(w, 0, 0, Edit { at: i, del: 1, ins: Some(e) }, Edit::none(), mode), false);
    }
    let mut buf = [BLANK; FLAT_MAX + 1];
    for (i, x) in buf.iter_mut().enumerate().take(ne) {
        *x = n.entry(i);
    }
    // Appended last, so `e` follows any entries sharing its full hash, as
    // it would in a collision bucket.
    buf[ne] = e;
    let es = &mut buf[..ne + 1];
    trie_order(es, 0);
    if ne == FLAT_MAX {
        return (build(es, 0, flags, mode), true);
    }
    if mode.can_edit(n) && HDR + es.len() * 24 <= unsafe { obj(w).payload_size() } {
        unsafe {
            let p = n.0.add(HDR) as *mut u64;
            for (i, x) in es.iter().enumerate() {
                *p.add(i * 3) = x.hash;
                *p.add(i * 3 + 1) = x.key;
                *p.add(i * 3 + 2) = x.val;
            }
            *(n.0.add(12) as *mut u32) = es.len() as u32;
        }
        gc::barrier(w);
        return (w, true);
    }
    (alloc_owned(0, 0, flags, es, &[], mode), true)
}

/// The trie node holding `es` (distinct keys, at least two, at most
/// [`FLAT_MAX`] + 1, agreeing on their hashes below `shift`, in
/// [`trie_order`]): the shape insertions one by one would give.
fn build<M: Mutation>(es: &[Entry], shift: u32, flags: u32, mode: M) -> u64 {
    if collision(shift) {
        return alloc_owned(0, 0, flags, es, &[], mode);
    }
    let r = gc::Roots::new();
    let (mut dm, mut nm, mut alone) = (0, 0, 0);
    let mut children = [0; FLAT_MAX / 2];
    let mut nc = 0;
    let mut i = 0;
    while i < es.len() {
        let b = bit(es[i].hash, shift);
        let n = es[i..].iter().take_while(|e| bit(e.hash, shift) == b).count();
        if n == 1 {
            dm |= b;
            alone += 1;
        } else {
            nm |= b;
            children[nc] = r.add(build(&es[i..i + n], shift + BITS, flags, mode));
            nc += 1;
        }
        i += n;
    }
    alloc_owned(dm, nm, flags, &es[..alone], &children[..nc], mode)
}

fn insert_at<M: Mutation>(
    w: u64,
    e: Entry,
    shift: u32,
    flags: u32,
    eq: &dyn Fn(u64, u64) -> bool,
    mode: M,
) -> (u64, bool) {
    let n = Node::of(w);
    if collision(shift) {
        for i in 0..n.entries() {
            let x = n.entry(i);
            if x.hash == e.hash && eq(x.key, e.key) {
                return (edit(w, 0, 0, Edit { at: i, del: 1, ins: Some(e) }, Edit::none(), mode), false);
            }
        }
        return (edit(w, 0, 0, Edit { at: n.entries(), del: 0, ins: Some(e) }, Edit::none(), mode), true);
    }
    let b = bit(e.hash, shift);
    let (dm, nm) = (n.datamap(), n.nodemap());
    if dm & b != 0 {
        let i = index(dm, b);
        let x = n.entry(i);
        if x.hash == e.hash && eq(x.key, e.key) {
            return (edit(w, dm, nm, Edit { at: i, del: 1, ins: Some(e) }, Edit::none(), mode), false);
        }
        // the resident entry and the new one move into a child
        let r = gc::Roots::new();
        let c = r.add(pair(x, e, shift + BITS, flags, mode));
        let j = index(nm, b);
        let out = edit(
            w,
            dm & !b,
            nm | b,
            Edit { at: i, del: 1, ins: None },
            Edit { at: j, del: 0, ins: Some(c) },
            mode,
        );
        return (out, true);
    }
    if nm & b != 0 {
        let j = index(nm, b);
        let old_child = n.child(j);
        let (c, added) = insert_at(old_child, e, shift + BITS, flags, eq, mode);
        if c == old_child && mode.can_edit(n) {
            return (w, added);
        }
        let r = gc::Roots::new();
        r.add(c);
        return (edit(w, dm, nm, Edit::none(), Edit { at: j, del: 1, ins: Some(c) }, mode), added);
    }
    (edit(w, dm | b, nm, Edit { at: index(dm, b), del: 0, ins: Some(e) }, Edit::none(), mode), true)
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
    let n = Node::of(root);
    if n.flat() || count_upto(root, FLAT_MAX + 2) == FLAT_MAX + 1 {
        // the result is flat (a trie root holds more than FLAT_MAX entries)
        let mut buf = [BLANK; FLAT_MAX + 1];
        let mut k = 0;
        for_each(root, &mut |e| {
            if e.hash != hash || !eq(e.key, key) {
                buf[k] = e;
                k += 1;
            }
        });
        if k == 0 {
            return Some(0);
        }
        trie_order(&mut buf[..k], 0);
        return Some(alloc_node(0, 0, n.flags(), &buf[..k], &[]));
    }
    Some(match remove_at(root, hash, key, 0, eq) {
        Removed::Node(w) => w,
        Removed::Lone(e) => alloc_node(0, 0, n.flags(), &[e], &[]),
    })
}

/// The number of entries below `w`, or some number from `limit` up once
/// there are at least `limit`.
fn count_upto(w: u64, limit: usize) -> usize {
    let n = Node::of(w);
    let mut count = n.entries();
    for i in 0..n.children() {
        if count >= limit {
            break;
        }
        count += count_upto(n.child(i), limit - count);
    }
    count
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
/// current entry address sits at [`CURSOR_ENTRY`] for generated code to
/// load the key and value from after [`cursor_next`] (the trie nodes are
/// rooted by the cursor, so the interior address needs no root of its own).
pub const CURSOR_ENTRY: i32 = 0;
/// Kept for the cursor payload layout (unused by new code).
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
        // CURSOR_ENTRY is an interior pointer into a node rooted below;
        // tracing it as an object would be wrong. The node's own trace
        // covers the entry's key and value.
        let _ = w(CURSOR_ENTRY as usize);
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

/// Advance cursor `c` to the next entry: 1 when there is one (its node
/// entry address is then at [`CURSOR_ENTRY`]), 0 at the end.
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
                let i = *pos as usize;
                *pos += 1;
                // generated code adds the object header again when
                // loading, so key is at offset -8 from the header the load
                // adds and value at offset 0
                *(p as *mut u64) = n.0.add(HDR + i * 24) as u64;
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
        // fewer steps under Miri, which interprets every one
        let steps = if cfg!(miri) { 1500 } else { 20000u64 };
        for step in 0..steps {
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
        // Keep all managed-heap model checks in the same test: this heap's
        // unit-test mutator is single-threaded, rather than a task runtime.
        transient_matches_a_model_and_stops_allocating_for_replacements();
        owned_collision_buckets_preserve_all_epochs();
        flat_roots_iterate_like_tries();
    }

    /// Check the representation and the CHAMP invariants of a root
    /// holding `len` distinct keys.
    fn check(root: u64, len: usize) {
        fn node(w: u64, shift: u32, prefix: u64, seen: &mut Vec<u64>) {
            let n = Node::of(w);
            let (dm, nm) = (n.datamap(), n.nodemap());
            let low = |h: u64| if collision(shift) { h } else { h & ((1u64 << shift) - 1) };
            if collision(shift) {
                assert!(n.entries() >= 2 && n.children() == 0);
            }
            let mut bits = (0..32).filter(|i| dm & (1 << i) != 0);
            for i in 0..n.entries() {
                let e = n.entry(i);
                assert_eq!(low(e.hash), prefix);
                if !collision(shift) {
                    assert_eq!(chunk(e.hash, shift), bits.next().unwrap());
                }
                seen.push(e.key);
            }
            let mut bits = (0..32u32).filter(|i| nm & (1 << i) != 0);
            for i in 0..n.children() {
                let c = bits.next().unwrap();
                assert!(count_upto(n.child(i), 2) >= 2, "a child holds at least two entries");
                node(n.child(i), shift + BITS, prefix | ((c as u64) << shift), seen);
            }
        }
        if len == 0 {
            assert_eq!(root, 0);
            return;
        }
        let n = Node::of(root);
        assert_eq!(n.flat(), len <= FLAT_MAX);
        let mut seen = vec![];
        if n.flat() {
            assert_eq!(n.entries(), len);
            for_each(root, &mut |e| seen.push(e.key));
        } else {
            node(root, 0, 0, &mut seen);
        }
        assert_eq!(seen.len(), len);
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), len, "no duplicate keys");
    }

    /// Insert `keys` into a trie, never a flat root.
    fn trie_only(keys: &[u64], hash: fn(u64) -> u64) -> u64 {
        let eq = |a: u64, b: u64| a == b;
        let e = |k| Entry { hash: hash(k), key: k, val: k };
        let mut t = alloc_node(bit(hash(keys[0]), 0), 0, 0, &[e(keys[0])], &[]);
        for &k in &keys[1..] {
            t = insert_at(t, e(k), 0, 0, &eq, Persistent).0;
        }
        t
    }

    fn order(root: u64) -> Vec<u64> {
        let mut out = vec![];
        for_each(root, &mut |e| out.push(e.key));
        out
    }

    fn flat_roots_iterate_like_tries() {
        let eq = |a: u64, b: u64| a == b;
        // dense low chunks, one full collision class, and spread hashes
        let hashes: [fn(u64) -> u64; 3] = [|k| k * 33, |k| if k % 3 == 0 { 7 } else { k << 60 | k }, h];
        let mut x: u64 = 7;
        for hash in hashes {
            for _ in 0..if cfg!(miri) { 4 } else { 40 } {
                let mut keys: Vec<u64> = (0..14).collect();
                for i in (1..keys.len()).rev() {
                    x ^= x << 13;
                    x ^= x >> 7;
                    x ^= x << 17;
                    keys.swap(i, (x % (i as u64 + 1)) as usize);
                }
                let e = |k| Entry { hash: hash(k), key: k, val: k };
                let mut t = 0;
                let mut builder = 0;
                for n in 1..=keys.len() {
                    t = insert(t, e(keys[n - 1]), 0, &eq).0;
                    builder = insert_mut(builder, e(keys[n - 1]), 0, &eq).0;
                    check(t, n);
                    check(builder, n);
                    // A flat root visits its entries as the trie would.
                    assert_eq!(order(t), order(trie_only(&keys[..n], hash)));
                    assert_eq!(order(builder), order(t));
                }
                // Remove in another order, through the flat boundary.
                let mut left = keys.clone();
                for i in 0..keys.len() {
                    let k = keys[i * 5 % keys.len()];
                    left.retain(|&y| y != k);
                    t = remove(t, hash(k), k, &eq).unwrap();
                    assert!(remove(t, hash(k), k, &eq).is_none());
                    check(t, left.len());
                    if !left.is_empty() {
                        // replay the survivors in their original insertion order
                        let survivors: Vec<u64> = keys.iter().copied().filter(|y| left.contains(y)).collect();
                        assert_eq!(order(t), order(trie_only(&survivors, hash)));
                    }
                }
                assert_eq!(t, 0);
            }
        }
    }

    fn owned_collision_buckets_preserve_all_epochs() {
        // More than 64 equal full hashes must not alias entry count with tokens.
        // Full-bucket lookups make this quadratic. Keep 24 saved epochs and
        // >64 entries under Miri without interpreting millions of comparisons.
        let keys = if cfg!(miri) { 128 } else { 512 };
        let mut root = 0;
        let mut model = std::collections::HashMap::new();
        let mut versions = vec![];
        let mut token = MAX_OWNER;
        for step in 0..keys * 6 {
            let k = (step * 173) % keys;
            let (next, added) =
                insert_owned(root, Entry { hash: 42, key: k, val: step }, 0, &|a, b| a == b, token);
            assert_eq!(added, model.insert(k, step).is_none());
            root = next;
            assert_eq!(Node::of(root).owner(), token);
            if (step + 1) % (keys / 4) == 0 {
                versions.push((root, model.clone()));
                token -= 1;
            }
        }
        for (version, reference) in versions {
            let mut count = 0;
            for_each(version, &mut |e| {
                count += 1;
                assert_eq!(e.hash, 42);
                assert_eq!(reference.get(&e.key), Some(&e.val));
            });
            assert_eq!(count, reference.len());
            for k in 0..keys {
                assert_eq!(get(version, 42, k, &|a, b| a == b), reference.get(&k).copied());
            }
            // Legacy persistent cloning strips owner tags from new nodes.
            let (changed, _) = insert(version, Entry { hash: 42, key: 0, val: 99999 }, 0, &|a, b| a == b);
            assert_eq!(Node::of(changed).owner(), 0);
            assert_eq!(get(version, 42, 0, &|a, b| a == b), reference.get(&0).copied());
        }
    }

    fn transient_matches_a_model_and_stops_allocating_for_replacements() {
        use std::collections::HashMap;
        let eq = |a: u64, b: u64| a == b;
        let mut root = 0;
        let mut model = HashMap::new();
        // Ascending, descending and permuted insertion orders exercise data
        // compaction, child-array movement, and reused capacity after splits.
        for order in 0..3 {
            root = 0;
            model.clear();
            for i in 0..512 {
                let k = match order {
                    0 => i,
                    1 => 511 - i,
                    _ => (i * 173) % 512,
                };
                let (next, added) = insert_mut(root, Entry { hash: h(k), key: k, val: i }, 0, &eq);
                assert!(added);
                root = next;
                model.insert(k, i);
                if i % 31 == 0 {
                    for (&key, &value) in &model {
                        assert_eq!(get(root, h(key), key, &eq), Some(value));
                    }
                }
            }
            let before = gc::stats().0;
            for i in 0..4096 {
                let k = (i * 173) % 512;
                let (next, added) = insert_mut(root, Entry { hash: h(k), key: k, val: i }, 0, &eq);
                assert!(!added);
                assert_eq!(next, root);
                model.insert(k, i);
            }
            assert_eq!(gc::stats().0, before, "replacement-only builder updates allocated nodes");
            let mut visited = 0;
            for_each(root, &mut |entry| {
                visited += 1;
                assert_eq!(model.get(&entry.key), Some(&entry.val));
            });
            assert_eq!(visited, model.len());
        }
        // Once frozen, updates/removals must copy; a shared root or descendant
        // is never enough evidence to use the transient insertion path.
        let frozen = root;
        let (changed, _) = insert(frozen, Entry { hash: h(0), key: 0, val: 99999 }, 0, &eq);
        let removed = remove(changed, h(7), 7, &eq).unwrap();
        assert_eq!(get(changed, h(0), 0, &eq), Some(99999));
        assert_eq!(get(removed, h(7), 7, &eq), None);
        for (&key, &value) in &model {
            assert_eq!(get(frozen, h(key), key, &eq), Some(value));
        }
    }
}
