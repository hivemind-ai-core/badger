//! `badger_valkey`: the Valkey/Redis client native module behind
//! `std.valkey` (redis-rs on a small tokio runtime of the module's own).
//!
//! A client is one multiplexed connection that reconnects on failure
//! (commands from many tasks share it; each is an async call, so a waiting
//! task parks). Replies come back as `Data`: nil is `null`, integers,
//! doubles and booleans as themselves, bulk and simple strings as strings,
//! arrays and sets as lists, and maps as objects. `command_bytes` returns a
//! bulk reply's raw bytes instead. A subscription is a connection of its
//! own; `next` waits for its next message. A cancelled call stops waiting
//! (a command already sent still runs on the server; a message not yet
//! received stays queued for the next `next`).

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use badger_native::{Args, CallId, Completion, Error, Handler, Handles, ModuleDef, Outcome, Value};
use futures_util::StreamExt;
use redis::aio::{ConnectionManager, ConnectionManagerConfig};
use tokio::sync::oneshot;

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("badger-valkey")
            .enable_all()
            .build()
            .expect("the Valkey module's runtime")
    })
}

type Messages = Pin<Box<dyn futures_util::Stream<Item = redis::Msg> + Send>>;

enum Target {
    Client(ConnectionManager),
    Subscription(tokio::sync::Mutex<Messages>),
}

static HANDLES: Handles<Target> = Handles::new();

/// Waiting calls, so `cancel` can stop them.
static WAITING: Mutex<Option<HashMap<CallId, oneshot::Sender<()>>>> = Mutex::new(None);

/// Run `work` for the call `done`; a cancel completes it at once.
fn spawn_call(done: Completion, work: impl Future<Output = Outcome> + Send + 'static) {
    let id = done.id();
    let (stop, stopped) = oneshot::channel();
    WAITING.lock().unwrap().get_or_insert_default().insert(id, stop);
    runtime().spawn(async move {
        let outcome = tokio::select! {
            o = work => o,
            _ = stopped => Err(Error::new("valkey/cancelled", "cancelled")),
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

fn error(e: redis::RedisError) -> Error {
    let kind = if e.is_connection_refusal() || e.is_connection_dropped() || e.is_io_error() {
        "valkey/connection"
    } else if e.kind() == redis::ErrorKind::InvalidClientConfig {
        "valkey/config"
    } else if e.is_timeout() {
        "valkey/timeout"
    } else if e.kind() == redis::ErrorKind::Server(redis::ServerErrorKind::ResponseError)
        || e.code().is_some()
    {
        "valkey/server"
    } else {
        "valkey/error"
    };
    Error::new(kind, e.to_string())
}

fn text(b: Vec<u8>) -> Value {
    match String::from_utf8(b) {
        Ok(s) => Value::Str(s),
        Err(e) => Value::Str(String::from_utf8_lossy(e.as_bytes()).into_owned()),
    }
}

/// A reply as `Data` (a server error inside an array fails the call).
fn data(v: redis::Value) -> Result<Value, Error> {
    use redis::Value as V;
    Ok(match v {
        V::Nil => Value::Null,
        V::Int(i) => Value::Int(i),
        V::Double(f) => Value::Float(f),
        V::Boolean(b) => Value::Bool(b),
        V::BulkString(b) => text(b),
        V::SimpleString(s) => Value::Str(s),
        V::Okay => Value::Str("OK".into()),
        V::VerbatimString { text, .. } => Value::Str(text),
        V::BigNumber(n) => Value::Str(format!("{n:?}")),
        V::Array(xs) | V::Set(xs) => Value::List(xs.into_iter().map(data).collect::<Result<_, _>>()?),
        V::Push { data: xs, .. } => Value::List(xs.into_iter().map(data).collect::<Result<_, _>>()?),
        V::Map(kvs) => Value::Map(
            kvs.into_iter()
                .map(|(k, v)| {
                    let k = match data(k)? {
                        Value::Str(s) => s,
                        Value::Int(i) => i.to_string(),
                        other => format!("{other:?}"),
                    };
                    Ok((k, data(v)?))
                })
                .collect::<Result<_, Error>>()?,
        ),
        V::Attribute { data: d, .. } => data(*d)?,
        V::ServerError(e) => return Err(Error::new("valkey/server", format!("{e:?}"))),
        other => Value::Str(format!("{other:?}")),
    })
}

fn command_of(args: &[Value]) -> Result<redis::Cmd, Error> {
    let mut parts = args.iter();
    let name = match parts.next() {
        Some(Value::Str(s)) => s.clone(),
        Some(Value::Bytes(b)) => String::from_utf8_lossy(b).into_owned(),
        _ => return Err(Error::new("valkey/command", "a command needs a name")),
    };
    let mut cmd = redis::cmd(&name);
    for p in parts {
        match p {
            Value::Str(s) => cmd.arg(s.as_bytes()),
            Value::Bytes(b) => cmd.arg(b.as_slice()),
            Value::Int(i) => cmd.arg(*i),
            other => return Err(Error::new("valkey/command", format!("unsupported argument {other:?}"))),
        };
    }
    Ok(cmd)
}

fn client(a: &Args, i: usize) -> Result<ConnectionManager, Error> {
    match HANDLES.get(a.handle(i)?)?.as_ref() {
        Target::Client(c) => Ok(c.clone()),
        Target::Subscription(_) => Err(Error::new("valkey/handle", "not a client")),
    }
}

/// Connect (waits): `redis://`, `rediss://` (TLS), `valkey://`, `valkeys://`.
fn connect(a: Args, done: Completion) {
    let url = match a.str(0) {
        Ok(u) => normalize(u),
        Err(e) => return done.complete(Err(e)),
    };
    spawn_call(done, async move {
        let c = redis::Client::open(url.as_str()).map_err(error)?;
        // no reply time limit (blocking commands wait; a task bounds its
        // own wait with `task.timeout`); a refused connection fails soon
        let config = ConnectionManagerConfig::new()
            .set_response_timeout(None)
            .set_connection_timeout(Some(Duration::from_secs(5)))
            .set_number_of_retries(2)
            .set_max_delay(Duration::from_secs(2));
        let conn = ConnectionManager::new_with_config(c, config).await.map_err(error)?;
        Ok(HANDLES.insert(Target::Client(conn)))
    });
}

/// `valkey://` and `valkeys://` are the Valkey spellings of the Redis URLs.
fn normalize(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("valkeys://") {
        format!("rediss://{rest}")
    } else if let Some(rest) = url.strip_prefix("valkey://") {
        format!("redis://{rest}")
    } else {
        url.to_string()
    }
}

fn command(a: Args, done: Completion) {
    let (mut conn, cmd) = match (client(&a, 0), a.list(1).and_then(command_of)) {
        (Ok(c), Ok(cmd)) => (c, cmd),
        (Err(e), _) | (_, Err(e)) => return done.complete(Err(e)),
    };
    spawn_call(done, async move {
        let v: redis::Value = cmd.query_async(&mut conn).await.map_err(error)?;
        data(v)
    });
}

/// The raw bytes of a bulk reply (`none` for nil).
fn command_bytes(a: Args, done: Completion) {
    let (mut conn, cmd) = match (client(&a, 0), a.list(1).and_then(command_of)) {
        (Ok(c), Ok(cmd)) => (c, cmd),
        (Err(e), _) | (_, Err(e)) => return done.complete(Err(e)),
    };
    spawn_call(done, async move {
        let v: redis::Value = cmd.query_async(&mut conn).await.map_err(error)?;
        match v {
            redis::Value::Nil => Ok(Value::Null),
            redis::Value::BulkString(b) => Ok(Value::Bytes(b)),
            redis::Value::SimpleString(s) => Ok(Value::Bytes(s.into_bytes())),
            redis::Value::Okay => Ok(Value::Bytes(b"OK".to_vec())),
            other => Err(Error::new("valkey/reply", format!("not a string reply: {other:?}"))),
        }
    });
}

/// Commands sent together (in one round trip); `atomic` wraps them in
/// MULTI/EXEC.
fn pipeline(a: Args, done: Completion) {
    let built = (|| {
        let conn = client(&a, 0)?;
        let mut p = redis::pipe();
        if a.bool(2)? {
            p.atomic();
        }
        for c in a.list(1)? {
            match c {
                Value::List(args) => p.add_command(command_of(args)?),
                _ => return Err(Error::new("valkey/command", "a pipeline holds commands")),
            };
        }
        Ok((conn, p))
    })();
    let (mut conn, p) = match built {
        Ok(x) => x,
        Err(e) => return done.complete(Err(e)),
    };
    spawn_call(done, async move {
        let vs: Vec<redis::Value> = p.query_async(&mut conn).await.map_err(error)?;
        Ok(Value::List(vs.into_iter().map(data).collect::<Result<_, _>>()?))
    });
}

/// A subscription on a connection of its own to `channels` and `patterns`.
fn subscribe(a: Args, done: Completion) {
    let parsed = (|| {
        let url = normalize(a.str(0)?);
        let names = |i: usize| -> Result<Vec<String>, Error> {
            a.list(i)?
                .iter()
                .map(|v| match v {
                    Value::Str(s) => Ok(s.clone()),
                    _ => Err(Error::new("valkey/command", "channel names are strings")),
                })
                .collect()
        };
        Ok((url, names(1)?, names(2)?))
    })();
    let (url, channels, patterns) = match parsed {
        Ok(x) => x,
        Err(e) => return done.complete(Err(e)),
    };
    spawn_call(done, async move {
        let c = redis::Client::open(url.as_str()).map_err(error)?;
        let mut ps = c.get_async_pubsub().await.map_err(error)?;
        for ch in &channels {
            ps.subscribe(ch).await.map_err(error)?;
        }
        for p in &patterns {
            ps.psubscribe(p).await.map_err(error)?;
        }
        let messages: Messages = Box::pin(ps.into_on_message());
        Ok(HANDLES.insert(Target::Subscription(tokio::sync::Mutex::new(messages))))
    });
}

/// The next message (waits); `none` once the connection is gone.
fn next(a: Args, done: Completion) {
    let sub = match a.handle(0).and_then(|h| HANDLES.get(h)) {
        Ok(s) => s,
        Err(e) => return done.complete(Err(e)),
    };
    spawn_call(done, async move {
        let Target::Subscription(m) = sub.as_ref() else {
            return Err(Error::new("valkey/handle", "not a subscription"));
        };
        let mut m = m.lock().await;
        Ok(match m.next().await {
            Some(msg) => Value::Map(vec![
                ("channel".into(), Value::Str(msg.get_channel_name().to_string())),
                (
                    "pattern".into(),
                    match msg.get_pattern::<Option<String>>() {
                        Ok(Some(p)) => Value::Str(p),
                        _ => Value::Null,
                    },
                ),
                ("payload".into(), text(msg.get_payload_bytes().to_vec())),
            ]),
            None => Value::Null,
        })
    });
}

fn close(a: &Args) -> Outcome {
    HANDLES.remove(a.handle(0)?);
    Ok(Value::Unit)
}

fn drop_handle(h: u64) {
    HANDLES.remove(h);
}

pub static DEF: ModuleDef = ModuleDef {
    name: "badger_valkey",
    functions: &[
        ("connect", Handler::Async(connect)),
        ("command", Handler::Async(command)),
        ("command_bytes", Handler::Async(command_bytes)),
        ("pipeline", Handler::Async(pipeline)),
        ("subscribe", Handler::Async(subscribe)),
        ("next", Handler::Async(next)),
        ("close", Handler::Inline(close)),
    ],
    drop_handle: Some(drop_handle),
    cancel: Some(cancel),
    recorded: &[
        ("connect", "valkey connect", 1),
        ("command", "valkey", 2),
        ("command_bytes", "valkey", 2),
        ("pipeline", "valkey", 3),
        ("subscribe", "valkey subscribe", 3),
        ("next", "valkey message", 1),
        ("close", "valkey close", 1),
    ],
    pure: &[],
};

badger_native::export!("badger_valkey", DEF);
