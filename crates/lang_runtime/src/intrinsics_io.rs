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
    use std::io::ErrorKind::*;
    match k {
        NotFound => "io/not_found",
        PermissionDenied => "io/permission_denied",
        AlreadyExists => "io/already_exists",
        InvalidData => "io/invalid_data",
        InvalidInput => "io/invalid_input",
        _ => "io/other",
    }
    .to_string()
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
    let r = if stderr != 0 {
        let mut e = std::io::stderr().lock();
        e.write_all(text.as_bytes()).and_then(|_| if newline & 1 != 0 { e.write_all(b"\n") } else { Ok(()) })
    } else {
        let mut o = std::io::stdout().lock();
        o.write_all(text.as_bytes())
            .and_then(|_| if newline & 1 != 0 { o.write_all(b"\n") } else { Ok(()) })
            .and_then(|_| o.flush())
    };
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

fn write_data(w: u64, out: &mut String) {
    let (tag, fields) = enum_parts(w);
    match tag {
        0 => out.push_str("null"),
        1 => out.push_str(if fields[0] & 1 != 0 { "true" } else { "false" }),
        2 => out.push_str(&(fields[0] as i64).to_string()),
        3 => out.push_str(&crate::value::fmt_float(f64::from_bits(fields[0]))),
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
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
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
    vec![("lang_rt_i_write_out", write_out as *const u8)]
}
