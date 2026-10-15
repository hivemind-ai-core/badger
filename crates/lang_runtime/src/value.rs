//! Descriptor-driven structural operations on managed values: equality,
//! hashing, ordering, and display.

use std::cmp::Ordering;
use std::hash::Hasher;

use crate::desc::{Kind, Repr, TypeDesc};
use crate::gc::obj;
use crate::string::str_of;
use crate::task::TaskCtx;

/// Interpret a word of a scalar type as a signed 64-bit integer.
pub fn as_i64(d: &TypeDesc, w: u64) -> i64 {
    match &d.kind {
        Kind::Int { bits, signed: true } => match bits {
            8 => w as u8 as i8 as i64,
            16 => w as u16 as i16 as i64,
            32 => w as u32 as i32 as i64,
            _ => w as i64,
        },
        Kind::Int { bits, signed: false } => match bits {
            8 => w as u8 as i64,
            16 => w as u16 as i64,
            32 => w as u32 as i64,
            _ => w as i64,
        },
        _ => w as i64,
    }
}

pub fn as_f64(d: &TypeDesc, w: u64) -> f64 {
    match &d.kind {
        Kind::Float { bits: 32 } => f32::from_bits(w as u32) as f64,
        _ => f64::from_bits(w),
    }
}

fn field_word(base: *mut u8, offset: u32, repr: Repr) -> u64 {
    unsafe { repr.load(base.add(offset as usize)) }
}

/// The descriptor of a `Dynamic` value: its object's own.
fn dynamic_desc(w: u64) -> &'static TypeDesc {
    unsafe { crate::gc::obj(w).desc() }
}

pub fn eq(d: &TypeDesc, a: u64, b: u64) -> bool {
    if matches!(d.kind, Kind::Dynamic) {
        return if a == 0 || b == 0 {
            a == b
        } else {
            std::ptr::eq(dynamic_desc(a), dynamic_desc(b)) && eq(dynamic_desc(a), a, b)
        };
    }
    match &d.kind {
        Kind::Unit => true,
        Kind::Bool | Kind::Char | Kind::Duration | Kind::Instant | Kind::Timestamp => {
            (a & mask(d)) == (b & mask(d))
        }
        Kind::Int { .. } => as_i64(d, a) == as_i64(d, b),
        Kind::Float { .. } => as_f64(d, a) == as_f64(d, b),
        Kind::Symbol => a == b || str_of(a) == str_of(b),
        Kind::String | Kind::Bytes => a == b || crate::string::bytes_of(a) == crate::string::bytes_of(b),
        Kind::Record { fields, .. } => {
            if a == b {
                return true;
            }
            let (pa, pb) = unsafe { (obj(a).payload(), obj(b).payload()) };
            fields
                .iter()
                .all(|f| eq(f.desc, field_word(pa, f.offset, f.repr), field_word(pb, f.offset, f.repr)))
        }
        Kind::Enum { variants, .. } => {
            if a == b {
                return true;
            }
            let (pa, pb) = unsafe { (obj(a).payload(), obj(b).payload()) };
            let (ta, tb) = unsafe { (*(pa as *const u32), *(pb as *const u32)) };
            if ta != tb {
                return false;
            }
            variants[ta as usize]
                .fields
                .iter()
                .all(|f| eq(f.desc, field_word(pa, f.offset, f.repr), field_word(pb, f.offset, f.repr)))
        }
        Kind::List { elem } => {
            let (la, lb) = (crate::list::len(a), crate::list::len(b));
            la == lb && (0..la).all(|i| eq(elem, crate::list::get(a, i), crate::list::get(b, i)))
        }
        Kind::Map { .. } | Kind::Set { .. } => crate::map::eq(a, b),
        Kind::Range { .. } => {
            let (pa, pb) = unsafe { (obj(a).payload(), obj(b).payload()) };
            unsafe { std::slice::from_raw_parts(pa, 24) == std::slice::from_raw_parts(pb, 24) }
        }
        _ => a == b,
    }
}

fn mask(d: &TypeDesc) -> u64 {
    match d.repr() {
        Repr::I8 => 0xff,
        Repr::I16 => 0xffff,
        Repr::I32 | Repr::F32 => 0xffff_ffff,
        _ => u64::MAX,
    }
}

pub fn hash(d: &TypeDesc, w: u64, h: &mut impl Hasher) {
    if matches!(d.kind, Kind::Dynamic) && w != 0 {
        return hash(dynamic_desc(w), w, h);
    }
    match &d.kind {
        Kind::Unit => {}
        Kind::Int { .. } => h.write_i64(as_i64(d, w)),
        Kind::Float { .. } => h.write_u64(as_f64(d, w).to_bits()),
        Kind::Bool | Kind::Char | Kind::Duration | Kind::Instant | Kind::Timestamp => {
            h.write_u64(w & mask(d))
        }
        Kind::Symbol | Kind::String | Kind::Bytes => h.write(crate::string::bytes_of(w)),
        Kind::Record { fields, .. } => {
            let p = unsafe { obj(w).payload() };
            for f in fields {
                hash(f.desc, field_word(p, f.offset, f.repr), h);
            }
        }
        Kind::Enum { variants, .. } => {
            let p = unsafe { obj(w).payload() };
            let t = unsafe { *(p as *const u32) };
            h.write_u32(t);
            for f in &variants[t as usize].fields {
                hash(f.desc, field_word(p, f.offset, f.repr), h);
            }
        }
        Kind::List { elem } => {
            let n = crate::list::len(w);
            h.write_usize(n);
            for i in 0..n {
                hash(elem, crate::list::get(w, i), h);
            }
        }
        _ => h.write_u64(w),
    }
}

pub fn cmp(d: &TypeDesc, a: u64, b: u64) -> Ordering {
    if matches!(d.kind, Kind::Dynamic) && a != 0 && b != 0 {
        return cmp(dynamic_desc(a), a, b);
    }
    match &d.kind {
        Kind::Int { signed: false, .. } => (a & mask(d)).cmp(&(b & mask(d))),
        Kind::Int { .. } | Kind::Duration | Kind::Instant | Kind::Timestamp => {
            as_i64(d, a).cmp(&as_i64(d, b))
        }
        Kind::Float { .. } => as_f64(d, a).partial_cmp(&as_f64(d, b)).unwrap_or(Ordering::Equal),
        Kind::Bool | Kind::Char => (a & mask(d)).cmp(&(b & mask(d))),
        Kind::String | Kind::Symbol | Kind::Bytes => {
            crate::string::bytes_of(a).cmp(crate::string::bytes_of(b))
        }
        _ => Ordering::Equal,
    }
}

pub fn fmt_float(f: f64) -> String {
    if f.is_finite() && f == f.trunc() && f.abs() < 1e16 { format!("{f:.1}") } else { format!("{f}") }
}

pub fn fmt_duration(nanos: i64) -> String {
    let n = nanos.unsigned_abs();
    let sign = if nanos < 0 { "-" } else { "" };
    if n == 0 {
        "0s".into()
    } else if n.is_multiple_of(3_600_000_000_000) {
        format!("{sign}{}h", n / 3_600_000_000_000)
    } else if n.is_multiple_of(60_000_000_000) {
        format!("{sign}{}min", n / 60_000_000_000)
    } else if n.is_multiple_of(1_000_000_000) {
        format!("{sign}{}s", n / 1_000_000_000)
    } else if n >= 1_000_000_000 {
        format!("{sign}{:.3}s", n as f64 / 1e9)
    } else if n >= 1_000_000 {
        format!("{sign}{:.3}ms", n as f64 / 1e6)
    } else if n >= 1_000 {
        format!("{sign}{:.3}us", n as f64 / 1e3)
    } else {
        format!("{sign}{n}ns")
    }
}

/// Display a value. Strings display raw at the top level and quoted inside
/// aggregates. User `Display` implementations registered on descriptors are
/// used when present.
pub fn display(ctx: *mut TaskCtx, d: &TypeDesc, w: u64, top: bool) -> String {
    if matches!(d.kind, Kind::Dynamic) && w != 0 {
        return display(ctx, dynamic_desc(w), w, top);
    }
    if let Some(f) = d.display_fn() {
        let s = if ctx.is_null() {
            // outside any task (reporting main's error): no collection
            let _g = crate::gc::NoGc::new();
            let mut tmp =
                TaskCtx { shadow: std::ptr::null_mut(), budget: i64::MAX, flags: 0, task: 0, stack_limit: 0 };
            f(&mut tmp, 0, &w)
        } else {
            // the hook is compiled code: it runs outside the runtime lock
            crate::task::leave_runtime();
            let r = f(ctx, 0, &w);
            crate::task::enter_runtime();
            r
        };
        return str_of(s).to_string();
    }
    match &d.kind {
        Kind::Unit => "()".into(),
        Kind::Bool => {
            if w & 0xff != 0 {
                "true".into()
            } else {
                "false".into()
            }
        }
        Kind::Int { signed: false, .. } => (w & mask(d)).to_string(),
        Kind::Int { .. } => as_i64(d, w).to_string(),
        Kind::Float { .. } => fmt_float(as_f64(d, w)),
        Kind::Char => char::from_u32(w as u32)
            .map(|c| if top { c.to_string() } else { format!("{c:?}") })
            .unwrap_or_default(),
        Kind::Symbol => {
            if top {
                str_of(w).to_string()
            } else {
                format!(":{}", str_of(w))
            }
        }
        Kind::String => {
            if top {
                str_of(w).to_string()
            } else {
                format!("{:?}", str_of(w))
            }
        }
        Kind::Bytes => format!("<{} bytes>", crate::string::bytes_of(w).len()),
        Kind::Duration => fmt_duration(w as i64),
        Kind::Instant => format!("<instant {}>", w as i64),
        Kind::Timestamp => format!("<timestamp {}>", w as i64),
        Kind::Record { fields, tuple, nominal, .. } => {
            let p = unsafe { obj(w).payload() };
            let parts: Vec<String> = fields
                .iter()
                .map(|f| {
                    let v = display(ctx, f.desc, field_word(p, f.offset, f.repr), false);
                    if *tuple { v } else { format!("{}: {v}", f.name) }
                })
                .collect();
            if *tuple {
                format!("({})", parts.join(", "))
            } else if *nominal {
                format!("{} {{ {} }}", d.name, parts.join(", "))
            } else {
                format!("{{{}}}", parts.join(", "))
            }
        }
        Kind::Enum { variants, .. } => {
            let p = unsafe { obj(w).payload() };
            let t = unsafe { *(p as *const u32) };
            let v = &variants[t as usize];
            if v.fields.is_empty() {
                return v.name.clone();
            }
            let parts: Vec<String> =
                v.fields.iter().map(|f| display(ctx, f.desc, field_word(p, f.offset, f.repr), top)).collect();
            if top && d.name != "Option" && d.name != "Result" {
                // error-style display: `variant: payload`
                format!("{}: {}", v.name.replace('_', " "), parts.join(", "))
            } else {
                format!("{}({})", v.name, parts.join(", "))
            }
        }
        Kind::List { elem } => {
            let n = crate::list::len(w);
            let parts: Vec<String> =
                (0..n).map(|i| display(ctx, elem, crate::list::get(w, i), false)).collect();
            format!("[{}]", parts.join(", "))
        }
        Kind::Map { key, val } => {
            let mut parts: Vec<String> = crate::map::entries(w)
                .into_iter()
                .map(|(k, v)| format!("{} => {}", display(ctx, key, k, false), display(ctx, val, v, false)))
                .collect();
            parts.sort();
            format!("map {{ {} }}", parts.join(", "))
        }
        Kind::Set { elem } => {
            let mut parts: Vec<String> =
                crate::map::entries(w).into_iter().map(|(k, _)| display(ctx, elem, k, false)).collect();
            parts.sort();
            format!("set {{ {} }}", parts.join(", "))
        }
        Kind::Range { .. } => {
            let p = unsafe { obj(w).payload() as *const i64 };
            let (s, e, incl) = unsafe { (*p, *p.add(1), *p.add(2)) };
            format!("{s}{}{e}", if incl != 0 { "..=" } else { ".." })
        }
        Kind::Fn | Kind::Closure { .. } => "<fn>".into(),
        Kind::Opaque { .. } | Kind::Dynamic => format!("<{}>", d.name),
    }
}
