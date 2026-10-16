//! `lang_http`: the HTTP/1.1 server native module behind `std.http`
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
//! seconds is answered 500. Each connection closes after its reply.

use std::collections::VecDeque;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited, combinators::BoxBody};
use hyper::body::{Frame, Incoming};
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use lang_native::{Args, CallId, Completion, Error, Handler, Handles, ModuleDef, Outcome, Value};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, oneshot};

type Body = BoxBody<Bytes, std::io::Error>;

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("lang-http")
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
}

enum Target {
    Server(Arc<Server>),
    Exchange(Arc<Exchange>),
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

fn response(status: i64, headers: &[(String, Value)], body: Body) -> Response<Body> {
    let mut b = Response::builder().status(StatusCode::from_u16(status as u16).unwrap_or(StatusCode::OK));
    let mut typed = false;
    for (k, v) in headers {
        if let Value::Str(v) = v {
            typed |= k.eq_ignore_ascii_case("content-type");
            b = b.header(k.as_str(), v.as_str());
        }
    }
    if !typed {
        b = b.header("content-type", "application/json");
    }
    b.header("connection", "close").body(body).unwrap_or_else(|_| simple(500, "invalid response"))
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
                c.complete(Ok(request_value(p, self.limits.stream_buffer)));
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
fn request_value(p: Pending, buffer: usize) -> Value {
    let ex = Exchange { reply: Mutex::new(Some(p.reply)), stream: Mutex::new(None), buffer };
    Value::Map(vec![
        ("exchange".into(), HANDLES.insert(Target::Exchange(Arc::new(ex)))),
        ("method".into(), Value::Symbol(p.method)),
        ("path".into(), Value::Str(p.path)),
        ("headers".into(), Value::Map(p.headers.into_iter().map(|(k, v)| (k, Value::Str(v))).collect())),
        ("body".into(), Value::Bytes(p.body)),
    ])
}

async fn handle(server: Arc<Server>, req: Request<Incoming>) -> Result<Response<Body>, Infallible> {
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
    // bounded admission: beyond the limit, answer without the program
    if server.in_flight.fetch_add(1, Ordering::SeqCst) >= lim.in_flight {
        server.in_flight.fetch_sub(1, Ordering::SeqCst);
        return reject(503, "server busy");
    }
    let method = req.method().as_str().to_ascii_lowercase();
    let path = req.uri().path_and_query().map(|p| p.to_string()).unwrap_or_else(|| "/".into());
    let headers: Vec<(String, String)> = req
        .headers()
        .iter()
        .map(|(k, v)| (k.as_str().to_string(), String::from_utf8_lossy(v.as_bytes()).into_owned()))
        .collect();
    let body = match Limited::new(req.into_body(), lim.body).collect().await {
        Ok(b) => b.to_bytes().to_vec(),
        Err(_) => {
            server.in_flight.fetch_sub(1, Ordering::SeqCst);
            return reject(413, "request body too large");
        }
    };
    let (tx, rx) = oneshot::channel();
    server.deliver(Pending { method, path, headers, body, reply: tx });
    let resp = match tokio::time::timeout(Duration::from_secs(60), rx).await {
        Ok(Ok(r)) => r,
        _ => simple(500, "internal server error"),
    };
    server.in_flight.fetch_sub(1, Ordering::SeqCst);
    Ok(resp)
}

fn field(v: Option<&Value>, default: usize) -> usize {
    match v {
        Some(Value::Int(n)) if *n > 0 => *n as usize,
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
    let addr = format!("{address}:{}", port & 0xffff);
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
                let _ = hyper::server::conn::http1::Builder::new()
                    .keep_alive(false)
                    .max_buf_size(max_buf)
                    .serve_connection(TokioIo::new(stream), svc)
                    .await;
            });
        }
    });
    if std::env::var_os("LANG_HTTP_ANNOUNCE").is_some() {
        eprintln!("listening on http://{local}");
    }
    Ok(HANDLES.insert(Target::Server(server)))
}

fn server(a: &Args, i: usize) -> Result<Arc<Server>, Error> {
    match HANDLES.get(a.handle(i)?)?.as_ref() {
        Target::Server(s) => Ok(s.clone()),
        Target::Exchange(_) => Err(Error::new("http/argument", "not a server")),
    }
}

fn exchange(a: &Args, i: usize) -> Result<Arc<Exchange>, Error> {
    match HANDLES.get(a.handle(i)?)?.as_ref() {
        Target::Exchange(e) => Ok(e.clone()),
        Target::Server(_) => Err(Error::new("http/argument", "not an exchange")),
    }
}

/// The next admitted request (waits; fails with `:http/closed` once the
/// server stopped).
fn accept(a: Args, done: Completion) {
    let s = match server(&a, 0) {
        Ok(s) => s,
        Err(e) => return done.complete(Err(e)),
    };
    let mut q = s.queue.lock().unwrap();
    if let Some(p) = q.requests.pop_front() {
        drop(q);
        done.complete(Ok(request_value(p, s.limits.stream_buffer)));
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
    let frames = futures_stream(rx);
    let body = BoxBody::new(frames);
    let _ = tx.send(response(status, &headers, body));
    *ex.stream.lock().unwrap() = Some((ctx, sem));
    Ok(Value::Unit)
}

/// The receiver as a stream of body frames.
fn futures_stream(
    rx: mpsc::Receiver<Chunk>,
) -> impl hyper::body::Body<Data = Bytes, Error = std::io::Error> + Send + Sync + 'static {
    struct Frames(Mutex<mpsc::Receiver<Chunk>>);
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
    Frames(Mutex::new(rx))
}

/// Send one chunk of a started stream; waits while the client is behind.
fn stream_chunk(a: Args, done: Completion) {
    let (ex, data) = match (exchange(&a, 0), a.bytes(1)) {
        (Ok(e), Ok(b)) => (e, Bytes::from(b.to_vec())),
        (Err(e), _) | (_, Err(e)) => return done.complete(Err(e)),
    };
    if data.is_empty() {
        return done.complete(Ok(Value::Unit));
    }
    let Some((tx, sem)) = ex.stream.lock().unwrap().clone() else {
        return done.complete(Ok(Value::Unit));
    };
    // a chunk larger than the whole buffer waits for all of it
    let want = data.len().min(ex.buffer).max(1) as u32;
    runtime().spawn(async move {
        if let Ok(permit) = sem.acquire_many_owned(want).await {
            // a closed connection drops the chunk
            let _ = tx.send(Ok((data, permit))).await;
        }
        done.complete(Ok(Value::Unit));
    });
}

fn stream_end(a: &Args) -> Outcome {
    let ex = exchange(a, 0)?;
    ex.stream.lock().unwrap().take();
    Ok(Value::Unit)
}

/// The request's task failed: 500 if nothing was sent, else cut the
/// stream short (no terminal chunk).
fn abort_exchange(ex: &Exchange) {
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
        None => {}
    }
}

pub static DEF: ModuleDef = ModuleDef {
    name: "lang_http",
    functions: &[
        ("listen", Handler::Inline(listen)),
        ("accept", Handler::Async(accept)),
        ("respond", Handler::Inline(respond)),
        ("stream_begin", Handler::Inline(stream_begin)),
        ("stream_chunk", Handler::Async(stream_chunk)),
        ("stream_end", Handler::Inline(stream_end)),
        ("abort", Handler::Inline(abort)),
        ("stop", Handler::Inline(stop)),
        ("port", Handler::Inline(port)),
        ("stats", Handler::Inline(stats)),
    ],
    drop_handle: Some(drop_handle),
    cancel: Some(cancel),
    recorded: &[],
    pure: &[],
};

lang_native::export!("lang_http", DEF);
