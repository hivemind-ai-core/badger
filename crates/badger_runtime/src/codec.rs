//! Schema-directed JSON encoding and decoding.
//!
//! Descriptors are the derived schemas: records/structs encode as objects
//! (field names are the wire names), enums as `{"variant": {...fields}}` or a
//! bare string for nullary variants (`Option` encodes as the value or
//! `null`; as `[value]` when the value can be `null` itself, so `some(none)`
//! is not `none`), `Bytes` as base64 text, lists and sets as arrays, maps with string keys as objects, and `Data` as
//! its natural JSON value. Decoding validates against the schema and reports
//! the failing path.

use base64::Engine;
use serde_json::Value as J;

use crate::desc::{Kind, TypeDesc};
use crate::gc::{self, Roots, obj};
use crate::intrinsics_io::write_json_quote;
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

/// The descriptor name of `config.Secret`: decoded from a string, encoded
/// as `"[redacted]"` (except by `encode_revealed`).
pub const SECRET: &str = "std.Secret";

thread_local! {
    static REVEAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// `encode`, with secrets' values (configuration defaults, which are given
/// in source and resolved in-process).
pub fn encode_revealed(d: &TypeDesc, w: u64) -> String {
    let prev = REVEAL.with(|c| c.replace(true));
    let s = encode(d, w);
    REVEAL.with(|c| c.set(prev));
    s
}

/// The wire name of member `member` of type `ty`.
pub fn wire_name(ty: &str, member: &str) -> String {
    with_name(ty, member, str::to_owned)
}

// Keep the common, unversioned member borrowed. The callback only writes
// native output, so a thread-local borrow never spans a managed allocation.
pub(crate) fn with_name<R>(ty: &str, member: &str, f: impl FnOnce(&str) -> R) -> R {
    WIRE.with(|c| match &*c.borrow() {
        Some(w) => f(w.names.get(&format!("{ty}.{member}")).map_or(member, String::as_str)),
        None => f(member),
    })
}

pub(crate) fn name_matches(ty: &str, member: &str, name: &str) -> bool {
    WIRE.with(|c| {
        let wire = c.borrow();
        let Some(w) = &*wire else { return member == name };
        let key = format!("{ty}.{member}");
        w.names.get(&key).map_or(member, String::as_str) == name
            || w.aliases.get(&key).is_some_and(|aliases| aliases.iter().any(|a| a == name))
    })
}

/// Whether any wire naming (renames or accepted aliases) is active on this
/// thread. Callers can then compare member names directly instead of paying a
/// thread-local lookup per key.
pub(crate) fn wire_active() -> bool {
    WIRE.with(|c| c.borrow().is_some())
}

/// How a JSON key ranks for member `member` of type `ty`: 0 for the current
/// wire name, 1+ for the position of an accepted alias, `None` for no match.
/// Mirrors [`lookup`]'s precedence (current name, then the first declared
/// alias that is present).
pub(crate) fn wire_rank(ty: &str, member: &str, key: &str) -> Option<usize> {
    WIRE.with(|c| {
        let wire = c.borrow();
        let Some(w) = &*wire else { return (member == key).then_some(0) };
        let k = format!("{ty}.{member}");
        if w.names.get(&k).map_or(member, String::as_str) == key {
            return Some(0);
        }
        w.aliases.get(&k).and_then(|a| a.iter().position(|x| x == key)).map(|i| i + 1)
    })
}

/// Whether decoding a SQLite row (which has no boolean type) is active.
pub(crate) fn int_bools() -> bool {
    INT_BOOLS.with(|c| c.get())
}

fn lookup<'j>(m: &'j serde_json::Map<String, J>, ty: &str, member: &str) -> Option<&'j J> {
    WIRE.with(|c| {
        let wire = c.borrow();
        let Some(w) = &*wire else { return m.get(member) };
        let key = format!("{ty}.{member}");
        // Current name wins over aliases, which retain their declared order.
        m.get(w.names.get(&key).map_or(member, String::as_str))
            .or_else(|| w.aliases.get(&key).and_then(|aliases| aliases.iter().find_map(|a| m.get(a))))
    })
}

/// How deeply an encoded or decoded value may nest: each level is a native
/// stack frame, and decoded input may be hostile.
pub(crate) const MAX_DEPTH: usize = 512;

thread_local! {
    static ENC_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub fn encode(d: &TypeDesc, w: u64) -> String {
    // Most encoded values are small objects; one modest reservation avoids
    // the first few doublings of the output buffer.
    let mut s = String::with_capacity(64);
    let outer = ENC_DEPTH.with(|c| c.replace(0));
    enc(d, w, &mut s);
    ENC_DEPTH.with(|c| c.set(outer));
    s
}

/// One more level of an encoded value: past the limit it is a defect (its
/// encoding could not be decoded again), not a native stack overflow.
fn enc_deeper() {
    let depth = ENC_DEPTH.with(|c| {
        c.set(c.get() + 1);
        c.get()
    });
    if depth > MAX_DEPTH {
        ENC_DEPTH.with(|c| c.set(0));
        crate::task::panic_current(
            format!("a value nested more than {MAX_DEPTH} levels deep cannot be encoded"),
            crate::task::caller_location(),
        );
    }
}

/// Append `v` in decimal. Encoders write many integers; this avoids the
/// `core::fmt` machinery (and its formatting buffer) for each one.
pub(crate) fn write_u64(o: &mut String, v: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    let mut v = v;
    loop {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    o.push_str(std::str::from_utf8(&buf[i..]).expect("decimal digits"));
}

pub(crate) fn write_i64(o: &mut String, v: i64) {
    if v < 0 {
        o.push('-');
        write_u64(o, v.unsigned_abs());
    } else {
        write_u64(o, v as u64);
    }
}

/// Write member `member`'s wire name, or its plain name when no wire naming
/// is active for this encode (checked once per record or enum).
fn write_member_name(ty: &str, member: &str, o: &mut String, wire: bool) {
    if wire {
        with_name(ty, member, |name| write_json_quote(name, o));
    } else {
        write_json_quote(member, o);
    }
}

fn enc(d: &TypeDesc, w: u64, o: &mut String) {
    if matches!(d.kind, Kind::Dynamic) && w != 0 {
        return enc(unsafe { obj(w).desc() }, w, o);
    }
    enc_deeper();
    enc_at(d, w, o);
    ENC_DEPTH.with(|c| c.set(c.get() - 1));
}

fn enc_at(d: &TypeDesc, w: u64, o: &mut String) {
    match &d.kind {
        Kind::Unit => o.push_str("null"),
        Kind::Bool => o.push_str(if w & 1 != 0 { "true" } else { "false" }),
        Kind::Int { signed: false, bits } => {
            let m = if *bits == 64 { u64::MAX } else { (1u64 << bits) - 1 };
            write_u64(o, w & m)
        }
        Kind::Int { .. } => write_i64(o, crate::value::as_i64(d, w)),
        Kind::Float { bits } => {
            // an F32 encodes its own shortest digits, not the widened ones
            let f = if *bits == 32 {
                f32::from_bits(w as u32).to_string().parse::<f64>().unwrap_or(f64::NAN)
            } else {
                crate::value::as_f64(d, w)
            };
            if f.is_finite() {
                o.push_str(&serde_json::Number::from_f64(f).map(|n| n.to_string()).unwrap_or("null".into()))
            } else {
                o.push_str("null")
            }
        }
        Kind::Char => {
            // encode into a stack buffer: a `String` per character is an
            // allocation for a value that is at most four bytes
            let mut buf = [0u8; 4];
            let s: &str = match char::from_u32(w as u32) {
                Some(c) => c.encode_utf8(&mut buf),
                None => "",
            };
            write_json_quote(s, o)
        }
        Kind::Symbol | Kind::String => write_json_quote(str_of(w), o),
        Kind::Bytes => {
            let text = base64::engine::general_purpose::STANDARD.encode(bytes_of(w));
            // a statement parameter keeps its kind: the driver binds a blob
            if SQL_BYTES.with(|c| c.get()) {
                o.push_str("{\"$bytes\":");
                write_json_quote(&text, o);
                o.push('}');
            } else {
                write_json_quote(&text, o);
            }
        }
        Kind::Duration | Kind::Instant | Kind::Timestamp => write_i64(o, w as i64),
        Kind::Record { fields, .. } if d.name == SECRET => {
            if REVEAL.with(|c| c.get()) {
                let p = unsafe { obj(w).payload() };
                enc(fields[0].desc, unsafe { fields[0].repr.load(p.add(fields[0].offset as usize)) }, o)
            } else {
                o.push_str("\"[redacted]\"")
            }
        }
        // `@flatten`: the one field's value
        Kind::Record { fields, flat: true, .. } => {
            let p = unsafe { obj(w).payload() };
            enc(fields[0].desc, unsafe { fields[0].repr.load(p.add(fields[0].offset as usize)) }, o)
        }
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
                // One wire check for the whole record instead of one per field.
                let wire = crate::codec::wire_active();
                for (i, f) in fields.iter().enumerate() {
                    if i > 0 {
                        o.push(',');
                    }
                    write_member_name(&d.name, &f.name, o, wire);
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
                crate::intrinsics_io::write_data(w, o);
                return;
            }
            if d.name == "Option" {
                if v.fields.is_empty() {
                    o.push_str("null");
                } else if nullable(v.fields[0].desc) {
                    o.push('[');
                    enc(v.fields[0].desc, vals[0], o);
                    o.push(']');
                } else {
                    enc(v.fields[0].desc, vals[0], o);
                }
                return;
            }
            let wire = crate::codec::wire_active();
            if v.fields.is_empty() {
                write_member_name(&d.name, &v.name, o, wire);
                return;
            }
            o.push('{');
            write_member_name(&d.name, &v.name, o, wire);
            o.push_str(":{");
            for (i, (f, val)) in v.fields.iter().zip(vals).enumerate() {
                if i > 0 {
                    o.push(',');
                }
                if wire {
                    with_name(&format!("{}.{}", d.name, v.name), &f.name, |name| write_json_quote(name, o));
                } else {
                    write_json_quote(&f.name, o);
                }
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
                write_json_quote(&key_text(key, k), o);
                o.push(':');
                enc(val, v, o);
            }
            o.push('}');
        }
        Kind::Set { elem } => {
            o.push('[');
            // in order, as a map's keys are (the encoding is deterministic)
            let mut es = map::entries(w);
            es.sort_by(|a, b| crate::value::cmp(elem, a.0, b.0));
            for (i, (k, _)) in es.into_iter().enumerate() {
                if i > 0 {
                    o.push(',');
                }
                enc(elem, k, o);
            }
            o.push(']');
        }
        Kind::Opaque { .. } if crate::buffer::array_elem(d).is_some() => {
            let elem = crate::buffer::array_elem(d).expect("array");
            o.push('[');
            for (i, x) in crate::buffer::items(w).iter().enumerate() {
                if i > 0 {
                    o.push(',');
                }
                enc(elem, x, o);
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

impl DecodeError {
    // Child errors are relative to their own root. Prepend segments only
    // while unwinding a failure; successful decoding builds no path strings.
    pub(crate) fn at(self, segment: std::fmt::Arguments<'_>) -> Self {
        Self { path: format!("${segment}{}", &self.path[1..]), message: self.message }
    }

    pub(crate) fn name(self, name: &str) -> Self {
        self.at(format_args!(".{name}"))
    }

    pub(crate) fn field(self, ty: &str, member: &str) -> Self {
        with_name(ty, member, |name| self.name(name))
    }

    pub(crate) fn index(self, i: usize) -> Self {
        self.at(format_args!("[{i}]"))
    }
}

fn fail<T>(msg: impl Into<String>) -> Result<T, DecodeError> {
    Err(DecodeError { path: "$".into(), message: msg.into() })
}

/// Decode JSON text against a descriptor.
pub fn decode(d: &'static TypeDesc, text: &str) -> Result<u64, DecodeError> {
    crate::json::decode(d, text)
}

/// Decode an already-parsed JSON value against a descriptor, without the text
/// round trip (used where the value is built from typed data, not parsed).
pub(crate) fn decode_value(d: &'static TypeDesc, j: &J) -> Result<u64, DecodeError> {
    let _g = Roots::new();
    dec(d, j)
}

/// Decode a `Data` value against a descriptor.
pub fn from_data(d: &'static TypeDesc, w: u64) -> Result<u64, DecodeError> {
    // `w` (and everything reachable through it) must stay alive: the walk
    // allocates while it holds words read out of the input.
    let _g = Roots::new();
    _g.add(w);
    dec_data(d, w)
}

thread_local! {
    /// Decoding a SQLite row: SQLite has no boolean type, so `Bool` also
    /// accepts the integers 0 and 1 (as `badger check` allows for INTEGER).
    static INT_BOOLS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

thread_local! {
    /// Encoding statement parameters: `Bytes` is tagged (`{"$bytes": ..}`)
    /// so the driver binds a blob, not text.
    static SQL_BYTES: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Statement parameters as JSON text, `Bytes` tagged for the driver.
pub fn encode_sql_params(d: &TypeDesc, w: u64) -> String {
    let outer = SQL_BYTES.with(|c| c.replace(true));
    let s = encode(d, w);
    SQL_BYTES.with(|c| c.set(outer));
    s
}

/// `Bytes` from its base64 text. A row's column comes as base64 when it is a
/// blob; a text column read into `Bytes` is its UTF-8 bytes.
fn bytes_from_text(s: &str) -> Result<u64, DecodeError> {
    match base64::engine::general_purpose::STANDARD.decode(s) {
        Ok(b) => Ok(root(crate::string::new_bytes(&b))),
        Err(_) if INT_BOOLS.with(|c| c.get()) => Ok(root(crate::string::new_bytes(s.as_bytes()))),
        Err(_) => fail("expected base64 text"),
    }
}

/// Decode a SQLite row (`Data`) against a descriptor.
pub fn from_sql_row(d: &'static TypeDesc, w: u64) -> Result<u64, DecodeError> {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            INT_BOOLS.with(|c| c.set(false));
        }
    }
    INT_BOOLS.with(|c| c.set(true));
    let _r = Reset;
    from_data(d, w)
}

fn root(w: u64) -> u64 {
    gc::push_root(w);
    w
}

/// Can a value of this type encode as `null`? An `Option` of one encodes
/// `some(v)` as `[v]`, so `some(none)` and `none` stay apart.
pub(crate) fn nullable(d: &TypeDesc) -> bool {
    match &d.kind {
        Kind::Unit => true,
        Kind::Enum { .. } => d.name == "Option" || d.name == "Data",
        Kind::Record { fields, flat: true, .. } if fields.len() == 1 => nullable(fields[0].desc),
        _ => false,
    }
}

/// Can a field of this type be left out? An `Option` (absent is `none`), or
/// a `@flatten` struct whose one field can (absent wraps that field's).
pub(crate) fn omittable(d: &TypeDesc) -> bool {
    match &d.kind {
        Kind::Enum { .. } => d.name == "Option",
        Kind::Record { fields, flat: true, .. } if fields.len() == 1 => omittable(fields[0].desc),
        _ => false,
    }
}

/// The value of a left-out field of an [`omittable`] type (rooted).
pub(crate) fn absent(d: &'static TypeDesc) -> u64 {
    match &d.kind {
        Kind::Record { fields, size, flat: true, .. } if fields.len() == 1 => {
            let v = absent(fields[0].desc);
            flat_record(d, *size, &fields[0], v)
        }
        _ => root(crate::make_enum(d, 0, &[])),
    }
}

/// A `@flatten` struct holding `v` as its one field.
pub(crate) fn flat_record(d: &'static TypeDesc, size: u32, field: &crate::desc::Field, v: u64) -> u64 {
    if field.desc.is_ptr() {
        root(v);
    }
    let w = root(gc::alloc(d, size as usize));
    unsafe { field.repr.store(obj(w).payload().add(field.offset as usize), v) };
    w
}

fn dec(d: &'static TypeDesc, j: &J) -> Result<u64, DecodeError> {
    match &d.kind {
        Kind::Unit => Ok(0),
        Kind::Bool => match j {
            J::Bool(b) => Ok(*b as u64),
            J::Number(n)
                if INT_BOOLS.with(|c| c.get()) && (n.as_i64() == Some(0) || n.as_i64() == Some(1)) =>
            {
                Ok(n.as_i64().unwrap() as u64)
            }
            _ => fail("expected a boolean"),
        },
        Kind::Int { bits, signed } => {
            let v = if *signed { j.as_i64() } else { j.as_u64().map(|u| u as i64) };
            let Some(v) = v else { return fail("expected an integer") };
            let ok = match (bits, signed) {
                (64, _) => true,
                (b, true) => v >= -(1i64 << (b - 1)) && v < (1i64 << (b - 1)),
                (b, false) => v >= 0 && v < (1i64 << b),
            };
            if !ok {
                return fail(format!("integer out of range for {}", d.name));
            }
            Ok(v as u64)
        }
        Kind::Float { bits } => {
            let Some(f) = j.as_f64() else { return fail("expected a number") };
            Ok(if *bits == 32 { (f as f32).to_bits() as u64 } else { f.to_bits() })
        }
        Kind::String => match j.as_str() {
            Some(s) => Ok(root(new_string(s))),
            None => fail("expected a string"),
        },
        Kind::Symbol => match j.as_str() {
            Some(s) => crate::string::external_symbol(s).or_else(|m| fail(&m)),
            None => fail("expected a symbol string"),
        },
        Kind::Char => match j.as_str().and_then(|s| s.chars().next()) {
            Some(c) => Ok(c as u64),
            None => fail("expected a character"),
        },
        Kind::Bytes => match j.as_str() {
            Some(s) => bytes_from_text(s),
            None => fail("expected a string (base64)"),
        },
        Kind::Duration | Kind::Instant | Kind::Timestamp => {
            j.as_i64().map(|v| v as u64).ok_or(()).or_else(|_| fail("expected an integer"))
        }
        Kind::Record { fields, size, .. } if d.name == SECRET => {
            let Some(text) = j.as_str() else { return fail("expected a string") };
            let v = root(new_string(text));
            let w = root(gc::alloc(d, *size as usize));
            unsafe { fields[0].repr.store(obj(w).payload().add(fields[0].offset as usize), v) };
            Ok(w)
        }
        Kind::Record { fields, size, flat: true, .. } => {
            let v = dec(fields[0].desc, j)?;
            Ok(flat_record(d, *size, &fields[0], v))
        }
        Kind::Record { fields, tuple, size, .. } => {
            let mut vals = Vec::with_capacity(fields.len());
            if *tuple {
                let Some(a) = j.as_array() else { return fail("expected an array") };
                if a.len() != fields.len() {
                    return fail(format!("expected {} elements", fields.len()));
                }
                for (i, f) in fields.iter().enumerate() {
                    vals.push(dec(f.desc, &a[i]).map_err(|e| e.index(i))?);
                }
            } else {
                let Some(m) = j.as_object() else { return fail("expected an object") };
                for f in fields {
                    let value = match lookup(m, &d.name, &f.name) {
                        Some(v) => dec(f.desc, v),
                        None if omittable(f.desc) => Ok(absent(f.desc)),
                        None => fail("missing required field"),
                    };
                    vals.push(value.map_err(|e| e.field(&d.name, &f.name))?);
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
                let p = variants[1].fields[0].desc;
                let v = if nullable(p) {
                    match j.as_array().map(|a| a.as_slice()) {
                        Some([x]) => dec(p, x)?,
                        _ => return fail("expected `null` or a one-element array (`[value]`)"),
                    }
                } else {
                    dec(p, j)?
                };
                return Ok(root(make_enum(d, 1, &[v])));
            }
            if let Some(s) = j.as_str() {
                if let Some(i) =
                    variants.iter().position(|v| name_matches(&d.name, &v.name, s) && v.fields.is_empty())
                {
                    return Ok(root(make_enum(d, i as u32, &[])));
                }
                return fail(format!("unknown variant `{s}`"));
            }
            let Some(m) = j.as_object().filter(|m| m.len() == 1) else {
                return fail("expected a variant");
            };
            let (name, body) = m.iter().next().unwrap();
            let Some(i) = variants.iter().position(|v| name_matches(&d.name, &v.name, name)) else {
                return fail(format!("unknown variant `{name}`"));
            };
            let mut vals = Vec::with_capacity(variants[i].fields.len());
            let vty = format!("{}.{}", d.name, variants[i].name);
            for f in &variants[i].fields {
                let Some(body) = body.as_object() else {
                    return fail("expected an object").map_err(|e| e.name(name));
                };
                let value = match lookup(body, &vty, &f.name) {
                    Some(v) => dec(f.desc, v),
                    None => fail("missing required field"),
                };
                vals.push(value.map_err(|e| e.field(&vty, &f.name).name(name))?);
            }
            Ok(root(make_enum(d, i as u32, &vals)))
        }
        Kind::List { elem } => {
            let Some(a) = j.as_array() else { return fail("expected an array") };
            let l = root(list::new(d));
            for (i, x) in a.iter().enumerate() {
                let v = dec(elem, x).map_err(|e| e.index(i))?;
                list::push(l, v);
            }
            Ok(l)
        }
        Kind::Set { elem } => {
            let Some(a) = j.as_array() else { return fail("expected an array") };
            let out = root(map::new(d));
            for (i, x) in a.iter().enumerate() {
                let v = dec(elem, x).map_err(|e| e.index(i))?;
                map::insert_mut(out, v, 0);
            }
            Ok(out)
        }
        Kind::Map { key, val } => {
            let Some(m) = j.as_object() else { return fail("expected an object") };
            let out = root(map::new(d));
            for (k, v) in m {
                let kw = key_value(key, k).map_err(|e| e.name(k))?;
                let vw = dec(val, v).map_err(|e| e.name(k))?;
                map::insert_mut(out, kw, vw);
            }
            Ok(out)
        }
        Kind::Opaque { .. } if crate::buffer::array_elem(d).is_some() => {
            let elem = crate::buffer::array_elem(d).expect("array");
            let Some(a) = j.as_array() else { return fail("expected an array") };
            // decoded elements stay rooted (`root`) until the array holds them
            let mut items = Vec::with_capacity(a.len());
            for (i, x) in a.iter().enumerate() {
                let v = dec(elem, x).map_err(|e| e.index(i))?;
                if elem.is_ptr() {
                    gc::push_root(v);
                }
                items.push(v);
            }
            Ok(root(crate::buffer::frozen(elem, items)))
        }
        _ => fail(format!("type `{}` has no decoder", d.name)),
    }
}

/// Decode a `Data` value against a descriptor. `dec` over a
/// `serde_json::Value` is the reference; this mirrors it kind by kind so a
/// `Data` input needs no intermediate JSON value (no `String`/`Vec`/map per
/// node). Error paths and messages are identical.
fn dec_data(d: &'static TypeDesc, w: u64) -> Result<u64, DecodeError> {
    // `Unit` ignores its input; `Data` is already the target value.
    if matches!(d.kind, Kind::Unit) {
        return Ok(0);
    }
    if d.name == "Data" && matches!(d.kind, Kind::Enum { .. }) {
        return Ok(w);
    }
    let (tag, fields) = enum_parts(w);
    match &d.kind {
        Kind::Unit => Ok(0),
        Kind::Bool => match tag {
            1 => Ok(fields[0] & 1),
            2 if INT_BOOLS.with(|c| c.get()) && (fields[0] as i64 == 0 || fields[0] as i64 == 1) => {
                Ok(fields[0] as i64 as u64)
            }
            _ => fail("expected a boolean"),
        },
        Kind::Int { bits, signed } => {
            // a JSON number: an integer only (no float), and non-negative for
            // an unsigned type
            let v = match tag {
                2 if *signed || fields[0] as i64 >= 0 => fields[0] as i64,
                _ => return fail("expected an integer"),
            };
            let ok = match (bits, signed) {
                (64, _) => true,
                (b, true) => v >= -(1i64 << (b - 1)) && v < (1i64 << (b - 1)),
                (b, false) => v >= 0 && v < (1i64 << b),
            };
            if !ok {
                return fail(format!("integer out of range for {}", d.name));
            }
            Ok(v as u64)
        }
        Kind::Float { bits } => {
            let f = match tag {
                2 => fields[0] as i64 as f64,
                3 => f64::from_bits(fields[0]),
                _ => return fail("expected a number"),
            };
            Ok(if *bits == 32 { (f as f32).to_bits() as u64 } else { f.to_bits() })
        }
        Kind::String => match tag {
            4 => Ok(root(new_string(str_of(fields[0])))),
            _ => fail("expected a string"),
        },
        Kind::Symbol => match tag {
            4 => crate::string::external_symbol(str_of(fields[0])).or_else(|m| fail(&m)),
            _ => fail("expected a symbol string"),
        },
        Kind::Char => match tag {
            4 => match str_of(fields[0]).chars().next() {
                Some(c) => Ok(c as u64),
                None => fail("expected a character"),
            },
            _ => fail("expected a character"),
        },
        Kind::Bytes => match tag {
            4 => bytes_from_text(str_of(fields[0])),
            _ => fail("expected a string (base64)"),
        },
        Kind::Duration | Kind::Instant | Kind::Timestamp => match tag {
            2 => Ok(fields[0]),
            _ => fail("expected an integer"),
        },
        Kind::Record { fields: rfields, size, .. } if d.name == SECRET => {
            if tag != 4 {
                return fail("expected a string");
            }
            let v = root(new_string(str_of(fields[0])));
            let w = root(gc::alloc(d, *size as usize));
            unsafe { rfields[0].repr.store(obj(w).payload().add(rfields[0].offset as usize), v) };
            Ok(w)
        }
        Kind::Record { fields: rfields, size, flat: true, .. } => {
            let v = dec_data(rfields[0].desc, w)?;
            Ok(flat_record(d, *size, &rfields[0], v))
        }
        Kind::Record { fields: rfields, tuple, size, .. } => {
            let mut vals = Vec::with_capacity(rfields.len());
            if *tuple {
                if tag != 5 {
                    return fail("expected an array");
                }
                let l = fields[0];
                if list::len(l) != rfields.len() {
                    return fail(format!("expected {} elements", rfields.len()));
                }
                for (i, f) in rfields.iter().enumerate() {
                    vals.push(dec_data(f.desc, list::get(l, i)).map_err(|e| e.index(i))?);
                }
            } else {
                if tag != 6 {
                    return fail("expected an object");
                }
                // one entry scan reused for every field (the entries are
                // reachable from `w`, which the caller keeps rooted)
                let entries = map::entries(fields[0]);
                for f in rfields {
                    let value = match lookup_data(&entries, &d.name, &f.name) {
                        Some(v) => dec_data(f.desc, v),
                        None if omittable(f.desc) => Ok(absent(f.desc)),
                        None => fail("missing required field"),
                    };
                    vals.push(value.map_err(|e| e.field(&d.name, &f.name))?);
                }
            }
            let w = root(gc::alloc(d, *size as usize));
            let pl = unsafe { obj(w).payload() };
            for (f, v) in rfields.iter().zip(vals) {
                unsafe { f.repr.store(pl.add(f.offset as usize), v) };
            }
            Ok(w)
        }
        Kind::Enum { variants, .. } => {
            if d.name == "Option" {
                if tag == 0 {
                    return Ok(root(make_enum(d, 0, &[])));
                }
                let p = variants[1].fields[0].desc;
                let v = if nullable(p) {
                    if tag != 5 || list::len(fields[0]) != 1 {
                        return fail("expected `null` or a one-element array (`[value]`)");
                    }
                    dec_data(p, list::get(fields[0], 0))?
                } else {
                    dec_data(p, w)?
                };
                return Ok(root(make_enum(d, 1, &[v])));
            }
            if tag == 4 {
                let s = str_of(fields[0]);
                if let Some(i) =
                    variants.iter().position(|v| name_matches(&d.name, &v.name, s) && v.fields.is_empty())
                {
                    return Ok(root(make_enum(d, i as u32, &[])));
                }
                return fail(format!("unknown variant `{s}`"));
            }
            if tag != 6 {
                return fail("expected a variant");
            }
            let entries = map::entries(fields[0]);
            if entries.len() != 1 {
                return fail("expected a variant");
            }
            let (k, body) = entries[0];
            let name = str_of(k).to_string();
            let Some(i) = variants.iter().position(|v| name_matches(&d.name, &v.name, &name)) else {
                return fail(format!("unknown variant `{name}`"));
            };
            let (btag, bfields) = enum_parts(body);
            let body_entries = if btag == 6 { map::entries(bfields[0]) } else { Vec::new() };
            let mut vals = Vec::with_capacity(variants[i].fields.len());
            let vty = format!("{}.{}", d.name, variants[i].name);
            for f in &variants[i].fields {
                if btag != 6 {
                    return fail("expected an object").map_err(|e| e.name(&name));
                }
                let value = match lookup_data(&body_entries, &vty, &f.name) {
                    Some(v) => dec_data(f.desc, v),
                    None => fail("missing required field"),
                };
                vals.push(value.map_err(|e| e.field(&vty, &f.name).name(&name))?);
            }
            Ok(root(make_enum(d, i as u32, &vals)))
        }
        Kind::List { elem } => {
            if tag != 5 {
                return fail("expected an array");
            }
            let l = fields[0];
            let out = root(list::new(d));
            for i in 0..list::len(l) {
                let v = dec_data(elem, list::get(l, i)).map_err(|e| e.index(i))?;
                list::push(out, v);
            }
            Ok(out)
        }
        Kind::Set { elem } => {
            if tag != 5 {
                return fail("expected an array");
            }
            let l = fields[0];
            let out = root(map::new(d));
            for i in 0..list::len(l) {
                let v = dec_data(elem, list::get(l, i)).map_err(|e| e.index(i))?;
                map::insert_mut(out, v, 0);
            }
            Ok(out)
        }
        Kind::Map { key, val } => {
            if tag != 6 {
                return fail("expected an object");
            }
            let out = root(map::new(d));
            for (k, v) in map::entries(fields[0]) {
                let ks = str_of(k);
                let kw = key_value(key, ks).map_err(|e| e.name(ks))?;
                let vw = dec_data(val, v).map_err(|e| e.name(ks))?;
                map::insert_mut(out, kw, vw);
            }
            Ok(out)
        }
        Kind::Opaque { .. } if crate::buffer::array_elem(d).is_some() => {
            let elem = crate::buffer::array_elem(d).expect("array");
            if tag != 5 {
                return fail("expected an array");
            }
            let l = fields[0];
            // decoded elements stay rooted (`root`) until the array holds them
            let mut items = Vec::with_capacity(list::len(l));
            for i in 0..list::len(l) {
                let v = dec_data(elem, list::get(l, i)).map_err(|e| e.index(i))?;
                if elem.is_ptr() {
                    gc::push_root(v);
                }
                items.push(v);
            }
            Ok(root(crate::buffer::frozen(elem, items)))
        }
        _ => fail(format!("type `{}` has no decoder", d.name)),
    }
}

/// Look up member `member` of `ty` among a `Data` object's `entries`,
/// mirroring [`lookup`]'s wire-name precedence.
fn lookup_data(entries: &[(u64, u64)], ty: &str, member: &str) -> Option<u64> {
    let find = |name: &str| entries.iter().find(|e| str_of(e.0) == name).map(|e| e.1);
    WIRE.with(|c| {
        let wire = c.borrow();
        let Some(w) = &*wire else { return find(member) };
        let key = format!("{ty}.{member}");
        find(w.names.get(&key).map_or(member, String::as_str))
            .or_else(|| w.aliases.get(&key).and_then(|aliases| aliases.iter().find_map(|a| find(a))))
    })
}

/// Decode a `Data` object key (a managed string) against `d`, mirroring
/// `dec(d, &J::String(s))` for the types that can be map keys. Other key
/// types keep `dec`'s exact errors through the JSON path.
/// A map key as its object member name: the key's own encoding, unquoted
/// when that is a string (`"red"` is `red`, `1` is `1`, `[1,"a"]` as is), so
/// it decodes back with [`key_value`] whatever the key type.
pub(crate) fn key_text(key: &TypeDesc, k: u64) -> String {
    let mut t = String::new();
    enc(key, k, &mut t);
    if t.starts_with('"') { serde_json::from_str::<String>(&t).unwrap_or(t) } else { t }
}

/// Does a key type always encode as a JSON string (its member name is the
/// string itself, never JSON text)?
fn key_is_text(d: &TypeDesc) -> bool {
    match &d.kind {
        Kind::String | Kind::Symbol | Kind::Char | Kind::Bytes => true,
        Kind::Enum { variants, .. } => {
            d.name == "Data" || (d.name != "Option" && variants.iter().all(|v| v.fields.is_empty()))
        }
        Kind::Record { fields, flat: true, .. } if fields.len() == 1 => key_is_text(fields[0].desc),
        _ => false,
    }
}

/// The key named `name` in an encoded map (see [`key_text`]).
pub(crate) fn key_value(key: &'static TypeDesc, name: &str) -> Result<u64, DecodeError> {
    if key_is_text(key) {
        return dec_str_key(key, name);
    }
    // JSON text (`1`, `true`, `[1,"a"]`), or a string variant of a mixed enum
    match serde_json::from_str::<J>(name) {
        Ok(j) => dec(key, &j),
        Err(_) => dec(key, &J::String(name.to_owned())),
    }
}

fn dec_str_key(d: &'static TypeDesc, s: &str) -> Result<u64, DecodeError> {
    match &d.kind {
        Kind::String => Ok(root(new_string(s))),
        Kind::Symbol => crate::string::external_symbol(s).or_else(|m| fail(&m)),
        Kind::Char => match s.chars().next() {
            Some(c) => Ok(c as u64),
            None => fail("expected a character"),
        },
        Kind::Bytes => bytes_from_text(s),
        Kind::Enum { .. } if d.name == "Data" => {
            let w = root(new_string(s));
            Ok(root(make_enum(d, 4, &[w])))
        }
        Kind::Enum { variants, .. } => {
            match variants.iter().position(|v| name_matches(&d.name, &v.name, s) && v.fields.is_empty()) {
                Some(i) => Ok(root(make_enum(d, i as u32, &[]))),
                None => fail(format!("unknown variant `{s}`")),
            }
        }
        _ => dec(d, &J::String(s.to_owned())),
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
