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

use crate::intrinsics::{rdesc, targ};
use crate::string::{new_string, str_of};
use crate::task::{self};

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
    let mut head =
        vec![("ts", q(&rfc3339(ts))), ("level", q(lname)), ("msg", q(message)), ("task", task.to_string())];
    // a record inside a span names it (for correlation with the trace)
    if let Some((tr, sp)) = task::current().map(|t| t.span).filter(|s| s.0 != 0) {
        head.push(("trace_id", q(&format!("{tr:032x}"))));
        head.push(("span_id", q(&format!("{sp:016x}"))));
    }
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

// ------------------------------------------------------------------ spans

/// Random ids for traces and spans (never 0).
fn random_u64() -> u64 {
    use std::cell::Cell;
    thread_local! {
        static STATE: Cell<u64> = Cell::new((now_nanos() ^ (std::process::id() as u64).rotate_left(32)) | 1);
    }
    STATE.with(|s| {
        // xorshift64*
        let mut x = s.get();
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        s.set(x);
        x.wrapping_mul(0x2545F4914F6CDD1D).max(1)
    })
}

struct Span {
    trace: u128,
    id: u64,
    parent: u64,
    name: String,
    start: u64,
    attrs: String,
    /// The task's span before this one began (restored at its end).
    outer: (u128, u64),
}

static SPANS: Mutex<Vec<Option<Span>>> = Mutex::new(Vec::new());
static EXPORT_SPANS: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

/// Begin a span in the current task (a child of its current span, or the
/// root of a new trace); its handle for [`span_end`].
pub fn span_begin(name: &str, attrs: String) -> u64 {
    let outer = task::current().map(|t| t.span).unwrap_or((0, 0));
    let trace = if outer.0 != 0 { outer.0 } else { ((random_u64() as u128) << 64) | random_u64() as u128 };
    let id = random_u64();
    if let Some(t) = task::current() {
        t.span = (trace, id);
    }
    let s = Span { trace, id, parent: outer.1, name: name.to_string(), start: now_nanos(), attrs, outer };
    let mut g = SPANS.lock().unwrap();
    match g.iter().position(Option::is_none) {
        Some(i) => {
            g[i] = Some(s);
            i as u64
        }
        None => {
            g.push(Some(s));
            (g.len() - 1) as u64
        }
    }
}

/// End span `h` (`ok` false: it failed), restoring the task's outer span.
pub fn span_end(h: u64, ok: bool) {
    let Some(s) = SPANS.lock().unwrap().get_mut(h as usize).and_then(Option::take) else { return };
    if let Some(t) = task::current() {
        t.span = s.outer;
    }
    let end = now_nanos();
    let cap = {
        let e = EXPORT.lock().unwrap();
        if !e.attached {
            return;
        }
        e.capacity
    };
    let parent = if s.parent == 0 { "null".to_string() } else { format!("\"{:016x}\"", s.parent) };
    let attrs = if s.attrs.starts_with('{') { s.attrs.as_str() } else { "{}" };
    let rec = format!(
        "{{\"trace_id\":\"{:032x}\",\"span_id\":\"{:016x}\",\"parent_span_id\":{parent},\"name\":{},\"start_nanos\":{},\"end_nanos\":{end},\"ok\":{ok},\"attributes\":{attrs}}}",
        s.trace,
        s.id,
        serde_json::Value::from(s.name.as_str()),
        s.start,
    );
    let mut q = EXPORT_SPANS.lock().unwrap();
    if q.len() >= cap {
        q.pop_front();
        EXPORT.lock().unwrap().dropped += 1;
    }
    q.push_back(rec);
}

/// The current task's W3C `traceparent` (`00-TRACE-SPAN-01`), if it is in
/// a span.
pub fn traceparent() -> Option<String> {
    task::current().map(|t| t.span).filter(|s| s.0 != 0).map(|(t, s)| format!("00-{t:032x}-{s:016x}-01"))
}

/// Continue the trace a `traceparent` names in the current task.
pub fn adopt(traceparent: &str) -> bool {
    let parts: Vec<&str> = traceparent.trim().split('-').collect();
    let (Some(tr), Some(sp)) = (
        parts.get(1).and_then(|t| u128::from_str_radix(t, 16).ok()),
        parts.get(2).and_then(|s| u64::from_str_radix(s, 16).ok()),
    ) else {
        return false;
    };
    if parts.len() != 4 || tr == 0 || sp == 0 {
        return false;
    }
    if let Some(t) = task::current() {
        t.span = (tr, sp);
    }
    true
}

// ------------------------------------------------------------------ metrics

/// OpenTelemetry's default histogram bounds.
pub const BOUNDS: [f64; 15] =
    [0.0, 5.0, 10.0, 25.0, 50.0, 75.0, 100.0, 250.0, 500.0, 750.0, 1000.0, 2500.0, 5000.0, 7500.0, 10000.0];

#[derive(Clone)]
enum Metric {
    Counter(f64),
    Gauge(f64),
    Histogram { count: u64, sum: f64, min: f64, max: f64, buckets: [u64; BOUNDS.len() + 1] },
}

type Metrics = Vec<((String, String), Metric)>;
static METRICS: Mutex<Metrics> = Mutex::new(Vec::new());
static METRICS_START: AtomicU64 = AtomicU64::new(0);

fn metric(name: &str, attrs: String, f: impl FnOnce(Option<&mut Metric>) -> Option<Metric>) {
    let _ = METRICS_START.compare_exchange(0, now_nanos(), Ordering::Relaxed, Ordering::Relaxed);
    let key = (name.to_string(), if attrs.starts_with('{') { attrs } else { "{}".into() });
    let mut g = METRICS.lock().unwrap();
    match g.iter_mut().find(|(k, _)| *k == key) {
        Some((_, m)) => {
            f(Some(m));
        }
        None => {
            if let Some(m) = f(None) {
                g.push((key, m));
            }
        }
    }
}

pub fn metric_add(name: &str, v: f64, attrs: String) {
    metric(name, attrs, |m| match m {
        Some(Metric::Counter(c)) => {
            *c += v;
            None
        }
        Some(_) => None,
        None => Some(Metric::Counter(v)),
    })
}

pub fn metric_set(name: &str, v: f64, attrs: String) {
    metric(name, attrs, |m| match m {
        Some(Metric::Gauge(g)) => {
            *g = v;
            None
        }
        Some(_) => None,
        None => Some(Metric::Gauge(v)),
    })
}

pub fn metric_observe(name: &str, v: f64, attrs: String) {
    let i = BOUNDS.iter().position(|b| v <= *b).unwrap_or(BOUNDS.len());
    metric(name, attrs, |m| match m {
        Some(Metric::Histogram { count, sum, min, max, buckets }) => {
            *count += 1;
            *sum += v;
            *min = min.min(v);
            *max = max.max(v);
            buckets[i] += 1;
            None
        }
        Some(_) => None,
        None => {
            let mut buckets = [0u64; BOUNDS.len() + 1];
            buckets[i] = 1;
            Some(Metric::Histogram { count: 1, sum: v, min: v, max: v, buckets })
        }
    })
}

/// Every metric's cumulative value since the first was recorded (JSON
/// objects: name, kind, attributes, and the value or histogram).
pub fn metrics_snapshot() -> Vec<String> {
    let start = METRICS_START.load(Ordering::Relaxed);
    let now = now_nanos();
    let num = |f: f64| serde_json::Number::from_f64(f).map(|n| n.to_string()).unwrap_or("0".into());
    METRICS
        .lock()
        .unwrap()
        .iter()
        .map(|((name, attrs), m)| {
            let head = format!(
                "\"name\":{},\"attributes\":{attrs},\"start_nanos\":{start},\"time_nanos\":{now}",
                serde_json::Value::from(name.as_str())
            );
            match m {
                Metric::Counter(c) => format!("{{{head},\"kind\":\"counter\",\"value\":{}}}", num(*c)),
                Metric::Gauge(g) => format!("{{{head},\"kind\":\"gauge\",\"value\":{}}}", num(*g)),
                Metric::Histogram { count, sum, min, max, buckets } => format!(
                    "{{{head},\"kind\":\"histogram\",\"count\":{count},\"sum\":{},\"min\":{},\"max\":{},\"bounds\":{},\"buckets\":{}}}",
                    num(*sum),
                    num(*min),
                    num(*max),
                    serde_json::to_string(&BOUNDS.to_vec()).unwrap_or_default(),
                    serde_json::to_string(&buckets.to_vec()).unwrap_or_default(),
                ),
            }
        })
        .collect()
}

// ------------------------------------------------------------------ OTLP

use serde_json::{Value as J, json};

/// An OTLP attribute list from a JSON object.
fn otlp_attrs(v: Option<&J>) -> J {
    let Some(J::Object(m)) = v else { return json!([]) };
    J::Array(
        m.iter()
            .map(|(k, v)| {
                let value = match v {
                    J::Bool(b) => json!({ "boolValue": b }),
                    J::Number(n) if n.is_i64() || n.is_u64() => json!({ "intValue": n.to_string() }),
                    J::Number(n) => json!({ "doubleValue": n.as_f64() }),
                    J::String(s) => json!({ "stringValue": s }),
                    other => json!({ "stringValue": other.to_string() }),
                };
                json!({ "key": k, "value": value })
            })
            .collect(),
    )
}

fn resource(service: &str) -> J {
    json!({ "attributes": [{ "key": "service.name", "value": { "stringValue": service } }] })
}

fn scope() -> J {
    json!({ "name": "badger", "version": env!("CARGO_PKG_VERSION") })
}

/// Drain the kept log records as an OTLP/HTTP JSON body (`/v1/logs`).
pub fn otlp_logs(service: &str) -> Option<String> {
    let recs = drain();
    if recs.is_empty() {
        return None;
    }
    let records: Vec<J> = recs
        .iter()
        .filter_map(|r| serde_json::from_str::<J>(r).ok())
        .map(|r| {
            let level = r["level"].as_str().unwrap_or("info");
            let severity = match level {
                "trace" => 1,
                "debug" => 5,
                "info" => 9,
                "warn" => 13,
                _ => 17,
            };
            let mut attrs = serde_json::Map::new();
            if let J::Object(m) = &r {
                for (k, v) in m {
                    if !matches!(k.as_str(), "ts" | "ts_nanos" | "level" | "msg" | "trace_id" | "span_id") {
                        attrs.insert(k.clone(), v.clone());
                    }
                }
            }
            let mut out = json!({
                "timeUnixNano": r["ts_nanos"].as_u64().unwrap_or(0).to_string(),
                "severityNumber": severity,
                "severityText": level.to_uppercase(),
                "body": { "stringValue": r["msg"].as_str().unwrap_or("") },
                "attributes": otlp_attrs(Some(&J::Object(attrs))),
            });
            if let (Some(t), Some(s)) = (r["trace_id"].as_str(), r["span_id"].as_str()) {
                out["traceId"] = t.into();
                out["spanId"] = s.into();
            }
            out
        })
        .collect();
    Some(json!({ "resourceLogs": [{ "resource": resource(service), "scopeLogs": [{ "scope": scope(), "logRecords": records }] }] }).to_string())
}

/// Drain the ended spans as an OTLP/HTTP JSON body (`/v1/traces`).
pub fn otlp_traces(service: &str) -> Option<String> {
    let recs: Vec<String> = EXPORT_SPANS.lock().unwrap().drain(..).collect();
    if recs.is_empty() {
        return None;
    }
    let spans: Vec<J> = recs
        .iter()
        .filter_map(|r| serde_json::from_str::<J>(r).ok())
        .map(|s| {
            let mut out = json!({
                "traceId": s["trace_id"],
                "spanId": s["span_id"],
                "name": s["name"],
                "kind": 1,
                "startTimeUnixNano": s["start_nanos"].as_u64().unwrap_or(0).to_string(),
                "endTimeUnixNano": s["end_nanos"].as_u64().unwrap_or(0).to_string(),
                "attributes": otlp_attrs(s.get("attributes")),
                "status": { "code": if s["ok"].as_bool().unwrap_or(true) { 1 } else { 2 } },
            });
            if let Some(p) = s["parent_span_id"].as_str() {
                out["parentSpanId"] = p.into();
            }
            out
        })
        .collect();
    Some(json!({ "resourceSpans": [{ "resource": resource(service), "scopeSpans": [{ "scope": scope(), "spans": spans }] }] }).to_string())
}

/// Every metric's cumulative value as an OTLP/HTTP JSON body
/// (`/v1/metrics`).
pub fn otlp_metrics(service: &str) -> Option<String> {
    let snap = metrics_snapshot();
    if snap.is_empty() {
        return None;
    }
    let mut by_name: Vec<(String, String, Vec<J>)> = vec![];
    for m in snap.iter().filter_map(|r| serde_json::from_str::<J>(r).ok()) {
        let name = m["name"].as_str().unwrap_or("").to_string();
        let kind = m["kind"].as_str().unwrap_or("").to_string();
        let mut point = json!({
            "attributes": otlp_attrs(m.get("attributes")),
            "startTimeUnixNano": m["start_nanos"].as_u64().unwrap_or(0).to_string(),
            "timeUnixNano": m["time_nanos"].as_u64().unwrap_or(0).to_string(),
        });
        if kind == "histogram" {
            point["count"] = m["count"].as_u64().unwrap_or(0).to_string().into();
            point["sum"] = m["sum"].clone();
            point["min"] = m["min"].clone();
            point["max"] = m["max"].clone();
            point["explicitBounds"] = m["bounds"].clone();
            point["bucketCounts"] = J::Array(
                m["buckets"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|b| b.as_u64().unwrap_or(0).to_string().into())
                    .collect(),
            );
        } else {
            point["asDouble"] = m["value"].clone();
        }
        match by_name.iter_mut().find(|(n, k, _)| *n == name && *k == kind) {
            Some((_, _, pts)) => pts.push(point),
            None => by_name.push((name, kind, vec![point])),
        }
    }
    let metrics: Vec<J> = by_name
        .into_iter()
        .map(|(name, kind, points)| match kind.as_str() {
            "counter" => json!({ "name": name, "sum": { "dataPoints": points, "aggregationTemporality": 2, "isMonotonic": true } }),
            "gauge" => json!({ "name": name, "gauge": { "dataPoints": points } }),
            _ => json!({ "name": name, "histogram": { "dataPoints": points, "aggregationTemporality": 2 } }),
        })
        .collect();
    Some(json!({ "resourceMetrics": [{ "resource": resource(service), "scopeMetrics": [{ "scope": scope(), "metrics": metrics }] }] }).to_string())
}

/// A `List<String>` of `items`.
use crate::intrinsics::strings;

define_intrinsics! {
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
    fn span_begin_i(ctx, targs, ret, name, attrs) {
        span_begin(str_of(name), crate::codec::encode(targ(targs, 0), attrs))
    }
    fn span_end_i(ctx, targs, ret, h, ok) { span_end(h, ok & 1 != 0); 0 }
    fn span_traceparent(ctx, targs, ret) {
        match traceparent() {
            Some(s) => { let w = new_string(&s); crate::make_variant(rdesc(ret), "some", &[w]) }
            None => crate::make_variant(rdesc(ret), "none", &[]),
        }
    }
    fn span_adopt(ctx, targs, ret, tp) { adopt(str_of(tp)) as u64 }
    fn metric_add_i(ctx, targs, ret, name, v, attrs) {
        metric_add(str_of(name), f64::from_bits(v), crate::codec::encode(targ(targs, 0), attrs));
        0
    }
    fn metric_set_i(ctx, targs, ret, name, v, attrs) {
        metric_set(str_of(name), f64::from_bits(v), crate::codec::encode(targ(targs, 0), attrs));
        0
    }
    fn metric_observe_i(ctx, targs, ret, name, v, attrs) {
        metric_observe(str_of(name), f64::from_bits(v), crate::codec::encode(targ(targs, 0), attrs));
        0
    }
    fn metrics_snapshot_i(ctx, targs, ret) { strings(rdesc(ret), metrics_snapshot()) }
    // OTLP/HTTP JSON bodies: `Option<String>` (none: nothing to send)
    fn otlp_body(ctx, targs, ret, signal, service) {
        let body = match str_of(signal) {
            "logs" => otlp_logs(str_of(service)),
            "traces" => otlp_traces(str_of(service)),
            _ => otlp_metrics(str_of(service)),
        };
        match body {
            Some(b) => { let w = new_string(&b); crate::make_variant(rdesc(ret), "some", &[w]) }
            None => crate::make_variant(rdesc(ret), "none", &[]),
        }
    }
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
