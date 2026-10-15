//! Contiguous storage: `Buffer<T>` (affine, mutable, grows by relocation)
//! and `Array<T>` (immutable, fixed length; a frozen buffer reuses the
//! allocation).

use crate::desc::{Kind, TypeDesc};
use crate::gc::{self, Header, obj};

pub struct BufData {
    pub elem: &'static TypeDesc,
    pub items: Vec<u64>,
    pub frozen: bool,
}

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
        d.items.iter().for_each(|w| f(*w));
    }
}

fn finalize(o: *mut Header) {
    unsafe {
        let p = (*o).payload() as *mut *mut BufData;
        if !(*p).is_null() {
            drop(Box::from_raw(*p));
            *p = std::ptr::null_mut();
        }
    }
}

pub fn is_buffer(w: u64) -> bool {
    std::ptr::eq(unsafe { obj(w).desc() }, desc())
}

pub fn new(elem: &'static TypeDesc, cap: usize) -> u64 {
    let w = gc::alloc(desc(), 8);
    let d = Box::new(BufData { elem, items: Vec::with_capacity(cap), frozen: false });
    unsafe { *(obj(w).payload() as *mut *mut BufData) = Box::into_raw(d) };
    w
}

pub fn data<'a>(w: u64) -> &'a mut BufData {
    unsafe { &mut **(obj(w).payload() as *mut *mut BufData) }
}

/// Element type of an `Array<T>` compiler descriptor: arrays are immutable
/// values compared, hashed, and encoded by their elements.
pub fn array_elem(d: &TypeDesc) -> Option<&'static TypeDesc> {
    match &d.kind {
        Kind::Opaque { args, .. } if d.name == "Array" && args.len() == 1 => Some(args[0]),
        _ => None,
    }
}

/// The elements of a buffer or array.
pub fn items<'a>(w: u64) -> &'a [u64] {
    &data(w).items
}

/// A new array holding `items`.
pub fn frozen(elem: &'static TypeDesc, items: Vec<u64>) -> u64 {
    let w = new(elem, 0);
    let d = data(w);
    d.items = items;
    d.frozen = true;
    w
}

/// Element type of a `Buffer<T>`/`Array<T>` compiler descriptor.
pub fn elem_of(d: &'static TypeDesc) -> &'static TypeDesc {
    match &d.kind {
        Kind::Opaque { args, .. } if !args.is_empty() => args[0],
        Kind::List { elem } => elem,
        _ => crate::desc::builtin::unit(),
    }
}
