//! Schema-directed JSON encoding and decoding.
//!
//! Descriptors are the derived schemas: records/structs encode as objects
//! (field names are the wire names), enums as `{"variant": {...fields}}` or a
//! bare string for nullary variants (`Option` encodes as the value or
//! `null`), lists as arrays, maps with string keys as objects, and `Data` as
//! its natural JSON value. Decoding validates against the schema and reports
//! the failing path.

use serde_json::Value as J;

use crate::desc::{Kind, TypeDesc};
use crate::gc::{self, Roots, obj};
use crate::intrinsics_io::json_quote;
use crate::string::{bytes_of, new_string, str_of};
use crate::{enum_parts, list, make_enum, map};

/// Versioned wire names: `Type.member` (field or variant) or
/// `Type.variant.field` mapped to the name used on the wire, plus older
/// names still accepted when decoding.
#[derive(Default)]
pub struct WireNames {
    pub names: std::collections::HashMap<String, String>,
    pub aliases: std::collections::HashMap<String, Vec<String>>,
}

thread_local! {
    static WIRE: std::cell::RefCell<Option<std::rc::Rc<WireNames>>> = const { std::cell::RefCell::new(None) };
}

/// Run `f` with `w` as the active wire naming.
pub fn with_wire<R>(w: WireNames, f: impl FnOnce() -> R) -> R {
    let prev = WIRE.with(|c| c.borrow_mut().replace(std::rc::Rc::new(w)));
    let r = f();
    WIRE.with(|c| *c.borrow_mut() = prev);
    r
}

/// The wire name of member `member` of type `ty`.
pub fn wire_name(ty: &str, member: &str) -> String {
    WIRE.with(|c| match &*c.borrow() {
        Some(w) => w.names.get(&format!("{ty}.{member}")).cloned().unwrap_or_else(|| member.to_string()),
        None => member.to_string(),
    })
}

/// Names accepted when decoding member `member` of `ty`: the current wire
/// name first, then any aliases from older versions.
fn accepted(ty: &str, member: &str) -> Vec<String> {
    let mut out = vec![wire_name(ty, member)];
    WIRE.with(|c| {
        if let Some(w) = &*c.borrow()
            && let Some(a) = w.aliases.get(&format!("{ty}.{member}"))
        {
            out.extend(a.iter().cloned());
        }
    });
    out
}

fn lookup<'j>(m: &'j serde_json::Map<String, J>, ty: &str, member: &str) -> Option<&'j J> {
    accepted(ty, member).iter().find_map(|n| m.get(n))
}

pub fn encode(d: &TypeDesc, w: u64) -> String {
    let mut s = String::new();
    enc(d, w, &mut s);
    s
}

fn enc(d: &TypeDesc, w: u64, o: &mut String) {
    if matches!(d.kind, Kind::Dynamic) && w != 0 {
        return enc(unsafe { obj(w).desc() }, w, o);
    }
    match &d.kind {
        Kind::Unit => o.push_str("null"),
        Kind::Bool => o.push_str(if w & 1 != 0 { "true" } else { "false" }),
        Kind::Int { signed: false, bits } => {
            let m = if *bits == 64 { u64::MAX } else { (1u64 << bits) - 1 };
            o.push_str(&(w & m).to_string())
        }
        Kind::Int { .. } => o.push_str(&crate::value::as_i64(d, w).to_string()),
        Kind::Float { .. } => {
            let f = crate::value::as_f64(d, w);
            if f.is_finite() {
                o.push_str(&serde_json::Number::from_f64(f).map(|n| n.to_string()).unwrap_or("null".into()))
            } else {
                o.push_str("null")
            }
        }
        Kind::Char => {
            o.push_str(&json_quote(&char::from_u32(w as u32).map(String::from).unwrap_or_default()))
        }
        Kind::Symbol | Kind::String => o.push_str(&json_quote(str_of(w))),
        Kind::Bytes => o.push_str(&json_quote(&String::from_utf8_lossy(bytes_of(w)))),
        Kind::Duration | Kind::Instant | Kind::Timestamp => o.push_str(&(w as i64).to_string()),
        Kind::Record { fields, tuple, .. } => {
            let p = unsafe { obj(w).payload() };
            if *tuple {
                o.push('[');
                for (i, f) in fields.iter().enumerate() {
                    if i > 0 {
                        o.push(',');
                    }
                    enc(f.desc, unsafe { f.repr.load(p.add(f.offset as usize)) }, o);
                }
                o.push(']');
            } else {
                o.push('{');
                for (i, f) in fields.iter().enumerate() {
                    if i > 0 {
                        o.push(',');
                    }
                    o.push_str(&json_quote(&wire_name(&d.name, &f.name)));
                    o.push(':');
                    enc(f.desc, unsafe { f.repr.load(p.add(f.offset as usize)) }, o);
                }
                o.push('}');
            }
        }
        Kind::Enum { variants, .. } => {
            let (tag, vals) = enum_parts(w);
            let v = &variants[tag as usize];
            if d.name == "Data" {
                o.push_str(&crate::intrinsics_io::data_json(w));
                return;
            }
            if d.name == "Option" {
                if v.fields.is_empty() {
                    o.push_str("null");
                } else {
                    enc(v.fields[0].desc, vals[0], o);
                }
                return;
            }
            let vname = wire_name(&d.name, &v.name);
            if v.fields.is_empty() {
                o.push_str(&json_quote(&vname));
                return;
            }
            o.push('{');
            o.push_str(&json_quote(&vname));
            o.push_str(":{");
            for (i, (f, val)) in v.fields.iter().zip(vals).enumerate() {
                if i > 0 {
                    o.push(',');
                }
                o.push_str(&json_quote(&wire_name(&format!("{}.{}", d.name, v.name), &f.name)));
                o.push(':');
                enc(f.desc, val, o);
            }
            o.push_str("}}");
        }
        Kind::List { elem } => {
            o.push('[');
            for i in 0..list::len(w) {
                if i > 0 {
                    o.push(',');
                }
                enc(elem, list::get(w, i), o);
            }
            o.push(']');
        }
        Kind::Map { key, val } => {
            o.push('{');
            let mut es = map::entries(w);
            es.sort_by(|a, b| crate::value::cmp(key, a.0, b.0));
            for (i, (k, v)) in es.into_iter().enumerate() {
                if i > 0 {
                    o.push(',');
                }
                o.push_str(&json_quote(&crate::value::display(std::ptr::null_mut(), key, k, true)));
                o.push(':');
                enc(val, v, o);
            }
            o.push('}');
        }
        Kind::Set { elem } => {
            o.push('[');
            for (i, (k, _)) in map::entries(w).into_iter().enumerate() {
                if i > 0 {
                    o.push(',');
                }
                enc(elem, k, o);
            }
            o.push(']');
        }
        _ => o.push_str("null"),
    }
}

pub struct DecodeError {
    pub path: String,
    pub message: String,
}

fn fail<T>(path: &str, msg: impl Into<String>) -> Result<T, DecodeError> {
    Err(DecodeError { path: if path.is_empty() { "$".into() } else { path.into() }, message: msg.into() })
}

/// Decode JSON text against a descriptor.
pub fn decode(d: &'static TypeDesc, text: &str) -> Result<u64, DecodeError> {
    let j: J = serde_json::from_str(text)
        .map_err(|e| DecodeError { path: "$".into(), message: format!("invalid JSON: {e}") })?;
    let _g = Roots::new();
    dec(d, &j, "$")
}

/// Decode a `Data` value against a descriptor.
pub fn from_data(d: &'static TypeDesc, w: u64) -> Result<u64, DecodeError> {
    let j: J = serde_json::from_str(&crate::intrinsics_io::data_json(w)).unwrap_or(J::Null);
    let _g = Roots::new();
    dec(d, &j, "$")
}

fn root(w: u64) -> u64 {
    gc::push_root(w);
    w
}

fn dec(d: &'static TypeDesc, j: &J, path: &str) -> Result<u64, DecodeError> {
    match &d.kind {
        Kind::Unit => Ok(0),
        Kind::Bool => j.as_bool().map(|b| b as u64).ok_or(()).or_else(|_| fail(path, "expected a boolean")),
        Kind::Int { bits, signed } => {
            let v = if *signed { j.as_i64() } else { j.as_u64().map(|u| u as i64) };
            let Some(v) = v else { return fail(path, "expected an integer") };
            let ok = match (bits, signed) {
                (64, _) => true,
                (b, true) => v >= -(1i64 << (b - 1)) && v < (1i64 << (b - 1)),
                (b, false) => v >= 0 && v < (1i64 << b),
            };
            if !ok {
                return fail(path, format!("integer out of range for {}", d.name));
            }
            Ok(v as u64)
        }
        Kind::Float { bits } => {
            let Some(f) = j.as_f64() else { return fail(path, "expected a number") };
            Ok(if *bits == 32 { (f as f32).to_bits() as u64 } else { f.to_bits() })
        }
        Kind::String => match j.as_str() {
            Some(s) => Ok(root(new_string(s))),
            None => fail(path, "expected a string"),
        },
        Kind::Symbol => match j.as_str() {
            Some(s) => Ok(crate::string::symbol(s)),
            None => fail(path, "expected a symbol string"),
        },
        Kind::Char => match j.as_str().and_then(|s| s.chars().next()) {
            Some(c) => Ok(c as u64),
            None => fail(path, "expected a character"),
        },
        Kind::Bytes => match j.as_str() {
            Some(s) => Ok(root(crate::string::new_bytes(s.as_bytes()))),
            None => fail(path, "expected a string"),
        },
        Kind::Duration | Kind::Instant | Kind::Timestamp => {
            j.as_i64().map(|v| v as u64).ok_or(()).or_else(|_| fail(path, "expected an integer"))
        }
        Kind::Record { fields, tuple, size, .. } => {
            let mut vals = vec![];
            if *tuple {
                let Some(a) = j.as_array() else { return fail(path, "expected an array") };
                if a.len() != fields.len() {
                    return fail(path, format!("expected {} elements", fields.len()));
                }
                for (i, f) in fields.iter().enumerate() {
                    vals.push(dec(f.desc, &a[i], &format!("{path}[{i}]"))?);
                }
            } else {
                let Some(m) = j.as_object() else { return fail(path, "expected an object") };
                for f in fields {
                    let p = format!("{path}.{}", wire_name(&d.name, &f.name));
                    match lookup(m, &d.name, &f.name) {
                        Some(v) => vals.push(dec(f.desc, v, &p)?),
                        None if f.desc.name == "Option" => vals.push(dec(f.desc, &J::Null, &p)?),
                        None => return fail(&p, "missing required field"),
                    }
                }
            }
            let w = root(gc::alloc(d, *size as usize));
            let pl = unsafe { obj(w).payload() };
            for (f, v) in fields.iter().zip(vals) {
                unsafe { f.repr.store(pl.add(f.offset as usize), v) };
            }
            Ok(w)
        }
        Kind::Enum { variants, .. } => {
            if d.name == "Data" {
                return Ok(root(data_of(d, j)));
            }
            if d.name == "Option" {
                if j.is_null() {
                    return Ok(root(make_enum(d, 0, &[])));
                }
                let v = dec(variants[1].fields[0].desc, j, path)?;
                return Ok(root(make_enum(d, 1, &[v])));
            }
            if let Some(s) = j.as_str() {
                if let Some(i) = variants
                    .iter()
                    .position(|v| accepted(&d.name, &v.name).iter().any(|n| n == s) && v.fields.is_empty())
                {
                    return Ok(root(make_enum(d, i as u32, &[])));
                }
                return fail(path, format!("unknown variant `{s}`"));
            }
            let Some(m) = j.as_object().filter(|m| m.len() == 1) else {
                return fail(path, "expected a variant");
            };
            let (name, body) = m.iter().next().unwrap();
            let Some(i) = variants.iter().position(|v| accepted(&d.name, &v.name).iter().any(|n| n == name))
            else {
                return fail(path, format!("unknown variant `{name}`"));
            };
            let mut vals = vec![];
            let vty = format!("{}.{}", d.name, variants[i].name);
            for f in &variants[i].fields {
                let p = format!("{path}.{name}.{}", wire_name(&vty, &f.name));
                let Some(body) = body.as_object() else {
                    return fail(&format!("{path}.{name}"), "expected an object");
                };
                match lookup(body, &vty, &f.name) {
                    Some(v) => vals.push(dec(f.desc, v, &p)?),
                    None => return fail(&p, "missing required field"),
                }
            }
            Ok(root(make_enum(d, i as u32, &vals)))
        }
        Kind::List { elem } => {
            let Some(a) = j.as_array() else { return fail(path, "expected an array") };
            let l = root(list::new(d));
            for (i, x) in a.iter().enumerate() {
                let v = dec(elem, x, &format!("{path}[{i}]"))?;
                list::push(l, v);
            }
            Ok(l)
        }
        Kind::Map { key, val } => {
            let Some(m) = j.as_object() else { return fail(path, "expected an object") };
            let out = root(map::new(d));
            for (k, v) in m {
                let kw = dec(key, &J::String(k.clone()), &format!("{path}.{k}"))?;
                let vw = dec(val, v, &format!("{path}.{k}"))?;
                map::insert_mut(out, kw, vw);
            }
            Ok(out)
        }
        _ => fail(path, format!("type `{}` has no decoder", d.name)),
    }
}

/// An integer of the encoded value outside `Int` (a `U64` above
/// `I64::MAX`), which `Data` cannot hold exactly.
pub fn first_unrepresentable(j: &J) -> Option<u64> {
    match j {
        J::Number(n) if n.is_u64() && n.as_i64().is_none() => n.as_u64(),
        J::Array(xs) => xs.iter().find_map(first_unrepresentable),
        J::Object(m) => m.values().find_map(first_unrepresentable),
        _ => None,
    }
}

/// Build a `Data` value from JSON.
pub fn data_of(d: &'static TypeDesc, j: &J) -> u64 {
    let r = Roots::new();
    match j {
        J::Null => make_enum(d, 0, &[]),
        J::Bool(b) => make_enum(d, 1, &[*b as u64]),
        J::Number(n) => match n.as_i64() {
            Some(i) => make_enum(d, 2, &[i as u64]),
            None => make_enum(d, 3, &[n.as_f64().unwrap_or(0.0).to_bits()]),
        },
        J::String(s) => {
            let w = r.add(new_string(s));
            make_enum(d, 4, &[w])
        }
        J::Array(a) => {
            let list_desc = d.variant(5).unwrap().fields[0].desc;
            let l = r.add(list::new(list_desc));
            for x in a {
                let v = r.add(data_of(d, x));
                list::push(l, v);
            }
            make_enum(d, 5, &[l])
        }
        J::Object(m) => {
            let map_desc = d.variant(6).unwrap().fields[0].desc;
            let mw = r.add(map::new(map_desc));
            for (k, v) in m {
                let kw = r.add(new_string(k));
                let vw = r.add(data_of(d, v));
                map::insert_mut(mw, kw, vw);
            }
            make_enum(d, 6, &[mw])
        }
    }
}
