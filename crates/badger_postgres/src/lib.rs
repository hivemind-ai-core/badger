//! `badger_postgres`: the PostgreSQL client native module behind
//! `std.postgres` (tokio-postgres with a deadpool pool, on a small tokio
//! runtime of the module's own).
//!
//! Every call is async: the calling task parks until the reply. A pool
//! hands each statement a connection of its own; a transaction holds one
//! from `begin` to `finish` and always ends in `COMMIT` or `ROLLBACK`
//! before the connection is reused (one whose rollback fails is closed).
//! A cancelled statement is cancelled on the server too, and its
//! connection returns to the pool once the server has answered.
//!
//! Parameters are `Data` sent in text format, so the server parses them as
//! the statement's parameter types (numbers, text, timestamps, UUIDs,
//! enums...); a list is an array literal, and any other value given for a
//! `json` or `jsonb` parameter is sent as JSON (null is SQL NULL). Results come back as `Data`
//! objects by column name: integers, floats and booleans as themselves,
//! `numeric` as exact decimal text, dates and times as ISO 8601 text,
//! `json`/`jsonb` as their values, arrays as lists, `bytea` as `\x` hex
//! text, and other text-like types (enums, `citext`) as text.

use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use badger_native::{Args, CallId, Completion, Error, Handler, Handles, ModuleDef, Outcome, Value};
use bytes::BytesMut;
use deadpool_postgres::{Manager, ManagerConfig, Object, Pool, RecyclingMethod};
use tokio::sync::oneshot;
use tokio_postgres::types::{Format, FromSql, IsNull, Kind, ToSql, Type, to_sql_checked};
use tokio_postgres::{CancelToken, Statement};

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .on_thread_start(badger_native::module_thread)
            .worker_threads(2)
            .thread_name("badger-postgres")
            .enable_all()
            .build()
            .expect("the Postgres module's runtime")
    })
}

/// A transaction's connection, until `finish` (or the handle's drop)
/// gives it back.
struct Tx {
    conn: tokio::sync::Mutex<Option<Object>>,
    token: CancelToken,
    /// A statement failed on the server, which aborts the transaction (a
    /// `COMMIT` would roll back without an error).
    failed: AtomicBool,
}

enum Target {
    Pool(Pool),
    Tx(Box<Tx>),
}

static HANDLES: Handles<Target> = Handles::new();

/// Waiting calls, so `cancel` can stop them.
static WAITING: Mutex<Option<HashMap<CallId, oneshot::Sender<()>>>> = Mutex::new(None);

/// The connection a running statement uses, for a server-side cancel.
type TokenSlot = Arc<Mutex<Option<CancelToken>>>;

/// Run `work` for the call `done`. A cancel completes the call at once,
/// asks the server to cancel the statement, and lets `work` finish (so its
/// connection goes back to the pool in a known state).
fn spawn_call<F>(done: Completion, work: impl FnOnce(TokenSlot) -> F)
where
    F: Future<Output = Outcome> + Send + 'static,
{
    let id = done.id();
    let (stop, stopped) = oneshot::channel();
    WAITING.lock().unwrap().get_or_insert_default().insert(id, stop);
    let slot: TokenSlot = Arc::new(Mutex::new(None));
    let mut task = runtime().spawn(work(slot.clone()));
    runtime().spawn(async move {
        let outcome = tokio::select! {
            o = &mut task => o.unwrap_or_else(|e| Err(Error::new("postgres/error", format!("the call failed: {e}")))),
            _ = stopped => {
                let token = slot.lock().unwrap().take();
                if let Some(t) = token {
                    tokio::spawn(async move {
                        let _ = t.cancel_query(tokio_postgres::NoTls).await;
                    });
                }
                Err(Error::new("postgres/cancelled", "cancelled"))
            }
        };
        if let Some(w) = WAITING.lock().unwrap().as_mut() {
            w.remove(&id);
        }
        done.complete(outcome);
    });
}

fn cancel(id: CallId) {
    let stop = WAITING.lock().unwrap().as_mut().and_then(|w| w.remove(&id));
    if let Some(stop) = stop {
        let _ = stop.send(());
    }
}

// ---------------------------------------------------------------- errors

fn error(e: tokio_postgres::Error) -> Error {
    let Some(db) = e.as_db_error() else {
        if e.is_closed() || e.to_string().contains("connect") {
            return Error::new("postgres/connection", e.to_string());
        }
        return Error::new("postgres/error", e.to_string());
    };
    let kind = match db.code().code() {
        "23505" => "postgres/unique_violation",
        "23503" => "postgres/foreign_key_violation",
        "23502" => "postgres/not_null_violation",
        "23514" => "postgres/check_violation",
        "40001" => "postgres/serialization_failure",
        "40P01" => "postgres/deadlock",
        "57014" => "postgres/cancelled",
        "42601" => "postgres/syntax",
        "42P01" => "postgres/undefined_table",
        "42703" => "postgres/undefined_column",
        "28P01" | "28000" => "postgres/auth",
        "3D000" => "postgres/connection",
        _ => "postgres/error",
    };
    let mut message = format!("{} (SQLSTATE {})", db.message(), db.code().code());
    if let Some(d) = db.detail() {
        message.push_str(&format!(": {d}"));
    }
    Error::new(kind, message)
}

fn pool_error(e: deadpool_postgres::PoolError) -> Error {
    match e {
        deadpool_postgres::PoolError::Backend(e) => error(e),
        deadpool_postgres::PoolError::Closed => Error::new("postgres/closed", "the pool is closed"),
        other => Error::new("postgres/connection", other.to_string()),
    }
}

// ---------------------------------------------------------------- parameters

/// A parameter as Postgres text.
struct Param<'a>(&'a Value);

impl std::fmt::Debug for Param<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

fn is_json(ty: &Type) -> bool {
    *ty == Type::JSON || *ty == Type::JSONB
}

fn element(ty: &Type) -> Option<&Type> {
    match ty.kind() {
        Kind::Array(e) => Some(e),
        Kind::Domain(t) => element(t),
        _ => None,
    }
}

fn json_of(v: &Value) -> serde_json::Value {
    use serde_json::Value as J;
    match v {
        Value::Unit | Value::Null => J::Null,
        Value::Bool(b) => J::Bool(*b),
        Value::Int(i) => J::from(*i),
        Value::Float(f) => serde_json::Number::from_f64(*f).map_or(J::Null, J::Number),
        Value::Str(s) | Value::Symbol(s) => J::String(s.clone()),
        Value::Char(c) => J::String(c.to_string()),
        Value::Bytes(b) => J::String(String::from_utf8_lossy(b).into_owned()),
        Value::List(xs) => J::Array(xs.iter().map(json_of).collect()),
        Value::Map(kvs) => J::Object(kvs.iter().map(|(k, v)| (k.clone(), json_of(v))).collect()),
        Value::Variant(n, xs) => match xs.as_slice() {
            [] => J::String(n.clone()),
            [x] => json_of(x),
            xs => J::Array(xs.iter().map(json_of).collect()),
        },
        Value::Handle(h) => J::from(*h),
    }
}

fn text_of(v: &Value, ty: &Type) -> Result<String, String> {
    if is_json(ty) {
        return Ok(json_of(v).to_string());
    }
    Ok(match v {
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) if f.is_nan() => "NaN".into(),
        Value::Float(f) if f.is_infinite() => if *f > 0.0 { "Infinity" } else { "-Infinity" }.into(),
        Value::Float(f) => format!("{f:?}"),
        Value::Str(s) | Value::Symbol(s) => s.clone(),
        Value::Char(c) => c.to_string(),
        Value::Bytes(b) => {
            let mut s = String::from("\\x");
            for x in b {
                s.push_str(&format!("{x:02x}"));
            }
            s
        }
        Value::List(xs) => {
            let Some(elem) = element(ty) else {
                return Err(format!("a list was given for a parameter of type {}", ty.name()));
            };
            let mut s = String::from("{");
            for (i, x) in xs.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                match x {
                    Value::Null | Value::Unit => s.push_str("NULL"),
                    Value::List(_) => s.push_str(&text_of(x, ty)?),
                    x => {
                        s.push('"');
                        for c in text_of(x, elem)?.chars() {
                            if c == '"' || c == '\\' {
                                s.push('\\');
                            }
                            s.push(c);
                        }
                        s.push('"');
                    }
                }
            }
            s.push('}');
            s
        }
        Value::Map(_) => return Err(format!("an object was given for a parameter of type {}", ty.name())),
        Value::Unit | Value::Null => unreachable!("NULL is sent as such"),
        Value::Variant(n, xs) if xs.is_empty() => n.clone(),
        other => return Err(format!("{other:?} cannot be a parameter of type {}", ty.name())),
    })
}

impl ToSql for Param<'_> {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
        match self.0 {
            Value::Null | Value::Unit => Ok(IsNull::Yes),
            v => {
                out.extend_from_slice(text_of(v, ty)?.as_bytes());
                Ok(IsNull::No)
            }
        }
    }

    fn accepts(_: &Type) -> bool {
        true
    }

    fn encode_format(&self, _ty: &Type) -> Format {
        Format::Text
    }

    to_sql_checked!();
}

// ---------------------------------------------------------------- results

/// A column value as `Data`.
struct Cell(Value);

const POSTGRES_EPOCH_DAYS: i64 = 10957; // 2000-01-01 from 1970-01-01

/// The civil date of a day count from 1970-01-01 (Howard Hinnant's
/// algorithm).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

fn date_text(days_from_2000: i64) -> String {
    let (y, m, d) = civil(days_from_2000 + POSTGRES_EPOCH_DAYS);
    if !(0..=9999).contains(&y) {
        // years outside 0000-9999 carry a sign (ISO 8601 expanded)
        format!("{y:+05}-{m:02}-{d:02}")
    } else {
        format!("{y:04}-{m:02}-{d:02}")
    }
}

fn time_text(micros: i64) -> String {
    let (s, us) = (micros.div_euclid(1_000_000), micros.rem_euclid(1_000_000));
    let t = format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60);
    if us == 0 { t } else { format!("{t}.{us:06}") }
}

fn timestamp_text(micros: i64, zone: bool) -> String {
    match micros {
        i64::MAX => "infinity".into(),
        i64::MIN => "-infinity".into(),
        _ => {
            let days = micros.div_euclid(86_400_000_000);
            let rest = micros.rem_euclid(86_400_000_000);
            format!("{}T{}{}", date_text(days), time_text(rest), if zone { "Z" } else { "" })
        }
    }
}

/// Binary `numeric` as exact decimal text.
fn numeric_text(raw: &[u8]) -> Result<String, String> {
    let word = |i: usize| -> Result<i16, String> {
        raw.get(i..i + 2).map(|b| i16::from_be_bytes([b[0], b[1]])).ok_or_else(|| "short numeric".to_string())
    };
    let (ndigits, weight, sign, dscale) =
        (word(0)? as usize, word(2)? as i64, word(4)? as u16, word(6)? as usize);
    match sign {
        0xC000 => return Ok("NaN".into()),
        0xD000 => return Ok("Infinity".into()),
        0xF000 => return Ok("-Infinity".into()),
        _ => {}
    }
    let digits: Vec<i16> = (0..ndigits).map(|i| word(8 + 2 * i)).collect::<Result<_, _>>()?;
    let mut int = String::new();
    for w in 0..=weight.max(0) {
        let d = if w <= weight { digits.get(w as usize).copied().unwrap_or(0) } else { 0 };
        if int.is_empty() {
            if weight >= 0 {
                int = d.to_string();
            }
        } else {
            int.push_str(&format!("{d:04}"));
        }
    }
    if int.is_empty() {
        int = "0".into();
    }
    let mut frac = String::new();
    let mut w = weight + 1;
    while frac.len() < dscale {
        let d = if w < 0 { 0 } else { digits.get(w as usize).copied().unwrap_or(0) };
        frac.push_str(&format!("{d:04}"));
        w += 1;
    }
    frac.truncate(dscale);
    let neg = if sign == 0x4000 { "-" } else { "" };
    Ok(if frac.is_empty() { format!("{neg}{int}") } else { format!("{neg}{int}.{frac}") })
}

fn value_of(v: serde_json::Value) -> Value {
    use serde_json::Value as J;
    match v {
        J::Null => Value::Null,
        J::Bool(b) => Value::Bool(b),
        J::Number(n) => {
            n.as_i64().map(Value::Int).unwrap_or_else(|| Value::Float(n.as_f64().unwrap_or(f64::NAN)))
        }
        J::String(s) => Value::Str(s),
        J::Array(xs) => Value::List(xs.into_iter().map(value_of).collect()),
        J::Object(kvs) => Value::Map(kvs.into_iter().map(|(k, v)| (k, value_of(v))).collect()),
    }
}

fn decode(ty: &Type, raw: &[u8]) -> Result<Value, String> {
    use postgres_protocol::types as pt;
    let e = |e: Box<dyn std::error::Error + Sync + Send>| e.to_string();
    Ok(match *ty {
        Type::BOOL => Value::Bool(pt::bool_from_sql(raw).map_err(e)?),
        Type::INT2 => Value::Int(pt::int2_from_sql(raw).map_err(e)?.into()),
        Type::INT4 => Value::Int(pt::int4_from_sql(raw).map_err(e)?.into()),
        Type::INT8 => Value::Int(pt::int8_from_sql(raw).map_err(e)?),
        Type::OID => Value::Int(pt::oid_from_sql(raw).map_err(e)?.into()),
        Type::FLOAT4 => Value::Float(pt::float4_from_sql(raw).map_err(e)?.into()),
        Type::FLOAT8 => Value::Float(pt::float8_from_sql(raw).map_err(e)?),
        Type::NUMERIC => Value::Str(numeric_text(raw)?),
        Type::CHAR => Value::Str((pt::char_from_sql(raw).map_err(e)? as u8 as char).to_string()),
        Type::BYTEA => {
            let mut s = String::from("\\x");
            for x in pt::bytea_from_sql(raw) {
                s.push_str(&format!("{x:02x}"));
            }
            Value::Str(s)
        }
        Type::JSON => value_of(serde_json::from_slice(raw).map_err(|e| e.to_string())?),
        Type::JSONB => {
            let body = raw.strip_prefix(&[1u8]).ok_or("unknown jsonb version")?;
            value_of(serde_json::from_slice(body).map_err(|e| e.to_string())?)
        }
        Type::UUID => {
            let b = pt::uuid_from_sql(raw).map_err(e)?;
            let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
            Value::Str(format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..]))
        }
        Type::DATE => match pt::date_from_sql(raw).map_err(e)? {
            i32::MAX => Value::Str("infinity".into()),
            i32::MIN => Value::Str("-infinity".into()),
            d => Value::Str(date_text(d.into())),
        },
        Type::TIME => Value::Str(time_text(pt::time_from_sql(raw).map_err(e)?)),
        Type::TIMESTAMP => Value::Str(timestamp_text(pt::timestamp_from_sql(raw).map_err(e)?, false)),
        Type::TIMESTAMPTZ => Value::Str(timestamp_text(pt::timestamp_from_sql(raw).map_err(e)?, true)),
        _ => match ty.kind() {
            Kind::Array(elem) => {
                let a = pt::array_from_sql(raw).map_err(e)?;
                let mut items = vec![];
                let mut values = a.values();
                while let Some(v) = fallible_next(&mut values)? {
                    items.push(match v {
                        Some(b) => decode(elem, b)?,
                        None => Value::Null,
                    });
                }
                // multi-dimensional arrays nest by their dimensions
                let dims: Vec<usize> = {
                    let mut d = a.dimensions();
                    let mut out = vec![];
                    while let Some(x) = fallible_dim(&mut d)? {
                        out.push(x);
                    }
                    out
                };
                nest(items, &dims)
            }
            Kind::Domain(base) => decode(base, raw)?,
            Kind::Enum(_) => Value::Str(String::from_utf8_lossy(raw).into_owned()),
            _ => match (ty.name(), std::str::from_utf8(raw)) {
                ("text" | "varchar" | "bpchar" | "name" | "citext" | "unknown" | "xml", Ok(s)) => {
                    Value::Str(s.into())
                }
                (_, Ok(s)) if is_texty(ty) => Value::Str(s.into()),
                _ => {
                    return Err(format!(
                        "values of type {} are not supported; cast the column (`::text`)",
                        ty.name()
                    ));
                }
            },
        },
    })
}

/// Types whose binary form is their text.
fn is_texty(ty: &Type) -> bool {
    matches!(*ty, Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME | Type::UNKNOWN | Type::XML)
        || ty.name() == "citext"
}

fn fallible_next<'a>(
    v: &mut postgres_protocol::types::ArrayValues<'a>,
) -> Result<Option<Option<&'a [u8]>>, String> {
    use fallible_iterator::FallibleIterator;
    v.next().map_err(|e| e.to_string())
}

fn fallible_dim(d: &mut postgres_protocol::types::ArrayDimensions<'_>) -> Result<Option<usize>, String> {
    use fallible_iterator::FallibleIterator;
    d.next().map(|o| o.map(|x| x.len.max(0) as usize)).map_err(|e| e.to_string())
}

fn nest(items: Vec<Value>, dims: &[usize]) -> Value {
    if dims.len() <= 1 {
        return Value::List(items);
    }
    let inner: usize = dims[1..].iter().product();
    let mut out = vec![];
    let mut it = items.into_iter();
    for _ in 0..dims[0] {
        let chunk: Vec<Value> = it.by_ref().take(inner).collect();
        out.push(nest(chunk, &dims[1..]));
    }
    Value::List(out)
}

impl<'a> FromSql<'a> for Cell {
    fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Cell, Box<dyn std::error::Error + Sync + Send>> {
        decode(ty, raw).map(Cell).map_err(Into::into)
    }

    fn from_sql_null(_: &Type) -> Result<Cell, Box<dyn std::error::Error + Sync + Send>> {
        Ok(Cell(Value::Null))
    }

    fn accepts(_: &Type) -> bool {
        true
    }
}

fn row_value(row: &tokio_postgres::Row) -> Result<Value, Error> {
    let mut fields = Vec::with_capacity(row.len());
    for (i, c) in row.columns().iter().enumerate() {
        let v = row.try_get::<_, Cell>(i).map_err(|e| {
            Error::new("postgres/decode", format!("column {}: {}", c.name(), source_text(&e)))
        })?;
        fields.push((c.name().to_string(), v.0));
    }
    Ok(Value::Map(fields))
}

fn source_text(e: &tokio_postgres::Error) -> String {
    use std::error::Error as _;
    e.source().map_or_else(|| e.to_string(), |s| s.to_string())
}

// ---------------------------------------------------------------- calls

#[derive(Clone, Copy)]
enum Op {
    Execute,
    Query,
    QueryOne,
}

/// The statement's parameters, checked against its parameter count.
fn params<'a>(st: &Statement, values: &'a [Value]) -> Result<Vec<Param<'a>>, Error> {
    if st.params().len() != values.len() {
        return Err(Error::new(
            "postgres/parameters",
            format!("the statement takes {} parameter(s), {} given", st.params().len(), values.len()),
        ));
    }
    Ok(values.iter().map(Param).collect())
}

async fn run_on(client: &deadpool_postgres::ClientWrapper, sql: &str, values: &[Value], op: Op) -> Outcome {
    if let (Op::Execute, true) = (op, values.is_empty()) {
        // several statements may run together (a migration script)
        let msgs = client.simple_query(sql).await.map_err(error)?;
        let n = msgs
            .iter()
            .filter_map(|m| match m {
                tokio_postgres::SimpleQueryMessage::CommandComplete(n) => Some(*n),
                _ => None,
            })
            .next_back()
            .unwrap_or(0);
        return Ok(Value::Int(n as i64));
    }
    let st = client.prepare_cached(sql).await.map_err(error)?;
    let ps = params(&st, values)?;
    let refs: Vec<&(dyn ToSql + Sync)> = ps.iter().map(|p| p as &(dyn ToSql + Sync)).collect();
    match op {
        Op::Execute => Ok(Value::Int(client.execute(&st, &refs).await.map_err(error)? as i64)),
        Op::Query => {
            let rows = client.query(&st, &refs).await.map_err(error)?;
            Ok(Value::List(rows.iter().map(row_value).collect::<Result<_, _>>()?))
        }
        Op::QueryOne => {
            let rows = client.query(&st, &refs).await.map_err(error)?;
            match rows.as_slice() {
                [row] => row_value(row),
                rows => {
                    Err(Error::new("postgres/row_count", format!("expected one row, got {}", rows.len())))
                }
            }
        }
    }
}

/// `:name` placeholders as `$1`, `$2`, ... (a repeated name reuses its
/// number), with the names in number order. Strings, quoted identifiers,
/// comments, dollar-quoted bodies, and `::` casts are left alone.
fn number_named(sql: &str) -> (String, Vec<String>) {
    let cs: Vec<char> = sql.chars().collect();
    let (mut out, mut names) = (String::new(), Vec::<String>::new());
    let skip_quoted = |i: usize, q: char| {
        let mut j = i + 1;
        while j < cs.len() {
            if cs[j] == q {
                if cs.get(j + 1) == Some(&q) {
                    j += 2;
                    continue;
                }
                return j + 1;
            }
            j += 1;
        }
        cs.len()
    };
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        match c {
            '\'' | '"' => {
                let j = skip_quoted(i, c);
                out.extend(&cs[i..j]);
                i = j;
            }
            '-' if cs.get(i + 1) == Some(&'-') => {
                while i < cs.len() && cs[i] != '\n' {
                    out.push(cs[i]);
                    i += 1;
                }
            }
            '/' if cs.get(i + 1) == Some(&'*') => {
                let start = i;
                i += 2;
                while i + 1 < cs.len() && !(cs[i] == '*' && cs[i + 1] == '/') {
                    i += 1;
                }
                i = (i + 2).min(cs.len());
                out.extend(&cs[start..i]);
            }
            '$' => {
                // a dollar-quoted body: $$ ... $$ or $tag$ ... $tag$
                let mut j = i + 1;
                while j < cs.len() && (cs[j].is_alphanumeric() || cs[j] == '_') {
                    j += 1;
                }
                if j < cs.len() && cs[j] == '$' && !cs[i + 1..j].first().is_some_and(|c| c.is_ascii_digit()) {
                    let tag: String = cs[i..=j].iter().collect();
                    let rest: String = cs[j + 1..].iter().collect();
                    let end = rest
                        .find(&tag)
                        .map_or(cs.len(), |k| j + 1 + rest[..k].chars().count() + tag.chars().count());
                    out.extend(&cs[i..end]);
                    i = end;
                } else {
                    out.push(c);
                    i += 1;
                }
            }
            ':' if cs.get(i + 1) == Some(&':') => {
                out.push_str("::");
                i += 2;
            }
            ':' if cs.get(i + 1).is_some_and(|c| c.is_ascii_alphabetic() || *c == '_')
                && !(i > 0 && (cs[i - 1].is_alphanumeric() || cs[i - 1] == '_')) =>
            {
                let mut j = i + 1;
                while j < cs.len() && (cs[j].is_ascii_alphanumeric() || cs[j] == '_') {
                    j += 1;
                }
                let name: String = cs[i + 1..j].iter().collect();
                let n = match names.iter().position(|x| *x == name) {
                    Some(k) => k + 1,
                    None => {
                        names.push(name);
                        names.len()
                    }
                };
                out.push_str(&format!("${n}"));
                i = j;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    (out, names)
}

/// The statement and its positional values: a record binds `:name`
/// placeholders by field (see [`number_named`]); a list binds `$1`, ... in
/// order, and a single value is `$1`.
fn bind(sql: String, params: &Value) -> Result<(String, Vec<Value>), Error> {
    match params {
        Value::List(xs) => Ok((sql, xs.clone())),
        Value::Map(fs) if fs.is_empty() => Ok((sql, vec![])),
        Value::Map(fs) => {
            let (sql, names) = number_named(&sql);
            let values = names
                .iter()
                .map(|n| {
                    fs.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone()).ok_or_else(|| {
                        Error::new("postgres/parameters", format!("no field `{n}` in the parameters for `:{n}`"))
                    })
                })
                .collect::<Result<_, _>>()?;
            Ok((sql, values))
        }
        v => Ok((sql, vec![v.clone()])),
    }
}

fn statement(a: Args, done: Completion, op: Op) {
    let parsed = (|| {
        let target = HANDLES.get(a.handle(0)?)?;
        let params = a.get(2).ok_or_else(|| Error::new("postgres/parameters", "missing parameters"))?;
        let (sql, values) = bind(a.str(1)?.to_string(), params)?;
        Ok((target, sql, values))
    })();
    let (target, sql, values) = match parsed {
        Ok(x) => x,
        Err(e) => return done.complete(Err(e)),
    };
    spawn_call(done, move |slot| async move {
        match target.as_ref() {
            Target::Pool(pool) => {
                let conn = pool.get().await.map_err(pool_error)?;
                *slot.lock().unwrap() = Some(conn.cancel_token());
                run_on(&conn, &sql, &values, op).await
            }
            Target::Tx(tx) => {
                let guard = tx.conn.lock().await;
                let Some(conn) = guard.as_ref() else {
                    return Err(Error::new("postgres/tx_closed", "the transaction has ended"));
                };
                *slot.lock().unwrap() = Some(tx.token.clone());
                let r = run_on(conn, &sql, &values, op).await;
                if let Err(e) = &r
                    && !matches!(
                        e.kind.as_str(),
                        "postgres/parameters" | "postgres/decode" | "postgres/row_count"
                    )
                {
                    tx.failed.store(true, Ordering::SeqCst);
                }
                r
            }
        }
    });
}

fn execute(a: Args, done: Completion) {
    statement(a, done, Op::Execute)
}

fn query(a: Args, done: Completion) {
    statement(a, done, Op::Query)
}

fn query_one(a: Args, done: Completion) {
    statement(a, done, Op::QueryOne)
}

/// A pool for `url` of up to `max` connections; one is opened now, so a
/// bad address or credentials fail here.
fn connect(a: Args, done: Completion) {
    let parsed = (|| Ok((a.str(0)?.to_string(), a.int(1)?)))();
    let (url, max) = match parsed {
        Ok(x) => x,
        Err(e) => return done.complete(Err(e)),
    };
    spawn_call(done, move |_| async move {
        let mut config: tokio_postgres::Config =
            url.parse().map_err(|e: tokio_postgres::Error| Error::new("postgres/config", e.to_string()))?;
        if config.get_connect_timeout().is_none() {
            config.connect_timeout(Duration::from_secs(10));
        }
        let manager =
            Manager::from_config(config, tls(), ManagerConfig { recycling_method: RecyclingMethod::Fast });
        let pool = Pool::builder(manager)
            .max_size(max.clamp(1, 1024) as usize)
            .runtime(deadpool_postgres::Runtime::Tokio1)
            .build()
            .map_err(|e| Error::new("postgres/config", e.to_string()))?;
        drop(pool.get().await.map_err(pool_error)?);
        Ok(HANDLES.insert(Target::Pool(pool)))
    });
}

/// TLS by the Mozilla roots, for `sslmode=prefer` (the default) and
/// `require`; `sslmode=disable` connects in the clear.
fn tls() -> tokio_postgres_rustls::MakeRustlsConnect {
    static CONFIG: OnceLock<rustls::ClientConfig> = OnceLock::new();
    let config = CONFIG.get_or_init(|| {
        let roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
        rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .expect("TLS protocol versions")
            .with_root_certificates(roots)
            .with_no_client_auth()
    });
    tokio_postgres_rustls::MakeRustlsConnect::new(config.clone())
}

fn begin(a: Args, done: Completion) {
    let pool = match a.handle(0).and_then(|h| HANDLES.get(h)) {
        Ok(p) => p,
        Err(e) => return done.complete(Err(e)),
    };
    spawn_call(done, move |_| async move {
        let Target::Pool(pool) = pool.as_ref() else {
            return Err(Error::new("postgres/handle", "transactions begin on a pool"));
        };
        let conn = pool.get().await.map_err(pool_error)?;
        conn.batch_execute("BEGIN").await.map_err(error)?;
        let token = conn.cancel_token();
        Ok(HANDLES.insert(Target::Tx(Box::new(Tx {
            conn: tokio::sync::Mutex::new(Some(conn)),
            token,
            failed: AtomicBool::new(false),
        }))))
    });
}

/// End a transaction: `COMMIT` or `ROLLBACK`. The connection goes back
/// to the pool only in a clean state.
async fn end(tx: &Tx, commit: bool) -> Outcome {
    let Some(conn) = tx.conn.lock().await.take() else {
        return Err(Error::new("postgres/tx_closed", "the transaction has ended"));
    };
    let failed = tx.failed.load(Ordering::SeqCst);
    let r = conn.batch_execute(if commit && !failed { "COMMIT" } else { "ROLLBACK" }).await;
    if r.is_err() || conn.is_closed() {
        // its state is unknown: close it rather than reuse it
        drop(Object::take(conn));
    }
    r.map_err(error)?;
    if commit && failed {
        return Err(Error::new(
            "postgres/tx_aborted",
            "the transaction was rolled back: a statement in it failed",
        ));
    }
    Ok(Value::Unit)
}

fn finish(a: Args, done: Completion) {
    let parsed = (|| Ok((HANDLES.get(a.handle(0)?)?, a.bool(1)?)))();
    let (tx, commit) = match parsed {
        Ok(x) => x,
        Err(e) => return done.complete(Err(e)),
    };
    spawn_call(done, move |_| async move {
        let Target::Tx(tx) = tx.as_ref() else {
            return Err(Error::new("postgres/handle", "not a transaction"));
        };
        end(tx, commit).await
    });
}

/// A statement's parameter types and result columns (`badger generate
/// sql` and checks): each column's type and whether it can be NULL (a
/// column read straight from a table is nullable unless it is NOT NULL;
/// an expression's nullability is unknown).
fn describe(a: Args, done: Completion) {
    let parsed = (|| Ok((HANDLES.get(a.handle(0)?)?, a.str(1)?.to_string())))();
    let (target, sql) = match parsed {
        Ok(x) => x,
        Err(e) => return done.complete(Err(e)),
    };
    spawn_call(done, move |_| async move {
        let Target::Pool(pool) = target.as_ref() else {
            return Err(Error::new("postgres/handle", "describe takes a pool"));
        };
        let conn = pool.get().await.map_err(pool_error)?;
        let d = describe_on(&conn, &sql).await?;
        let nullable = |n: Option<bool>| n.map_or(Value::Null, Value::Bool);
        Ok(Value::Map(vec![
            ("params".into(), Value::List(d.params.into_iter().map(Value::Str).collect())),
            (
                "columns".into(),
                Value::List(
                    d.columns
                        .into_iter()
                        .map(|c| {
                            Value::Map(vec![
                                ("name".into(), Value::Str(c.name)),
                                ("sql_type".into(), Value::Str(c.sql_type)),
                                ("nullable".into(), nullable(c.nullable)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ]))
    });
}

/// A statement as the server describes it.
pub struct Described {
    /// Parameter types (`int8`, `text[]`, `enum mood`).
    pub params: Vec<String>,
    pub columns: Vec<DescribedColumn>,
}

pub struct DescribedColumn {
    pub name: String,
    pub sql_type: String,
    /// Known for a column read straight from a table; `None` for an
    /// expression.
    pub nullable: Option<bool>,
}

async fn describe_on(conn: &tokio_postgres::Client, sql: &str) -> Result<Described, Error> {
    let st = conn.prepare(sql).await.map_err(error)?;
    let mut columns = vec![];
    for c in st.columns() {
        let nullable = match (c.table_oid(), c.column_id()) {
            (Some(t), Some(n)) if t != 0 && n > 0 => conn
                .query_opt(
                    "SELECT attnotnull FROM pg_attribute WHERE attrelid = $1 AND attnum = $2",
                    &[&t, &n],
                )
                .await
                .map_err(error)?
                .map(|r| !r.get::<_, bool>(0)),
            _ => None,
        };
        columns.push(DescribedColumn { name: c.name().into(), sql_type: type_name(c.type_()), nullable });
    }
    Ok(Described { params: st.params().iter().map(type_name).collect(), columns })
}

/// Describe statements on one connection to `url` (`badger generate sql`):
/// each statement's description, or why the server rejects it. Fails if
/// the server cannot be reached. Call it from outside any async runtime.
pub fn describe_statements(url: &str, sqls: &[String]) -> Result<Vec<Result<Described, String>>, String> {
    let url = url.to_string();
    let sqls = sqls.to_vec();
    runtime().block_on(async move {
        let mut config: tokio_postgres::Config =
            url.parse().map_err(|e: tokio_postgres::Error| e.to_string())?;
        if config.get_connect_timeout().is_none() {
            config.connect_timeout(Duration::from_secs(10));
        }
        let (client, connection) = config.connect(tls()).await.map_err(|e| error(e).message)?;
        let driver = tokio::spawn(connection);
        let mut out = vec![];
        for sql in &sqls {
            out.push(describe_on(&client, sql).await.map_err(|e| e.message));
        }
        drop(client);
        let _ = driver.await;
        Ok(out)
    })
}

/// A type's name, with `[]` for arrays and the base type for domains.
fn type_name(ty: &Type) -> String {
    match ty.kind() {
        Kind::Array(e) => format!("{}[]", type_name(e)),
        Kind::Domain(b) => type_name(b),
        Kind::Enum(_) => format!("enum {}", ty.name()),
        _ => ty.name().to_string(),
    }
}

fn close(a: &Args) -> Outcome {
    if let Some(t) = HANDLES.remove(a.handle(0)?) {
        release(t);
    }
    Ok(Value::Unit)
}

fn is_closed(a: &Args) -> Outcome {
    Ok(Value::Bool(match HANDLES.get(a.handle(0)?) {
        Ok(t) => matches!(t.as_ref(), Target::Pool(p) if p.is_closed()),
        Err(_) => true,
    }))
}

/// A pool closes; a transaction nobody finished rolls back.
fn release(t: Arc<Target>) {
    match t.as_ref() {
        Target::Pool(p) => p.close(),
        Target::Tx(_) => {
            runtime().spawn(async move {
                if let Target::Tx(tx) = t.as_ref() {
                    let _ = end(tx, false).await;
                }
            });
        }
    }
}

fn drop_handle(h: u64) {
    if let Some(t) = HANDLES.remove(h) {
        release(t);
    }
}

pub static DEF: ModuleDef = ModuleDef {
    name: "badger_postgres",
    functions: &[
        ("connect", Handler::Async(connect)),
        ("execute", Handler::Async(execute)),
        ("query", Handler::Async(query)),
        ("query_one", Handler::Async(query_one)),
        ("begin", Handler::Async(begin)),
        ("finish", Handler::Async(finish)),
        ("describe", Handler::Async(describe)),
        ("close", Handler::Inline(close)),
        ("is_closed", Handler::Inline(is_closed)),
    ],
    drop_handle: Some(drop_handle),
    cancel: Some(cancel),
    recorded: &[
        ("connect", "postgres connect", 1),
        ("execute", "postgres", 2),
        ("query", "postgres", 2),
        ("query_one", "postgres", 2),
        ("begin", "postgres begin", 1),
        ("finish", "postgres finish", 2),
        ("describe", "postgres describe", 2),
        ("close", "postgres close", 1),
        ("is_closed", "postgres is_closed", 1),
    ],
    pure: &[],
};

badger_native::export!("badger_postgres", DEF);

#[cfg(test)]
mod tests {
    #[test]
    fn named_parameters_are_numbered() {
        let (sql, names) = number_named(
            "SELECT a::int, ':no', \"q:x\", $$:body$$ FROM t WHERE a = :a AND b = :b OR a = :a -- :c\n",
        );
        assert_eq!(sql, "SELECT a::int, ':no', \"q:x\", $$:body$$ FROM t WHERE a = $1 AND b = $2 OR a = $1 -- :c\n");
        assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
        let fields = Value::Map(vec![("b".into(), Value::Int(2)), ("a".into(), Value::Int(1))]);
        let (_, values) = bind("SELECT :a + :b".into(), &fields).unwrap();
        assert!(matches!(values.as_slice(), [Value::Int(1), Value::Int(2)]));
        assert!(bind("SELECT :missing".into(), &fields).is_err());
        let (_, one) = bind("SELECT $1".into(), &Value::Int(7)).unwrap();
        assert!(matches!(one.as_slice(), [Value::Int(7)]));
    }

    use super::*;

    #[test]
    fn dates_and_times() {
        assert_eq!(date_text(0), "2000-01-01");
        assert_eq!(date_text(-1), "1999-12-31");
        assert_eq!(date_text(9555), "2026-02-28");
        assert_eq!(timestamp_text(86_400_000_000 + 3_723_000_001, true), "2000-01-02T01:02:03.000001Z");
        assert_eq!(timestamp_text(-1, false), "1999-12-31T23:59:59.999999");
        assert_eq!(time_text(45_296_000_000), "12:34:56");
    }

    #[test]
    fn numerics() {
        let n = |ndigits: i16, weight: i16, sign: u16, dscale: i16, digits: &[i16]| {
            let mut b = vec![];
            for w in [ndigits, weight, sign as i16, dscale].iter().chain(digits) {
                b.extend_from_slice(&w.to_be_bytes());
            }
            numeric_text(&b).unwrap()
        };
        assert_eq!(n(2, 0, 0, 2, &[12, 3400]), "12.34");
        assert_eq!(n(1, 1, 0, 0, &[5]), "50000");
        assert_eq!(n(1, -1, 0x4000, 4, &[5]), "-0.0005");
        assert_eq!(n(0, 0, 0, 0, &[]), "0");
        assert_eq!(n(3, 1, 0, 1, &[1, 2345, 6000]), "12345.6");
    }

    #[test]
    fn array_parameters() {
        let ty = Type::TEXT_ARRAY;
        let v = Value::List(vec![Value::str("a\"b"), Value::Null, Value::str("c,d")]);
        assert_eq!(text_of(&v, &ty).unwrap(), r#"{"a\"b",NULL,"c,d"}"#);
        assert_eq!(
            text_of(&Value::Map(vec![("k".into(), Value::Int(1))]), &Type::JSONB).unwrap(),
            r#"{"k":1}"#
        );
    }
}
