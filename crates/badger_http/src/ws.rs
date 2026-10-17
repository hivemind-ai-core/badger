//! WebSockets: a server-side upgrade of an admitted request (`ws_accept`)
//! and client connections (`ws_connect`), behind one socket handle. The
//! send and receive halves wait independently, so one task can read while
//! another writes. Pings are answered by the library; a close frame or a
//! dropped connection ends the receive side (`recv` gives `none`).

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use badger_native::{Args, CallId, Completion, Error, Outcome, Value};
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use hyper_util::rt::TokioIo;
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, Role, WebSocketConfig, frame::coding::CloseCode};
use tokio_tungstenite::tungstenite::{Error as WsError, Message};

use crate::{HANDLES, Target, exchange, runtime};

type Tx = Pin<Box<dyn Sink<Message, Error = WsError> + Send>>;
type Rx = Pin<Box<dyn Stream<Item = Result<Message, WsError>> + Send>>;

pub(crate) struct Socket {
    tx: tokio::sync::Mutex<Tx>,
    rx: tokio::sync::Mutex<Rx>,
}

/// Waiting socket calls, so `cancel` can stop them.
static WAITING: Mutex<Option<HashMap<CallId, oneshot::Sender<()>>>> = Mutex::new(None);

fn spawn_call(done: Completion, work: impl Future<Output = Outcome> + Send + 'static) {
    let id = done.id();
    let (stop, stopped) = oneshot::channel();
    WAITING.lock().unwrap().get_or_insert_default().insert(id, stop);
    runtime().spawn(async move {
        let outcome = tokio::select! {
            o = work => o,
            _ = stopped => Err(Error::new("websocket/cancelled", "cancelled")),
        };
        if let Some(w) = WAITING.lock().unwrap().as_mut() {
            w.remove(&id);
        }
        done.complete(outcome);
    });
}

/// Stop a waiting socket call; whether it was one.
pub(crate) fn cancel(id: CallId) -> bool {
    let stop = WAITING.lock().unwrap().as_mut().and_then(|w| w.remove(&id));
    match stop {
        Some(s) => {
            let _ = s.send(());
            true
        }
        None => false,
    }
}

fn config(max_message: usize) -> WebSocketConfig {
    WebSocketConfig::default().max_message_size(Some(max_message)).max_frame_size(Some(max_message))
}

fn socket_value<S>(ws: tokio_tungstenite::WebSocketStream<S>) -> Value
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let (tx, rx) = ws.split();
    let s = Socket { tx: tokio::sync::Mutex::new(Box::pin(tx)), rx: tokio::sync::Mutex::new(Box::pin(rx)) };
    Value::Map(vec![("handle".into(), HANDLES.insert(Target::Socket(Arc::new(s))))])
}

fn socket(a: &Args) -> Result<Arc<Socket>, Error> {
    match HANDLES.get(a.handle(0)?)?.as_ref() {
        Target::Socket(s) => Ok(s.clone()),
        _ => Err(Error::new("websocket/handle", "not a websocket")),
    }
}

fn ws_error(e: WsError) -> Error {
    match e {
        WsError::ConnectionClosed | WsError::AlreadyClosed => {
            Error::new("websocket/closed", "the connection is closed")
        }
        WsError::Capacity(c) => Error::new("websocket/too_large", c.to_string()),
        WsError::Protocol(p) => Error::new("websocket/protocol", p.to_string()),
        WsError::Url(u) => Error::new("websocket/url", u.to_string()),
        WsError::Http(r) => Error::new("websocket/handshake", format!("the server answered {}", r.status())),
        other => Error::new("websocket/io", other.to_string()),
    }
}

/// Answer an upgrade request (101) and take over its connection.
pub(crate) fn accept(a: Args, done: Completion) {
    let parsed = (|| Ok((exchange(&a, 0)?, a.str(1)?.to_string())))();
    let (ex, protocol) = match parsed {
        Ok(x) => x,
        Err(e) => return done.complete(Err(e)),
    };
    spawn_call(done, async move {
        let (Some(key), Some(upgrade)) = (ex.ws_key.clone(), ex.upgrade.lock().unwrap().take()) else {
            return Err(Error::new("websocket/not_upgrade", "the request is not a websocket upgrade"));
        };
        let Some(reply) = ex.reply.lock().unwrap().take() else {
            return Err(Error::new("websocket/answered", "the request was already answered"));
        };
        let mut resp = hyper::Response::builder()
            .status(hyper::StatusCode::SWITCHING_PROTOCOLS)
            .header("upgrade", "websocket")
            .header("connection", "upgrade")
            .header(
                "sec-websocket-accept",
                tokio_tungstenite::tungstenite::handshake::derive_accept_key(key.as_bytes()),
            );
        if !protocol.is_empty() {
            resp = resp.header("sec-websocket-protocol", protocol);
        }
        let resp =
            resp.body(crate::empty_body()).map_err(|e| Error::new("websocket/handshake", e.to_string()))?;
        let _ = reply.send(resp);
        let upgraded = upgrade.await.map_err(|e| Error::new("websocket/handshake", e.to_string()))?;
        let ws = tokio_tungstenite::WebSocketStream::from_raw_socket(
            TokioIo::new(upgraded),
            Role::Server,
            Some(config(ex.max_message)),
        )
        .await;
        Ok(socket_value(ws))
    });
}

/// Connect to a `ws://` or `wss://` URL.
pub(crate) fn connect(a: Args, done: Completion) {
    let parsed = (|| {
        let url = a.str(0)?.to_string();
        let mut headers = vec![];
        if let Some(Value::Map(h)) = a.get(1) {
            for (k, v) in h {
                if let Value::Str(v) = v {
                    headers.push((k.clone(), v.clone()));
                }
            }
        }
        Ok((url, headers, a.int(2)?))
    })();
    let (url, headers, max) = match parsed {
        Ok(x) => x,
        Err(e) => return done.complete(Err(e)),
    };
    spawn_call(done, async move {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        let mut req = url.as_str().into_client_request().map_err(ws_error)?;
        for (k, v) in headers {
            let name = hyper::header::HeaderName::from_bytes(k.as_bytes())
                .map_err(|e| Error::new("websocket/header", format!("{k}: {e}")))?;
            let value = hyper::header::HeaderValue::from_str(&v)
                .map_err(|e| Error::new("websocket/header", format!("{k}: {e}")))?;
            req.headers_mut().insert(name, value);
        }
        let (ws, _) =
            tokio_tungstenite::connect_async_with_config(req, Some(config(max.max(1) as usize)), false)
                .await
                .map_err(ws_error)?;
        Ok(socket_value(ws))
    });
}

/// The next text or binary message; `none` once the peer closed.
pub(crate) fn recv(a: Args, done: Completion) {
    let s = match socket(&a) {
        Ok(s) => s,
        Err(e) => return done.complete(Err(e)),
    };
    spawn_call(done, async move {
        let mut rx = s.rx.lock().await;
        loop {
            return match rx.next().await {
                Some(Ok(Message::Text(t))) => {
                    Ok(Value::Variant("text".into(), vec![Value::Str(t.to_string())]))
                }
                Some(Ok(Message::Binary(b))) => {
                    Ok(Value::Variant("binary".into(), vec![Value::Bytes(b.to_vec())]))
                }
                Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => continue,
                Some(Ok(Message::Close(_))) | None => Ok(Value::Null),
                Some(Err(WsError::ConnectionClosed | WsError::AlreadyClosed)) => Ok(Value::Null),
                Some(Err(e)) => Err(ws_error(e)),
            };
        }
    });
}

/// Send a message (waits while the peer is behind).
pub(crate) fn send(a: Args, done: Completion) {
    let parsed = (|| {
        let s = socket(&a)?;
        let m = match a.get(1) {
            Some(Value::Variant(k, xs)) if k == "text" => match xs.first() {
                Some(Value::Str(t)) => Message::text(t.clone()),
                _ => return Err(Error::new("websocket/argument", "text(value: String)")),
            },
            Some(Value::Variant(k, xs)) if k == "binary" => match xs.first() {
                Some(Value::Bytes(b)) => Message::binary(b.clone()),
                _ => return Err(Error::new("websocket/argument", "binary(value: Bytes)")),
            },
            _ => return Err(Error::new("websocket/argument", "a message")),
        };
        Ok((s, m))
    })();
    let (s, m) = match parsed {
        Ok(x) => x,
        Err(e) => return done.complete(Err(e)),
    };
    spawn_call(done, async move {
        s.tx.lock().await.send(m).await.map_err(ws_error)?;
        Ok(Value::Unit)
    });
}

/// Send a close frame (`code`, `reason`) and stop sending.
pub(crate) fn close(a: Args, done: Completion) {
    let parsed = (|| Ok((socket(&a)?, a.int(1)?, a.str(2)?.to_string())))();
    let (s, code, reason) = match parsed {
        Ok(x) => x,
        Err(e) => return done.complete(Err(e)),
    };
    spawn_call(done, async move {
        let frame =
            CloseFrame { code: CloseCode::from(code.clamp(1000, 4999) as u16), reason: reason.into() };
        let mut tx = s.tx.lock().await;
        match tx.send(Message::Close(Some(frame))).await {
            Ok(()) | Err(WsError::ConnectionClosed | WsError::AlreadyClosed) => {}
            Err(e) => return Err(ws_error(e)),
        }
        let _ = tx.close().await;
        Ok(Value::Unit)
    });
}

/// A socket nobody can reach closes normally.
pub(crate) fn release(s: Arc<Socket>) {
    runtime().spawn(async move {
        let mut tx = s.tx.lock().await;
        let _ = tx.send(Message::Close(None)).await;
        let _ = tx.close().await;
    });
}
