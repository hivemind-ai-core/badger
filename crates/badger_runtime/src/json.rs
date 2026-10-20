//! Streaming, schema-directed JSON decoding.
//!
//! `codec::decode` used to build a `serde_json::Value` tree and then walk it;
//! this module reads the text directly and builds only the values the
//! descriptor asks for. It produces the same values and the same `$`-rooted
//! error paths and messages as the tree-walking decoder (pinned by
//! `tests/run/codec_paths` and the schema tests), so `codec::decode` uses it
//! for JSON text and keeps `dec` for `Data`-valued inputs.

use std::borrow::Cow;

use crate::codec::{DecodeError, int_bools, name_matches, wire_rank};
use crate::desc::{Field, Kind, TypeDesc};
use crate::gc::{self, Roots, obj};
use crate::string::{external_symbol, new_string};
use crate::{make_enum, map};

/// A decoding failure: a JSON syntax error (reported at the root as
/// `invalid JSON: ...`) or a schema/type error (with a relative path the
/// caller extends as it unwinds).
enum PErr {
    Json(String),
    Dec(DecodeError),
}

impl From<DecodeError> for PErr {
    fn from(e: DecodeError) -> Self {
        PErr::Dec(e)
    }
}

fn jerr<T>(msg: impl Into<String>) -> Result<T, PErr> {
    Err(PErr::Json(msg.into()))
}

/// Four hex digits of a `\uXXXX` escape at `*k`, advancing past them.
fn hex4_at(b: &[u8], k: &mut usize) -> Result<u32, PErr> {
    if *k + 4 > b.len() {
        return jerr("unexpected end of hex escape");
    }
    let mut v = 0u32;
    for _ in 0..4 {
        let d = (b[*k] as char).to_digit(16).ok_or_else(|| PErr::Json("invalid hex escape".into()))?;
        v = v * 16 + d;
        *k += 1;
    }
    Ok(v)
}

/// Decode the body of a JSON string (between the quotes) into `out`, which
/// must have room for at least `body.len()` bytes: an escape never decodes to
/// more bytes than it occupies. Returns the decoded length.
///
/// The result is valid UTF-8: copied runs start and end on ASCII bytes (`\` or
/// a run boundary), and escapes append complete encoded characters.
fn decode_body(body: &[u8], out: &mut [u8]) -> Result<usize, PErr> {
    let mut k = 0;
    let mut n = 0;
    while k < body.len() {
        let c = body[k];
        k += 1;
        if c != b'\\' {
            out[n] = c;
            n += 1;
            continue;
        }
        let e = *body.get(k).ok_or_else(|| PErr::Json("unexpected end of escape".into()))?;
        k += 1;
        match e {
            b'"' => {
                out[n] = b'"';
                n += 1;
            }
            b'\\' => {
                out[n] = b'\\';
                n += 1;
            }
            b'/' => {
                out[n] = b'/';
                n += 1;
            }
            b'b' => {
                out[n] = 0x08;
                n += 1;
            }
            b'f' => {
                out[n] = 0x0c;
                n += 1;
            }
            b'n' => {
                out[n] = b'\n';
                n += 1;
            }
            b'r' => {
                out[n] = b'\r';
                n += 1;
            }
            b't' => {
                out[n] = b'\t';
                n += 1;
            }
            b'u' => {
                let mut cp = hex4_at(body, &mut k)?;
                if (0xd800..0xdc00).contains(&cp) {
                    // a high surrogate must be followed by a low one
                    if body.get(k) != Some(&b'\\') {
                        return jerr("lone leading surrogate in hex escape");
                    }
                    k += 1;
                    if body.get(k) != Some(&b'u') {
                        return jerr("unexpected end of hex escape");
                    }
                    k += 1;
                    let lo = hex4_at(body, &mut k)?;
                    if !(0xdc00..0xe000).contains(&lo) {
                        return jerr("lone leading surrogate in hex escape");
                    }
                    cp = 0x10000 + ((cp - 0xd800) << 10) + (lo - 0xdc00);
                } else if (0xdc00..0xe000).contains(&cp) {
                    return jerr("lone trailing surrogate in hex escape");
                }
                let ch = char::from_u32(cp).ok_or_else(|| PErr::Json("invalid escape".into()))?;
                let mut buf = [0u8; 4];
                let enc = ch.encode_utf8(&mut buf).as_bytes();
                out[n..n + enc.len()].copy_from_slice(enc);
                n += enc.len();
            }
            _ => return jerr("invalid escape"),
        }
    }
    Ok(n)
}

/// A schema error at the current node (path `$` before unwinding).
fn derr<T>(msg: impl Into<String>) -> Result<T, PErr> {
    Err(PErr::Dec(DecodeError { path: "$".into(), message: msg.into() }))
}

/// A JSON number, classified as serde_json classifies it: an integral
/// literal that fits `u64`/`i64`, otherwise a float.
enum Num {
    U(u64),
    I(i64),
    F(f64),
}

impl Num {
    fn as_i64(&self) -> Option<i64> {
        match *self {
            Num::U(u) => i64::try_from(u).ok(),
            Num::I(i) => Some(i),
            Num::F(_) => None,
        }
    }
    fn as_u64(&self) -> Option<u64> {
        match *self {
            Num::U(u) => Some(u),
            Num::I(i) => u64::try_from(i).ok(),
            Num::F(_) => None,
        }
    }
    fn as_f64(&self) -> Option<f64> {
        match *self {
            Num::U(u) => Some(u as f64),
            Num::I(i) => Some(i as f64),
            Num::F(f) => Some(f),
        }
    }
}

struct P<'a> {
    text: &'a str,
    b: &'a [u8],
    i: usize,
}

impl<'a> P<'a> {
    fn new(text: &'a str) -> P<'a> {
        P { text, b: text.as_bytes(), i: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    /// Consume `c`, or fail with a schema error naming `what` (the shapes
    /// `dec` reports as type errors, not syntax errors).
    fn expect_dec(&mut self, c: u8, what: &str) -> Result<(), PErr> {
        self.ws();
        if self.peek() != Some(c) {
            return derr(format!("expected {what}"));
        }
        self.i += 1;
        Ok(())
    }

    fn literal(&mut self, lit: &[u8]) -> Result<(), PErr> {
        if self.b[self.i..].starts_with(lit) {
            self.i += lit.len();
            Ok(())
        } else {
            jerr("invalid literal")
        }
    }

    /// The extent of a JSON string token at the cursor: the body's byte range
    /// (between the quotes) and whether it contains escapes. Leaves the cursor
    /// after the closing quote. This scan is the hot path for every string
    /// token, including field names.
    fn string_body(&mut self) -> Result<(usize, usize, bool), PErr> {
        if self.peek() != Some(b'"') {
            return jerr("expected a string");
        }
        self.i += 1;
        let body_start = self.i;
        let rest = &self.b[self.i..];
        let mut j = 0;
        let mut escaped = false;
        loop {
            let stop = match rest[j..].iter().position(|&c| c == b'"' || c == b'\\' || c < 0x20) {
                Some(k) => j + k,
                None => return jerr("unexpected end of string"),
            };
            match rest[stop] {
                b'"' => {
                    j = stop;
                    break;
                }
                b'\\' => {
                    escaped = true;
                    j = stop + 2;
                    if j > rest.len() {
                        return jerr("unexpected end of string");
                    }
                }
                _ => return jerr("control character in string"),
            }
        }
        self.i = body_start + j + 1;
        Ok((body_start, body_start + j, escaped))
    }

    /// A JSON string. Borrows the input when it has no escapes (the common
    /// case, including every field name); otherwise unescapes into an owned
    /// string sized to the raw token.
    fn string(&mut self) -> Result<Cow<'a, str>, PErr> {
        let (s, e, escaped) = self.string_body()?;
        if !escaped {
            return Ok(Cow::Borrowed(&self.text[s..e]));
        }
        let mut out = vec![0u8; e - s];
        let n = decode_body(&self.b[s..e], &mut out)?;
        out.truncate(n);
        // SAFETY: see `decode_body`: the copied spans and escape encodings are
        // each valid UTF-8, so the concatenation is.
        Ok(Cow::Owned(unsafe { String::from_utf8_unchecked(out) }))
    }

    /// A managed string (`Kind::String`) or byte string (`Kind::Bytes`) cell
    /// holding the next JSON string's decoded bytes. Unescaped tokens are
    /// copied straight in; escaped ones decode through a small stack buffer,
    /// so the cell is exactly the decoded size and no native allocation is
    /// made for the common short string.
    fn managed_bytes(&mut self, desc: &'static TypeDesc) -> Result<u64, PErr> {
        let (s, e, escaped) = self.string_body()?;
        let body = &self.b[s..e];
        if !escaped {
            return Ok(root(crate::string::alloc_bytes(desc, body)));
        }
        let mut stack = [0u8; 128];
        if body.len() <= stack.len() {
            let n = decode_body(body, &mut stack)?;
            Ok(root(crate::string::alloc_bytes(desc, &stack[..n])))
        } else {
            let mut buf = vec![0u8; body.len()];
            let n = decode_body(body, &mut buf)?;
            Ok(root(crate::string::alloc_bytes(desc, &buf[..n])))
        }
    }

    fn number(&mut self) -> Result<Num, PErr> {
        let start = self.i;
        let neg = match self.peek() {
            Some(b'-') => {
                self.i += 1;
                true
            }
            _ => false,
        };
        let mut mag: u64 = 0;
        let mut overflow = false;
        let mut any = false;
        while let Some(c) = self.peek() {
            if !c.is_ascii_digit() {
                break;
            }
            any = true;
            match mag.checked_mul(10).and_then(|m| m.checked_add((c - b'0') as u64)) {
                Some(v) => mag = v,
                None => overflow = true,
            }
            self.i += 1;
        }
        if !any {
            return jerr("invalid number");
        }
        let mut float = overflow;
        if self.peek() == Some(b'.') {
            float = true;
            self.i += 1;
            let mut digits = false;
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    digits = true;
                    self.i += 1;
                } else {
                    break;
                }
            }
            if !digits {
                return jerr("invalid number");
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            float = true;
            self.i += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            let mut digits = false;
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    digits = true;
                    self.i += 1;
                } else {
                    break;
                }
            }
            if !digits {
                return jerr("invalid number");
            }
        }
        if float {
            let f: f64 = self.text[start..self.i].parse().map_err(|_| PErr::Json("invalid number".into()))?;
            return Ok(Num::F(f));
        }
        if neg {
            if mag <= i64::MAX as u64 {
                Ok(Num::I(-(mag as i64)))
            } else if mag == i64::MAX as u64 + 1 {
                Ok(Num::I(i64::MIN))
            } else {
                Ok(Num::F(-(mag as f64)))
            }
        } else {
            Ok(Num::U(mag))
        }
    }

    /// Consume any JSON value, discarding it (for keys and members that do
    /// not decode into anything).
    fn skip_value(&mut self) -> Result<(), PErr> {
        self.ws();
        match self.peek() {
            Some(b'"') => {
                self.string()?;
            }
            Some(b'{') => {
                self.i += 1;
                self.ws();
                if self.peek() == Some(b'}') {
                    self.i += 1;
                    return Ok(());
                }
                loop {
                    self.ws();
                    self.string()?;
                    self.ws();
                    if self.peek() != Some(b':') {
                        return jerr("expected a colon");
                    }
                    self.i += 1;
                    self.skip_value()?;
                    self.ws();
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            break;
                        }
                        _ => return jerr("expected `,` or `}`"),
                    }
                }
            }
            Some(b'[') => {
                self.i += 1;
                self.ws();
                if self.peek() == Some(b']') {
                    self.i += 1;
                    return Ok(());
                }
                loop {
                    self.skip_value()?;
                    self.ws();
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            break;
                        }
                        _ => return jerr("expected `,` or `]`"),
                    }
                }
            }
            Some(b't') => self.literal(b"true")?,
            Some(b'f') => self.literal(b"false")?,
            Some(b'n') => self.literal(b"null")?,
            Some(c) if c == b'-' || c.is_ascii_digit() => {
                self.number()?;
            }
            _ => return jerr("expected value"),
        }
        Ok(())
    }

    /// Decode the next value as `d`, mirroring `codec::dec`.
    fn value(&mut self, d: &'static TypeDesc) -> Result<u64, PErr> {
        self.ws();
        match &d.kind {
            Kind::Unit => {
                self.skip_value()?;
                Ok(0)
            }
            Kind::Bool => match self.peek() {
                Some(b't') => {
                    self.literal(b"true")?;
                    Ok(1)
                }
                Some(b'f') => {
                    self.literal(b"false")?;
                    Ok(0)
                }
                Some(c) if (c == b'-' || c.is_ascii_digit()) && int_bools() => {
                    let n = self.number()?;
                    match n.as_i64() {
                        Some(0) => Ok(0),
                        Some(1) => Ok(1),
                        _ => derr("expected a boolean"),
                    }
                }
                _ => derr("expected a boolean"),
            },
            Kind::Int { bits, signed } => {
                match self.peek() {
                    Some(c) if c == b'-' || c.is_ascii_digit() => {}
                    _ => return derr("expected an integer"),
                }
                let n = self.number()?;
                let v = if *signed { n.as_i64() } else { n.as_u64().map(|u| u as i64) };
                let Some(v) = v else { return derr("expected an integer") };
                let ok = match (bits, signed) {
                    (64, _) => true,
                    (b, true) => v >= -(1i64 << (b - 1)) && v < (1i64 << (b - 1)),
                    (b, false) => v >= 0 && v < (1i64 << b),
                };
                if !ok {
                    return derr(format!("integer out of range for {}", d.name));
                }
                Ok(v as u64)
            }
            Kind::Float { bits } => {
                match self.peek() {
                    Some(c) if c == b'-' || c.is_ascii_digit() => {}
                    _ => return derr("expected a number"),
                }
                let n = self.number()?;
                let Some(f) = n.as_f64() else { return derr("expected a number") };
                Ok(if *bits == 32 { (f as f32).to_bits() as u64 } else { f.to_bits() })
            }
            Kind::String => match self.peek() {
                Some(b'"') => self.managed_bytes(crate::desc::builtin::string()),
                _ => derr("expected a string"),
            },
            Kind::Symbol => match self.peek() {
                Some(b'"') => {
                    let s = self.string()?;
                    external_symbol(&s).or_else(derr)
                }
                _ => derr("expected a symbol string"),
            },
            Kind::Char => match self.peek() {
                Some(b'"') => {
                    let s = self.string()?;
                    match s.chars().next() {
                        Some(c) => Ok(c as u64),
                        None => derr("expected a character"),
                    }
                }
                _ => derr("expected a character"),
            },
            Kind::Bytes => match self.peek() {
                Some(b'"') => self.managed_bytes(crate::desc::builtin::bytes()),
                _ => derr("expected a string"),
            },
            Kind::Duration | Kind::Instant | Kind::Timestamp => {
                match self.peek() {
                    Some(c) if c == b'-' || c.is_ascii_digit() => {}
                    _ => return derr("expected an integer"),
                }
                let n = self.number()?;
                match n.as_i64() {
                    Some(v) => Ok(v as u64),
                    None => derr("expected an integer"),
                }
            }
            Kind::Record { fields, size, .. } if d.name == crate::codec::SECRET => {
                if self.peek() != Some(b'"') {
                    return derr("expected a string");
                }
                let text = self.string()?;
                let v = root(new_string(&text));
                let w = root(gc::alloc(d, *size as usize));
                unsafe { fields[0].repr.store(obj(w).payload().add(fields[0].offset as usize), v) };
                Ok(w)
            }
            Kind::Record { fields, size, flat: true, .. } => {
                let v = self.value(fields[0].desc)?;
                Ok(crate::codec::flat_record(d, *size, &fields[0], v))
            }
            Kind::Record { fields, tuple, size, .. } => {
                if *tuple {
                    let vals = self.tuple(fields)?;
                    Ok(record(d, *size, fields, vals))
                } else {
                    self.record(d, fields, *size)
                }
            }
            Kind::Enum { .. } if d.name == "Data" => Ok(root(self.data_value(d)?)),
            Kind::Enum { variants, .. } if d.name == "Option" => {
                if self.peek() == Some(b'n') {
                    self.literal(b"null")?;
                    return Ok(root(make_enum(d, 0, &[])));
                }
                let p = variants[1].fields[0].desc;
                if !crate::codec::nullable(p) {
                    let v = self.value(p)?;
                    return Ok(root(make_enum(d, 1, &[v])));
                }
                // `[value]`: a value that can be `null` itself
                self.expect_dec(b'[', "`null` or a one-element array (`[value]`)")?;
                self.ws();
                let v = self.value(p).map_err(|e| add_index(e, 0))?;
                self.ws();
                self.expect_dec(b']', "`]` after the one element")?;
                Ok(root(make_enum(d, 1, &[v])))
            }
            Kind::Enum { variants, .. } => self.enumeration(d, variants),
            Kind::List { elem } => {
                self.expect_dec(b'[', "an array")?;
                self.ws();
                let l = root(crate::list::new(d));
                if self.peek() == Some(b']') {
                    self.i += 1;
                    return Ok(l);
                }
                let mut i = 0usize;
                loop {
                    let v = self.value(elem).map_err(|e| add_index(e, i))?;
                    crate::list::push(l, v);
                    i += 1;
                    self.ws();
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            break;
                        }
                        _ => return jerr("expected `,` or `]`"),
                    }
                }
                Ok(l)
            }
            Kind::Map { key, val } => self.mapping(d, key, val),
            Kind::Set { elem } => {
                self.expect_dec(b'[', "an array")?;
                self.ws();
                let out = root(crate::map::new(d));
                if self.peek() == Some(b']') {
                    self.i += 1;
                    return Ok(out);
                }
                let mut i = 0usize;
                loop {
                    let v = self.value(elem).map_err(|e| add_index(e, i))?;
                    crate::map::insert_mut(out, v, 0);
                    i += 1;
                    self.ws();
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            break;
                        }
                        _ => return jerr("expected `,` or `]`"),
                    }
                }
                Ok(out)
            }
            Kind::Opaque { .. } if crate::buffer::array_elem(d).is_some() => {
                let elem = crate::buffer::array_elem(d).expect("array");
                self.expect_dec(b'[', "an array")?;
                self.ws();
                let mut items = Vec::new();
                if self.peek() == Some(b']') {
                    self.i += 1;
                } else {
                    let mut i = 0usize;
                    loop {
                        let v = self.value(elem).map_err(|e| add_index(e, i))?;
                        if elem.is_ptr() {
                            gc::push_root(v);
                        }
                        items.push(v);
                        i += 1;
                        self.ws();
                        match self.peek() {
                            Some(b',') => self.i += 1,
                            Some(b']') => {
                                self.i += 1;
                                break;
                            }
                            _ => return jerr("expected `,` or `]`"),
                        }
                    }
                }
                Ok(root(crate::buffer::frozen(elem, items)))
            }
            _ => derr(format!("type `{}` has no decoder", d.name)),
        }
    }

    /// A tuple: an array with exactly one element per field.
    fn tuple(&mut self, fields: &[Field]) -> Result<Vec<u64>, PErr> {
        self.expect_dec(b'[', "an array")?;
        let mut vals = Vec::with_capacity(fields.len());
        loop {
            self.ws();
            if self.peek() == Some(b']') {
                self.i += 1;
                break;
            }
            if !vals.is_empty() {
                if self.peek() != Some(b',') {
                    return jerr("expected `,` or `]`");
                }
                self.i += 1;
            }
            match fields.get(vals.len()) {
                Some(f) => {
                    let v = self.value(f.desc).map_err(|e| add_index(e, vals.len()))?;
                    vals.push(v);
                }
                None => self.skip_value()?,
            }
        }
        if vals.len() != fields.len() {
            return derr(format!("expected {} elements", fields.len()));
        }
        Ok(vals)
    }

    /// A named record: an object whose members select fields by wire name.
    fn record(&mut self, ty: &'static TypeDesc, fields: &[Field], size: u32) -> Result<u64, PErr> {
        let present = self.object(&ty.name, fields)?;
        record_value(ty, size, fields, present)
    }

    /// An enum: a bare variant name, `Data`, `Option`, or `{"variant": {...}}`.
    fn enumeration(&mut self, d: &'static TypeDesc, variants: &[crate::desc::Variant]) -> Result<u64, PErr> {
        let wire = crate::codec::wire_active();
        if self.peek() == Some(b'"') {
            let s = self.string()?;
            let hit = |v: &crate::desc::Variant| {
                (if wire { name_matches(&d.name, &v.name, &s) } else { v.name == *s }) && v.fields.is_empty()
            };
            if let Some(i) = variants.iter().position(hit) {
                return Ok(root(make_enum(d, i as u32, &[])));
            }
            return derr(format!("unknown variant `{s}`"));
        }
        if self.peek() != Some(b'{') {
            return derr("expected a variant");
        }
        self.i += 1;
        self.ws();
        if self.peek() == Some(b'}') {
            return derr("expected a variant");
        }
        let name = self.string()?.into_owned();
        self.ws();
        if self.peek() != Some(b':') {
            return jerr("expected a colon");
        }
        self.i += 1;
        // Confirm the object has exactly one member before decoding the body,
        // so an extra member reports "expected a variant" like the tree walk.
        let vstart = self.i;
        self.skip_value()?;
        self.ws();
        if self.peek() != Some(b'}') {
            return derr("expected a variant");
        }
        self.i = vstart;
        let Some(i) = variants
            .iter()
            .position(|v| if wire { name_matches(&d.name, &v.name, &name) } else { v.name == name })
        else {
            return derr(format!("unknown variant `{name}`"));
        };
        let vty = format!("{}.{}", d.name, variants[i].name);
        let vfields = &variants[i].fields;
        let present = self.object(&vty, vfields).map_err(|e| add_name(e, &name))?;
        let mut vals = Vec::with_capacity(vfields.len());
        for (idx, f) in vfields.iter().enumerate() {
            match present[idx] {
                Some((v, _)) => vals.push(v),
                None => {
                    return Err(PErr::Dec(
                        DecodeError { path: "$".into(), message: "missing required field".into() }
                            .field(&vty, &f.name)
                            .name(&name),
                    ));
                }
            }
        }
        self.ws();
        if self.peek() != Some(b'}') {
            return jerr("expected `}`");
        }
        self.i += 1;
        Ok(root(make_enum(d, i as u32, &vals)))
    }

    /// A map or set-like object: string keys decoded as the key type.
    fn mapping(
        &mut self,
        d: &'static TypeDesc,
        key: &'static TypeDesc,
        val: &'static TypeDesc,
    ) -> Result<u64, PErr> {
        self.expect_dec(b'{', "an object")?;
        self.ws();
        let out = root(map::new(d));
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(out);
        }
        loop {
            self.ws();
            let kstart = self.i;
            let k = self.string()?.into_owned();
            let kend = self.i;
            self.ws();
            if self.peek() != Some(b':') {
                return jerr("expected a colon");
            }
            self.i += 1;
            // A map key is decoded from its JSON string; reparse the key's own
            // text so its type sees the same input `dec` gave it.
            let kw = match P::new(&self.text[kstart..kend]).value(key) {
                Ok(v) => v,
                Err(PErr::Dec(e)) => return Err(PErr::Dec(e.name(&k))),
                Err(PErr::Json(m)) => {
                    return Err(PErr::Dec(
                        DecodeError { path: "$".into(), message: format!("invalid JSON: {m}") }.name(&k),
                    ));
                }
            };
            let vw = self.value(val).map_err(|e| add_name(e, &k))?;
            map::insert_mut(out, kw, vw);
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    break;
                }
                _ => return jerr("expected `,` or `}`"),
            }
        }
        Ok(out)
    }

    /// Decode an object's members into `fields`, returning each field's value
    /// and matched rank (or `None`). Duplicate or aliased keys keep
    /// `codec::lookup`'s precedence: the current wire name, then the first
    /// declared alias present.
    fn object(&mut self, ty: &str, fields: &[Field]) -> Result<Vec<Option<(u64, usize)>>, PErr> {
        self.expect_dec(b'{', "an object")?;
        // Value and rank share one slot: rank 0 is the current wire name, 1+
        // an alias position. One allocation per object, not three.
        let mut vals: Vec<Option<(u64, usize)>> = vec![None; fields.len()];
        // Wire naming is off for ordinary decoding: match names directly,
        // instead of a thread-local lookup per key per field.
        let wire = crate::codec::wire_active();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(vals);
        }
        loop {
            self.ws();
            let key = self.string()?;
            self.ws();
            if self.peek() != Some(b':') {
                return jerr("expected a colon");
            }
            self.i += 1;
            let found: Option<(usize, usize)> = if wire {
                let mut found = None;
                for (idx, f) in fields.iter().enumerate() {
                    if let Some(r) = wire_rank(ty, &f.name, &key)
                        && found.is_none_or(|(_, best)| r < best)
                    {
                        found = Some((idx, r));
                    }
                }
                found
            } else {
                fields.iter().position(|f| f.name == *key).map(|idx| (idx, 0))
            };
            match found {
                Some((idx, r)) => {
                    // A lower rank (current name, or an earlier alias) wins; an
                    // equal rank is a duplicate key, and the last one wins like
                    // the tree walk's map.
                    if vals[idx].is_none_or(|(_, best)| r <= best) {
                        let v =
                            self.value(fields[idx].desc).map_err(|e| add_field(e, ty, &fields[idx].name))?;
                        vals[idx] = Some((v, r));
                    } else {
                        self.skip_value()?;
                    }
                }
                None => self.skip_value()?,
            }
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    break;
                }
                _ => return jerr("expected `,` or `}`"),
            }
        }
        Ok(vals)
    }

    /// Build a `Data` value from the next JSON value, mirroring `data_of`.
    fn data_value(&mut self, d: &'static TypeDesc) -> Result<u64, PErr> {
        self.ws();
        match self.peek() {
            Some(b'n') => {
                self.literal(b"null")?;
                Ok(make_enum(d, 0, &[]))
            }
            Some(b't') => {
                self.literal(b"true")?;
                Ok(make_enum(d, 1, &[1]))
            }
            Some(b'f') => {
                self.literal(b"false")?;
                Ok(make_enum(d, 1, &[0]))
            }
            Some(c) if c == b'-' || c.is_ascii_digit() => {
                let n = self.number()?;
                match n.as_i64() {
                    Some(i) => Ok(make_enum(d, 2, &[i as u64])),
                    None => Ok(make_enum(d, 3, &[n.as_f64().unwrap_or(0.0).to_bits()])),
                }
            }
            Some(b'"') => {
                let s = self.string()?;
                let w = root(new_string(&s));
                Ok(make_enum(d, 4, &[w]))
            }
            Some(b'[') => {
                let list_desc = d.variant(5).unwrap().fields[0].desc;
                self.i += 1;
                self.ws();
                let l = root(crate::list::new(list_desc));
                if self.peek() == Some(b']') {
                    self.i += 1;
                } else {
                    loop {
                        let v = root(self.data_value(d)?);
                        crate::list::push(l, v);
                        self.ws();
                        match self.peek() {
                            Some(b',') => self.i += 1,
                            Some(b']') => {
                                self.i += 1;
                                break;
                            }
                            _ => return jerr("expected `,` or `]`"),
                        }
                    }
                }
                Ok(make_enum(d, 5, &[l]))
            }
            Some(b'{') => {
                let map_desc = d.variant(6).unwrap().fields[0].desc;
                self.i += 1;
                self.ws();
                let mw = root(map::new(map_desc));
                if self.peek() == Some(b'}') {
                    self.i += 1;
                } else {
                    loop {
                        self.ws();
                        let k = self.string()?;
                        self.ws();
                        if self.peek() != Some(b':') {
                            return jerr("expected a colon");
                        }
                        self.i += 1;
                        let kw = root(new_string(&k));
                        let vw = root(self.data_value(d)?);
                        map::insert_mut(mw, kw, vw);
                        self.ws();
                        match self.peek() {
                            Some(b',') => self.i += 1,
                            Some(b'}') => {
                                self.i += 1;
                                break;
                            }
                            _ => return jerr("expected `,` or `}`"),
                        }
                    }
                }
                Ok(make_enum(d, 6, &[mw]))
            }
            _ => jerr("expected value"),
        }
    }
}

/// Build a record from decoded member slots. A missing `Option` field is
/// `none` (what `dec` decodes from `null`); any other missing field is an
/// error, reported before the record is allocated.
fn record_value(
    ty: &'static TypeDesc,
    size: u32,
    fields: &[Field],
    present: Vec<Option<(u64, usize)>>,
) -> Result<u64, PErr> {
    for (f, p) in fields.iter().zip(&present) {
        if p.is_none() && !crate::codec::omittable(f.desc) {
            return Err(PErr::Dec(
                DecodeError { path: "$".into(), message: "missing required field".into() }
                    .field(&ty.name, &f.name),
            ));
        }
    }
    let w = root(gc::alloc(ty, size as usize));
    let pl = unsafe { obj(w).payload() };
    for (f, p) in fields.iter().zip(present) {
        let v = match p {
            Some((v, _)) => v,
            None => crate::codec::absent(f.desc),
        };
        unsafe { f.repr.store(pl.add(f.offset as usize), v) };
    }
    Ok(w)
}

/// Build a record from decoded field values in order (already rooted by their
/// decoders).
fn record(d: &'static TypeDesc, size: u32, fields: &[Field], vals: Vec<u64>) -> u64 {
    let w = root(gc::alloc(d, size as usize));
    let pl = unsafe { obj(w).payload() };
    for (f, v) in fields.iter().zip(vals) {
        unsafe { f.repr.store(pl.add(f.offset as usize), v) };
    }
    w
}

fn add_index(e: PErr, i: usize) -> PErr {
    match e {
        PErr::Dec(d) => PErr::Dec(d.index(i)),
        other => other,
    }
}

fn add_name(e: PErr, name: &str) -> PErr {
    match e {
        PErr::Dec(d) => PErr::Dec(d.name(name)),
        other => other,
    }
}

fn add_field(e: PErr, ty: &str, member: &str) -> PErr {
    match e {
        PErr::Dec(d) => PErr::Dec(d.field(ty, member)),
        other => other,
    }
}

fn root(w: u64) -> u64 {
    gc::push_root(w);
    w
}

/// Decode JSON text against a descriptor, building only the target value
/// (no intermediate `serde_json::Value`).
pub fn decode(d: &'static TypeDesc, text: &str) -> Result<u64, DecodeError> {
    let _g = Roots::new();
    let mut p = P::new(text);
    let v = match p.value(d) {
        Ok(v) => v,
        Err(PErr::Dec(e)) => return Err(e),
        Err(PErr::Json(m)) => {
            return Err(DecodeError { path: "$".into(), message: format!("invalid JSON: {m}") });
        }
    };
    p.ws();
    if p.i != p.b.len() {
        return Err(DecodeError { path: "$".into(), message: "invalid JSON: trailing characters".into() });
    }
    Ok(v)
}
