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

macro_rules! codec_intrinsics {
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

codec_intrinsics! {
    fn wire_encode(ctx, targs, ret, names, aliases, v) {
        let w = wire_names(names, aliases);
        codec::with_wire(w, || new_string(&codec::encode(targ(targs, 0), v)))
    }
    fn wire_decode(ctx, targs, ret, names, aliases, s) {
        let text = str_of(s).to_string();
        let w = wire_names(names, aliases);
        let r = codec::with_wire(w, || codec::decode(targ(targs, 0), &text));
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
            let field = d.field(&key).map(|f| f.desc);
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
        let text = serde_json::Value::Object(obj).to_string();
        decode_result(rdesc(ret), codec::decode(d, &text))
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
    fn codec_encode_bytes(ctx, targs, ret, v) { crate::string::new_bytes(codec::encode(targ(targs, 0), v).as_bytes()) }
    fn codec_decode_text(ctx, targs, ret, s) {
        let text = str_of(s).to_string();
        decode_result(rdesc(ret), codec::decode(targ(targs, 0), &text))
    }
    fn codec_decode_bytes(ctx, targs, ret, b) {
        let text = String::from_utf8_lossy(bytes_of(b)).into_owned();
        decode_result(rdesc(ret), codec::decode(targ(targs, 0), &text))
    }
    fn codec_from_data(ctx, targs, ret, d) { decode_result(rdesc(ret), codec::from_data(targ(targs, 0), d)) }
    fn codec_to_data(ctx, targs, ret, v) {
        let text = codec::encode(targ(targs, 0), v);
        let j: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
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
    /// Bounded queue to the `LANG_TRACE` stderr exporter thread.
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
        if std::env::var_os("LANG_TRACE").is_some() {
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
/// with `[profile.NAME]` from `lang.toml`), set by the driver before `main`.
static PROFILE: std::sync::Mutex<Vec<(String, String)>> = std::sync::Mutex::new(Vec::new());

pub fn set_profile(entries: Vec<(String, String)>) {
    *PROFILE.lock().unwrap() = entries;
}

pub extern "C" fn config_from_env(ctx: *mut TaskCtx, targs: Targs, ret: *const TypeDesc, prefix: u64) -> u64 {
    config_decode(ctx, targs, ret, prefix, false)
}

/// Profile data overlaid by prefixed environment variables.
pub extern "C" fn config_load(ctx: *mut TaskCtx, targs: Targs, ret: *const TypeDesc, prefix: u64) -> u64 {
    config_decode(ctx, targs, ret, prefix, true)
}

/// Build the JSON object for a configuration record: each field comes from
/// the environment (`PREFIX` + upper-cased path, `_`-joined) or the profile
/// data (dotted path); nested records recurse.
fn config_object(
    t: &TypeDesc,
    env_pre: &str,
    key_pre: &str,
    profile_data: &[(String, String)],
    profile: bool,
) -> Result<serde_json::Map<String, serde_json::Value>, (String, String)> {
    use crate::desc::Kind;
    let Kind::Record { fields, .. } = &t.kind else { return Ok(serde_json::Map::new()) };
    let mut obj = serde_json::Map::new();
    for f in fields {
        let path = format!("{key_pre}{}", f.name);
        if let Kind::Record { nominal: false, .. } | Kind::Record { nominal: true, tuple: false, .. } =
            &f.desc.kind
        {
            let inner = config_object(
                f.desc,
                &format!("{env_pre}{}_", f.name.to_uppercase()),
                &format!("{path}."),
                profile_data,
                profile,
            )?;
            obj.insert(f.name.clone(), serde_json::Value::Object(inner));
            continue;
        }
        let env_key = format!("{env_pre}{}", f.name.to_uppercase());
        let (key, raw) = match std::env::var(&env_key) {
            Ok(v) => (env_key, Ok(v)),
            Err(e) => match profile_data.iter().find(|(k, _)| *k == path) {
                // `${NAME}` and `${NAME:-default}` in a configured value
                Some((k, v)) => (k.clone(), Ok(expand_env(v).map_err(|m| (k.clone(), m))?)),
                None => (env_key, Err(e)),
            },
        };
        let bad = |k: &str, m: String| (k.to_string(), m);
        let j = match (&f.desc.kind, raw) {
            (_, Err(_)) if f.desc.name == "Option" => serde_json::Value::Null,
            (_, Err(_)) if profile => {
                return Err(bad(
                    &path,
                    format!(
                        "missing required setting (profile key `{path}` or environment variable `{key}`)"
                    ),
                ));
            }
            (_, Err(_)) => return Err(bad(&key, "missing required environment variable".into())),
            (Kind::String | Kind::Symbol, Ok(v)) => serde_json::Value::String(v),
            (Kind::Bool, Ok(v)) => match v.as_str() {
                "true" | "1" => serde_json::Value::Bool(true),
                "false" | "0" => serde_json::Value::Bool(false),
                _ => return Err(bad(&key, "expected true or false".into())),
            },
            (Kind::Int { .. }, Ok(v)) => match v.trim().parse::<i64>() {
                Ok(n) => serde_json::Value::from(n),
                Err(_) => return Err(bad(&key, format!("expected an integer, found `{v}`"))),
            },
            (Kind::Float { .. }, Ok(v)) => match v.trim().parse::<f64>() {
                Ok(n) => serde_json::json!(n),
                Err(_) => return Err(bad(&key, format!("expected a number, found `{v}`"))),
            },
            (_, Ok(v)) => serde_json::from_str(&v).unwrap_or(serde_json::Value::String(v)),
        };
        obj.insert(f.name.clone(), j);
    }
    Ok(obj)
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
        match (std::env::var(name).ok().filter(|x| !x.is_empty() || default.is_none()), default) {
            (Some(x), _) => out.push_str(&x),
            (None, Some(d)) => out.push_str(d),
            (None, None) => return Err(format!("environment variable `{name}` is not set")),
        }
        rest = &rest[i + end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

fn config_decode(_ctx: *mut TaskCtx, targs: Targs, ret: *const TypeDesc, prefix: u64, profile: bool) -> u64 {
    use crate::desc::Kind;
    let t = targ(targs, 0);
    let ret = rdesc(ret);
    let pre = str_of(prefix).to_string();
    let profile_data = if profile { PROFILE.lock().unwrap().clone() } else { vec![] };
    let err = |key: &str, msg: &str| {
        let g = Roots::new();
        let err_desc = ret.variant(1).unwrap().fields[0].desc;
        let k = g.add(new_string(key));
        let m = g.add(new_string(msg));
        let e = g.add(make_record(err_desc, &[("key", k), ("message", m)]));
        make_variant(ret, "err", &[e])
    };
    let Kind::Record { .. } = &t.kind else { return err(&pre, "configuration type must be a record") };
    let obj = match config_object(t, &pre, "", &profile_data, profile) {
        Ok(o) => o,
        Err((k, m)) => return err(&k, &m),
    };
    match codec::decode(t, &serde_json::Value::Object(obj).to_string()) {
        Ok(w) => make_variant(ret, "ok", &[w]),
        Err(e) => {
            let key = format!("{pre}{}", e.path.trim_start_matches("$.").to_uppercase());
            err(&key, &e.message)
        }
    }
}

pub fn extra_symbols() -> Vec<(&'static str, *const u8)> {
    vec![
        ("lang_rt_i_config_from_env", config_from_env as *const u8),
        ("lang_rt_i_config_load", config_load as *const u8),
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
    let j = serde_json::json!({
        "objects": objects,
        "bytes": bytes,
        "collections": collections,
        "pause_total_ns": pause_total,
        "pause_max_ns": pause_max,
    });
    codec::data_of(rdesc(ret), &j)
}

pub fn inspect_symbols() -> Vec<(&'static str, *const u8)> {
    vec![
        ("lang_rt_i_inspect_tasks", inspect_tasks as *const u8),
        ("lang_rt_i_inspect_heap", inspect_heap as *const u8),
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
