//! Packed contiguous storage for affine buffers and immutable arrays.
use crate::desc::{Kind, Repr, TypeDesc};
use crate::gc::{self, Header, obj};
use std::alloc::{Layout, alloc, dealloc, handle_alloc_error, realloc};

/// Stable layout read by generated array accesses. Element words are stored
/// at their actual width; pointer elements retain their full traced word.
#[repr(C)]
pub struct Items {
    ptr: *mut u8,
    len: usize,
    cap: usize,
    repr: Repr,
}

fn bytes(cap: usize, repr: Repr) -> usize {
    cap.checked_mul(repr.size() as usize)
        .filter(|n| *n <= isize::MAX as usize)
        .unwrap_or_else(|| crate::task::panic_current("buffer capacity overflow".into(), String::new()))
}

impl Items {
    pub fn new(repr: Repr, cap: usize) -> Items {
        let mut items = Items { ptr: 8usize as *mut u8, len: 0, cap: 0, repr };
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
        let (old, new) = (self.capacity_bytes(), bytes(cap, self.repr));
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
        self.ptr = ptr;
        self.cap = cap;
        gc::external_alloc(new - old);
    }
    pub fn push(&mut self, w: u64) {
        if self.len == self.cap {
            self.reserve(self.cap.saturating_mul(2).max(4));
        }
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
    fn from_words(repr: Repr, words: Vec<u64>) -> Items {
        if repr.size() == 8 {
            let mut words = std::mem::ManuallyDrop::new(words);
            let out =
                Items { ptr: words.as_mut_ptr() as *mut u8, len: words.len(), cap: words.capacity(), repr };
            gc::external_alloc(out.capacity_bytes());
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
        let bytes = self.capacity_bytes();
        if bytes > 0 {
            unsafe { dealloc(self.ptr, Layout::from_size_align(bytes, 8).unwrap()) };
            gc::external_free(bytes);
        }
    }
}

#[repr(C)]
pub struct BufData {
    pub elem: &'static TypeDesc,
    pub items: Items,
    pub frozen: bool,
}

/// The first word preserves the generated array-access ABI. Metadata lives
/// beside it in the stable managed cell, so there is no separate native Box.
#[repr(C)]
struct BufferObject {
    data: *mut BufData,
    inline: BufData,
}

// Actual offsets in BufData (unlike MIR Load offsets, which add HEADER).
pub const DATA_OFFSET: i32 = 0;
pub const ITEMS_PTR: i32 = (std::mem::offset_of!(BufData, items) + std::mem::offset_of!(Items, ptr)) as i32;
pub const ITEMS_LEN: i32 = (std::mem::offset_of!(BufData, items) + std::mem::offset_of!(Items, len)) as i32;

pub fn desc() -> &'static TypeDesc {
    static D: std::sync::OnceLock<&'static TypeDesc> = std::sync::OnceLock::new();
    D.get_or_init(|| {
        TypeDesc::new("Buffer", Kind::Opaque { trace: Some(trace), finalize: Some(finalize), args: vec![] })
            .leak()
    })
}
fn trace(o: *mut Header, f: &mut dyn FnMut(u64)) {
    let d = unsafe { &*(*((*o).payload() as *const *mut BufData)) };
    if d.elem.is_ptr() {
        d.items.iter().for_each(f);
    }
}
fn finalize(o: *mut Header) {
    unsafe {
        let p = (*o).payload() as *mut *mut BufData;
        if !(*p).is_null() {
            std::ptr::drop_in_place(*p);
            *p = std::ptr::null_mut();
        }
    }
}
pub fn is_buffer(w: u64) -> bool {
    std::ptr::eq(unsafe { obj(w).desc() }, desc())
}
pub fn new(elem: &'static TypeDesc, cap: usize) -> u64 {
    gc::budget_check(gc::HEADER + std::mem::size_of::<BufferObject>() + bytes(cap, elem.repr()));
    let w = gc::alloc(desc(), std::mem::size_of::<BufferObject>());
    let d = BufData { elem, items: Items::new(elem.repr(), cap), frozen: false };
    unsafe {
        let p = obj(w).payload() as *mut BufferObject;
        let inline = std::ptr::addr_of_mut!((*p).inline);
        inline.write(d);
        std::ptr::addr_of_mut!((*p).data).write(inline);
    }
    w
}
pub fn data<'a>(w: u64) -> &'a mut BufData {
    unsafe { &mut **(obj(w).payload() as *mut *mut BufData) }
}
pub fn push(w: u64, value: u64) {
    let d = data(w);
    if d.frozen {
        crate::task::panic_current("append to a frozen buffer".into(), crate::task::caller_location());
    }
    let (cap, len, repr) = (d.items.capacity(), d.items.len(), d.elem.repr());
    if len == cap {
        let r = gc::Roots::new();
        r.add(w);
        if repr == Repr::Ptr {
            r.add(value);
        }
        let next = cap.saturating_mul(2).max(4);
        gc::budget_check(bytes(next, repr) - bytes(cap, repr));
        data(w).items.reserve(next);
    }
    data(w).items.push(value);
    if repr == Repr::Ptr {
        gc::barrier(w);
    }
}
pub fn array_elem(d: &TypeDesc) -> Option<&'static TypeDesc> {
    match &d.kind {
        Kind::Opaque { args, .. } if d.name == "Array" && args.len() == 1 => Some(args[0]),
        _ => None,
    }
}
pub fn items<'a>(w: u64) -> &'a Items {
    unsafe { &(**(obj(w).payload() as *const *const BufData)).items }
}
pub fn frozen(elem: &'static TypeDesc, words: Vec<u64>) -> u64 {
    gc::budget_check(gc::HEADER + std::mem::size_of::<BufferObject>() + bytes(words.capacity(), elem.repr()));
    let w = new(elem, 0);
    let d = data(w);
    d.items = Items::from_words(elem.repr(), words);
    d.frozen = true;
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
                let d = *(obj(pointers).payload().add(DATA_OFFSET as usize) as *const *const u8);
                assert_eq!(*(d.add(ITEMS_PTR as usize) as *const *const u8), items(pointers).as_ptr());
                assert_eq!(*(d.add(ITEMS_LEN as usize) as *const usize), 512);
                assert!((d as usize) >= obj(pointers).payload() as usize);
                assert!(
                    (d as usize) + std::mem::size_of::<BufData>()
                        <= obj(pointers).payload() as usize + std::mem::size_of::<BufferObject>()
                );
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
