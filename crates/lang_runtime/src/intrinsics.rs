//! Standard-library intrinsics with the uniform ABI:
//! `fn(ctx, targs, ret, args...) -> u64`.
//!
//! `targs` points at the descriptors of the intrinsic's type arguments and
//! `ret` is the descriptor of its result type, so intrinsics can construct
//! typed results (`Result`, `Option`, records) directly.

use crate::desc::{Kind, TypeDesc};
use crate::string::{new_string, str_of};
use crate::task::TaskCtx;
use crate::value;

pub type Targs = *const &'static TypeDesc;

pub fn targ(t: Targs, i: usize) -> &'static TypeDesc {
    unsafe { *t.add(i) }
}

pub fn rdesc(r: *const TypeDesc) -> &'static TypeDesc {
    unsafe { &*r }
}

macro_rules! intrinsics {
    ($( fn $name:ident($ctx:ident, $targs:ident, $ret:ident $(, $arg:ident)*) $body:block )*) => {
        $(
            #[allow(unused_variables)]
            pub extern "C" fn $name($ctx: *mut TaskCtx, $targs: Targs, $ret: *const TypeDesc $(, $arg: u64)*) -> u64 $body
        )*
        pub fn symbols() -> Vec<(&'static str, *const u8)> {
            let mut v: Vec<(&'static str, *const u8)> = vec![$((concat!("lang_rt_i_", stringify!($name)), $name as *const u8)),*];
            v.extend(crate::intrinsics_io::symbols());
            v.extend(crate::intrinsics_std::symbols());
            v.extend(crate::intrinsics_task::symbols());
            v.extend(crate::intrinsics_codec::symbols());
            v.extend(crate::intrinsics_codec::extra_symbols());
            v.extend(crate::intrinsics_codec::inspect_symbols());
            v.extend(crate::sqlite::all_symbols());
            v.extend(crate::http::symbols());
            v.extend(crate::intrinsics_os::symbols());
            v.extend(crate::parse::symbols());
            v
        }
    };
}

intrinsics! {
    // whether a numeric conversion involving a float is exact: a float
    // converts to an integer only when finite, integral, and in range; an
    // integer or F64 converts to a float only when it round-trips
    fn num_fits(ctx, targs, ret, v) {
        let (from, to) = (targ(targs, 0), targ(targs, 1));
        let exact = match (&from.kind, &to.kind) {
            (Kind::Float { .. }, Kind::Int { bits, signed }) => {
                let f = value::as_f64(from, v);
                let (lo, hi) = if *signed {
                    (-(2f64.powi(*bits as i32 - 1)), 2f64.powi(*bits as i32 - 1))
                } else {
                    (0.0, 2f64.powi(*bits as i32))
                };
                f.is_finite() && f.fract() == 0.0 && f >= lo && f < hi
            }
            (Kind::Int { signed, .. }, Kind::Float { bits }) => {
                if *signed {
                    let i = value::as_i64(from, v);
                    if *bits == 32 { (i as f32) as i128 == i as i128 && (i as f32).abs() < 2f32.powi(63) } else { (i as f64) as i128 == i as i128 && (i as f64).abs() < 2f64.powi(63) }
                } else {
                    let u = value::as_i64(from, v) as u64;
                    if *bits == 32 { (u as f32) < 2f32.powi(64) && (u as f32) as u64 == u } else { (u as f64) < 2f64.powi(64) && (u as f64) as u64 == u }
                }
            }
            (Kind::Float { bits: 64 }, Kind::Float { bits: 32 }) => {
                let f = value::as_f64(from, v);
                f.is_nan() || (f as f32) as f64 == f
            }
            _ => true,
        };
        exact as u64
    }
    fn int_to_string(ctx, targs, ret, v) {
        let d = targ(targs, 0);
        let s = match &d.kind {
            Kind::Int { signed: false, bits } => {
                let m = if *bits == 64 { u64::MAX } else { (1u64 << bits) - 1 };
                (v & m).to_string()
            }
            _ => value::as_i64(d, v).to_string(),
        };
        new_string(&s)
    }
    fn float_to_string(ctx, targs, ret, v) {
        new_string(&value::fmt_float_of(targ(targs, 0), v))
    }
    fn char_to_string(ctx, targs, ret, v) {
        new_string(&char::from_u32(v as u32).map(|c| c.to_string()).unwrap_or_default())
    }
    fn symbol_to_string(ctx, targs, ret, v) {
        new_string(str_of(v))
    }
    fn duration_to_string(ctx, targs, ret, v) {
        new_string(&value::fmt_duration(v as i64))
    }
    fn data_to_string(ctx, targs, ret, v) {
        new_string(&crate::intrinsics_io::data_json(v))
    }
    fn display_value(ctx, targs, ret, v) {
        new_string(&value::display(ctx, targ(targs, 0), v, true))
    }
}
