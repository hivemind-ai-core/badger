//! The C boundary: `repr c` struct images and raw-pointer intrinsics.
//!
//! A managed `repr c` struct keeps its fields in declared order; at a
//! foreign call it is packed into a C-layout image (natural alignment,
//! nested structs inline) and results are unpacked into new managed objects.
//! `ffi.Ptr<T>` values are raw addresses, never traced or freed by the
//! collector.

use crate::desc::{Kind, TypeDesc};
use crate::gc::{self, Roots};
use crate::intrinsics::{Targs, rdesc, targ};
use crate::string::{new_string, str_of};
use crate::task::TaskCtx;

fn align(off: usize, a: usize) -> usize {
    off.div_ceil(a) * a
}

/// C size and alignment of a type (scalars and `repr c` records).
pub fn c_size_align(d: &TypeDesc) -> (usize, usize) {
    match &d.kind {
        Kind::Bool => (1, 1),
        Kind::Int { bits, .. } => ((*bits / 8) as usize, (*bits / 8) as usize),
        Kind::Float { bits } => ((*bits / 8) as usize, (*bits / 8) as usize),
        Kind::Char => (4, 4),
        Kind::Record { fields, .. } => {
            let (mut off, mut al) = (0usize, 1usize);
            for f in fields {
                let (s, a) = c_size_align(f.desc);
                off = align(off, a) + s;
                al = al.max(a);
            }
            (align(off.max(1), al), al)
        }
        _ => (8, 8),
    }
}

/// Write the C image of managed value `w` (of type `d`) at `buf`.
unsafe fn pack(d: &TypeDesc, w: u64, buf: *mut u8) {
    unsafe {
        match &d.kind {
            Kind::Record { fields, .. } => {
                let p = gc::obj(w).payload();
                let mut off = 0usize;
                for f in fields {
                    let (s, a) = c_size_align(f.desc);
                    off = align(off, a);
                    let v = f.repr.load(p.add(f.offset as usize));
                    pack(f.desc, v, buf.add(off));
                    off += s;
                }
            }
            _ => {
                let (s, _) = c_size_align(d);
                let bytes = w.to_le_bytes();
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, s.min(8));
            }
        }
    }
}

/// Read a value of type `d` from the C image at `buf` (allocating managed
/// structs). Scalars return their word.
unsafe fn unpack(d: &'static TypeDesc, buf: *const u8) -> u64 {
    unsafe {
        match &d.kind {
            Kind::Record { fields, .. } => {
                let r = Roots::new();
                let obj = r.add(gc::alloc(d, d.payload_size() as usize));
                let mut off = 0usize;
                for f in fields {
                    let (s, a) = c_size_align(f.desc);
                    off = align(off, a);
                    let v = unpack(f.desc, buf.add(off));
                    if f.desc.is_ptr() {
                        r.add(v);
                    }
                    f.repr.store(gc::obj(obj).payload().add(f.offset as usize), v);
                    off += s;
                }
                obj
            }
            _ => {
                let (s, _) = c_size_align(d);
                let mut bytes = [0u8; 8];
                std::ptr::copy_nonoverlapping(buf, bytes.as_mut_ptr(), s.min(8));
                let mut w = u64::from_le_bytes(bytes);
                // sign-extend narrow signed integers
                if let Kind::Int { bits, signed: true } = d.kind
                    && bits < 64
                {
                    let sh = 64 - bits as u32;
                    w = (((w << sh) as i64) >> sh) as u64;
                }
                w
            }
        }
    }
}

/// Codegen: pack a `repr c` struct argument into `buf`.
pub extern "C" fn badger_rt_ffi_pack(_ctx: *mut TaskCtx, obj: u64, desc: u64, buf: u64) {
    unsafe { pack(&*(desc as *const TypeDesc), obj, buf as *mut u8) }
}

/// Codegen: build a `repr c` struct result from the image at `buf`.
pub extern "C" fn badger_rt_ffi_unpack(_ctx: *mut TaskCtx, desc: u64, buf: u64) -> u64 {
    unsafe { unpack(&*(desc as *const TypeDesc), buf as *const u8) }
}

pub extern "C" fn ptr_read(_ctx: *mut TaskCtx, targs: Targs, _ret: *const TypeDesc, p: u64) -> u64 {
    if p == 0 {
        crate::task::panic_current("read through a null pointer".into(), String::new());
    }
    unsafe { unpack(targ(targs, 0), p as *const u8) }
}

pub extern "C" fn ptr_write(_ctx: *mut TaskCtx, targs: Targs, _ret: *const TypeDesc, p: u64, v: u64) -> u64 {
    if p == 0 {
        crate::task::panic_current("write through a null pointer".into(), String::new());
    }
    unsafe { pack(targ(targs, 0), v, p as *mut u8) };
    0
}

pub extern "C" fn ptr_offset(_ctx: *mut TaskCtx, targs: Targs, _ret: *const TypeDesc, p: u64, n: u64) -> u64 {
    let (s, _) = c_size_align(targ(targs, 0));
    (p as i64).wrapping_add((n as i64).wrapping_mul(s as i64)) as u64
}

pub extern "C" fn c_alloc(_ctx: *mut TaskCtx, targs: Targs, _ret: *const TypeDesc, n: u64) -> u64 {
    let (s, _) = c_size_align(targ(targs, 0));
    unsafe { libc::calloc((n as usize).max(1), s) as u64 }
}

pub extern "C" fn c_free(_ctx: *mut TaskCtx, _targs: Targs, _ret: *const TypeDesc, p: u64) -> u64 {
    unsafe { libc::free(p as *mut libc::c_void) };
    0
}

pub extern "C" fn cstring_new(_ctx: *mut TaskCtx, _targs: Targs, _ret: *const TypeDesc, s: u64) -> u64 {
    let text = str_of(s);
    unsafe {
        let p = libc::malloc(text.len() + 1) as *mut u8;
        std::ptr::copy_nonoverlapping(text.as_ptr(), p, text.len());
        *p.add(text.len()) = 0;
        p as u64
    }
}

pub extern "C" fn cstring_read(_ctx: *mut TaskCtx, _targs: Targs, ret: *const TypeDesc, p: u64) -> u64 {
    let _ = rdesc(ret);
    if p == 0 {
        return new_string("");
    }
    let s = unsafe { std::ffi::CStr::from_ptr(p as *const libc::c_char) };
    new_string(&s.to_string_lossy())
}

pub fn symbols() -> Vec<(&'static str, *const u8)> {
    vec![
        ("badger_rt_ffi_pack", badger_rt_ffi_pack as *const u8),
        ("badger_rt_ffi_unpack", badger_rt_ffi_unpack as *const u8),
        ("badger_rt_i_ptr_read", ptr_read as *const u8),
        ("badger_rt_i_ptr_write", ptr_write as *const u8),
        ("badger_rt_i_ptr_offset", ptr_offset as *const u8),
        ("badger_rt_i_c_alloc", c_alloc as *const u8),
        ("badger_rt_i_c_free", c_free as *const u8),
        ("badger_rt_i_cstring_new", cstring_new as *const u8),
        ("badger_rt_i_cstring_read", cstring_read as *const u8),
    ]
}
