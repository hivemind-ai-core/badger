//! `badger_http`: the HTTP/1.1 server native module behind `std.http`
//! (hyper on a small tokio runtime of the module's own).
//!
//! The module reads requests and enforces the explicit limits (a body over
//! the limit is answered 413, headers over it 431, a request arriving while
//! `max_in_flight` are being handled 503, all without reaching the
//! program); the program takes admitted requests with `accept` (an async
//! call) and answers each through its exchange handle: `respond`, or
//! `stream_begin` / `stream_chunk` / `stream_end` for a chunked reply whose
//! chunks wait (the task parks) while more than `max_stream_buffer_bytes`
//! are still unwritten. `abort` answers 500, or truncates a started stream,
//! for a request whose task failed. A request not answered within 60
//! seconds is answered 500. Each connection closes after its reply, but
//! for a websocket upgrade (`ws_accept`), which takes the connection over;
//! `ws_connect` opens client websockets (see `ws.rs`).

use std::collections::VecDeque;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use badger_native::{Args, CallId, Completion, Error, Handler, Handles, ModuleDef, Outcome, Value};
use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited, combinators::BoxBody};
use hyper::body::{Frame, Incoming};
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot, watch};

mod ws;

type Body = BoxBody<Bytes, std::io::Error>;

pub(crate) fn empty_body() -> Body {
    Full::new(Bytes::new()).map_err(|e| match e {}).boxed()
}

pub(crate) fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .on_thread_start(badger_native::module_thread)
            .worker_threads(2)
            .thread_name("badger-http")
            .enable_all()
            .build()
            .expect("the HTTP module's runtime")
    })
}

#[derive(Clone, Copy)]
struct Limits {
    body: usize,
    header: usize,
    in_flight: usize,
    stream_buffer: usize,
}

struct Pending {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    reply: oneshot::Sender<Response<Body>>,
    /// A websocket upgrade request: its key and the upgraded connection.
    ws: Option<(String, hyper::upgrade::OnUpgrade)>,
}

struct Queue {
    requests: VecDeque<Pending>,
    acceptors: VecDeque<Completion>,
    closed: bool,
}

struct Server {
    addr: SocketAddr,
    limits: Limits,
    rejected: AtomicU64,
    in_flight: AtomicUsize,
    stopped: AtomicBool,
    stop: tokio::sync::Notify,
    queue: Mutex<Queue>,
}

type Chunk = Result<(Bytes, OwnedSemaphorePermit), std::io::Error>;

struct Exchange {
    reply: Mutex<Option<oneshot::Sender<Response<Body>>>>,
    /// A started stream: its chunks, and permits for the bytes not yet
    /// written (`buffer` of them).
    stream: Mutex<Option<(mpsc::Sender<Chunk>, Arc<Semaphore>)>>,
    buffer: usize,
    /// The websocket key and upgrade of an upgrade request.
    ws_key: Option<String>,
    upgrade: Mutex<Option<hyper::upgrade::OnUpgrade>>,
    /// The largest websocket message accepted (the body limit).
    max_message: usize,
    /// OPEN, then ENDED (the program finished or aborted the reply) or
    /// GONE (hyper dropped the body: the client went away), whichever
    /// comes first.
    phase: Arc<watch::Sender<u8>>,
}

const OPEN: u8 = 0;
const ENDED: u8 = 1;
const GONE: u8 = 2;

fn settle(phase: &watch::Sender<u8>, to: u8) {
    phase.send_if_modified(|p| {
        let open = *p == OPEN;
        if open {
            *p = to;
        }
        open
    });
}

enum Target {
    Server(Arc<Server>),
    Exchange(Arc<Exchange>),
    Socket(Arc<ws::Socket>),
}

static HANDLES: Handles<Target> = Handles::new();

fn simple(status: u16, message: &str) -> Response<Body> {
    let body = format!("{{\"error\":\"{message}\"}}");
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .header("connection", "close")
        .body(Full::new(Bytes::from(body)).map_err(|e| match e {}).boxed())
        .expect("a valid response")
}

/// A reply. An invalid status or header is the program's mistake: it
/// panics, which the dispatcher reports to the program as a defect (rather
/// than silently answering 200, or 500 while the program sees success).
fn response(status: i64, headers: &[(String, Value)], body: Body) -> Response<Body> {
    let code = u16::try_from(status)
        .ok()
        .and_then(|s| StatusCode::from_u16(s).ok())
        .unwrap_or_else(|| panic!("http: {status} is not a valid response status (100-999)"));
    let mut b = Response::builder().status(code);
    let mut typed = false;
    for (k, v) in headers {
        if let Value::Str(v) = v {
            if hyper::header::HeaderName::from_bytes(k.as_bytes()).is_err() {
                panic!("http: {k:?} is not a valid header name");
            }
            if hyper::header::HeaderValue::from_str(v).is_err() {
                panic!(
                    "http: the value of header {k:?} is not valid (control characters or non-visible bytes)"
                );
            }
            typed |= k.eq_ignore_ascii_case("content-type");
            b = b.header(k.as_str(), v.as_str());
        }
    }
    if !typed {
        b = b.header("content-type", "application/json");
    }
    b.header("connection", "close").body(body).expect("a response of checked parts")
}

impl Server {
    /// Hand an admitted request to a waiting `accept`, or queue it.
    fn deliver(&self, p: Pending) {
        let mut q = self.queue.lock().unwrap();
        if q.closed {
            let _ = p.reply.send(simple(503, "shutting down"));
            return;
        }
        match q.acceptors.pop_front() {
            Some(c) => {
                drop(q);
                c.complete(Ok(request_value(p, self.limits)));
            }
            None => q.requests.push_back(p),
        }
    }

    fn shut(self: &Arc<Self>) {
        SERVERS.lock().unwrap().retain(|s| !Arc::ptr_eq(s, self));
        self.stopped.store(true, Ordering::SeqCst);
        self.stop.notify_waiters();
        let (acceptors, requests) = {
            let mut q = self.queue.lock().unwrap();
            q.closed = true;
            (std::mem::take(&mut q.acceptors), std::mem::take(&mut q.requests))
        };
        for c in acceptors {
            c.complete(Err(Error::new("http/closed", "the server stopped")));
        }
        for p in requests {
            let _ = p.reply.send(simple(503, "shutting down"));
        }
    }
}

/// An admitted request as the program sees it, with its exchange.
fn request_value(p: Pending, limits: Limits) -> Value {
    let (ws_key, upgrade) = match p.ws {
        Some((k, u)) => (Some(k), Some(u)),
        None => (None, None),
    };
    let ex = Exchange {
        reply: Mutex::new(Some(p.reply)),
        stream: Mutex::new(None),
        buffer: limits.stream_buffer,
        ws_key,
        upgrade: Mutex::new(upgrade),
        max_message: limits.body,
        phase: Arc::new(watch::channel(OPEN).0),
    };
    Value::Map(vec![
        ("exchange".into(), HANDLES.insert(Target::Exchange(Arc::new(ex)))),
        ("method".into(), Value::Symbol(p.method)),
        ("path".into(), Value::Str(p.path)),
        ("headers".into(), Value::Map(p.headers.into_iter().map(|(k, v)| (k, Value::Str(v))).collect())),
        ("body".into(), Value::Bytes(p.body)),
    ])
}

async fn handle(server: Arc<Server>, mut req: Request<Incoming>) -> Result<Response<Body>, Infallible> {
    let lim = server.limits;
    let reject = |status: u16, msg: &str| {
        server.rejected.fetch_add(1, Ordering::SeqCst);
        Ok(simple(status, msg))
    };
    if server.stopped.load(Ordering::SeqCst) {
        return Ok(simple(503, "shutting down"));
    }
    let head: usize = req.uri().to_string().len()
        + req.headers().iter().map(|(k, v)| k.as_str().len() + v.len() + 4).sum::<usize>()
        + 16;
    if head > lim.header {
        return reject(431, "request headers too large");
    }
    let declared = req
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(0);
    if declared > lim.body {
        return reject(413, "request body too large");
    }
    // bounded admission: beyond the limit, answer without the program. The
    // slot is released however this ends, including when the client goes
    // away and hyper drops this future mid-await.
    let Some(_slot) = Slot::take(&server.in_flight, lim.in_flight) else {
        return reject(503, "server busy");
    };
    let ws = websocket_key(&req).map(|k| (k, hyper::upgrade::on(&mut req)));
    let method = req.method().as_str().to_ascii_lowercase();
    let path = req.uri().path_and_query().map(|p| p.to_string()).unwrap_or_else(|| "/".into());
    let headers: Vec<(String, String)> = req
        .headers()
        .iter()
        .map(|(k, v)| (k.as_str().to_string(), String::from_utf8_lossy(v.as_bytes()).into_owned()))
        .collect();
    let body = match Limited::new(req.into_body(), lim.body).collect().await {
        Ok(b) => b.to_bytes().to_vec(),
        Err(e) if e.downcast_ref::<http_body_util::LengthLimitError>().is_some() => {
            return reject(413, "request body too large");
        }
        // the client stopped sending (or sent a malformed body)
        Err(_) => return reject(400, "incomplete request body"),
    };
    let (tx, rx) = oneshot::channel();
    server.deliver(Pending { method, path, headers, body, reply: tx, ws });
    let resp = match tokio::time::timeout(Duration::from_secs(60), rx).await {
        Ok(Ok(r)) => r,
        _ => simple(500, "internal server error"),
    };
    Ok(resp)
}

/// One admitted request's place in the in-flight count, released on drop.
struct Slot<'a>(&'a AtomicUsize);

impl<'a> Slot<'a> {
    fn take(count: &'a AtomicUsize, limit: usize) -> Option<Slot<'a>> {
        if count.fetch_add(1, Ordering::SeqCst) >= limit {
            count.fetch_sub(1, Ordering::SeqCst);
            return None;
        }
        Some(Slot(count))
    }
}

impl Drop for Slot<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// The `sec-websocket-key` of a websocket upgrade request.
fn websocket_key(req: &Request<Incoming>) -> Option<String> {
    let h = req.headers();
    let has = |name: &str, word: &str| {
        h.get_all(name)
            .iter()
            .any(|v| v.to_str().is_ok_and(|v| v.split(',').any(|w| w.trim().eq_ignore_ascii_case(word))))
    };
    (req.method() == hyper::Method::GET && has("upgrade", "websocket") && has("connection", "upgrade"))
        .then(|| h.get("sec-websocket-key")?.to_str().ok().map(str::to_string))
        .flatten()
}

/// A positive limit (at most what a semaphore can count), else `default`.
fn field(v: Option<&Value>, default: usize) -> usize {
    match v {
        Some(Value::Int(n)) if *n > 0 => (*n as usize).min(Semaphore::MAX_PERMITS),
        _ => default,
    }
}

fn listen(a: &Args) -> Outcome {
    let (address, port) = (a.str(0)?, a.int(1)?);
    let l = a.get(2).ok_or_else(|| Error::new("http/argument", "no limits"))?;
    let limits = Limits {
        body: field(l.field("max_body_bytes"), 16 << 20),
        header: field(l.field("max_header_bytes"), 64 << 10),
        in_flight: field(l.field("max_in_flight"), 1024),
        stream_buffer: field(l.field("max_stream_buffer_bytes"), 1 << 20),
    };
    let port = u16::try_from(port)
        .map_err(|_| Error::new("http/argument", format!("{port} is not a port (0-65535)")))?;
    let addr = format!("{address}:{port}");
    let std_listener = std::net::TcpListener::bind(&addr)
        .map_err(|e| Error::new("http/listen", format!("cannot listen on {addr}: {e}")))?;
    std_listener.set_nonblocking(true).map_err(|e| Error::new("http/listen", e.to_string()))?;
    let local = std_listener.local_addr().map_err(|e| Error::new("http/listen", e.to_string()))?;
    let server = Arc::new(Server {
        addr: local,
        limits,
        rejected: AtomicU64::new(0),
        in_flight: AtomicUsize::new(0),
        stopped: AtomicBool::new(false),
        stop: tokio::sync::Notify::new(),
        queue: Mutex::new(Queue { requests: VecDeque::new(), acceptors: VecDeque::new(), closed: false }),
    });
    let rt = runtime();
    let listener = {
        let _g = rt.enter();
        tokio::net::TcpListener::from_std(std_listener)
            .map_err(|e| Error::new("http/listen", e.to_string()))?
    };
    SERVERS.lock().unwrap().push(server.clone());
    let s = server.clone();
    rt.spawn(async move {
        loop {
            let stopped = s.stop.notified();
            tokio::pin!(stopped);
            if s.stopped.load(Ordering::SeqCst) {
                break;
            }
            let conn = tokio::select! {
                _ = &mut stopped => break,
                c = listener.accept() => c,
            };
            let Ok((stream, _)) = conn else { continue };
            let s = s.clone();
            tokio::spawn(async move {
                let max_buf = (s.limits.header + 1024).max(8192);
                let svc = service_fn(move |req| handle(s.clone(), req));
                // every reply says `connection: close` (but a websocket
                // upgrade's 101, which `keep_alive(false)` would rewrite)
                let _ = hyper::server::conn::http1::Builder::new()
                    .max_buf_size(max_buf)
                    .serve_connection(TokioIo::new(stream), svc)
                    .with_upgrades()
                    .await;
            });
        }
    });
    if std::env::var_os("BADGER_HTTP_ANNOUNCE").is_some() {
        eprintln!("listening on http://{local}");
    }
    Ok(HANDLES.insert(Target::Server(server)))
}

fn server(a: &Args, i: usize) -> Result<Arc<Server>, Error> {
    match HANDLES.get(a.handle(i)?)?.as_ref() {
        Target::Server(s) => Ok(s.clone()),
        _ => Err(Error::new("http/argument", "not a server")),
    }
}

pub(crate) fn exchange(a: &Args, i: usize) -> Result<Arc<Exchange>, Error> {
    match HANDLES.get(a.handle(i)?)?.as_ref() {
        Target::Exchange(e) => Ok(e.clone()),
        _ => Err(Error::new("http/argument", "not an exchange")),
    }
}

/// The next admitted request (waits; fails with `#http/closed` once the
/// server stopped).
fn accept(a: Args, done: Completion) {
    let s = match server(&a, 0) {
        Ok(s) => s,
        Err(e) => return done.complete(Err(e)),
    };
    let mut q = s.queue.lock().unwrap();
    if let Some(p) = q.requests.pop_front() {
        drop(q);
        done.complete(Ok(request_value(p, s.limits)));
    } else if q.closed {
        drop(q);
        done.complete(Err(Error::new("http/closed", "the server stopped")));
    } else {
        q.acceptors.push_back(done);
    }
}

/// A waiting `accept` whose task was cancelled stops waiting (so no
/// request is handed to it).
fn cancel(id: CallId) {
    if ws::cancel(id) {
        return;
    }
    let servers = SERVERS.lock().unwrap().clone();
    for s in servers {
        let mut q = s.queue.lock().unwrap();
        if let Some(i) = q.acceptors.iter().position(|c| c.id() == id) {
            let c = q.acceptors.remove(i);
            drop(q);
            // the host discards a withdrawn call's completion
            if let Some(c) = c {
                c.complete(Err(Error::new("http/closed", "cancelled")));
            }
            return;
        }
    }
}

/// Running servers (for `cancel`).
static SERVERS: Mutex<Vec<Arc<Server>>> = Mutex::new(Vec::new());

fn respond(a: &Args) -> Outcome {
    let ex = exchange(a, 0)?;
    let status = a.int(1)?;
    let headers = match a.get(2) {
        Some(Value::Map(h)) => h.clone(),
        _ => vec![],
    };
    let body = Bytes::from(a.bytes(3)?.to_vec());
    if let Some(tx) = ex.reply.lock().unwrap().take() {
        let _ = tx.send(response(status, &headers, Full::new(body).map_err(|e| match e {}).boxed()));
    }
    Ok(Value::Unit)
}

fn stream_begin(a: &Args) -> Outcome {
    let ex = exchange(a, 0)?;
    let status = a.int(1)?;
    let headers = match a.get(2) {
        Some(Value::Map(h)) => h.clone(),
        _ => vec![],
    };
    let Some(tx) = ex.reply.lock().unwrap().take() else { return Ok(Value::Unit) };
    let (ctx, rx) = mpsc::channel::<Chunk>(64);
    let sem = Arc::new(Semaphore::new(ex.buffer));
    // a chunk's permits return as hyper takes it to write
    let frames = futures_stream(rx, sem.clone(), ex.phase.clone());
    let body = BoxBody::new(frames);
    let _ = tx.send(response(status, &headers, body));
    *ex.stream.lock().unwrap() = Some((ctx, sem));
    Ok(Value::Unit)
}

/// The receiver as a stream of body frames.
/// Dropping it before the program ended the stream means the client went
/// away: senders waiting for buffer space fail rather than wait forever.
fn futures_stream(
    rx: mpsc::Receiver<Chunk>,
    sem: Arc<Semaphore>,
    phase: Arc<watch::Sender<u8>>,
) -> impl hyper::body::Body<Data = Bytes, Error = std::io::Error> + Send + Sync + 'static {
    struct Frames(Mutex<mpsc::Receiver<Chunk>>, Arc<Semaphore>, Arc<watch::Sender<u8>>);
    impl Drop for Frames {
        fn drop(&mut self) {
            settle(&self.2, GONE);
            self.1.close();
        }
    }
    impl hyper::body::Body for Frames {
        type Data = Bytes;
        type Error = std::io::Error;
        fn poll_frame(
            self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Result<Frame<Bytes>, std::io::Error>>> {
            let mut rx = self.0.lock().unwrap();
            match rx.poll_recv(cx) {
                std::task::Poll::Ready(Some(Ok((b, permit)))) => {
                    drop(permit);
                    std::task::Poll::Ready(Some(Ok(Frame::data(b))))
                }
                std::task::Poll::Ready(Some(Err(e))) => std::task::Poll::Ready(Some(Err(e))),
                std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
                std::task::Poll::Pending => std::task::Poll::Pending,
            }
        }
    }
    Frames(Mutex::new(rx), sem, phase)
}

/// Send one chunk of a started stream; waits while the client is behind.
/// False once the client has gone away (the chunk is dropped).
fn stream_chunk(a: Args, done: Completion) {
    let (ex, data) = match (exchange(&a, 0), a.bytes(1)) {
        (Ok(e), Ok(b)) => (e, Bytes::from(b.to_vec())),
        (Err(e), _) | (_, Err(e)) => return done.complete(Err(e)),
    };
    let Some((tx, sem)) = ex.stream.lock().unwrap().clone() else {
        return done.complete(Ok(Value::Bool(false)));
    };
    if data.is_empty() {
        return done.complete(Ok(Value::Bool(!tx.is_closed())));
    }
    // a chunk larger than the whole buffer waits for all of it
    let want = data.len().min(ex.buffer).max(1) as u32;
    runtime().spawn(async move {
        let sent = match sem.acquire_many_owned(want).await {
            Ok(permit) => tx.send(Ok((data, permit))).await.is_ok(),
            Err(_) => false,
        };
        done.complete(Ok(Value::Bool(sent)));
    });
}

fn stream_end(a: &Args) -> Outcome {
    let ex = exchange(a, 0)?;
    settle(&ex.phase, ENDED);
    ex.stream.lock().unwrap().take();
    Ok(Value::Unit)
}

/// Wait until a started stream ends: true if the client went away first
/// (the program should stop producing), false if the program ended it.
fn stream_watch(a: Args, done: Completion) {
    let ex = match exchange(&a, 0) {
        Ok(e) => e,
        Err(e) => return done.complete(Err(e)),
    };
    let mut phase = ex.phase.subscribe();
    drop(ex);
    runtime().spawn(async move {
        let gone = match phase.wait_for(|p| *p != OPEN).await {
            Ok(p) => *p == GONE,
            Err(_) => false,
        };
        done.complete(Ok(Value::Bool(gone)));
    });
}

/// The request's task failed: 500 if nothing was sent, else cut the
/// stream short (no terminal chunk).
fn abort_exchange(ex: &Exchange) {
    settle(&ex.phase, ENDED);
    if let Some(tx) = ex.reply.lock().unwrap().take() {
        let _ = tx.send(simple(500, "internal server error"));
    }
    if let Some((tx, _)) = ex.stream.lock().unwrap().take() {
        let _ = tx.try_send(Err(std::io::Error::other("the request task failed")));
    }
}

fn abort(a: &Args) -> Outcome {
    let ex = exchange(a, 0)?;
    abort_exchange(&ex);
    Ok(Value::Unit)
}

fn stop(a: &Args) -> Outcome {
    server(a, 0)?.shut();
    Ok(Value::Unit)
}

fn port(a: &Args) -> Outcome {
    Ok(Value::Int(server(a, 0)?.addr.port() as i64))
}

fn stats(a: &Args) -> Outcome {
    let s = server(a, 0)?;
    Ok(Value::Map(vec![
        ("max_body_bytes".into(), Value::Int(s.limits.body as i64)),
        ("max_header_bytes".into(), Value::Int(s.limits.header as i64)),
        ("max_in_flight".into(), Value::Int(s.limits.in_flight as i64)),
        ("rejected".into(), Value::Int(s.rejected.load(Ordering::SeqCst) as i64)),
    ]))
}

fn drop_handle(h: u64) {
    match HANDLES.remove(h).as_deref() {
        Some(Target::Exchange(e)) => abort_exchange(e),
        Some(Target::Server(s)) => s.shut(),
        Some(Target::Socket(s)) => ws::release(s.clone()),
        None => {}
    }
}

pub static DEF: ModuleDef = ModuleDef {
    name: "badger_http",
    functions: &[
        ("listen", Handler::Inline(listen)),
        ("accept", Handler::Async(accept)),
        ("respond", Handler::Inline(respond)),
        ("stream_begin", Handler::Inline(stream_begin)),
        ("stream_chunk", Handler::Async(stream_chunk)),
        ("stream_end", Handler::Inline(stream_end)),
        ("stream_watch", Handler::Async(stream_watch)),
        ("abort", Handler::Inline(abort)),
        ("stop", Handler::Inline(stop)),
        ("port", Handler::Inline(port)),
        ("stats", Handler::Inline(stats)),
        ("ws_accept", Handler::Async(ws::accept)),
        ("ws_connect", Handler::Async(ws::connect)),
        ("ws_recv", Handler::Async(ws::recv)),
        ("ws_send", Handler::Async(ws::send)),
        ("ws_close", Handler::Async(ws::close)),
    ],
    drop_handle: Some(drop_handle),
    cancel: Some(cancel),
    recorded: &[],
    pure: &[],
};

badger_native::export!("badger_http", DEF);
