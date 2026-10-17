//! Runtime support linked into the host process and called from JIT code.
//!
//! - [`gc`]: precise nonmoving mark-sweep heap with shadow-stack roots
//! - [`desc`]: runtime type descriptors (layout, tracing, schema)
//! - [`task`], [`scope`]: stackful tasks, scheduler, cancellation, cleanup
//! - [`value`]: descriptor-driven equality, hashing, ordering, display
//! - [`string`], [`list`], [`map`]: core managed data
//! - [`intrinsics`]: `extern "C"` entry points used by compiled code
//!
//! Compiled code calls the runtime through `extern "C"` functions named
//! `lang_rt_*`; [`symbols`] lists them for the JIT linker. Standard-library
//! intrinsics use a uniform ABI:
//!
//! ```text
//! fn(ctx: *mut TaskCtx, targs: *const &TypeDesc, ret: &TypeDesc, args: u64...) -> u64
//! ```
#![allow(clippy::type_complexity)]
// Entry points take raw words and pointers from compiled code by design.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

pub mod abi;
pub mod buffer;
pub mod channel;
pub mod codec;
pub mod desc;
pub mod ffi;
pub mod gc;
pub mod hamt;
pub mod http;
pub mod inspect;
pub mod intrinsics;
pub mod intrinsics_codec;
pub mod intrinsics_io;
pub mod intrinsics_os;
pub mod intrinsics_std;
pub mod intrinsics_task;
pub mod list;
pub mod live;
pub mod map;
pub mod native;
pub mod observe;
pub mod parse;
pub mod profile;
pub mod reply;
pub mod repro;
pub mod scope;
pub mod string;
pub mod task;
pub mod telemetry;
pub mod value;

pub use desc::{Field, Kind, Repr, TypeDesc, Variant};
pub use task::TaskCtx;

/// Signature of a closure's word adapter: arguments and result as 64-bit
/// words (floats as bits, small integers zero-extended).
pub type Adapter = extern "C" fn(ctx: *mut TaskCtx, closure: u64, args: *const u64) -> u64;

/// Closure payload layout: code pointer, adapter pointer, captures.
pub const CLOSURE_CODE: usize = 0;
pub const CLOSURE_ADAPTER: usize = 8;
pub const CLOSURE_CAPTURES: usize = 16;

/// Call a managed closure with word arguments.
///
/// # Safety
/// `closure` must be a live closure object whose signature matches `args`.
pub unsafe fn call_closure(ctx: *mut TaskCtx, closure: u64, args: &[u64]) -> u64 {
    unsafe { call_closure_returning(ctx, closure, args, false) }
}

/// [`call_closure`] whose result is a managed pointer when `ptr_result`:
/// it stays rooted while this worker waits to re-enter the runtime.
///
/// # Safety
/// As [`call_closure`].
pub unsafe fn call_closure_returning(ctx: *mut TaskCtx, closure: u64, args: &[u64], ptr_result: bool) -> u64 {
    unsafe {
        let p = gc::obj(closure).payload();
        let adapter = *(p.add(CLOSURE_ADAPTER) as *const usize);
        let f: Adapter = std::mem::transmute(adapter);
        // compiled code runs outside the runtime lock (several workers)
        task::leave_runtime();
        let r = f(ctx, closure, args.as_ptr());
        if ptr_result && r != 0 {
            task::enter_runtime_holding(r);
        } else {
            task::enter_runtime_for(task::reason::CALLBACK_RETURN);
        }
        r
    }
}

/// A task's entry: [`call_closure_returning`], except that with several
/// workers `exit_free` is first offered the result, still outside the
/// runtime lock; when it takes it (`None`), the lock is not taken.
///
/// # Safety
/// As [`call_closure`].
pub unsafe fn call_entry(
    ctx: *mut TaskCtx,
    closure: u64,
    args: &[u64],
    ptr_result: bool,
    exit_free: &mut dyn FnMut(u64) -> bool,
) -> Option<u64> {
    unsafe {
        let p = gc::obj(closure).payload();
        let adapter = *(p.add(CLOSURE_ADAPTER) as *const usize);
        let f: Adapter = std::mem::transmute(adapter);
        // started under the runtime lock, or (several workers) by a worker
        // without it, already counted as running compiled code
        if task::holds_lock() {
            task::leave_runtime();
        }
        let r = f(ctx, closure, args.as_ptr());
        if task::multi() && exit_free(r) {
            return None;
        }
        if ptr_result && r != 0 {
            task::enter_runtime_holding(r);
        } else {
            task::enter_runtime_for(task::reason::CALLBACK_RETURN);
        }
        Some(r)
    }
}

/// Construct an enum value `desc` with variant `tag` and field words.
pub fn make_enum(desc: &'static TypeDesc, tag: u32, fields: &[u64]) -> u64 {
    if let Some(w) = shared_unit_variant(desc, tag) {
        return w;
    }
    let _r = gc::Roots::new();
    // only pointer-typed payload words are roots
    if let Some(v) = desc.variant(tag) {
        for (fd, &f) in v.fields.iter().zip(fields) {
            if fd.desc.is_ptr() {
                _r.add(f);
            }
        }
    }
    let w = gc::alloc(desc, desc.payload_size() as usize);
    unsafe {
        let p = gc::obj(w).payload();
        *(p as *mut u32) = tag;
        if let Some(v) = desc.variant(tag) {
            for (fd, &val) in v.fields.iter().zip(fields) {
                fd.repr.store(p.add(fd.offset as usize), val);
            }
        }
    }
    w
}

/// The one shared value of variant `tag` when its payload is all unit (so
/// every instance is equal and none needs allocating). `err` variants are
/// excluded: error origins are tracked by the error value's identity.
/// `TypeDesc::unit_variants` entry of a variant that has no shared value.
const NOT_SHARED: u64 = 1;

fn shared_unit_variant(desc: &'static TypeDesc, tag: u32) -> Option<u64> {
    use std::sync::atomic::Ordering;
    let slot = desc.unit_variants.get(tag as usize)?;
    let w = slot.load(Ordering::Acquire);
    // 1: known not to be shareable (it has a payload, or is an error)
    if w == NOT_SHARED {
        return None;
    }
    if w != 0 {
        return Some(w);
    }
    let v = desc.variant(tag)?;
    if v.name == "err" || !v.fields.iter().all(|f| matches!(f.desc.kind, desc::Kind::Unit)) {
        slot.store(NOT_SHARED, Ordering::Release);
        return None;
    }
    let locked = task::holds_lock();
    if !locked {
        task::enter_runtime_for(task::reason::CONSTANT);
    }
    // made under the runtime lock: a racing worker made it first, or this
    // one makes it (no second immortal copy)
    let w = match slot.load(Ordering::Acquire) {
        0 => {
            let w = gc::alloc_immortal(desc, desc.payload_size() as usize);
            unsafe { *(gc::obj(w).payload() as *mut u32) = tag };
            slot.store(w, Ordering::Release);
            w
        }
        w => w,
    };
    if !locked {
        task::leave_runtime();
    }
    Some(w)
}

/// Construct `Result.ok(value)` / `Option.some(value)` style values by
/// variant name.
pub fn make_variant(desc: &'static TypeDesc, name: &str, fields: &[u64]) -> u64 {
    let tag = desc.variant_index(name).unwrap_or_else(|| panic!("{} has no variant {name}", desc.name));
    make_enum(desc, tag, fields)
}

/// Read an enum's tag and variant field words.
pub fn enum_parts(w: u64) -> (u32, Vec<u64>) {
    unsafe {
        let o = gc::obj(w);
        let p = o.payload();
        let tag = *(p as *const u32);
        let v = o.desc().variant(tag);
        let fields = v
            .map(|v| v.fields.iter().map(|f| f.repr.load(p.add(f.offset as usize))).collect())
            .unwrap_or_default();
        (tag, fields)
    }
}

/// Construct a record/struct value from named field words.
pub fn make_record(desc: &'static TypeDesc, fields: &[(&str, u64)]) -> u64 {
    let _r = gc::Roots::new();
    for (name, f) in fields {
        if desc.field(name).is_some_and(|fd| fd.desc.is_ptr()) {
            _r.add(*f);
        }
    }
    let w = gc::alloc(desc, desc.payload_size() as usize);
    unsafe {
        let p = gc::obj(w).payload();
        for (name, val) in fields {
            if let Some(fd) = desc.field(name) {
                fd.repr.store(p.add(fd.offset as usize), *val);
            }
        }
    }
    w
}

/// Read a named field of a record/struct value.
pub fn record_field(w: u64, name: &str) -> Option<u64> {
    unsafe {
        let o = gc::obj(w);
        let fd = o.desc().field(name)?;
        Some(fd.repr.load(o.payload().add(fd.offset as usize)))
    }
}

/// Name and address of every runtime function JIT code may call.
pub fn symbols() -> Vec<(&'static str, *const u8)> {
    let mut v = abi::symbols();
    v.extend(intrinsics::symbols());
    v.extend(ffi::symbols());
    v.extend(observe::symbols());
    v.extend([
        ("lang_rt_enter", task::lang_rt_enter as *const u8),
        ("lang_rt_leave", task::lang_rt_leave as *const u8),
        ("lang_rt_safe_begin", task::lang_rt_safe_begin as *const u8),
        ("lang_rt_safe_end", task::lang_rt_safe_end as *const u8),
    ]);
    v
}

#[cfg(test)]
mod symbol_tests {
    /// Two runtime functions with one name: the JIT would bind one of them
    /// silently (as when a new `bytes_get` shadowed the index operator's).
    #[test]
    fn runtime_symbols_are_unique() {
        let mut seen = std::collections::HashSet::new();
        let dups: Vec<&str> =
            super::symbols().into_iter().map(|(n, _)| n).filter(|n| !seen.insert(*n)).collect();
        assert!(dups.is_empty(), "duplicate runtime symbols: {dups:?}");
    }
}
