//! Packed contiguous storage for affine buffers and immutable arrays.
use crate::desc::{Kind, Repr, TypeDesc};
#[cfg(feature = "gc-profile")]
use crate::gc::allocations::{BufferStorageEvent as Event, record_buffer_storage as record_storage};
use crate::gc::{self, Header, obj};
use std::alloc::{Layout, alloc, dealloc, handle_alloc_error, realloc};

pub(crate) const INLINE_BYTES: usize = 256;
fn inline_bytes() -> usize {
    #[cfg(feature = "gc-profile")]
    return gc::diagnostics::buffer_inline_bytes(INLINE_BYTES);
    #[cfg(not(feature = "gc-profile"))]
    INLINE_BYTES
}

/// Stable layout read by generated array accesses. Element words are stored
/// at their actual width; pointer elements retain their full traced word.
#[repr(C)]
pub struct Items {
    ptr: *mut u8,
    len: usize,
    cap: usize,
    repr: Repr,
    // Fits the existing padding; generated pointer/length offsets stay valid.
    inline: bool,
}

fn bytes(cap: usize, repr: Repr) -> usize {
    cap.checked_mul(repr.size() as usize)
        .filter(|n| *n <= isize::MAX as usize)
        .unwrap_or_else(|| crate::task::panic_current("buffer capacity overflow".into(), String::new()))
}

impl Items {
    pub fn new(repr: Repr, cap: usize) -> Items {
        let mut items = Items { ptr: 8usize as *mut u8, len: 0, cap: 0, repr, inline: false };
        items.reserve(cap);
        items
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn capacity(&self) -> usize {
        self.cap
    }
    pub fn capacity_bytes(&self) -> usize {
        bytes(self.cap, self.repr)
    }
    fn external_bytes(&self) -> usize {
        if self.inline { 0 } else { self.capacity_bytes() }
    }
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr
    }
    pub fn get(&self, i: usize) -> Option<u64> {
        (i < self.len).then(|| unsafe { self.repr.load(self.ptr.add(i * self.repr.size() as usize)) })
    }
    pub fn set(&mut self, i: usize, w: u64) -> bool {
        if i >= self.len {
            return false;
        }
        unsafe { self.repr.store(self.ptr.add(i * self.repr.size() as usize), w) };
        true
    }
    fn reserve(&mut self, cap: usize) {
        if cap <= self.cap {
            return;
        }
        let (old, new) = (self.external_bytes(), bytes(cap, self.repr));
        let was_inline = self.inline;
        let layout = Layout::from_size_align(new, 8).unwrap();
        let ptr = unsafe {
            if old == 0 {
                alloc(layout)
            } else {
                realloc(self.ptr, Layout::from_size_align(old, 8).unwrap(), new)
            }
        };
        if ptr.is_null() {
            handle_alloc_error(layout);
        }
        if was_inline {
            // Inline storage belongs to the managed cell: never realloc/free
            // it, and copy only the initialized prefix before publishing.
            unsafe { std::ptr::copy_nonoverlapping(self.ptr, ptr, bytes(self.len, self.repr)) };
        }
        self.ptr = ptr;
        self.cap = cap;
        self.inline = false;
        gc::external_alloc(new - old);
        #[cfg(feature = "gc-profile")]
        {
            record_storage(if old == 0 { Event::NativeAlloc } else { Event::NativeRealloc }, new - old);
            if was_inline {
                record_storage(Event::InlineGrow, 0);
            }
        }
    }
    pub fn push(&mut self, w: u64) {
        if self.len == self.cap {
            self.reserve(self.cap.saturating_mul(2).max(4));
        }
        self.push_reserved(w);
    }
    /// The caller has reserved room; publish only the initialized prefix.
    #[inline]
    fn push_reserved(&mut self, w: u64) {
        debug_assert!(self.len < self.cap);
        unsafe { self.repr.store(self.ptr.add(self.len * self.repr.size() as usize), w) };
        self.len += 1;
    }
    pub fn pop(&mut self) -> Option<u64> {
        let w = self.get(self.len.checked_sub(1)?)?;
        self.len -= 1;
        Some(w)
    }
    pub fn extend(&mut self, values: impl IntoIterator<Item = u64>) {
        for w in values {
            self.push(w);
        }
    }
    pub fn resize(&mut self, len: usize, value: u64) {
        self.reserve(len);
        while self.len < len {
            self.push(value);
        }
        self.len = len;
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = u64> + '_ {
        (0..self.len).map(|i| self.get(i).unwrap())
    }
    /// Raw element storage, for bulk copies that write packed words directly.
    pub fn as_mut_ptr(&self) -> *mut u8 {
        self.ptr
    }
    /// The stored length after a bulk copy has filled the reserved slots.
    pub fn set_len(&mut self, n: usize) {
        self.len = n;
    }
    fn from_words(repr: Repr, words: Vec<u64>) -> Items {
        if repr.size() == 8 {
            let mut words = std::mem::ManuallyDrop::new(words);
            let out = Items {
                ptr: words.as_mut_ptr() as *mut u8,
                len: words.len(),
                cap: words.capacity(),
                repr,
                inline: false,
            };
            gc::external_alloc(out.capacity_bytes());
            #[cfg(feature = "gc-profile")]
            if out.capacity_bytes() > 0 {
                record_storage(Event::NativeAdopt, out.capacity_bytes());
            }
            out
        } else {
            let mut out = Items::new(repr, words.len());
            out.extend(words);
            out
        }
    }
}
impl Drop for Items {
    fn drop(&mut self) {
        let bytes = self.external_bytes();
        if bytes > 0 {
            unsafe { dealloc(self.ptr, Layout::from_size_align(bytes, 8).unwrap()) };
            gc::external_free(bytes);
            #[cfg(feature = "gc-profile")]
            record_storage(Event::NativeFree, bytes);
        }
    }
}

#[repr(C)]
pub struct BufData {
    pub elem: &'static TypeDesc,
    pub items: Items,
    pub frozen: bool,
}

// Actual offsets in BufData (unlike MIR Load offsets, which add HEADER).
pub const ITEMS_PTR: i32 = (std::mem::offset_of!(BufData, items) + std::mem::offset_of!(Items, ptr)) as i32;
pub const ITEMS_LEN: i32 = (std::mem::offset_of!(BufData, items) + std::mem::offset_of!(Items, len)) as i32;

pub fn desc() -> &'static TypeDesc {
    static D: std::sync::OnceLock<&'static TypeDesc> = std::sync::OnceLock::new();
    D.get_or_init(|| {
        TypeDesc::new("Buffer", Kind::Opaque { trace: Some(trace), finalize: Some(finalize), args: vec![] })
            .leak()
    })
}
fn frozen_inline_desc() -> &'static TypeDesc {
    static D: std::sync::OnceLock<&'static TypeDesc> = std::sync::OnceLock::new();
    D.get_or_init(|| {
        // Elements still trace normally. The object owns no native resource,
        // so sweep can reclaim its entire cell without a finalizer callback.
        TypeDesc::new("Buffer", Kind::Opaque { trace: Some(trace), finalize: None, args: vec![] }).leak()
    })
}
fn trace(o: *mut Header, f: &mut dyn FnMut(u64)) {
    let d = unsafe { &*((*o).payload() as *const BufData) };
    if d.elem.is_ptr() {
        d.items.iter().for_each(f);
    }
}
fn finalize(o: *mut Header) {
    unsafe {
        // Sweep finalizes each object once, before freeing or quarantining its
        // managed cell. Dropping in place releases the native element storage.
        std::ptr::drop_in_place((*o).payload() as *mut BufData);
    }
}
pub fn is_buffer(w: u64) -> bool {
    let actual = unsafe { obj(w).desc() };
    std::ptr::eq(actual, desc()) || std::ptr::eq(actual, frozen_inline_desc())
}
pub fn new(elem: &'static TypeDesc, cap: usize) -> u64 {
    let storage = bytes(cap, elem.repr());
    let inline = storage > 0 && storage <= inline_bytes();
    let metadata = std::mem::size_of::<BufData>();
    gc::budget_check(gc::HEADER + metadata + storage);
    let w = gc::alloc(desc(), metadata + if inline { storage } else { 0 });
    let items = if inline {
        #[cfg(feature = "gc-profile")]
        record_storage(Event::Inline, storage);
        Items { ptr: unsafe { obj(w).payload().add(metadata) }, len: 0, cap, repr: elem.repr(), inline: true }
    } else {
        Items::new(elem.repr(), cap)
    };
    let d = BufData { elem, items, frozen: false };
    unsafe {
        (obj(w).payload() as *mut BufData).write(d);
    }
    w
}
pub fn data<'a>(w: u64) -> &'a mut BufData {
    unsafe { &mut *(obj(w).payload() as *mut BufData) }
}
pub fn push(w: u64, value: u64) {
    let d = data(w);
    if d.frozen {
        frozen_push();
    }
    // Items' representation is fixed at construction, including Vec adoption
    // and inline/native growth. Avoid re-decoding the descriptor per element.
    let repr = d.items.repr;
    if d.items.len == d.items.cap {
        grow_for_push(w, value);
    }
    // Growth can collect, so reborrow metadata only after its roots/charge work.
    data(w).items.push_reserved(value);
    if repr == Repr::Ptr {
        gc::barrier(w);
    }
}

#[cold]
#[inline(never)]
fn frozen_push() -> ! {
    crate::task::panic_current("append to a frozen buffer".into(), crate::task::caller_location())
}

#[cold]
#[inline(never)]
fn grow_for_push(w: u64, value: u64) {
    let r = gc::Roots::new();
    r.add(w);
    let repr = data(w).items.repr;
    if repr == Repr::Ptr {
        r.add(value);
    }
    let next = data(w).items.cap.saturating_mul(2).max(4);
    // Inline bytes stay in the managed cell after growing; the whole new
    // native capacity is additional pressure, not just its delta.
    gc::budget_check(bytes(next, repr) - data(w).items.external_bytes());
    data(w).items.reserve(next);
}
pub fn array_elem(d: &TypeDesc) -> Option<&'static TypeDesc> {
    match &d.kind {
        Kind::Opaque { args, .. } if d.name == "Array" && args.len() == 1 => Some(args[0]),
        _ => None,
    }
}
pub fn items<'a>(w: u64) -> &'a Items {
    unsafe { &(*(obj(w).payload() as *const BufData)).items }
}
pub fn frozen(elem: &'static TypeDesc, words: Vec<u64>) -> u64 {
    gc::budget_check(gc::HEADER + std::mem::size_of::<BufData>() + bytes(words.capacity(), elem.repr()));
    let w = new(elem, 0);
    let d = data(w);
    d.items = Items::from_words(elem.repr(), words);
    d.frozen = true;
    w
}

/// A frozen array of `n` elements copied verbatim from `src`, which holds
/// them contiguously at this element's representation width (list pages and
/// other packed storage do). Bulk element copies use this instead of a
/// per-element `push`, which costs a bounds check and store per word.
pub fn frozen_from_raw(elem: &'static TypeDesc, src: *const u8, n: usize) -> u64 {
    let r = gc::Roots::new();
    let w = r.add(new(elem, n));
    {
        let d = data(w);
        if n > 0 {
            unsafe {
                std::ptr::copy_nonoverlapping(src, d.items.as_mut_ptr(), n * elem.repr().size() as usize)
            };
        }
        d.items.set_len(n);
    }
    freeze(w)
}
pub fn freeze(w: u64) -> u64 {
    let d = data(w);
    d.frozen = true;
    if d.items.inline {
        // This nonallocating intrinsic runs in compiled execution (or under
        // the runtime lock), so stop-the-world excludes concurrent GC reads.
        // Freezing is irreversible: native growth/mutation is no longer legal.
        unsafe { (*(w as *mut Header)).desc = frozen_inline_desc() };
    }
    w
}
pub fn elem_of(d: &'static TypeDesc) -> &'static TypeDesc {
    match &d.kind {
        Kind::Opaque { args, .. } if !args.is_empty() => args[0],
        Kind::List { elem } => elem,
        _ => crate::desc::builtin::unit(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn inline_elements_trace_freeze_and_grow_without_native_leaks() {
        use super::*;
        if std::env::var_os("BADGER_INLINE_TEST_CHILD").is_none() {
            let limits: &[usize] =
                if cfg!(feature = "gc-profile") { &[0, 128, 256] } else { &[INLINE_BYTES] };
            let pools: &[usize] = if cfg!(feature = "gc-profile") { &[256, 512] } else { &[512] };
            let settings = limits.iter().flat_map(|limit| {
                pools.iter().flat_map(move |pool| {
                    ["0", "1"].into_iter().flat_map(move |generation| {
                        ["0", "1"].map(move |verify| (limit, pool, generation, verify))
                    })
                })
            });
            for (limit, pool, generational, verify) in settings {
                let mut command = std::process::Command::new(std::env::current_exe().unwrap());
                for (key, _) in std::env::vars_os() {
                    if key.to_string_lossy().starts_with("BADGER_") {
                        command.env_remove(key);
                    }
                }
                let output = command
                    .args([
                        "--exact",
                        "buffer::tests::inline_elements_trace_freeze_and_grow_without_native_leaks",
                    ])
                    .env("BADGER_INLINE_TEST_CHILD", "1")
                    .env("BADGER_GC_VERIFY", verify)
                    .env("BADGER_GC_GENERATIONAL", generational)
                    .env("BADGER_GC_POOL_MAX", pool.to_string())
                    .env("BADGER_BUFFER_INLINE_BYTES", limit.to_string())
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "inline{limit} pool{pool} gen{generational} verify{verify}: {} {}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            return;
        }
        let before = gc::memory_bytes().1;
        {
            let roots = gc::Roots::new();
            let mut arrays = vec![];
            for cap in [0, 1, 15, 16, 127, 128, 129, 255, 256, 257, 512] {
                let old_external = gc::memory_bytes().1;
                let w = roots.add(new(crate::desc::builtin::u8_(), cap));
                let inline = cap > 0 && cap <= inline_bytes();
                assert_eq!(data(w).items.inline, inline);
                assert_eq!(gc::memory_bytes().1 - old_external, if inline { 0 } else { cap });
                assert_eq!(
                    unsafe { obj(w).payload_size() },
                    std::mem::size_of::<BufData>() + if inline { cap } else { 0 }
                );
                for i in 0..cap {
                    push(w, (i % 251) as u64);
                }
                freeze(w);
                assert!(is_buffer(w));
                let Kind::Opaque { finalize, .. } = &unsafe { obj(w) }.desc().kind else { unreachable!() };
                assert_eq!(finalize.is_none(), inline);
                arrays.push((w, cap));
            }
            let mut packed = vec![];
            for (name, kind) in [
                ("Unit", Kind::Unit),
                ("Bool", Kind::Bool),
                ("I8", Kind::Int { bits: 8, signed: true }),
                ("I16", Kind::Int { bits: 16, signed: true }),
                ("I32", Kind::Int { bits: 32, signed: true }),
                ("U32", Kind::Int { bits: 32, signed: false }),
                ("I64", Kind::Int { bits: 64, signed: true }),
                ("F32", Kind::Float { bits: 32 }),
                ("F64", Kind::Float { bits: 64 }),
            ] {
                let elem = TypeDesc::new(name, kind).leak();
                let cap = (inline_bytes() / elem.repr().size() as usize).max(4);
                let w = roots.add(new(elem, cap));
                let mask = u64::MAX >> (64 - elem.repr().size() * 8);
                // Cross capacity in every representation, including packed
                // float bits, so the cold growth path rejoins the same store.
                let expected: Vec<_> =
                    (0..cap + 5).map(|i| [0, 1, u64::MAX, 0x7fc00001][i % 4] & mask).collect();
                for &word in &expected {
                    push(w, word);
                }
                freeze(w);
                packed.push((w, expected));
            }
            let growing = roots.add(new(crate::desc::builtin::string(), 32));
            let frozen_ptrs = roots.add(new(crate::desc::builtin::string(), 32));
            for i in 0..32 {
                let value = crate::string::new_string(&format!("value {i}"));
                push(growing, value);
                push(frozen_ptrs, value);
            }
            freeze(frozen_ptrs);
            for _ in 0..9 {
                gc::collect();
            }
            let original_payload = unsafe { obj(growing).payload_size() };
            let original_storage = items(growing).as_ptr();
            let was_inline = items(growing).inline;
            for i in 32..128 {
                push(growing, crate::string::new_string(&format!("value {i}")));
            }
            assert!(!items(growing).inline);
            if was_inline {
                assert_ne!(items(growing).as_ptr(), original_storage);
            }
            assert_eq!(unsafe { obj(growing).payload_size() }, original_payload);
            freeze(growing);
            let Kind::Opaque { finalize, .. } = &unsafe { obj(growing) }.desc().kind else { unreachable!() };
            assert!(finalize.is_some(), "native growth still needs finalization");
            gc::collect();
            for (w, cap) in arrays {
                assert_eq!(items(w).len(), cap);
                for i in 0..cap {
                    assert_eq!(items(w).get(i), Some((i % 251) as u64));
                }
            }
            for (w, expected) in packed {
                assert_eq!(items(w).iter().collect::<Vec<_>>(), expected);
            }
            for w in [growing, frozen_ptrs] {
                for (i, word) in items(w).iter().enumerate() {
                    assert_eq!(crate::string::str_of(word), format!("value {i}"));
                }
            }
        }
        for _ in 0..9 {
            gc::collect();
        }
        assert_eq!(gc::memory_bytes().1, before);
    }

    #[test]
    fn inline_metadata_traces_grows_and_reclaims_native_storage() {
        use super::*;
        if std::env::var_os("BADGER_BUFFER_TEST_CHILD").is_none() {
            for generational in ["0", "1"] {
                let mut command = std::process::Command::new(std::env::current_exe().unwrap());
                for (key, _) in std::env::vars_os() {
                    if key.to_string_lossy().starts_with("BADGER_") {
                        command.env_remove(key);
                    }
                }
                let output = command
                    .args([
                        "--exact",
                        "buffer::tests::inline_metadata_traces_grows_and_reclaims_native_storage",
                    ])
                    .env("BADGER_BUFFER_TEST_CHILD", "1")
                    .env("BADGER_GC_GENERATIONAL", generational)
                    .env("BADGER_GC_VERIFY", "1")
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "generational{generational}: {} {}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            return;
        }
        let external_before = gc::memory_bytes().1;
        {
            let roots = gc::Roots::new();
            let empty = roots.add(new(crate::desc::builtin::int(), 0));
            assert_eq!(gc::memory_bytes().1, external_before);
            let scalars = roots.add(frozen(crate::desc::builtin::int(), vec![10, 20, 30]));
            let pointers = roots.add(new(crate::desc::builtin::string(), 0));
            for i in 0..64 {
                let value = crate::string::new_string(&format!("before {i}"));
                push(pointers, value);
            }
            for _ in 0..9 {
                gc::collect();
            }
            // Mutate a promoted buffer with young strings; growth must retain
            // its metadata address, and the barrier must keep the children.
            let address = data(pointers) as *mut BufData;
            for i in 64..512 {
                let value = crate::string::new_string(&format!("after {i}"));
                push(pointers, value);
            }
            gc::collect();
            assert_eq!(data(pointers) as *mut BufData, address);
            assert!(data(empty).items.is_empty());
            assert_eq!(items(scalars).iter().collect::<Vec<_>>(), vec![10, 20, 30]);
            for (i, value) in items(pointers).iter().enumerate() {
                let prefix = if i < 64 { "before" } else { "after" };
                assert_eq!(crate::string::str_of(value), format!("{prefix} {i}"));
            }
            // The exact native charge is element capacity alone, including
            // the Vec adoption path; there is no metadata Box charge.
            assert_eq!(
                gc::memory_bytes().1 - external_before,
                items(scalars).capacity_bytes() + items(pointers).capacity_bytes()
            );
            unsafe {
                // Exercise the same pointer and length offsets read by JIT.
                let d = obj(pointers).payload() as *const u8;
                assert_eq!(*(d.add(ITEMS_PTR as usize) as *const *const u8), items(pointers).as_ptr());
                assert_eq!(*(d.add(ITEMS_LEN as usize) as *const usize), 512);
                assert_eq!(d, address as *const u8);
                assert_eq!(obj(pointers).payload_size(), std::mem::size_of::<BufData>());
            }
        }
        // A full collection must drop element storage exactly once, even in
        // verification mode where the managed cells stay quarantined.
        for _ in 0..9 {
            gc::collect();
        }
        assert_eq!(gc::memory_bytes().1, external_before);
    }

    #[test]
    fn packed_words_roundtrip_bits_at_every_width() {
        use crate::desc::Repr;
        for repr in [Repr::I8, Repr::I16, Repr::I32, Repr::F32, Repr::I64, Repr::F64, Repr::Ptr] {
            let mut xs = super::Items::new(repr, 3);
            let mask = u64::MAX >> (64 - repr.size() * 8);
            for w in [0, 1, u64::MAX, 0x8000000000000000, 0x7fc00001] {
                xs.push(w);
            }
            assert_eq!(xs.capacity_bytes(), xs.capacity() * repr.size() as usize);
            assert_eq!(
                xs.iter().collect::<Vec<_>>(),
                [0, 1, u64::MAX, 0x8000000000000000, 0x7fc00001].map(|w| w & mask)
            );
            assert!(xs.set(1, u64::MAX));
            assert_eq!(xs.get(1), Some(mask));
            assert!(!xs.set(99, 0));
            assert_eq!(xs.pop(), Some(0x7fc00001 & mask));
            xs.resize(1, 0);
            assert_eq!(xs.get(1), None);
            xs.resize(10, 42);
            assert_eq!(xs.get(9), Some(42));
        }
    }
}
