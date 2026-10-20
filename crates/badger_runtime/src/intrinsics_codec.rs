//! Codec and trace intrinsics (uniform ABI).

use crate::desc::TypeDesc;
use crate::gc::Roots;
use crate::intrinsics::{Targs, rdesc, targ};
use crate::string::{bytes_of, new_string, str_of};
use crate::task::TaskCtx;
use crate::{codec, make_record, make_variant};

fn decode_result(ret: &'static TypeDesc, r: Result<u64, codec::DecodeError>) -> u64 {
    let g = Roots::new();
    match r {
        // (`make_variant` roots the payload when it is a managed value; a
        // decoded scalar is not an object and must not be rooted)
        Ok(w) => make_variant(ret, "ok", &[w]),
        Err(e) => {
            let err_desc = ret.variant(1).unwrap().fields[0].desc;
            let p = g.add(new_string(&e.path));
            let m = g.add(new_string(&e.message));
            let ev = g.add(make_record(err_desc, &[("path", p), ("message", m)]));
            make_variant(ret, "err", &[ev])
        }
    }
}

define_intrinsics! {
    fn wire_encode(ctx, targs, ret, names, aliases, v) {
        let w = wire_names(names, aliases);
        codec::with_wire(w, || new_string(&codec::encode(targ(targs, 0), v)))
    }
    fn wire_decode(ctx, targs, ret, names, aliases, s) {
        let roots = Roots::new();
        roots.add(s);
        let w = wire_names(names, aliases);
        let r = codec::with_wire(w, || codec::decode(targ(targs, 0), str_of(s)));
        decode_result(rdesc(ret), r)
    }
    // Typed path parameters: each text value is read as the field's type
    // (numbers and booleans are parsed), then decoded and validated.
    fn params_decode(ctx, targs, ret, m) {
        let d = targ(targs, 0);
        let mut obj = serde_json::Map::new();
        for (k, v) in crate::map::entries(m) {
            let key = str_of(k).to_string();
            let text = str_of(v).to_string();
            // an optional parameter parses as its payload type
            let field = d.field(&key).map(|f| match &f.desc.kind {
                crate::desc::Kind::Enum { variants, .. } if f.desc.name == "Option" => variants[1].fields[0].desc,
                _ => f.desc,
            });
            let val = match field.map(|f| &f.kind) {
                Some(crate::desc::Kind::Int { .. }) => text.parse::<i64>().map(serde_json::Value::from).unwrap_or(serde_json::Value::String(text)),
                Some(crate::desc::Kind::Float { .. }) => text.parse::<f64>().ok().and_then(|f| serde_json::Number::from_f64(f).map(serde_json::Value::Number)).unwrap_or(serde_json::Value::String(text)),
                Some(crate::desc::Kind::Bool) => match text.as_str() {
                    "true" => serde_json::Value::Bool(true),
                    "false" => serde_json::Value::Bool(false),
                    _ => serde_json::Value::String(text),
                },
                _ => serde_json::Value::String(text),
            };
            obj.insert(key, val);
        }
        decode_result(rdesc(ret), codec::decode_value(d, &serde_json::Value::Object(obj)))
    }
    // A record's fields in canonical (label) order as (wire name, Data).
    fn record_fields(ctx, targs, ret, v) {
        let d = targ(targs, 0);
        let g = Roots::new();
        g.add(v);
        let out = g.add(crate::list::new(rdesc(ret)));
        let tuple = match &rdesc(ret).kind { crate::desc::Kind::List { elem } => *elem, _ => return out };
        let data_desc = tuple.field("1").map(|f| f.desc).unwrap();
        if let crate::desc::Kind::Record { fields, .. } = &d.kind {
            for f in fields {
                let w = unsafe { f.repr.load(crate::gc::obj(v).payload().add(f.offset as usize)) };
                let j: serde_json::Value = serde_json::from_str(&codec::encode(f.desc, w)).unwrap_or(serde_json::Value::Null);
                let dv = g.add(codec::data_of(data_desc, &j));
                let name = g.add(new_string(&codec::wire_name(&d.name, &f.name)));
                let t = g.add(make_record(tuple, &[("0", name), ("1", dv)]));
                crate::list::push(out, t);
            }
        }
        out
    }
    // Property names of an object JSON Schema.
    fn data_field_names(ctx, targs, ret, schema) {
        let j: serde_json::Value = serde_json::from_str(str_of(schema)).unwrap_or(serde_json::Value::Null);
        let g = Roots::new();
        let l = g.add(crate::list::new(rdesc(ret)));
        if let Some(props) = j.get("properties").and_then(|p| p.as_object()) {
            for k in props.keys() {
                let w = g.add(new_string(k));
                crate::list::push(l, w);
            }
        }
        l
    }
    // Property schemas (as JSON text) of an object JSON Schema.
    fn data_property_schemas(ctx, targs, ret, schema) {
        let j: serde_json::Value = serde_json::from_str(str_of(schema)).unwrap_or(serde_json::Value::Null);
        let g = Roots::new();
        let m = g.add(crate::map::new(rdesc(ret)));
        if let Some(props) = j.get("properties").and_then(|p| p.as_object()) {
            for (k, v) in props {
                let kw = g.add(new_string(k));
                let vw = g.add(new_string(&v.to_string()));
                crate::map::insert_mut(m, kw, vw);
            }
        }
        m
    }
    fn wire_json_schema(ctx, targs, ret, names, aliases) {
        let w = wire_names(names, aliases);
        codec::with_wire(w, || new_string(&crate::intrinsics_std::json_schema(targ(targs, 0), 0)))
    }
    fn wire_describe(ctx, targs, ret, names, aliases) {
        let w = wire_names(names, aliases);
        codec::with_wire(w, || new_string(&crate::intrinsics_std::describe(targ(targs, 0), 0)))
    }
    fn codec_encode(ctx, targs, ret, v) { new_string(&codec::encode(targ(targs, 0), v)) }
    fn codec_encode_revealed(ctx, targs, ret, v) { new_string(&codec::encode_revealed(targ(targs, 0), v)) }
    fn codec_encode_bytes(ctx, targs, ret, v) { crate::string::new_bytes(codec::encode(targ(targs, 0), v).as_bytes()) }
    fn codec_decode_text(ctx, targs, ret, s) {
        let roots = Roots::new();
        roots.add(s);
        decode_result(rdesc(ret), codec::decode(targ(targs, 0), str_of(s)))
    }
    fn codec_decode_bytes(ctx, targs, ret, b) {
        let roots = Roots::new();
        roots.add(b);
        let text = String::from_utf8_lossy(bytes_of(b));
        decode_result(rdesc(ret), codec::decode(targ(targs, 0), &text))
    }
    fn codec_from_data(ctx, targs, ret, d) { decode_result(rdesc(ret), codec::from_data(targ(targs, 0), d)) }
    fn codec_from_sql_row(ctx, targs, ret, d) { decode_result(rdesc(ret), codec::from_sql_row(targ(targs, 0), d)) }
    fn codec_to_data(ctx, targs, ret, v) {
        let text = codec::encode(targ(targs, 0), v);
        // the encoder's own output, nested at most `codec::MAX_DEPTH` deep:
        // past serde's default limit (128) it is still parsed
        let mut de = serde_json::Deserializer::from_str(&text);
        de.disable_recursion_limit();
        let j: serde_json::Value = serde::Deserialize::deserialize(&mut de).unwrap_or(serde_json::Value::Null);
        // `Data.int` holds an `Int`: a larger unsigned value has no exact form
        if let Some(n) = codec::first_unrepresentable(&j) {
            crate::task::panic_current(
                format!("{n} does not fit `Data.int` (conversions to Data do not truncate)"),
                crate::task::caller_location(),
            );
        }
        codec::data_of(rdesc(ret), &j)
    }
    fn data_parse(ctx, targs, ret, s) {
        // Result<Data, String>
        match serde_json::from_str::<serde_json::Value>(str_of(s)) {
            Ok(j) => {
                let g = Roots::new();
                let data_desc = rdesc(ret).variant(0).unwrap().fields[0].desc;
                let d = g.add(codec::data_of(data_desc, &j));
                make_variant(rdesc(ret), "ok", &[d])
            }
            Err(e) => {
                let m = new_string(&e.to_string());
                make_variant(rdesc(ret), "err", &[m])
            }
        }
    }
    fn trace_event(ctx, targs, ret, name, v) {
        crate::intrinsics_codec::record_event(str_of(name), &codec::encode(targ(targs, 0), v));
        0
    }
    fn trace_count(ctx, targs, ret, name) {
        SINK.lock().unwrap().events.iter().filter(|e| e.0 == str_of(name)).count() as u64
    }
    fn trace_last(ctx, targs, ret, name) {
        // Option<String>: last event payload (JSON) for `name`
        let n = str_of(name).to_string();
        let ev = SINK.lock().unwrap().events.iter().rev().find(|e| e.0 == n).map(|e| e.1.clone());
        match ev {
            Some(s) => { let w = new_string(&s); make_variant(rdesc(ret), "some", &[w]) }
            None => make_variant(rdesc(ret), "none", &[]),
        }
    }
    fn trace_stats(ctx, targs, ret) {
        let st = stats();
        let g = Roots::new();
        let policy = crate::string::symbol(st.policy);

        g.add(make_record(rdesc(ret), &[
            ("capacity", st.capacity as u64),
            ("recorded", st.recorded),
            ("dropped", st.dropped),
            ("export_dropped", st.export_dropped),
            ("policy", policy),
        ]))
    }
}

/// Overflow policy of the bounded trace sink.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Overflow {
    /// Evict the oldest retained event (default).
    DropOldest,
    /// Refuse the new event.
    DropNewest,
}

struct Sink {
    events: std::collections::VecDeque<(String, String)>,
    capacity: usize,
    policy: Overflow,
    recorded: u64,
    dropped: u64,
    export_dropped: u64,
    /// Bounded queue to the `BADGER_TRACE` stderr exporter thread.
    export: Option<std::sync::mpsc::SyncSender<String>>,
    export_init: bool,
}

static SINK: std::sync::Mutex<Sink> = std::sync::Mutex::new(Sink {
    events: std::collections::VecDeque::new(),
    capacity: 1024,
    policy: Overflow::DropOldest,
    recorded: 0,
    dropped: 0,
    export_dropped: 0,
    export: None,
    export_init: false,
});

/// Configure the sink (`[runtime] trace_capacity`, `trace_overflow`).
pub fn set_trace_setting(key: &str, value: &str) -> bool {
    let mut s = SINK.lock().unwrap();
    match key {
        "trace_capacity" => match value.parse::<usize>() {
            Ok(n) => s.capacity = n.max(1),
            Err(_) => return false,
        },
        "trace_overflow" => {
            s.policy = match value {
                "drop_newest" => Overflow::DropNewest,
                "drop_oldest" => Overflow::DropOldest,
                _ => return false,
            }
        }
        _ => return false,
    }
    true
}

pub struct TraceStats {
    pub capacity: usize,
    pub recorded: u64,
    pub dropped: u64,
    pub export_dropped: u64,
    pub policy: &'static str,
}

pub fn stats() -> TraceStats {
    let s = SINK.lock().unwrap();
    TraceStats {
        capacity: s.capacity,
        recorded: s.recorded,
        dropped: s.dropped,
        export_dropped: s.export_dropped,
        policy: if s.policy == Overflow::DropOldest { "drop_oldest" } else { "drop_newest" },
    }
}

static EXPORTER: std::sync::Mutex<Option<std::thread::JoinHandle<()>>> = std::sync::Mutex::new(None);

/// At exit: drain the exporter, then report sink overload separately from
/// the events (stderr).
pub fn report_overload() {
    SINK.lock().unwrap().export = None;
    if let Some(h) = EXPORTER.lock().unwrap().take() {
        let _ = h.join();
    }
    let st = stats();
    if st.dropped > 0 || st.export_dropped > 0 {
        eprintln!(
            "trace: sink overloaded: {} of {} events not retained ({}), {} not exported (capacity {})",
            st.dropped, st.recorded, st.policy, st.export_dropped, st.capacity
        );
    }
}

/// Record an event in the bounded sink. Diagnostics never block business
/// code: when the sink is full, the overflow policy decides which event is
/// lost, and losses are counted and reported separately.
/// An environment variable read by `std.config`: recorded and replayed like
/// `env.get`, so a replay sees the recorded configuration.
fn env_var(name: &str) -> Option<String> {
    crate::repro::external("env", name, None, crate::repro::secret_name(name), || std::env::var(name).ok())
}

pub fn record_event(name: &str, payload: &str) {
    let mut g = SINK.lock().unwrap();
    g.recorded += 1;
    if g.events.len() >= g.capacity {
        g.dropped += 1;
        match g.policy {
            Overflow::DropOldest => {
                g.events.pop_front();
            }
            Overflow::DropNewest => {
                drop(g);
                return export(name, payload);
            }
        }
    }
    g.events.push_back((name.to_string(), payload.to_string()));
    drop(g);
    export(name, payload);
}

fn export(name: &str, payload: &str) {
    let mut g = SINK.lock().unwrap();
    if !g.export_init {
        g.export_init = true;
        if std::env::var_os("BADGER_TRACE").is_some() {
            let (tx, rx) = std::sync::mpsc::sync_channel::<String>(g.capacity);
            let h = std::thread::spawn(move || {
                use std::io::Write;
                for line in rx {
                    let _ = writeln!(std::io::stderr(), "{line}");
                }
            });
            *EXPORTER.lock().unwrap() = Some(h);
            g.export = Some(tx);
        }
    }
    if let Some(tx) = &g.export
        && tx.try_send(format!("trace {name} {payload}")).is_err()
    {
        g.export_dropped += 1;
    }
}

/// `config.from_env<T>(prefix)`: decode a record from environment variables.
/// Effective configuration of the selected run profile (`[config]` merged
/// with `[profile.NAME]` from `badger.toml`), set by the driver before `main`.
/// Each entry is `(key, value, profile)`: the profile name, or empty for a
/// `[config]` value.
static PROFILE: std::sync::Mutex<Vec<(String, String, String)>> = std::sync::Mutex::new(Vec::new());

pub fn set_profile(entries: Vec<(String, String)>) {
    set_profile_sources(entries.into_iter().map(|(k, v)| (k, v, String::new())).collect());
}

/// The effective configuration with the source of each value.
pub fn set_profile_sources(entries: Vec<(String, String, String)>) {
    *PROFILE.lock().unwrap() = entries;
}

pub extern "C" fn config_from_env(
    _ctx: *mut TaskCtx,
    targs: Targs,
    ret: *const TypeDesc,
    prefix: u64,
) -> u64 {
    config_decode(targs, ret, prefix, Mode { strict: false, profile: false })
}

/// Profile data overlaid by prefixed environment variables.
pub extern "C" fn config_load(_ctx: *mut TaskCtx, targs: Targs, ret: *const TypeDesc, prefix: u64) -> u64 {
    config_decode(targs, ret, prefix, Mode { strict: false, profile: true })
}

/// `config.load` and `config.from_env`: the leaves of `T` from the profile
/// data (when `mode.profile`) and prefixed environment variables, without
/// `resolve`'s strictness, defaults, or overrides.
fn config_decode(targs: Targs, ret: *const TypeDesc, prefix: u64, mode: Mode) -> u64 {
    use crate::desc::Kind;
    let t = targ(targs, 0);
    let ret = rdesc(ret);
    let pre = str_of(prefix).to_string();
    let Kind::Record { .. } = &t.kind else {
        return resolve_err(ret, &pre, "configuration type must be a record");
    };
    decode_layers(t, ret, resolve_layers(t, &pre, &[], "{}", mode))
}

/// A configured value with `${NAME}` (an environment variable, which must
/// be set) and `${NAME:-default}` (or the default when unset or empty)
/// replaced; `$${` is a literal `${`.
pub fn expand_env(v: &str) -> Result<String, String> {
    if !v.contains("${") {
        return Ok(v.to_string());
    }
    let mut out = String::new();
    let mut rest = v;
    while let Some(i) = rest.find("${") {
        if rest[..i].ends_with('$') {
            out.push_str(&rest[..i - 1]);
            out.push_str("${");
            rest = &rest[i + 2..];
            continue;
        }
        out.push_str(&rest[..i]);
        let Some(end) = rest[i..].find('}') else { return Err(format!("unclosed `${{` in `{v}`")) };
        let inner = &rest[i + 2..i + end];
        let (name, default) = match inner.split_once(":-") {
            Some((n, d)) => (n, Some(d)),
            None => (inner, None),
        };
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(format!("`${{{inner}}}` does not name an environment variable"));
        }
        match (env_var(name).filter(|x| !x.is_empty() || default.is_none()), default) {
            (Some(x), _) => out.push_str(&x),
            (None, Some(d)) => out.push_str(d),
            (None, None) => return Err(format!("environment variable `{name}` is not set")),
        }
        rest = &rest[i + end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// A configuration leaf: its dotted path, environment variable, and type,
/// and the path of the optional record it is part of, if any (when none of
/// that record's leaves is set, the record is `none`).
struct Leaf {
    path: String,
    env: String,
    desc: &'static TypeDesc,
    optional: Option<String>,
}

/// Which sources a configuration reads, and whether it is strict.
#[derive(Clone, Copy)]
struct Mode {
    /// Unknown keys, variables, and overrides are errors (`resolve`).
    strict: bool,
    /// The `[config]` table and the selected profile are read (all but
    /// `from_env`).
    profile: bool,
}

/// The record payload of `Option<record>`, if `d` is one.
fn optional_record(d: &'static TypeDesc) -> Option<&'static TypeDesc> {
    use crate::desc::Kind;
    let Kind::Enum { variants, .. } = &d.kind else { return None };
    if d.name != "Option" {
        return None;
    }
    let p = variants.get(1)?.fields.first()?.desc;
    match &p.kind {
        Kind::Record { tuple: false, flat: false, .. } if p.name != crate::codec::SECRET => Some(p),
        _ => None,
    }
}

fn leaves(t: &'static TypeDesc, env_pre: &str, key_pre: &str, optional: Option<&str>, out: &mut Vec<Leaf>) {
    use crate::desc::Kind;
    let Kind::Record { fields, .. } = &t.kind else { return };
    for f in fields {
        let path = format!("{key_pre}{}", f.name);
        let env = format!("{env_pre}{}", f.name.to_uppercase());
        match &f.desc.kind {
            // (a `@flatten` struct is one setting, as its field is)
            Kind::Record { tuple: false, flat: false, .. } if f.desc.name != crate::codec::SECRET => {
                leaves(f.desc, &format!("{env}_"), &format!("{path}."), optional, out)
            }
            // an optional record: its fields, `none` when none is set
            _ if let Some(p) = optional_record(f.desc) => {
                leaves(p, &format!("{env}_"), &format!("{path}."), Some(optional.unwrap_or(&path)), out)
            }
            _ => out.push(Leaf { path, env, desc: f.desc, optional: optional.map(str::to_string) }),
        }
    }
}

/// The value type of a `Map<String, V>` leaf.
fn map_values(d: &TypeDesc) -> Option<&'static TypeDesc> {
    match &d.kind {
        crate::desc::Kind::Map { val, .. } => Some(val),
        _ => None,
    }
}

/// Whether values of `d` may hold a `config.Secret` (provenance redacts
/// them).
fn holds_secret(d: &TypeDesc, depth: u32) -> bool {
    use crate::desc::Kind;
    if d.name == crate::codec::SECRET {
        return true;
    }
    if depth > 8 {
        return false;
    }
    match &d.kind {
        Kind::Enum { variants, .. } => {
            variants.iter().any(|v| v.fields.iter().any(|f| holds_secret(f.desc, depth + 1)))
        }
        Kind::Record { fields, .. } => fields.iter().any(|f| holds_secret(f.desc, depth + 1)),
        Kind::List { elem } | Kind::Set { elem } => holds_secret(elem, depth + 1),
        Kind::Map { val, .. } => holds_secret(val, depth + 1),
        _ => false,
    }
}

/// A configured string as JSON of the leaf's type.
fn leaf_json(d: &TypeDesc, key: &str, v: String) -> Result<serde_json::Value, (String, String)> {
    use crate::desc::Kind;
    let bad = |m: String| Err((key.to_string(), m));
    let d = match &d.kind {
        // an optional setting's value is its payload's
        Kind::Enum { variants, .. } if d.name == "Option" => {
            match variants.get(1).and_then(|x| x.fields.first()) {
                Some(f) => f.desc,
                None => d,
            }
        }
        _ => d,
    };
    // a `@flatten` struct's setting is its field's
    let d = match &d.kind {
        Kind::Record { fields, flat: true, .. } => fields[0].desc,
        _ => d,
    };
    Ok(match &d.kind {
        Kind::String | Kind::Symbol => serde_json::Value::String(v),
        _ if d.name == crate::codec::SECRET => serde_json::Value::String(v),
        Kind::Bool => match v.as_str() {
            "true" | "1" => serde_json::Value::Bool(true),
            "false" | "0" => serde_json::Value::Bool(false),
            _ => return bad("expected true or false".into()),
        },
        Kind::Int { .. } => match v.trim().parse::<i64>() {
            Ok(n) => serde_json::Value::from(n),
            Err(_) => return bad(format!("expected an integer, found `{v}`")),
        },
        Kind::Float { .. } => match v.trim().parse::<f64>() {
            Ok(n) => serde_json::json!(n),
            Err(_) => return bad(format!("expected a number, found `{v}`")),
        },
        Kind::Duration | Kind::Instant | Kind::Timestamp => match duration_nanos(&v) {
            Some(n) => serde_json::Value::from(n),
            None => {
                return bad(format!(
                    "expected a duration (10s, 500ms) or a number of milliseconds, found `{v}`"
                ));
            }
        },
        _ => serde_json::from_str(&v).unwrap_or(serde_json::Value::String(v)),
    })
}

/// A duration written as the language spells it (`10s`, `500ms`, `2us`,
/// `1ns`, `5min`, `1h`) or as a plain number of milliseconds. The result
/// is nanoseconds, the unit the runtime keeps.
fn duration_nanos(text: &str) -> Option<i64> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    // a plain number is a number of milliseconds
    if let Ok(n) = t.parse::<i64>() {
        return n.checked_mul(1_000_000);
    }
    let unit_start = t.find(|c: char| c.is_ascii_alphabetic())?;
    let (number, unit) = t.split_at(unit_start);
    let value: i64 = number.parse().ok()?;
    let scale = match unit {
        "ns" => 1,
        "us" => 1_000,
        "ms" => 1_000_000,
        "s" => 1_000_000_000,
        "min" => 60_000_000_000,
        "h" => 3_600_000_000_000,
        _ => return None,
    };
    value.checked_mul(scale)
}

/// One source's value for a setting.
struct Candidate {
    source: &'static str,
    origin: String,
    value: serde_json::Value,
}

/// The layered resolution of `config.resolve` (and, without strictness,
/// defaults, or overrides, of `config.load` and `config.from_env`): the
/// configuration object, and each set leaf's candidates (lowest
/// precedence first).
#[allow(clippy::type_complexity)]
fn resolve_layers(
    t: &'static TypeDesc,
    pre: &str,
    overrides: &[String],
    defaults: &str,
    mode: Mode,
) -> Result<(serde_json::Map<String, serde_json::Value>, Vec<(Leaf, Vec<Candidate>)>), (String, String)> {
    let mut ls = vec![];
    leaves(t, pre, "", None, &mut ls);
    // one environment variable for two fields (`db_port` and `db.port`)
    for (i, a) in ls.iter().enumerate() {
        if let Some(b) = ls[i + 1..].iter().find(|b| b.env == a.env) {
            return Err((
                a.env.clone(),
                format!(
                    "fields `{}` and `{}` would both read the environment variable `{}`; rename one",
                    a.path, b.path, a.env
                ),
            ));
        }
    }
    let defaults: serde_json::Value = serde_json::from_str(defaults).unwrap_or(serde_json::Value::Null);
    let profile = if mode.profile { PROFILE.lock().unwrap().clone() } else { vec![] };
    // a key `p.k` belongs to the map leaf `p`
    let map_leaf = |k: &str| {
        ls.iter().find(|l| {
            map_values(l.desc).is_some() && k.strip_prefix(&l.path).is_some_and(|r| r.starts_with('.'))
        })
    };
    let known = |k: &str| ls.iter().any(|l| l.path == k) || map_leaf(k).is_some();
    if mode.strict {
        // every source key names a field
        for (k, _, src) in &profile {
            if !known(k) {
                let table = if src.is_empty() { "[config]".to_string() } else { format!("[profile.{src}]") };
                return Err((
                    k.clone(),
                    format!("unknown setting in {table}: the configuration type has no field `{k}`"),
                ));
            }
        }
    }
    let mut over: Vec<(String, String)> = vec![];
    for o in overrides {
        let Some((k, v)) = o.split_once('=') else {
            // (the text may be a secret: it is not repeated)
            return Err(("override".into(), "an override is `key=value`".into()));
        };
        let k = k.trim();
        if !ls.iter().any(|l| l.path == k) {
            return Err((
                k.to_string(),
                format!("unknown setting in an override: the configuration type has no field `{k}`"),
            ));
        }
        over.push((k.to_string(), v.to_string()));
    }
    if mode.strict && !pre.is_empty() {
        let names = crate::repro::external("env_names", pre, None, false, || {
            let mut v: Vec<String> = std::env::vars_os()
                .filter_map(|(k, _)| k.into_string().ok())
                .filter(|k| k.starts_with(pre))
                .collect();
            v.sort();
            v
        });
        for n in names {
            if !ls.iter().any(|l| l.env == n) {
                return Err((n.clone(), format!("unknown setting: no configuration field reads `{n}`")));
            }
        }
    }
    let mut cands: Vec<Vec<Candidate>> = vec![];
    for l in &ls {
        let mut cs: Vec<Candidate> = vec![];
        let mut d = &defaults;
        for part in l.path.split('.') {
            d = d.get(part).unwrap_or(&serde_json::Value::Null);
        }
        if !d.is_null() {
            cs.push(Candidate { source: "default", origin: l.path.clone(), value: d.clone() });
        }
        for (k, v, src) in &profile {
            let (source, origin) =
                if src.is_empty() { ("config", k.clone()) } else { ("profile", format!("{src}.{k}")) };
            if *k == l.path {
                let v = expand_env(v).map_err(|m| (k.clone(), m))?;
                cs.push(Candidate { source, origin, value: leaf_json(l.desc, k, v)? });
            } else if let Some(vals) = map_values(l.desc)
                && let Some(entry) = k.strip_prefix(&l.path).and_then(|r| r.strip_prefix('.'))
            {
                // a map's entries, as `[config.labels] a = "x"`: merged
                // into the map the earlier sources gave
                let v = expand_env(v).map_err(|m| (k.clone(), m))?;
                let item = leaf_json(vals, k, v)?;
                let origin = if src.is_empty() { l.path.clone() } else { format!("{src}.{}", l.path) };
                // entries of one table are one candidate
                let same = cs.last().is_some_and(|c| c.source == source && c.origin == origin);
                let mut base = match cs.last().map(|c| &c.value) {
                    Some(serde_json::Value::Object(m)) => m.clone(),
                    _ => serde_json::Map::new(),
                };
                base.insert(entry.to_string(), item);
                if same {
                    cs.pop();
                }
                cs.push(Candidate { source, origin, value: serde_json::Value::Object(base) });
            }
        }
        if let Some(v) = env_var(&l.env) {
            cs.push(Candidate { source: "env", origin: l.env.clone(), value: leaf_json(l.desc, &l.env, v)? });
        }
        for (k, v) in &over {
            if *k == l.path {
                let shown = if holds_secret(l.desc, 0) { "[redacted]" } else { v.as_str() };
                cs.push(Candidate {
                    source: "override",
                    origin: format!("{k}={shown}"),
                    value: leaf_json(l.desc, k, v.clone())?,
                });
            }
        }
        cands.push(cs);
    }
    // an optional record none of whose fields is set is `none`
    let unset: std::collections::HashSet<String> = ls
        .iter()
        .filter_map(|l| l.optional.clone())
        .filter(|g| {
            ls.iter().zip(&cands).all(|(m, cs)| m.optional.as_deref() != Some(g.as_str()) || cs.is_empty())
        })
        .collect();
    let mut obj = serde_json::Map::new();
    let mut rows = vec![];
    let place =
        |obj: &mut serde_json::Map<String, serde_json::Value>, path: &str, value: serde_json::Value| {
            let mut at = obj;
            let parts: Vec<&str> = path.split('.').collect();
            for p in &parts[..parts.len() - 1] {
                at = at
                    .entry(p.to_string())
                    .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
                    .as_object_mut()
                    .expect("nested configuration object");
            }
            at.insert(parts[parts.len() - 1].to_string(), value);
        };
    for g in &unset {
        place(&mut obj, g, serde_json::Value::Null);
    }
    for (l, cs) in ls.into_iter().zip(cands) {
        if l.optional.as_ref().is_some_and(|g| unset.contains(g)) {
            continue;
        }
        let value = match cs.last() {
            Some(c) => c.value.clone(),
            None if crate::codec::omittable(l.desc) => serde_json::Value::Null,
            None => {
                let msg = if !mode.profile {
                    "missing required environment variable".to_string()
                } else if mode.strict {
                    format!(
                        "missing required setting (`{}` in [config], environment variable `{}`, or an override)",
                        l.path, l.env
                    )
                } else {
                    format!(
                        "missing required setting (`{}` in [config] or environment variable `{}`)",
                        l.path, l.env
                    )
                };
                return Err((if mode.profile { l.path.clone() } else { l.env.clone() }, msg));
            }
        };
        place(&mut obj, &l.path, value);
        rows.push((l, cs));
    }
    Ok((obj, rows))
}

fn resolve_err(ret: &'static TypeDesc, key: &str, msg: &str) -> u64 {
    let g = Roots::new();
    let err_desc = ret.variant(1).unwrap().fields[0].desc;
    let k = g.add(new_string(key));
    let m = g.add(new_string(msg));
    let e = g.add(make_record(err_desc, &[("key", k), ("message", m)]));
    make_variant(ret, "err", &[e])
}

/// Decode resolved layers into `T`, naming a bad value's setting as its
/// source does.
#[allow(clippy::type_complexity)]
fn decode_layers(
    t: &'static TypeDesc,
    ret: &'static TypeDesc,
    layers: Result<
        (serde_json::Map<String, serde_json::Value>, Vec<(Leaf, Vec<Candidate>)>),
        (String, String),
    >,
) -> u64 {
    let (obj, rows) = match layers {
        Ok(x) => x,
        Err((k, m)) => return resolve_err(ret, &k, &m),
    };
    match codec::decode(t, &serde_json::Value::Object(obj).to_string()) {
        Ok(w) => make_variant(ret, "ok", &[w]),
        Err(e) => {
            let path = e.path.trim_start_matches('$').trim_start_matches('.');
            let field = path.split('[').next().unwrap_or(path);
            let key = rows
                .iter()
                .find(|(l, _)| l.path == field)
                .and_then(|(_, cs)| cs.last())
                .map(|c| c.origin.clone())
                .unwrap_or(field.to_string());
            resolve_err(ret, &key, &e.message)
        }
    }
}

/// `config.resolve`: the layered, strict configuration of `T`.
pub extern "C" fn config_resolve(
    _ctx: *mut TaskCtx,
    targs: Targs,
    ret: *const TypeDesc,
    prefix: u64,
    overrides: u64,
    defaults: u64,
) -> u64 {
    use crate::desc::Kind;
    let t = targ(targs, 0);
    let ret = rdesc(ret);
    let pre = str_of(prefix).to_string();
    let Kind::Record { .. } = &t.kind else {
        return resolve_err(ret, &pre, "configuration type must be a record");
    };
    let over: Vec<String> =
        crate::list::to_vec(overrides).into_iter().map(|w| str_of(w).to_string()).collect();
    decode_layers(
        t,
        ret,
        resolve_layers(t, &pre, &over, str_of(defaults), Mode { strict: true, profile: true }),
    )
}

/// `config.resolve`'s provenance: JSON of `List<config.Setting>`.
pub extern "C" fn config_provenance(
    _ctx: *mut TaskCtx,
    targs: Targs,
    _ret: *const TypeDesc,
    prefix: u64,
    overrides: u64,
    defaults: u64,
) -> u64 {
    let t = targ(targs, 0);
    let pre = str_of(prefix).to_string();
    let over: Vec<String> =
        crate::list::to_vec(overrides).into_iter().map(|w| str_of(w).to_string()).collect();
    let rows = resolve_layers(t, &pre, &over, str_of(defaults), Mode { strict: true, profile: true })
        .map(|r| r.1)
        .unwrap_or_default();
    let out: Vec<serde_json::Value> = rows
        .into_iter()
        .filter_map(|(l, cs)| {
            let c = cs.last()?;
            let secret = holds_secret(l.desc, 0) || crate::repro::secret_name(&l.path);
            let value = if secret {
                "[redacted]".to_string()
            } else {
                match &c.value {
                    serde_json::Value::String(s) => s.clone(),
                    v => v.to_string(),
                }
            };
            Some(serde_json::json!({
                "key": l.path,
                "source": c.source,
                "origin": c.origin,
                "value": value,
                "overridden": cs[..cs.len() - 1].iter().map(|x| x.source).collect::<Vec<_>>(),
            }))
        })
        .collect();
    new_string(&serde_json::Value::Array(out).to_string())
}

pub fn extra_symbols() -> Vec<(&'static str, *const u8)> {
    vec![
        ("badger_rt_i_config_from_env", config_from_env as *const u8),
        ("badger_rt_i_config_load", config_load as *const u8),
        ("badger_rt_i_config_resolve", config_resolve as *const u8),
        ("badger_rt_i_config_provenance", config_provenance as *const u8),
    ]
}

/// `inspect.tasks()`: a `Data` list of task records.
pub extern "C" fn inspect_tasks(_ctx: *mut TaskCtx, _t: Targs, ret: *const TypeDesc) -> u64 {
    let text = crate::task::snapshot_json();
    let j: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    codec::data_of(rdesc(ret), &j)
}

/// `inspect.heap()`: object count, bytes, and collection count.
pub extern "C" fn inspect_heap(_ctx: *mut TaskCtx, _t: Targs, ret: *const TypeDesc) -> u64 {
    let (objects, bytes, collections) = crate::gc::stats();
    let (pause_total, pause_max) = crate::gc::pauses();
    let (managed_live, external_live) = crate::gc::memory_bytes();
    let j = serde_json::json!({
        "objects": objects,
        "bytes": bytes,
        "managed_live_bytes": managed_live,
        "external_live_bytes": external_live,
        "pool_reserved_bytes": crate::gc::pool_reserved_bytes(),
        "collections": collections,
        "pause_total_ns": pause_total,
        "pause_max_ns": pause_max,
    });
    codec::data_of(rdesc(ret), &j)
}

pub fn inspect_symbols() -> Vec<(&'static str, *const u8)> {
    vec![
        ("badger_rt_i_inspect_tasks", inspect_tasks as *const u8),
        ("badger_rt_i_inspect_heap", inspect_heap as *const u8),
    ]
}

/// Wire naming from `Map<String, String>` names and
/// `Map<String, List<String>>` decode aliases.
fn wire_names(names: u64, aliases: u64) -> codec::WireNames {
    let mut w = codec::WireNames::default();
    for (k, v) in crate::map::entries(names) {
        w.names.insert(str_of(k).to_string(), str_of(v).to_string());
    }
    for (k, v) in crate::map::entries(aliases) {
        let olds = crate::list::to_vec(v).into_iter().map(|x| str_of(x).to_string()).collect();
        w.aliases.insert(str_of(k).to_string(), olds);
    }
    w
}

#[cfg(test)]
mod tests {
    use super::duration_nanos;

    #[test]
    fn durations_are_read_in_the_language_spelling() {
        assert_eq!(duration_nanos("10s"), Some(10_000_000_000));
        assert_eq!(duration_nanos("500ms"), Some(500_000_000));
        assert_eq!(duration_nanos("2us"), Some(2_000));
        assert_eq!(duration_nanos("7ns"), Some(7));
        assert_eq!(duration_nanos("5min"), Some(300_000_000_000));
        assert_eq!(duration_nanos("1h"), Some(3_600_000_000_000));
    }

    #[test]
    fn a_plain_number_is_milliseconds() {
        assert_eq!(duration_nanos("500"), Some(500_000_000));
        assert_eq!(duration_nanos(" 500 "), Some(500_000_000));
    }

    #[test]
    fn other_text_is_not_a_duration() {
        assert_eq!(duration_nanos(""), None);
        assert_eq!(duration_nanos("soon"), None);
        assert_eq!(duration_nanos("10days"), None);
        assert_eq!(duration_nanos("10 s"), None);
    }
}
