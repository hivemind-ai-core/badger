//! `badger_fetch`: the native module behind `stdx.fetch`. Requests run on
//! the runtime's blocking pool; redirects are not followed, every status
//! is a reply, and bodies are text (up to 16 MiB). Reproduction bundles
//! record each request by its method and URL, with a digest of what it
//! sends, so a replay answers it from the recording.

use std::sync::Mutex;

use badger_native::{Args, Error, Handler, ModuleDef, Outcome, Value};

/// `scheme://host[:port]` of an `http` or `https` URL that names only an
/// origin (an optional `/` after it, no user, path, query, or fragment).
pub fn origin_of(url: &str) -> Result<String, String> {
    let (scheme, rest) = url.split_once("://").ok_or_else(|| format!("`{url}` is not an http(s) URL"))?;
    if scheme != "http" && scheme != "https" {
        return Err(format!("`{url}`: only http and https are supported"));
    }
    let host = rest.strip_suffix('/').unwrap_or(rest);
    if host.is_empty()
        || host.contains(['/', '?', '#', '@', '\\', ' '])
        || host.chars().any(|c| c.is_control())
    {
        return Err(format!("`{url}` is not an origin (scheme://host[:port])"));
    }
    Ok(format!("{scheme}://{}", host.to_ascii_lowercase()))
}

/// `origin` + `path`, where the path stays on the origin: it starts with a
/// single `/` and has no control characters or backslashes.
pub fn join_origin(origin: &str, path: &str) -> Result<String, String> {
    if !path.starts_with('/')
        || path.starts_with("//")
        || path.contains('\\')
        || path.chars().any(|c| c.is_control() || c == ' ')
    {
        return Err(format!("`{path}` is not a path on {origin} (it must start with a single `/`)"));
    }
    Ok(format!("{origin}{path}"))
}

fn origin(a: &Args) -> Outcome {
    origin_of(a.str(0)?).map(Value::Str).map_err(|m| Error::new("io/invalid_input", m))
}

fn join(a: &Args) -> Outcome {
    join_origin(a.str(0)?, a.str(1)?).map(Value::Str).map_err(|m| Error::new("io/permission_denied", m))
}

/// Refuse a URL that does not start with a valid http(s) origin.
fn check_url(url: &str) -> Result<(), Error> {
    let authority_end =
        url.find("://").map(|k| k + 3).map(|k| k + url[k..].find(['/', '?', '#']).unwrap_or(url.len() - k));
    if authority_end.is_none_or(|end| origin_of(&url[..end]).is_err()) {
        return Err(Error::new("io/invalid_input", format!("`{url}` is not an http(s) URL")));
    }
    Ok(())
}

/// A client with a time limit that follows at most `redirects` redirects
/// (answering with the last response past that) and reports HTTP error
/// statuses as responses. Clients are kept per configuration, so requests
/// share their connection pool (and TLS setup) instead of each opening a
/// connection of its own; a few configurations are kept.
fn agent(timeout_ms: u64, redirects: u32) -> ureq::Agent {
    static AGENTS: Mutex<Vec<((u64, u32), ureq::Agent)>> = Mutex::new(Vec::new());
    const KEPT: usize = 32;
    let mut agents = AGENTS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, a)) = agents.iter().find(|(k, _)| *k == (timeout_ms, redirects)) {
        return a.clone();
    }
    let a: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_millis(timeout_ms)))
        .max_redirects(redirects)
        .max_redirects_will_error(false)
        .http_status_as_error(false)
        .build()
        .into();
    if agents.len() >= KEPT {
        agents.remove(0);
    }
    agents.push(((timeout_ms, redirects), a.clone()));
    a
}

/// A request failure as an `io.Error` naming the request.
fn failure(method: &str, url: &str, e: ureq::Error) -> Error {
    let kind = match &e {
        ureq::Error::Timeout(_) => "io/timed_out",
        ureq::Error::Io(_) | ureq::Error::ConnectionFailed | ureq::Error::HostNotFound => {
            "io/connection_failed"
        }
        ureq::Error::BodyExceedsLimit(_) => "io/too_large",
        _ => "io/other",
    };
    Error::new(kind, format!("{method} {url}: {e}"))
}

/// Response headers as `[name, value]` pairs (a value that is not valid
/// text is kept with replacement characters, not dropped).
fn header_pairs(h: &ureq::http::HeaderMap) -> Value {
    Value::List(
        h.iter()
            .map(|(k, v)| {
                Value::List(vec![
                    Value::str(k.as_str()),
                    Value::Str(String::from_utf8_lossy(v.as_bytes()).into_owned()),
                ])
            })
            .collect(),
    )
}

/// One request: `{status, headers, body}`.
fn request(a: &Args) -> Outcome {
    let (method, url, body, content_type) = (a.str(0)?, a.str(1)?, a.str(2)?, a.str(3)?);
    let timeout_ms = a.int(4)?.max(1) as u64;
    check_url(url)?;
    let agent = agent(timeout_ms, 0);
    let fail = |e| failure(method, url, e);
    let resp = match method {
        "GET" => agent.get(url).call(),
        "DELETE" => agent.delete(url).call(),
        "POST" => agent.post(url).header("content-type", content_type).send(body),
        "PUT" => agent.put(url).header("content-type", content_type).send(body),
        other => return Err(Error::new("io/invalid_input", format!("unsupported method {other}"))),
    };
    let mut resp = resp.map_err(fail)?;
    let status = resp.status().as_u16();
    let headers = header_pairs(resp.headers());
    let text = resp.body_mut().with_config().limit(16 << 20).read_to_string().map_err(fail)?;
    Ok(Value::Map(vec![
        ("status".into(), Value::Int(status as i64)),
        ("headers".into(), headers),
        ("body".into(), Value::Str(text)),
    ]))
}

/// Any request: method, URL, headers, a byte body, a time limit, how
/// many redirects to follow, and the largest body to read. The reply's
/// body is bytes, with the URL it finally came from.
fn send(a: &Args) -> Outcome {
    let (method, url) = (a.str(0)?, a.str(1)?);
    let headers: Vec<(String, String)> = match a.get(2) {
        Some(Value::Map(h)) => h
            .iter()
            .filter_map(|(k, v)| match v {
                Value::Str(v) => Some((k.clone(), v.clone())),
                _ => None,
            })
            .collect(),
        _ => vec![],
    };
    let body = a.bytes(3)?.to_vec();
    let (timeout_ms, redirects, max_body) =
        (a.int(4)?.max(1) as u64, a.int(5)?.clamp(0, 50) as u32, a.int(6)?.max(0) as u64);
    check_url(url)?;
    let agent = agent(timeout_ms, redirects);
    let fail = |e| failure(method, url, e);
    let mut req = ureq::http::Request::builder().method(method).uri(url);
    for (k, v) in &headers {
        req = req.header(k.as_str(), v.as_str());
    }
    let req = req.body(body).map_err(|e| Error::new("io/invalid_input", format!("{method} {url}: {e}")))?;
    let mut resp = agent.run(req).map_err(fail)?;
    use ureq::ResponseExt;
    let final_url = resp.get_uri().to_string();
    let status = resp.status().as_u16();
    let headers = header_pairs(resp.headers());
    let bytes = resp.body_mut().with_config().limit(max_body).read_to_vec().map_err(fail)?;
    Ok(Value::Map(vec![
        ("status".into(), Value::Int(status as i64)),
        ("headers".into(), headers),
        ("body".into(), Value::Bytes(bytes)),
        ("url".into(), Value::Str(final_url)),
    ]))
}

pub static DEF: ModuleDef = ModuleDef {
    name: "badger_fetch",
    functions: &[
        ("origin", Handler::Inline(origin)),
        ("join", Handler::Inline(join)),
        ("request", Handler::Blocking(request)),
        ("send", Handler::Blocking(send)),
    ],
    drop_handle: None,
    cancel: None,
    // a request is identified by its method and URL
    recorded: &[("request", "fetch", 2), ("send", "fetch", 2)],
    pure: &["origin", "join"],
};

badger_native::export!("badger_fetch", DEF);
