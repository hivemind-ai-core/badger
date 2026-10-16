//! Structured logs (`std.log`): levels, schema-encoded fields, per-task
//! context (inherited by spawned tasks), output to standard error as text
//! or JSON lines, and a bounded buffer an exporter (`std.otel`) drains.
//!
//! A disabled level costs one atomic load. Writing never fails or blocks
//! the caller beyond the stderr lock; when no exporter drains the buffer,
//! nothing is kept, and when one falls behind, records past the buffer's
//! capacity are dropped and counted.

use std::collections::VecDeque;
use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};

use crate::desc::TypeDesc;
use crate::intrinsics::{Targs, rdesc, targ};
use crate::string::{new_string, str_of};
use crate::task::{self, TaskCtx};

pub const TRACE: u8 = 0;
pub const DEBUG: u8 = 1;
pub const INFO: u8 = 2;
pub const WARN: u8 = 3;
pub const ERROR: u8 = 4;
pub const OFF: u8 = 5;

static LEVEL: AtomicU8 = AtomicU8::new(INFO);
/// 0: text, 1: JSON lines, 2: none (export only).
static FORMAT: AtomicU8 = AtomicU8::new(0);

const LEVELS: [&str; 5] = ["trace", "debug", "info", "warn", "error"];

pub fn level_of(name: &str) -> Option<u8> {
    match name {
        "off" => Some(OFF),
        n => LEVELS.iter().position(|l| *l == n).map(|i| i as u8),
    }
}

/// `[runtime] log_level` (`trace` .. `error`, `off`) and `log_format`
/// (`text`, `json`, `none`).
pub fn configure(key: &str, value: &str) -> bool {
    match key {
        "log_level" => match level_of(value) {
            Some(l) => LEVEL.store(l, Ordering::Relaxed),
            None => return false,
        },
        "log_format" => match value {
            "text" => FORMAT.store(0, Ordering::Relaxed),
            "json" => FORMAT.store(1, Ordering::Relaxed),
            "none" => FORMAT.store(2, Ordering::Relaxed),
            _ => return false,
        },
        _ => return false,
    }
    true
}

pub fn enabled(level: u8) -> bool {
    level >= LEVEL.load(Ordering::Relaxed)
}

struct Export {
    attached: bool,
    records: VecDeque<String>,
    capacity: usize,
    dropped: u64,
}

static EXPORT: Mutex<Export> =
    Mutex::new(Export { attached: false, records: VecDeque::new(), capacity: 8192, dropped: 0 });
static EMITTED: AtomicU64 = AtomicU64::new(0);

/// Nanoseconds since the Unix epoch.
pub fn now_nanos() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// RFC 3339 UTC with milliseconds (`2026-10-16T08:04:05.123Z`).
pub fn rfc3339(nanos: u64) -> String {
    let secs = (nanos / 1_000_000_000) as i64;
    let ms = (nanos / 1_000_000) % 1000;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // civil date from days since 1970-01-01 (Howard Hinnant's algorithm)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{ms:03}Z", rem / 3600, rem / 60 % 60, rem % 60)
}

/// One record's fields, in order: the context's (outermost first), then
/// the call's own; a later field replaces an earlier one of its name.
fn fields_of(own: &str) -> Vec<(String, serde_json::Value)> {
    let mut out: Vec<(String, serde_json::Value)> = vec![];
    let mut merge = |json: &str| {
        // the object's fields as written (not sorted)
        let mut de = serde_json::Deserializer::from_str(json);
        let Ok(fields) = <Ordered as serde::Deserialize>::deserialize(&mut de) else { return };
        for (k, v) in fields.0 {
            match out.iter_mut().find(|(x, _)| *x == k) {
                Some(slot) => slot.1 = v,
                None => out.push((k, v)),
            }
        }
    };
    if let Some(t) = task::current() {
        for c in &t.log_ctx {
            merge(c);
        }
    }
    merge(own);
    out
}

/// A JSON object's fields in their written order.
struct Ordered(Vec<(String, serde_json::Value)>);

impl<'de> serde::Deserialize<'de> for Ordered {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Ordered;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<Ordered, A::Error> {
                let mut out = vec![];
                while let Some((k, v)) = m.next_entry::<String, serde_json::Value>()? {
                    out.push((k, v));
                }
                Ok(Ordered(out))
            }
        }
        d.deserialize_map(V)
    }
}

fn json_line(fields: &[(&str, String)], rest: &[(String, serde_json::Value)]) -> String {
    let mut s = String::from("{");
    for (i, (k, v)) in fields.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&serde_json::Value::from(*k).to_string());
        s.push(':');
        s.push_str(v);
    }
    for (k, v) in rest {
        s.push(',');
        s.push_str(&serde_json::Value::from(k.as_str()).to_string());
        s.push(':');
        s.push_str(&v.to_string());
    }
    s.push('}');
    s
}

fn text_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) if !s.is_empty() && !s.contains([' ', '"', '=']) => s.clone(),
        other => other.to_string(),
    }
}

/// Write one record (already past the level check).
pub fn emit(level: u8, message: &str, fields_json: &str) {
    EMITTED.fetch_add(1, Ordering::Relaxed);
    let ts = now_nanos();
    let fields = fields_of(fields_json);
    let task = task::current().map(|t| t.id).unwrap_or(0);
    let lname = LEVELS.get(level as usize).copied().unwrap_or("info");
    let q = |s: &str| serde_json::Value::from(s).to_string();
    let head =
        [("ts", q(&rfc3339(ts))), ("level", q(lname)), ("msg", q(message)), ("task", task.to_string())];
    match FORMAT.load(Ordering::Relaxed) {
        0 => {
            let mut line = format!("{} {:<5} {message}", rfc3339(ts), lname.to_uppercase());
            for (k, v) in &fields {
                line.push(' ');
                line.push_str(k);
                line.push('=');
                line.push_str(&text_value(v));
            }
            let _ = writeln!(std::io::stderr().lock(), "{line}");
        }
        1 => {
            let _ = writeln!(std::io::stderr().lock(), "{}", json_line(&head, &fields));
        }
        _ => {}
    }
    let mut e = EXPORT.lock().unwrap();
    if e.attached {
        if e.records.len() >= e.capacity {
            e.records.pop_front();
            e.dropped += 1;
        }
        let mut head = head.to_vec();
        head.push(("ts_nanos", ts.to_string()));
        e.records.push_back(json_line(&head, &fields));
    }
}

/// Start keeping records for an exporter (`capacity` of them at most).
pub fn attach(capacity: usize) {
    let mut e = EXPORT.lock().unwrap();
    e.attached = true;
    e.capacity = capacity.max(1);
}

/// The records kept since the last drain (JSON objects).
pub fn drain() -> Vec<String> {
    EXPORT.lock().unwrap().records.drain(..).collect()
}

/// Records written, and records the export buffer dropped.
pub fn counts() -> (u64, u64) {
    (EMITTED.load(Ordering::Relaxed), EXPORT.lock().unwrap().dropped)
}

macro_rules! log_intrinsics {
    ($( fn $name:ident($ctx:ident, $targs:ident, $ret:ident $(, $arg:ident)*) $body:block )*) => {
        $(
            #[allow(unused_variables)]
            pub extern "C" fn $name($ctx: *mut TaskCtx, $targs: Targs, $ret: *const TypeDesc $(, $arg: u64)*) -> u64 $body
        )*
        pub fn symbols() -> Vec<(&'static str, *const u8)> {
            vec![$((concat!("lang_rt_i_", stringify!($name)), $name as *const u8)),*]
        }
    };
}

log_intrinsics! {
    fn log_enabled(ctx, targs, ret, level) { enabled(level as u8) as u64 }
    fn log_emit(ctx, targs, ret, level, message, fields) {
        if enabled(level as u8) {
            emit(level as u8, str_of(message), &crate::codec::encode(targ(targs, 0), fields));
        }
        0
    }
    fn log_push(ctx, targs, ret, fields) {
        let json = crate::codec::encode(targ(targs, 0), fields);
        if let Some(t) = task::current() {
            t.log_ctx.push(json);
        }
        0
    }
    fn log_pop(ctx, targs, ret) {
        if let Some(t) = task::current() {
            t.log_ctx.pop();
        }
        0
    }
    fn log_attach(ctx, targs, ret, capacity) { attach(capacity as usize); 0 }
    fn log_drain(ctx, targs, ret) {
        // List<String> of JSON records
        let g = crate::gc::Roots::new();
        let l = g.add(crate::list::new(rdesc(ret)));
        for r in drain() {
            let s = g.add(new_string(&r));
            crate::list::push(l, s);
        }
        l
    }
    fn log_dropped(ctx, targs, ret) { counts().1 }
}

#[cfg(test)]
mod tests {
    #[test]
    fn rfc3339_dates() {
        assert_eq!(super::rfc3339(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(super::rfc3339(1_760_601_845_123_000_000), "2025-10-16T08:04:05.123Z");
        assert_eq!(super::rfc3339(951_782_400_000_000_000), "2000-02-29T00:00:00.000Z");
    }
}
