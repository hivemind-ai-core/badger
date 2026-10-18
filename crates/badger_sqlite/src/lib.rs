//! `badger_sqlite`: the native module behind `std.sqlite`.
//!
//! A pool lends up to [`POOL_MAX`] connections, each owned by its own
//! thread; every operation is an async call queued to the pool, so a task
//! waiting for a free connection only parks (no runtime thread waits). A
//! transaction pins one connection's thread from `BEGIN IMMEDIATE` (which
//! takes the write lock first, so concurrent transactions queue instead of
//! deadlocking) until its `COMMIT` or `ROLLBACK`; a transaction the program
//! can no longer reach is rolled back. Rows are maps from column name to
//! value; blobs come back as text.

use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use badger_native::{Args, Completion, Error, Handler, Handles, ModuleDef, Outcome, Value};
use rusqlite::types::Value as Sql;
use rusqlite::{Connection, params_from_iter};

/// Connections a pool opens at most.
pub const POOL_MAX: usize = 4;

enum Target {
    Pool(Arc<Pool>),
    Tx(Arc<Tx>),
}

static HANDLES: Handles<Target> = Handles::new();

#[derive(Clone, Copy)]
enum Op {
    Execute,
    Query,
    QueryOne,
}

enum Job {
    Run { sql: String, params: Vec<Sql>, op: Op, done: Completion },
    Begin { done: Completion },
}

enum TxJob {
    Run { sql: String, params: Vec<Sql>, op: Op, done: Completion },
    Finish { commit: bool, done: Option<Completion> },
}

struct PoolState {
    jobs: VecDeque<Job>,
    workers: usize,
    /// Connections running a statement or holding a transaction; the
    /// others take the next job (a new connection opens only when every
    /// one is busy, so `:memory:` databases keep one connection unless
    /// work overlaps).
    busy: usize,
    closed: bool,
}

struct Pool {
    path: String,
    state: Mutex<PoolState>,
    cond: Condvar,
}

struct Tx {
    jobs: Mutex<(VecDeque<TxJob>, bool)>,
    cond: Condvar,
}

impl Tx {
    fn submit(&self, job: TxJob) {
        let mut g = self.jobs.lock().unwrap();
        if g.1 {
            drop(g);
            match job {
                TxJob::Run { done, .. } => {
                    done.complete(Err(Error::new("sqlite/tx_closed", "transaction is finished")))
                }
                TxJob::Finish { done: Some(done), .. } => done.complete(Ok(Value::Unit)),
                TxJob::Finish { done: None, .. } => {}
            }
            return;
        }
        if matches!(job, TxJob::Finish { .. }) {
            g.1 = true;
        }
        g.0.push_back(job);
        self.cond.notify_one();
    }
}

/// The path a pool opens: `:memory:` becomes a database of its own in the
/// `memdb` VFS, so every connection of the pool sees the same in-memory
/// database (with ordinary locking), where each `:memory:` connection
/// would otherwise get a separate empty one.
fn pool_path(path: &str) -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    if path == ":memory:" || path.is_empty() {
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        format!("file:/badger-memory-{}-{n}?vfs=memdb", std::process::id())
    } else {
        path.to_string()
    }
}

fn open_conn(path: &str) -> rusqlite::Result<Connection> {
    let c = if path.starts_with("file:/badger-memory-") {
        Connection::open_with_flags(
            path,
            rusqlite::OpenFlags::default() | rusqlite::OpenFlags::SQLITE_OPEN_URI,
        )?
    } else {
        Connection::open(path)?
    };
    c.busy_timeout(Duration::from_secs(5))?;
    Ok(c)
}

fn sql_error(e: impl std::fmt::Display) -> Error {
    Error::new("sqlite/error", e.to_string())
}

fn run(c: &Connection, sql: &str, params: &[Sql], op: Op) -> Outcome {
    if let Op::Execute = op {
        if params.is_empty() {
            c.execute_batch(sql).map_err(sql_error)?;
            return Ok(Value::Int(c.changes() as i64));
        }
        let n = c.execute(sql, params_from_iter(params.iter())).map_err(sql_error)?;
        return Ok(Value::Int(n as i64));
    }
    let mut st = c.prepare(sql).map_err(sql_error)?;
    let names: Vec<String> = st.column_names().iter().map(|s| s.to_string()).collect();
    let mut rows = st.query(params_from_iter(params.iter())).map_err(sql_error)?;
    let mut out = vec![];
    while let Some(r) = rows.next().map_err(sql_error)? {
        let mut row = vec![];
        for (i, n) in names.iter().enumerate() {
            let v = match r.get::<_, Sql>(i).map_err(sql_error)? {
                Sql::Null => Value::Null,
                Sql::Integer(i) => Value::Int(i),
                Sql::Real(f) => Value::Float(f),
                Sql::Text(s) => Value::Str(s),
                Sql::Blob(b) => Value::Str(String::from_utf8_lossy(&b).into_owned()),
            };
            row.push((n.clone(), v));
        }
        out.push(Value::Map(row));
    }
    match op {
        Op::QueryOne if out.len() == 1 => Ok(out.pop().unwrap()),
        Op::QueryOne => {
            Err(Error::new("sqlite/row_count", format!("expected exactly one row, got {}", out.len())))
        }
        _ => Ok(Value::List(out)),
    }
}

/// Serve the pool with connection `c` until the pool closes.
fn worker(pool: Arc<Pool>, c: Connection) {
    loop {
        let job = {
            let mut g = pool.state.lock().unwrap();
            loop {
                if let Some(j) = g.jobs.pop_front() {
                    g.busy += 1;
                    break Some(j);
                }
                if g.closed {
                    break None;
                }
                g = pool.cond.wait(g).unwrap();
            }
        };
        let Some(job) = job else { break };
        // the connection is free again before the caller hears back, so
        // its next statement reuses it rather than opening another
        let free = || pool.state.lock().unwrap().busy -= 1;
        match job {
            Job::Run { sql, params, op, done } => {
                let r = run(&c, &sql, &params, op);
                free();
                done.complete(r);
            }
            Job::Begin { done } => match c.execute_batch("BEGIN IMMEDIATE") {
                Ok(()) => {
                    let tx =
                        Arc::new(Tx { jobs: Mutex::new((VecDeque::new(), false)), cond: Condvar::new() });
                    done.complete(Ok(HANDLES.insert(Target::Tx(tx.clone()))));
                    let finished = serve_tx(&c, &tx);
                    free();
                    if let Some((done, r)) = finished {
                        done.complete(r);
                    }
                }
                Err(e) => {
                    free();
                    done.complete(Err(Error::new("sqlite/begin", e.to_string())));
                }
            },
        }
    }
    pool.state.lock().unwrap().workers -= 1;
}

/// Run a transaction's statements on this connection until it finishes;
/// the finishing call and its outcome, for the caller to complete once the
/// connection is free.
fn serve_tx(c: &Connection, tx: &Tx) -> Option<(Completion, Outcome)> {
    loop {
        let job = {
            let mut g = tx.jobs.lock().unwrap();
            loop {
                if let Some(j) = g.0.pop_front() {
                    break j;
                }
                g = tx.cond.wait(g).unwrap();
            }
        };
        match job {
            TxJob::Run { sql, params, op, done } => done.complete(run(c, &sql, &params, op)),
            TxJob::Finish { commit, done } => {
                let r = if commit {
                    let r = c.execute_batch("COMMIT").map_err(|e| Error::new("sqlite/commit", e.to_string()));
                    if r.is_err() {
                        // a failed commit leaves the transaction open
                        let _ = c.execute_batch("ROLLBACK");
                    }
                    r
                } else {
                    c.execute_batch("ROLLBACK").map_err(|e| Error::new("sqlite/rollback", e.to_string()))
                };
                let finished = done.map(|d| (d, r.map(|_| Value::Unit)));
                // later statements of this transaction fail as finished
                let mut g = tx.jobs.lock().unwrap();
                for j in std::mem::take(&mut g.0) {
                    if let TxJob::Run { done, .. } = j {
                        done.complete(Err(Error::new("sqlite/tx_closed", "transaction is finished")));
                    }
                }
                return finished;
            }
        }
    }
}

impl Pool {
    fn submit(self: &Arc<Pool>, job: Job) {
        let mut g = self.state.lock().unwrap();
        if g.closed {
            drop(g);
            let done = match job {
                Job::Run { done, .. } | Job::Begin { done } => done,
            };
            done.complete(Err(Error::new("sqlite/pool", "database pool is closed")));
            return;
        }
        g.jobs.push_back(job);
        if g.busy + g.jobs.len() <= g.workers {
            // a connection is (or is about to be) free for it
            self.cond.notify_one();
        } else if g.workers < POOL_MAX {
            g.workers += 1;
            drop(g);
            let pool = self.clone();
            std::thread::spawn(move || {
                badger_native::module_thread();
                match open_conn(&pool.path) {
                    Ok(c) => worker(pool, c),
                    Err(e) => {
                        // this connection could not open: fail the job it was for
                        let job = {
                            let mut g = pool.state.lock().unwrap();
                            g.workers -= 1;
                            g.jobs.pop_front()
                        };
                        if let Some(Job::Run { done, .. } | Job::Begin { done }) = job {
                            done.complete(Err(Error::new("sqlite/pool", e.to_string())));
                        }
                    }
                }
            });
        }
    }

    fn close(&self) {
        let mut g = self.state.lock().unwrap();
        g.closed = true;
        let jobs = std::mem::take(&mut g.jobs);
        self.cond.notify_all();
        drop(g);
        for j in jobs {
            let (Job::Run { done, .. } | Job::Begin { done }) = j;
            done.complete(Err(Error::new("sqlite/pool", "database pool is closed")));
        }
    }
}

/// A `Data` parameter as an SQL value (lists and maps as JSON text).
fn sql_param(v: &Value) -> Sql {
    match v {
        Value::Null | Value::Unit => Sql::Null,
        Value::Bool(b) => Sql::Integer(*b as i64),
        Value::Int(i) => Sql::Integer(*i),
        Value::Float(f) => Sql::Real(*f),
        Value::Str(s) | Value::Symbol(s) => Sql::Text(s.clone()),
        Value::Bytes(b) => Sql::Blob(b.clone()),
        other => Sql::Text(json(other).to_string()),
    }
}

fn json(v: &Value) -> serde_json::Value {
    use serde_json::Value as J;
    match v {
        Value::Null | Value::Unit => J::Null,
        Value::Bool(b) => J::Bool(*b),
        Value::Int(i) => J::from(*i),
        Value::Float(f) => serde_json::Number::from_f64(*f).map(J::Number).unwrap_or(J::Null),
        Value::Str(s) | Value::Symbol(s) => J::String(s.clone()),
        Value::Char(c) => J::String(c.to_string()),
        Value::Bytes(b) => J::String(String::from_utf8_lossy(b).into_owned()),
        Value::List(xs) => J::Array(xs.iter().map(json).collect()),
        Value::Map(fs) => J::Object(fs.iter().map(|(k, v)| (k.clone(), json(v))).collect()),
        Value::Variant(n, xs) => {
            J::Object([(n.clone(), J::Array(xs.iter().map(json).collect()))].into_iter().collect())
        }
        Value::Handle(h) => J::from(*h),
    }
}

fn target(a: &Args, i: usize) -> Result<Arc<Target>, Error> {
    HANDLES.get(a.handle(i)?).map_err(|_| Error::new("sqlite/pool", "database pool is closed"))
}

fn statement(a: Args, done: Completion, op: Op) {
    let parsed = (|| -> Result<(Arc<Target>, String, Vec<Sql>), Error> {
        Ok((target(&a, 0)?, a.str(1)?.to_string(), a.list(2)?.iter().map(sql_param).collect()))
    })();
    let (t, sql, params) = match parsed {
        Ok(x) => x,
        Err(e) => return done.complete(Err(e)),
    };
    match &*t {
        Target::Pool(p) => p.submit(Job::Run { sql, params, op, done }),
        Target::Tx(tx) => tx.submit(TxJob::Run { sql, params, op, done }),
    }
}

fn open(a: Args, done: Completion) {
    let path = match a.str(0) {
        Ok(p) => pool_path(p),
        Err(e) => return done.complete(Err(e)),
    };
    std::thread::spawn(move || {
        badger_native::module_thread();
        match open_conn(&path) {
            Ok(c) => {
                let pool = Arc::new(Pool {
                    path,
                    state: Mutex::new(PoolState {
                        jobs: VecDeque::new(),
                        workers: 1,
                        busy: 0,
                        closed: false,
                    }),
                    cond: Condvar::new(),
                });
                done.complete(Ok(HANDLES.insert(Target::Pool(pool.clone()))));
                worker(pool, c);
            }
            Err(e) => done.complete(Err(Error::new("sqlite/open", e.to_string()))),
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

fn begin(a: Args, done: Completion) {
    match target(&a, 0).as_deref() {
        Ok(Target::Pool(p)) => p.submit(Job::Begin { done }),
        Ok(Target::Tx(_)) => done.complete(Err(Error::new("sqlite/begin", "transactions do not nest"))),
        Err(e) => done.complete(Err(e.clone())),
    }
}

fn finish(a: Args, done: Completion) {
    let (h, commit) = match (a.handle(0), a.bool(1)) {
        (Ok(h), Ok(c)) => (h, c),
        (Err(e), _) | (_, Err(e)) => return done.complete(Err(e)),
    };
    match HANDLES.get(h).as_deref() {
        Ok(Target::Tx(tx)) => tx.submit(TxJob::Finish { commit, done: Some(done) }),
        // finishing again changes nothing
        _ => done.complete(Ok(Value::Unit)),
    }
}

fn close(a: &Args) -> Outcome {
    if let Target::Pool(p) = &*target(a, 0)? {
        p.close();
    }
    Ok(Value::Unit)
}

fn is_closed(a: &Args) -> Outcome {
    Ok(Value::Bool(match HANDLES.get(a.handle(0)?).as_deref() {
        Ok(Target::Pool(p)) => p.state.lock().unwrap().closed,
        _ => true,
    }))
}

/// The program can no longer reach `h`: roll back an unfinished
/// transaction, close a pool.
fn drop_handle(h: u64) {
    match HANDLES.remove(h).as_deref() {
        Some(Target::Tx(tx)) => tx.submit(TxJob::Finish { commit: false, done: None }),
        Some(Target::Pool(p)) => p.close(),
        None => {}
    }
}

pub static DEF: ModuleDef = ModuleDef {
    name: "badger_sqlite",
    functions: &[
        ("open", Handler::Async(open)),
        ("close", Handler::Inline(close)),
        ("is_closed", Handler::Inline(is_closed)),
        ("execute", Handler::Async(execute)),
        ("query", Handler::Async(query)),
        ("query_one", Handler::Async(query_one)),
        ("begin", Handler::Async(begin)),
        ("finish", Handler::Async(finish)),
    ],
    drop_handle: Some(drop_handle),
    cancel: None,
    recorded: &[],
    pure: &[],
};

badger_native::export!("badger_sqlite", DEF);
