//! Embedded SQLite adapter: pools, scoped transactions, and `Data` rows.
//!
//! All database work runs on the blocking pool; the calling task parks.
//! A pool lends connections; when none is free and the pool is at its limit
//! the task waits for a release. A transaction holds one connection from
//! `BEGIN` until `COMMIT`/`ROLLBACK`.

use std::collections::VecDeque;

use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, params_from_iter};

use crate::desc::{Kind, TypeDesc};
use crate::gc::{self, Header, Roots, obj};
use crate::intrinsics::{Targs, rdesc};
use crate::string::{new_string, str_of};
use crate::task::{self, TaskCtx, TaskId};
use crate::{enum_parts, list, make_enum, make_record, make_variant, map};

const POOL_MAX: usize = 4;

pub struct PoolData {
    pub path: String,
    pub idle: Vec<Connection>,
    pub open: usize,
    pub closed: bool,
    pub waiters: VecDeque<TaskId>,
}

pub struct TxData {
    pub pool: u64,
    pub conn: Option<Connection>,
    pub finished: bool,
}

fn pool_desc() -> &'static TypeDesc {
    static D: std::sync::OnceLock<&'static TypeDesc> = std::sync::OnceLock::new();
    D.get_or_init(|| {
        TypeDesc::new(
            "SqlitePool",
            Kind::Opaque { trace: None, finalize: Some(fin::<PoolData>), args: vec![] },
        )
        .leak()
    })
}

fn tx_desc() -> &'static TypeDesc {
    static D: std::sync::OnceLock<&'static TypeDesc> = std::sync::OnceLock::new();
    D.get_or_init(|| {
        TypeDesc::new(
            "SqliteTx",
            Kind::Opaque { trace: Some(trace_tx), finalize: Some(fin::<TxData>), args: vec![] },
        )
        .leak()
    })
}

fn fin<T>(o: *mut Header) {
    unsafe {
        let p = (*o).payload() as *mut *mut T;
        if !(*p).is_null() {
            drop(Box::from_raw(*p));
            *p = std::ptr::null_mut();
        }
    }
}

fn trace_tx(o: *mut Header, f: &mut dyn FnMut(u64)) {
    unsafe {
        let t = &*(*((*o).payload() as *const *mut TxData));
        f(t.pool);
    }
}

fn boxed<T>(desc: &'static TypeDesc, v: T) -> u64 {
    let w = gc::alloc(desc, 8);
    unsafe { *(obj(w).payload() as *mut *mut T) = Box::into_raw(Box::new(v)) };
    w
}

fn pool<'a>(w: u64) -> &'a mut PoolData {
    unsafe { &mut **(obj(w).payload() as *mut *mut PoolData) }
}

fn tx<'a>(w: u64) -> &'a mut TxData {
    unsafe { &mut **(obj(w).payload() as *mut *mut TxData) }
}

fn is_tx(w: u64) -> bool {
    std::ptr::eq(unsafe { obj(w).desc() }, tx_desc())
}

/// `Result<T, sqlite.Error>` error construction.
fn sql_err(ret: &'static TypeDesc, code: &str, msg: &str) -> u64 {
    let err_desc = ret.variant(1).unwrap().fields[0].desc;
    let r = Roots::new();
    let m = r.add(new_string(msg));
    let mut fields = vec![("code", crate::string::symbol(code)), ("message", m)];
    if let Some(c) = err_desc.field("cause") {
        fields.push(("cause", r.add(make_variant(c.desc, "none", &[]))));
    }
    let e = r.add(make_record(err_desc, &fields));
    make_variant(ret, "err", &[e])
}

fn ok(ret: &'static TypeDesc, w: u64) -> u64 {
    make_variant(ret, "ok", &[w])
}

fn open_conn(path: &str) -> rusqlite::Result<Connection> {
    let c = Connection::open(path)?;
    c.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(c)
}

/// Borrow a connection from a pool, waiting while all are in use.
fn acquire(p: u64) -> Result<Connection, String> {
    let _r = Roots::new();
    _r.add(p);
    loop {
        let d = pool(p);
        if d.closed {
            return Err("database pool is closed".into());
        }
        if let Some(c) = d.idle.pop() {
            return Ok(c);
        }
        if d.open < POOL_MAX {
            d.open += 1;
            let path = d.path.clone();
            return match task::run_blocking(move || open_conn(&path).map_err(|e| e.to_string())) {
                Ok(c) => Ok(c),
                Err(e) => {
                    pool(p).open -= 1;
                    Err(e)
                }
            };
        }
        d.waiters.push_back(task::current_id());
        task::park_for("database");
    }
}

fn release(p: u64, c: Connection) {
    let d = pool(p);
    if d.closed {
        d.open -= 1;
        drop(c);
        return;
    }
    d.idle.push(c);
    if let Some(w) = d.waiters.pop_front() {
        task::wake(w, task::Wake::Normal);
    }
}

fn to_sql(w: u64) -> SqlValue {
    let (tag, f) = enum_parts(w);
    match tag {
        0 => SqlValue::Null,
        1 => SqlValue::Integer((f[0] & 1) as i64),
        2 => SqlValue::Integer(f[0] as i64),
        3 => SqlValue::Real(f64::from_bits(f[0])),
        4 => SqlValue::Text(str_of(f[0]).to_string()),
        _ => SqlValue::Text(crate::intrinsics_io::data_json(w)),
    }
}

type Rows = Vec<Vec<(String, SqlValue)>>;

fn run_sql(c: &Connection, sql: &str, params: &[SqlValue], query: bool) -> rusqlite::Result<(usize, Rows)> {
    if !query {
        if params.is_empty() {
            c.execute_batch(sql)?;
            return Ok((c.changes() as usize, vec![]));
        }
        let n = c.execute(sql, params_from_iter(params.iter()))?;
        return Ok((n, vec![]));
    }
    let mut st = c.prepare(sql)?;
    let names: Vec<String> = st.column_names().iter().map(|s| s.to_string()).collect();
    let mut rows = st.query(params_from_iter(params.iter()))?;
    let mut out = vec![];
    while let Some(r) = rows.next()? {
        let mut row = vec![];
        for (i, n) in names.iter().enumerate() {
            row.push((n.clone(), r.get::<_, SqlValue>(i)?));
        }
        out.push(row);
    }
    Ok((out.len(), out))
}

/// Execute on a pool (borrowing a connection) or a transaction's connection.
fn with_conn(
    target: u64,
    sql: String,
    params: Vec<SqlValue>,
    query: bool,
) -> Result<(usize, Rows), (String, String)> {
    let _r = Roots::new();
    _r.add(target);
    if is_tx(target) {
        let t = tx(target);
        let Some(c) = t.conn.take() else {
            return Err(("sqlite/tx_closed".into(), "transaction is finished".into()));
        };
        let (c, r) = task::run_blocking(move || {
            let r = run_sql(&c, &sql, &params, query).map_err(|e| e.to_string());
            (c, r)
        });
        tx(target).conn = Some(c);
        return r.map_err(|e| ("sqlite/error".into(), e));
    }
    let c = acquire(target).map_err(|e| ("sqlite/pool".to_string(), e))?;
    let (c, r) = task::run_blocking(move || {
        let r = run_sql(&c, &sql, &params, query).map_err(|e| e.to_string());
        (c, r)
    });
    release(target, c);
    r.map_err(|e| ("sqlite/error".into(), e))
}

fn row_data(data: &'static TypeDesc, row: &[(String, SqlValue)]) -> u64 {
    let r = Roots::new();
    let map_desc = data.variant(6).unwrap().fields[0].desc;
    let m = r.add(map::new(map_desc));
    for (k, v) in row {
        let kw = r.add(new_string(k));
        let vw = r.add(match v {
            SqlValue::Null => make_enum(data, 0, &[]),
            SqlValue::Integer(i) => make_enum(data, 2, &[*i as u64]),
            SqlValue::Real(f) => make_enum(data, 3, &[f.to_bits()]),
            SqlValue::Text(s) => {
                let s = r.add(new_string(s));
                make_enum(data, 4, &[s])
            }
            SqlValue::Blob(b) => {
                let s = r.add(new_string(&String::from_utf8_lossy(b)));
                make_enum(data, 4, &[s])
            }
        });
        map::insert_mut(m, kw, vw);
    }
    make_enum(data, 6, &[m])
}

fn params_of(l: u64) -> Vec<SqlValue> {
    list::to_vec(l).into_iter().map(to_sql).collect()
}

macro_rules! sql_intrinsics {
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

sql_intrinsics! {
    fn sql_open(ctx, targs, ret, path) {
        let p = str_of(path).to_string();
        let p2 = p.clone();
        match task::run_blocking(move || open_conn(&p2).map_err(|e| e.to_string())) {
            Ok(c) => {
                let w = boxed(pool_desc(), PoolData { path: p, idle: vec![c], open: 1, closed: false, waiters: VecDeque::new() });
                ok(rdesc(ret), w)
            }
            Err(e) => sql_err(rdesc(ret), "sqlite/open", &e),
        }
    }
    fn sql_close(ctx, targs, ret, p) {
        let d = pool(p);
        d.closed = true;
        d.open -= d.idle.len();
        d.idle.clear();
        for w in std::mem::take(&mut d.waiters) {
            task::wake(w, task::Wake::Normal);
        }
        0
    }
    fn sql_is_closed(ctx, targs, ret, p) { pool(p).closed as u64 }
    fn sql_execute(ctx, targs, ret, c, sql, params) {
        let ps = params_of(params);
        match with_conn(c, str_of(sql).to_string(), ps, false) {
            Ok((n, _)) => ok(rdesc(ret), n as u64),
            Err((code, m)) => sql_err(rdesc(ret), &code, &m),
        }
    }
    fn sql_query(ctx, targs, ret, c, sql, params) {
        let ps = params_of(params);
        match with_conn(c, str_of(sql).to_string(), ps, true) {
            Ok((_, rows)) => {
                let r = Roots::new();
                let list_desc = rdesc(ret).variant(0).unwrap().fields[0].desc;
                let data = match &list_desc.kind { Kind::List { elem } => *elem, _ => unreachable!() };
                let l = r.add(list::new(list_desc));
                for row in &rows {
                    let d = r.add(row_data(data, row));
                    list::push(l, d);
                }
                ok(rdesc(ret), l)
            }
            Err((code, m)) => sql_err(rdesc(ret), &code, &m),
        }
    }
    fn sql_query_one(ctx, targs, ret, c, sql, params) {
        let ps = params_of(params);
        match with_conn(c, str_of(sql).to_string(), ps, true) {
            Ok((_, rows)) if rows.len() == 1 => {
                let data = rdesc(ret).variant(0).unwrap().fields[0].desc;
                let _r = Roots::new();
                let d = _r.add(row_data(data, &rows[0]));
                ok(rdesc(ret), d)
            }
            Ok((_, rows)) => sql_err(rdesc(ret), "sqlite/row_count", &format!("expected exactly one row, got {}", rows.len())),
            Err((code, m)) => sql_err(rdesc(ret), &code, &m),
        }
    }
    fn sql_begin(ctx, targs, ret, p) {
        let _r = Roots::new();
        _r.add(p);
        let c = match acquire(p) {
            Ok(c) => c,
            Err(e) => return sql_err(rdesc(ret), "sqlite/pool", &e),
        };
        let (c, r) = task::run_blocking(move || {
            let r = c.execute_batch("BEGIN").map_err(|e| e.to_string());
            (c, r)
        });
        match r {
            Ok(()) => {
                let w = boxed(tx_desc(), TxData { pool: p, conn: Some(c), finished: false });
                ok(rdesc(ret), w)
            }
            Err(e) => {
                release(p, c);
                sql_err(rdesc(ret), "sqlite/begin", &e)
            }
        }
    }
    fn sql_finish(ctx, targs, ret, t, commit) {
        let _r = Roots::new();
        _r.add(t);
        let d = tx(t);
        if d.finished {
            return ok(rdesc(ret), 0);
        }
        d.finished = true;
        let Some(c) = d.conn.take() else { return sql_err(rdesc(ret), "sqlite/tx_closed", "transaction connection in use") };
        let sql = if commit & 1 != 0 { "COMMIT" } else { "ROLLBACK" };
        let (c, r) = task::run_blocking(move || {
            let mut r = c.execute_batch(sql).map_err(|e| e.to_string());
            if r.is_err() && sql == "COMMIT" {
                // a failed commit leaves the transaction open: roll it back
                let _ = c.execute_batch("ROLLBACK");
            } else if r.is_err() {
                r = Ok(()).and(r);
            }
            (c, r)
        });
        let pool_w = tx(t).pool;
        release(pool_w, c);
        match r {
            Ok(()) => ok(rdesc(ret), 0),
            Err(e) => sql_err(rdesc(ret), if commit & 1 != 0 { "sqlite/commit" } else { "sqlite/rollback" }, &e),
        }
    }
}

/// Roll back an unfinished transaction during task cleanup.
pub fn rollback_on_abandon(t: u64) {
    let d = tx(t);
    if d.finished {
        return;
    }
    d.finished = true;
    if let Some(c) = d.conn.take() {
        let (c, _) = task::run_blocking(move || {
            let r = c.execute_batch("ROLLBACK");
            (c, r.is_ok())
        });
        let p = tx(t).pool;
        release(p, c);
    }
}

pub extern "C" fn tx_guard(_ctx: *mut TaskCtx, _t: Targs, _r: *const TypeDesc, t: u64) -> u64 {
    task::current().unwrap().roots.push(t);
    task::push_cleanup(task::Cleanup::Native(Box::new(move |_| rollback_on_abandon(t))))
}

pub extern "C" fn tx_unguard(_ctx: *mut TaskCtx, _t: Targs, _r: *const TypeDesc, h: u64, t: u64) -> u64 {
    task::remove_cleanup(h);
    let cur = task::current().unwrap();
    if let Some(i) = cur.roots.iter().rposition(|w| *w == t) {
        cur.roots.remove(i);
    }
    0
}

pub fn all_symbols() -> Vec<(&'static str, *const u8)> {
    let mut v = symbols();
    v.push(("lang_rt_i_tx_guard", tx_guard as *const u8));
    v.push(("lang_rt_i_tx_unguard", tx_unguard as *const u8));
    v
}
