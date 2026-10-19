//! I/O and environment intrinsics, plus `Data` helpers.

use std::io::Write;

use crate::desc::TypeDesc;
use crate::intrinsics::{Targs, rdesc};
use crate::string::{new_string, str_of};
use crate::task::TaskCtx;
use crate::{enum_parts, make_record, make_variant};

/// `Result<T, io.Error>` construction helpers.
pub fn io_ok(ret: &'static TypeDesc, v: u64) -> u64 {
    make_variant(ret, "ok", &[v])
}

pub fn io_err(ret: &'static TypeDesc, e: &std::io::Error) -> u64 {
    io_err_msg(ret, &kind_name(e.kind()), &e.to_string())
}

pub fn io_err_msg(ret: &'static TypeDesc, kind: &str, msg: &str) -> u64 {
    let err_desc = ret.variant(1).map(|v| v.fields[0].desc).expect("Result error type");
    let _r = crate::gc::Roots::new();
    let k = crate::string::symbol(kind);
    let m = _r.add(new_string(msg));
    let e = _r.add(make_record(err_desc, &[("kind", k), ("message", m)]));
    make_variant(ret, "err", &[e])
}

/// An I/O error as (kind, message), as recorded in reproduction bundles.
pub fn io_pair(e: &std::io::Error) -> (String, String) {
    (kind_name(e.kind()), e.to_string())
}

/// `Result<T, io.Error>` from a recorded outcome.
pub fn io_result(ret: &'static TypeDesc, r: Result<u64, (String, String)>) -> u64 {
    match r {
        Ok(v) => io_ok(ret, v),
        Err((k, m)) => io_err_msg(ret, &k, &m),
    }
}

fn kind_name(k: std::io::ErrorKind) -> String {
    badger_native::io_error_kind(k).to_string()
}

pub extern "C" fn write_out(
    _ctx: *mut TaskCtx,
    _t: Targs,
    ret: *const TypeDesc,
    s: u64,
    newline: u64,
    stderr: u64,
) -> u64 {
    let text = str_of(s);
    if stderr == 0 {
        crate::repro::output(text, newline & 1 != 0);
    }
    // called without the runtime lock (several workers): write a copy
    // outside the collector's count, so a blocked or slow terminal holds up
    // neither other tasks nor a collection
    let unlocked = crate::task::multi() && !crate::task::holds_lock();
    let owned;
    let text = if unlocked {
        owned = text.to_string();
        crate::task::badger_rt_safe_begin(std::ptr::null_mut());
        owned.as_str()
    } else {
        text
    };
    let r = if stderr != 0 {
        let mut e = std::io::stderr().lock();
        e.write_all(text.as_bytes()).and_then(|_| if newline & 1 != 0 { e.write_all(b"\n") } else { Ok(()) })
    } else {
        let mut o = std::io::stdout().lock();
        o.write_all(text.as_bytes())
            .and_then(|_| if newline & 1 != 0 { o.write_all(b"\n") } else { Ok(()) })
            .and_then(|_| o.flush())
    };
    if unlocked {
        crate::task::badger_rt_safe_end(std::ptr::null_mut());
    }
    match r {
        Ok(()) => io_ok(rdesc(ret), 0),
        Err(e) => io_err(rdesc(ret), &e),
    }
}

/// Render a `Data` value as JSON text.
pub fn data_json(w: u64) -> String {
    let mut s = String::new();
    write_data(w, &mut s);
    s
}

/// A `Data` value as a JSON value, built directly (what `data_json`
/// writes, without the text in between).
pub fn data_value(w: u64) -> serde_json::Value {
    use serde_json::Value as J;
    let (tag, fields) = enum_parts(w);
    match tag {
        0 => J::Null,
        1 => J::Bool(fields[0] & 1 != 0),
        2 => J::from(fields[0] as i64),
        3 => {
            let f = f64::from_bits(fields[0]);
            serde_json::Number::from_f64(f).map(J::Number).unwrap_or(J::Null)
        }
        4 => J::String(str_of(fields[0]).to_string()),
        5 => J::Array(crate::list::to_vec(fields[0]).into_iter().map(data_value).collect()),
        _ => J::Object(
            crate::map::entries(fields[0])
                .into_iter()
                .map(|(k, v)| (str_of(k).to_string(), data_value(v)))
                .collect(),
        ),
    }
}

fn write_data(w: u64, out: &mut String) {
    let (tag, fields) = enum_parts(w);
    match tag {
        0 => out.push_str("null"),
        1 => out.push_str(if fields[0] & 1 != 0 { "true" } else { "false" }),
        2 => out.push_str(&(fields[0] as i64).to_string()),
        // JSON has no NaN or infinity: they are `null` (as `codec.encode` writes them)
        3 => {
            let f = f64::from_bits(fields[0]);
            out.push_str(&if f.is_finite() { crate::value::fmt_float(f) } else { "null".to_string() })
        }
        4 => out.push_str(&json_quote(str_of(fields[0]))),
        5 => {
            out.push('[');
            for (i, x) in crate::list::to_vec(fields[0]).into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_data(x, out);
            }
            out.push(']');
        }
        _ => {
            out.push('{');
            let mut entries = crate::map::entries(fields[0]);
            entries.sort_by(|a, b| str_of(a.0).cmp(str_of(b.0)));
            for (i, (k, v)) in entries.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&json_quote(str_of(k)));
                out.push(':');
                write_data(v, out);
            }
            out.push('}');
        }
    }
}

pub fn json_quote(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    write_json_quote(s, &mut o);
    o
}

/// Append into an existing encoder buffer without a temporary quoted string.
/// Scans bytes and copies unescaped spans whole, so the common case (letters,
/// digits and multi-byte UTF-8, which never needs a `\u`) is one `push_str`
/// per run rather than one push per character.
pub(crate) fn write_json_quote(s: &str, o: &mut String) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    o.push('"');
    let bytes = s.as_bytes();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' || b == b'\\' || b < 0x20 {
            if start < i {
                o.push_str(&s[start..i]);
            }
            match b {
                b'"' => o.push_str("\\\""),
                b'\\' => o.push_str("\\\\"),
                b'\n' => o.push_str("\\n"),
                b'\r' => o.push_str("\\r"),
                b'\t' => o.push_str("\\t"),
                _ => {
                    o.push_str("\\u00");
                    o.push(HEX[(b >> 4) as usize] as char);
                    o.push(HEX[(b & 0xf) as usize] as char);
                }
            }
            i += 1;
            start = i;
        } else {
            i += 1;
        }
    }
    if start < bytes.len() {
        o.push_str(&s[start..]);
    }
    o.push('"');
}

static ARGS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Program arguments (excluding the executable) for `env.args()`.
pub fn set_args(a: Vec<String>) {
    *ARGS.lock().unwrap() = a;
}

pub fn args() -> Vec<String> {
    ARGS.lock().unwrap().clone()
}

pub fn symbols() -> Vec<(&'static str, *const u8)> {
    vec![("badger_rt_i_write_out", write_out as *const u8)]
}
