//! Runtime type descriptors (shape/schema descriptors).
//!
//! Every heap object's header points at its descriptor. Descriptors drive
//! precise tracing, structural equality/hashing/display, and schema-directed
//! encoding. The compiler creates one descriptor per concrete type and leaks
//! it (`&'static`); the runtime creates descriptors for its own objects.

use std::sync::atomic::{AtomicPtr, AtomicU64, AtomicUsize, Ordering};

use crate::gc::Header;

/// In-memory representation of a field or element.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Repr {
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    /// Managed pointer (traced).
    Ptr,
}

impl Repr {
    pub fn size(self) -> u32 {
        match self {
            Repr::I8 => 1,
            Repr::I16 => 2,
            Repr::I32 | Repr::F32 => 4,
            Repr::I64 | Repr::F64 | Repr::Ptr => 8,
        }
    }

    /// Load a value of this representation as a 64-bit word (sign- or
    /// zero-extension is decided by the descriptor, not here: raw bits).
    ///
    /// # Safety
    /// `p` must point to a readable value of this representation.
    pub unsafe fn load(self, p: *const u8) -> u64 {
        unsafe {
            match self {
                Repr::I8 => *p as u64,
                Repr::I16 => (p as *const u16).read_unaligned() as u64,
                Repr::I32 | Repr::F32 => (p as *const u32).read_unaligned() as u64,
                Repr::I64 | Repr::F64 | Repr::Ptr => (p as *const u64).read_unaligned(),
            }
        }
    }

    /// # Safety
    /// `p` must point to writable storage of this representation.
    pub unsafe fn store(self, p: *mut u8, w: u64) {
        unsafe {
            match self {
                Repr::I8 => *p = w as u8,
                Repr::I16 => (p as *mut u16).write_unaligned(w as u16),
                Repr::I32 | Repr::F32 => (p as *mut u32).write_unaligned(w as u32),
                Repr::I64 | Repr::F64 | Repr::Ptr => (p as *mut u64).write_unaligned(w),
            }
        }
    }
}

pub type TraceFn = fn(obj: *mut Header, mark: &mut dyn FnMut(u64));
pub type FinalizeFn = fn(obj: *mut Header);
/// User `Display` implementation registered by the compiler: the word
/// adapter of the `display` method instance (called with a null closure).
pub type DisplayFn = crate::Adapter;

#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub offset: u32,
    pub repr: Repr,
    pub desc: &'static TypeDesc,
}

#[derive(Debug)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Debug)]
pub enum Kind {
    Unit,
    Bool,
    Int {
        bits: u8,
        signed: bool,
    },
    Float {
        bits: u8,
    },
    Char,
    Symbol,
    Duration,
    Instant,
    Timestamp,
    String,
    Bytes,
    /// Records, tuples, and structs (`nominal`). Payload holds fields.
    Record {
        size: u32,
        fields: Vec<Field>,
        nominal: bool,
        tuple: bool,
        /// `@flatten`: a one-field struct encoded as its field's value
        /// (`BookId { value: 1 }` is `1` in JSON, data, and schemas).
        flat: bool,
    },
    /// Boxed enum: `u32` tag at payload offset 0, variant fields after.
    Enum {
        size: u32,
        variants: Vec<Variant>,
    },
    /// Closure environment: code pointer, adapter pointer, captures.
    Closure {
        size: u32,
        captures: Vec<Field>,
    },
    /// A function-typed value (fields of closure type); objects carry their
    /// own closure descriptor.
    Fn,
    /// A heap value of statically unknown type (the erased parameter of a
    /// polymorphic-recursion fallback): its own header descriptor is its
    /// type dictionary.
    Dynamic,
    List {
        elem: &'static TypeDesc,
    },
    Map {
        key: &'static TypeDesc,
        val: &'static TypeDesc,
    },
    Set {
        elem: &'static TypeDesc,
    },
    Range {
        elem: &'static TypeDesc,
    },
    /// Runtime-defined object with custom tracing/finalization.
    Opaque {
        trace: Option<TraceFn>,
        finalize: Option<FinalizeFn>,
        args: Vec<&'static TypeDesc>,
    },
}

#[derive(Debug)]
pub struct TypeDesc {
    pub name: String,
    pub kind: Kind,
    /// Optional user `Display` implementation for this type.
    pub display: AtomicPtr<()>,
    /// Unique id (used for hashing descriptors and debugging).
    pub id: usize,
    /// Shared immortal values of the first enum variants whose payload is
    /// all unit (`ok(())` and the like), made on first use.
    pub unit_variants: [AtomicU64; 4],
    /// Whether objects of this type hold no managed pointers (0: not yet
    /// known; 1: none; 2: some), cached for marking. Reset by code that
    /// fills in a descriptor's kind after creating it.
    pub leaf: std::sync::atomic::AtomicU8,
    /// Whether values of this type may contain a float (0: not yet known;
    /// 1: no; 2: may), cached for equality. Reset with `leaf`.
    pub floats: std::sync::atomic::AtomicU8,
}

static NEXT_ID: AtomicUsize = AtomicUsize::new(1);

impl TypeDesc {
    pub fn new(name: impl Into<String>, kind: Kind) -> TypeDesc {
        TypeDesc {
            name: name.into(),
            kind,
            display: AtomicPtr::new(std::ptr::null_mut()),
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            unit_variants: Default::default(),
            leaf: std::sync::atomic::AtomicU8::new(0),
            floats: std::sync::atomic::AtomicU8::new(0),
        }
    }

    /// Objects of this type hold no managed pointers: marking need not
    /// trace them.
    pub fn is_leaf(&self) -> bool {
        match self.leaf.load(Ordering::Relaxed) {
            1 => true,
            2 => false,
            _ => {
                let ptr = |fs: &[Field]| fs.iter().any(|f| f.repr == Repr::Ptr);
                let leaf = match &self.kind {
                    Kind::Record { fields, .. } => !ptr(fields),
                    Kind::Closure { captures, .. } => !ptr(captures),
                    Kind::Enum { variants, .. } => !variants.iter().any(|v| ptr(&v.fields)),
                    Kind::List { .. } | Kind::Map { .. } | Kind::Set { .. } => false,
                    Kind::Opaque { trace, .. } => trace.is_none(),
                    Kind::String | Kind::Bytes => true,
                    _ => false,
                };
                self.leaf.store(if leaf { 1 } else { 2 }, Ordering::Relaxed);
                leaf
            }
        }
    }

    pub fn leak(self) -> &'static TypeDesc {
        Box::leak(Box::new(self))
    }

    pub fn display_fn(&self) -> Option<DisplayFn> {
        let p = self.display.load(Ordering::Acquire);
        if p.is_null() { None } else { Some(unsafe { std::mem::transmute::<*mut (), DisplayFn>(p) }) }
    }

    pub fn set_display(&self, f: DisplayFn) {
        self.display.store(f as *mut (), Ordering::Release);
    }

    /// Representation of values of this type when stored in a field.
    pub fn repr(&self) -> Repr {
        match &self.kind {
            Kind::Unit | Kind::Bool => Repr::I8,
            Kind::Int { bits: 8, .. } => Repr::I8,
            Kind::Int { bits: 16, .. } => Repr::I16,
            Kind::Int { bits: 32, .. } | Kind::Char => Repr::I32,
            Kind::Int { .. } | Kind::Duration | Kind::Instant | Kind::Timestamp => Repr::I64,
            Kind::Float { bits: 32 } => Repr::F32,
            Kind::Float { .. } => Repr::F64,
            _ => Repr::Ptr,
        }
    }

    pub fn is_ptr(&self) -> bool {
        self.repr() == Repr::Ptr
    }

    pub fn variant(&self, tag: u32) -> Option<&Variant> {
        match &self.kind {
            Kind::Enum { variants, .. } => variants.get(tag as usize),
            _ => None,
        }
    }

    pub fn variant_index(&self, name: &str) -> Option<u32> {
        match &self.kind {
            Kind::Enum { variants, .. } => variants.iter().position(|v| v.name == name).map(|i| i as u32),
            _ => None,
        }
    }

    pub fn fields(&self) -> &[Field] {
        match &self.kind {
            Kind::Record { fields, .. } => fields,
            Kind::Closure { captures, .. } => captures,
            _ => &[],
        }
    }

    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields().iter().find(|f| f.name == name)
    }

    pub fn payload_size(&self) -> u32 {
        match &self.kind {
            Kind::Record { size, .. } | Kind::Enum { size, .. } | Kind::Closure { size, .. } => *size,
            _ => 0,
        }
    }
}

macro_rules! static_desc {
    ($name:ident, $text:expr, $kind:expr) => {
        pub fn $name() -> &'static TypeDesc {
            static D: std::sync::OnceLock<&'static TypeDesc> = std::sync::OnceLock::new();
            D.get_or_init(|| TypeDesc::new($text, $kind).leak())
        }
    };
}

/// Descriptors for built-in scalar and runtime types.
pub mod builtin {
    use super::*;
    static_desc!(unit, "Unit", Kind::Unit);
    static_desc!(bool_, "Bool", Kind::Bool);
    static_desc!(int, "Int", Kind::Int { bits: 64, signed: true });
    static_desc!(u8_, "U8", Kind::Int { bits: 8, signed: false });
    static_desc!(u16_, "U16", Kind::Int { bits: 16, signed: false });
    static_desc!(float, "Float", Kind::Float { bits: 64 });
    static_desc!(char_, "Char", Kind::Char);
    static_desc!(symbol, "Symbol", Kind::Symbol);
    static_desc!(string, "String", Kind::String);
    static_desc!(bytes, "Bytes", Kind::Bytes);
    static_desc!(duration, "Duration", Kind::Duration);
    static_desc!(instant, "Instant", Kind::Instant);
    static_desc!(fn_, "Fn", Kind::Fn);
}
