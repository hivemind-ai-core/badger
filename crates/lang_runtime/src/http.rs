//! Minimal HTTP/1.1 server adapter and process shutdown signals.
//!
//! An accept thread and per-connection reader threads parse requests off
//! the worker; each request is admitted on the worker as a task in the
//! server's scope with its defects contained, so one failing request never
//! fails its siblings. Responses travel back to the connection thread. If a
//! request task dies before responding, its cleanup answers 500.

use std::cell::UnsafeCell;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

use rustc_hash::FxHashMap;

use crate::desc::TypeDesc;
use crate::gc::{self, Roots};
use crate::intrinsics::{Targs, rdesc, targ};
use crate::string::{bytes_of, new_bytes, new_string, str_of};
use crate::task::{self, TaskCtx, TaskId};
use crate::{list, make_record, make_variant, map, scope};

/// Explicit request limits: body and header sizes, and bounded admission
/// (requests in flight; more are answered 503 without running a handler).
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_body_bytes: usize,
    pub max_header_bytes: usize,
    pub max_in_flight: usize,
}

static DEFAULT_BODY: AtomicU64 = AtomicU64::new(16 << 20);
static DEFAULT_HEADER: AtomicU64 = AtomicU64::new(64 << 10);
static DEFAULT_IN_FLIGHT: AtomicU64 = AtomicU64::new(1024);

/// Runtime configuration defaults (`[runtime] http_max_body_bytes`,
/// `http_max_header_bytes`, `http_max_in_flight`).
pub fn set_default_limit(key: &str, v: u64) -> bool {
    match key {
        "http_max_body_bytes" => DEFAULT_BODY.store(v, Ordering::SeqCst),
        "http_max_header_bytes" => DEFAULT_HEADER.store(v, Ordering::SeqCst),
        "http_max_in_flight" => DEFAULT_IN_FLIGHT.store(v.max(1), Ordering::SeqCst),
        _ => return false,
    }
    true
}

pub fn default_limits() -> Limits {
    Limits {
        max_body_bytes: DEFAULT_BODY.load(Ordering::SeqCst) as usize,
        max_header_bytes: DEFAULT_HEADER.load(Ordering::SeqCst) as usize,
        max_in_flight: DEFAULT_IN_FLIGHT.load(Ordering::SeqCst) as usize,
    }
}

/// A response message from the request task to its connection thread.
enum Out {
    /// A complete response.
    Full(Vec<u8>),
    /// Streaming: head or one chunk (already framed).
    Part(Vec<u8>),
    /// End of a streamed response.
    End,
}

struct Server {
    serve: u64,
    req_desc: &'static TypeDesc,
    scope: u64,
    stop: Arc<AtomicBool>,
    addr: std::net::SocketAddr,
    in_flight: Vec<TaskId>,
    limits: Limits,
    rejected: Arc<AtomicU64>,
}

struct Registry {
    next: u64,
    servers: FxHashMap<u64, Server>,
    conns: FxHashMap<u64, (mpsc::Sender<Out>, u64)>,
    shutdown_waiters: Vec<TaskId>,
    signal_installed: bool,
}

struct RegCell(UnsafeCell<Option<Registry>>);
unsafe impl Sync for RegCell {}
static REG: RegCell = RegCell(UnsafeCell::new(None));

fn reg() -> &'static mut Registry {
    unsafe {
        (*REG.0.get()).get_or_insert_with(|| Registry {
            next: 1,
            servers: FxHashMap::default(),
            conns: FxHashMap::default(),
            shutdown_waiters: vec![],
            signal_installed: false,
        })
    }
}

pub struct RawRequest {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Why a request could not be read.
enum ReadError {
    /// Connection closed or malformed input: no reply.
    Closed,
    /// A limit was exceeded: reply with this status.
    Limit(u64, &'static str),
}

fn read_request(s: &mut TcpStream, lim: Limits) -> Result<RawRequest, ReadError> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        let n = s.read(&mut tmp).map_err(|_| ReadError::Closed)?;
        if n == 0 {
            return Err(ReadError::Closed);
        }
        buf.extend_from_slice(&tmp[..n]);
        let mut hs = [httparse::EMPTY_HEADER; 64];
        let mut req = httparse::Request::new(&mut hs);
        let status = match req.parse(&buf) {
            Ok(st) => st,
            Err(httparse::Error::TooManyHeaders) => {
                return Err(ReadError::Limit(431, "too many request headers"));
            }
            Err(_) => return Err(ReadError::Limit(400, "malformed request")),
        };
        if let httparse::Status::Complete(off) = status {
            if off > lim.max_header_bytes {
                return Err(ReadError::Limit(431, "request headers too large"));
            }
            let headers: Vec<(String, String)> = req
                .headers
                .iter()
                .map(|h| (h.name.to_ascii_lowercase(), String::from_utf8_lossy(h.value).into_owned()))
                .collect();
            let len: usize = headers
                .iter()
                .find(|h| h.0 == "content-length")
                .and_then(|h| h.1.trim().parse().ok())
                .unwrap_or(0);
            if len > lim.max_body_bytes {
                return Err(ReadError::Limit(413, "request body too large"));
            }
            let mut body = buf[off..].to_vec();
            while body.len() < len {
                let n = s.read(&mut tmp).map_err(|_| ReadError::Closed)?;
                if n == 0 {
                    break;
                }
                body.extend_from_slice(&tmp[..n]);
            }
            body.truncate(len);
            return Ok(RawRequest {
                method: req.method.unwrap_or("GET").to_ascii_lowercase(),
                path: req.path.unwrap_or("/").to_string(),
                headers,
                body,
            });
        }
        if buf.len() > lim.max_header_bytes {
            return Err(ReadError::Limit(431, "request headers too large"));
        }
    }
}

fn status_text(code: u64) -> &'static str {
    match code {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        413 => "Payload Too Large",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

fn simple(code: u64, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {code} {}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        status_text(code),
        body.len()
    )
    .into_bytes()
}

fn handle_conn(
    mut s: TcpStream,
    server: u64,
    q: Arc<task::ExternalQueue>,
    lim: Limits,
    in_flight: Arc<AtomicUsize>,
    rejected: Arc<AtomicU64>,
) {
    let req = match read_request(&mut s, lim) {
        Ok(r) => r,
        Err(ReadError::Closed) => return,
        Err(ReadError::Limit(code, msg)) => {
            rejected.fetch_add(1, Ordering::SeqCst);
            let _ = s.write_all(&simple(code, &format!("{{\"error\":\"{msg}\"}}")));
            return;
        }
    };
    // bounded admission: beyond the limit, answer without running a handler
    if in_flight.fetch_add(1, Ordering::SeqCst) >= lim.max_in_flight {
        in_flight.fetch_sub(1, Ordering::SeqCst);
        rejected.fetch_add(1, Ordering::SeqCst);
        let _ = s.write_all(&simple(503, "{\"error\":\"server busy\"}"));
        return;
    }
    let (tx, rx) = mpsc::channel::<Out>();
    task::post(&q, task::External::Callback(Box::new(move || admit(server, req, tx))));
    match rx.recv_timeout(Duration::from_secs(60)) {
        Ok(Out::Full(resp)) => {
            let _ = s.write_all(&resp);
        }
        Ok(Out::Part(head)) => {
            // streamed: write parts as they arrive until the end marker
            let _ = s.write_all(&head);
            let _ = s.flush();
            loop {
                match rx.recv_timeout(Duration::from_secs(60)) {
                    Ok(Out::Part(chunk)) => {
                        if s.write_all(&chunk).is_err() {
                            break;
                        }
                        let _ = s.flush();
                    }
                    Ok(Out::End) => {
                        let _ = s.write_all(b"0\r\n\r\n");
                        break;
                    }
                    // the task died mid-stream: truncate (no terminal chunk)
                    Ok(Out::Full(_)) | Err(_) => break,
                }
            }
        }
        Ok(Out::End) | Err(_) => {
            let _ = s.write_all(&simple(500, "{\"error\":\"internal server error\"}"));
        }
    }
    let _ = s.flush();
    in_flight.fetch_sub(1, Ordering::SeqCst);
}

/// Worker side: admit a parsed request as a task in the server's scope.
fn admit(server: u64, req: RawRequest, tx: mpsc::Sender<Out>) {
    let r = reg();
    let Some(srv) = r.servers.get(&server) else {
        let _ = tx.send(Out::Full(simple(503, "{\"error\":\"shutting down\"}")));
        return;
    };
    if srv.stop.load(Ordering::SeqCst) {
        let _ = tx.send(Out::Full(simple(503, "{\"error\":\"shutting down\"}")));
        return;
    }
    let (serve, req_desc, sc) = (srv.serve, srv.req_desc, srv.scope);
    let conn = r.next;
    r.next += 1;
    r.conns.insert(conn, (tx, server));
    let g = Roots::new();
    let headers_desc = req_desc.field("headers").map(|f| f.desc).unwrap();
    let hm = g.add(map::new(headers_desc));
    for (k, v) in &req.headers {
        let kw = g.add(new_string(k));
        let vw = g.add(new_string(v));
        map::insert_mut(hm, kw, vw);
    }
    let path = g.add(new_string(&req.path));
    let body = g.add(new_bytes(&req.body));
    let method = crate::string::symbol(&req.method);
    let rq =
        g.add(make_record(req_desc, &[("method", method), ("path", path), ("headers", hm), ("body", body)]));
    // spawn the request task (defects contained) in the server scope
    let id = scope::spawn(sc, serve, true, false);
    if let Some(t) = task::task(id) {
        t.entry_args = vec![(rq, true), (conn, false)];
    }
    if let Some(s) = reg().servers.get_mut(&server) {
        s.in_flight.retain(|t| !task::is_done(*t));
        s.in_flight.push(id);
    }
}

fn head(status: u64, headers: Option<u64>, framing: &str) -> String {
    let mut head = format!("HTTP/1.1 {status} {}\r\n", status_text(status));
    let mut has_type = false;
    if let Some(h) = headers {
        for (k, v) in map::entries(h) {
            has_type |= str_of(k).eq_ignore_ascii_case("content-type");
            head.push_str(&format!("{}: {}\r\n", str_of(k), str_of(v)));
        }
    }
    if !has_type {
        head.push_str("content-type: application/json\r\n");
    }
    head.push_str(&format!("{framing}\r\nconnection: close\r\n\r\n"));
    head
}

fn serialize(resp: u64) -> Vec<u8> {
    let status = crate::record_field(resp, "status").unwrap_or(200);
    let body = crate::record_field(resp, "body").map(|b| bytes_of(b).to_vec()).unwrap_or_default();
    let mut out =
        head(status, crate::record_field(resp, "headers"), &format!("content-length: {}", body.len()))
            .into_bytes();
    out.extend(body);
    out
}

macro_rules! http_intrinsics {
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

fn http_err(ret: &'static TypeDesc, msg: &str) -> u64 {
    let err_desc = ret.variant(1).unwrap().fields[0].desc;
    let g = Roots::new();
    let m = g.add(new_string(msg));
    let e = g.add(make_record(err_desc, &[("message", m)]));
    make_variant(ret, "err", &[e])
}

http_intrinsics! {
    // Start listening; `serve(request, conn)` handles one request.
    fn http_listen(ctx, targs, ret, address, port, serve, limits) {
        crate::repro::uncaptured("serving HTTP");
        let addr = format!("{}:{}", str_of(address), port & 0xffff);
        let listener = match TcpListener::bind(&addr) {
            Ok(l) => l,
            Err(e) => return http_err(rdesc(ret), &format!("cannot listen on {addr}: {e}")),
        };
        let local = listener.local_addr().unwrap();
        let r = reg();
        let id = r.next;
        r.next += 1;
        gc::pin(serve);
        let stop = Arc::new(AtomicBool::new(false));
        let sc = scope::open();
        let field = |n: &str, d: usize| crate::record_field(limits, n).map(|v| v as usize).filter(|v| *v > 0).unwrap_or(d);
        let def = default_limits();
        let lim = Limits {
            max_body_bytes: field("max_body_bytes", def.max_body_bytes),
            max_header_bytes: field("max_header_bytes", def.max_header_bytes),
            max_in_flight: field("max_in_flight", def.max_in_flight),
        };
        let rejected = Arc::new(AtomicU64::new(0));
        r.servers.insert(id, Server { serve, req_desc: targ(targs, 0), scope: sc, stop: stop.clone(), addr: local, in_flight: vec![], limits: lim, rejected: rejected.clone() });
        task::set_listener(1);
        let q = task::rt().external.clone();
        let in_flight = Arc::new(AtomicUsize::new(0));
        std::thread::spawn(move || {
            for s in listener.incoming() {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                if let Ok(s) = s {
                    let (q, in_flight, rejected) = (q.clone(), in_flight.clone(), rejected.clone());
                    std::thread::spawn(move || handle_conn(s, id, q, lim, in_flight, rejected));
                }
            }
        });
        if std::env::var_os("LANG_HTTP_ANNOUNCE").is_some() {
            eprintln!("listening on http://{local}");
        }
        make_variant(rdesc(ret), "ok", &[id])
    }
    // Effective limits and the count of requests rejected by them.
    fn http_limits(ctx, targs, ret, server) {
        let (lim, rejected) = match reg().servers.get(&server) {
            Some(s) => (s.limits, s.rejected.load(Ordering::SeqCst)),
            None => (default_limits(), 0),
        };
        make_record(rdesc(ret), &[
            ("max_body_bytes", lim.max_body_bytes as u64),
            ("max_header_bytes", lim.max_header_bytes as u64),
            ("max_in_flight", lim.max_in_flight as u64),
            ("rejected", rejected),
        ])
    }
    fn http_default_limits(ctx, targs, ret) {
        let lim = default_limits();
        make_record(rdesc(ret), &[
            ("max_body_bytes", lim.max_body_bytes as u64),
            ("max_header_bytes", lim.max_header_bytes as u64),
            ("max_in_flight", lim.max_in_flight as u64),
        ])
    }
    fn http_port(ctx, targs, ret, server) {
        reg().servers.get(&server).map(|s| s.addr.port() as u64).unwrap_or(0)
    }
    // Stop admission, settle in-flight requests, and release the listener.
    fn http_stop(ctx, targs, ret, server) {
        let Some(s) = reg().servers.get(&server) else { return 0 };
        s.stop.store(true, Ordering::SeqCst);
        let addr = s.addr;
        let sc = s.scope;
        let _ = TcpStream::connect_timeout(&addr, Duration::from_millis(200));
        scope::close(sc, false);
        if let Some(s) = reg().servers.remove(&server) {
            gc::unpin(s.serve);
        }
        task::set_listener(-1);
        0
    }
    // Registers a cleanup answering 500 if the request task dies first.
    fn http_begin(ctx, targs, ret, conn) {
        task::push_cleanup(task::Cleanup::Native(Box::new(move |_| {
            if let Some((tx, _)) = reg().conns.remove(&conn) {
                let _ = tx.send(Out::Full(simple(500, "{\"error\":\"internal server error\"}")));
            }
        })))
    }
    fn http_respond(ctx, targs, ret, conn, guard, resp) {
        task::remove_cleanup(guard);
        if let Some((tx, _)) = reg().conns.remove(&conn) {
            let _ = tx.send(Out::Full(serialize(resp)));
        }
        0
    }
    // Streaming: send the head (chunked framing); the request task then
    // sends every chunk and ends the stream before its scope closes. If it
    // dies mid-stream, its cleanup truncates the reply.
    fn http_stream_begin(ctx, targs, ret, conn, guard, status, headers) {
        task::remove_cleanup(guard);
        if let Some((tx, _)) = reg().conns.get(&conn) {
            let _ = tx.send(Out::Part(head(status, Some(headers), "transfer-encoding: chunked").into_bytes()));
        }
        // a dying stream must not leave the connection waiting
        task::push_cleanup(task::Cleanup::Native(Box::new(move |_| {
            if let Some((tx, _)) = reg().conns.remove(&conn) {
                let _ = tx.send(Out::Full(vec![]));
            }
        })))
    }
    fn http_stream_chunk(ctx, targs, ret, conn, b) {
        let data = bytes_of(b);
        if data.is_empty() {
            return 0;
        }
        if let Some((tx, _)) = reg().conns.get(&conn) {
            let mut framed = format!("{:x}\r\n", data.len()).into_bytes();
            framed.extend_from_slice(data);
            framed.extend_from_slice(b"\r\n");
            let _ = tx.send(Out::Part(framed));
        }
        0
    }
    fn http_stream_end(ctx, targs, ret, conn, guard) {
        task::remove_cleanup(guard);
        if let Some((tx, _)) = reg().conns.remove(&conn) {
            let _ = tx.send(Out::End);
        }
        0
    }
    // Park until SIGINT/SIGTERM (or `LANG_SHUTDOWN_AFTER_MS` elapses).
    fn sys_wait_shutdown(ctx, targs, ret) {
        install_signals();
        let r = reg();
        r.shutdown_waiters.push(task::current_id());
        task::set_listener(1);
        if let Some(ms) = std::env::var("LANG_SHUTDOWN_AFTER_MS").ok().and_then(|v| v.parse::<u64>().ok()) {
            task::sleep(Duration::from_millis(ms));
        } else {
            while !SIGNALLED.load(Ordering::SeqCst) {
                task::park_for("shutdown signal");
            }
        }
        task::set_listener(-1);
        0
    }
    fn http_body_text(ctx, targs, ret, b) { new_string(&String::from_utf8_lossy(bytes_of(b))) }
    // running-system registration for inspection
    fn sys_started(ctx, targs, ret, labels, providers) {
        let nodes = (0..list::len(labels))
            .map(|i| (str_of(list::get(labels, i)).to_string(), str_of(list::get(providers, i)).to_string()))
            .collect();
        crate::inspect::system_started(nodes)
    }
    fn sys_stopping(ctx, targs, ret, id, done) {
        crate::inspect::system_stopping(id, done & 1 == 1);
        0
    }
}

static SIGNALLED: AtomicBool = AtomicBool::new(false);
static RAISED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: libc::c_int) {
    RAISED.store(true, Ordering::SeqCst);
}

fn install_signals() {
    let r = reg();
    if r.signal_installed {
        return;
    }
    r.signal_installed = true;
    unsafe {
        libc::signal(libc::SIGINT, on_signal as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, on_signal as *const () as libc::sighandler_t);
    }
    let q = task::rt().external.clone();
    std::thread::spawn(move || {
        loop {
            if RAISED.load(Ordering::SeqCst) {
                task::post(
                    &q,
                    task::External::Callback(Box::new(|| {
                        SIGNALLED.store(true, Ordering::SeqCst);
                        for w in std::mem::take(&mut reg().shutdown_waiters) {
                            task::wake(w, task::Wake::Normal);
                        }
                    })),
                );
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    });
}

#[allow(dead_code)]
fn unused(_: &[u64]) -> usize {
    list::len(0)
}
