//! Path, process, bytes, random, regex, and test-clock intrinsics.

use crate::desc::TypeDesc;
use crate::gc::Roots;
use crate::intrinsics::{Targs, rdesc};
use crate::intrinsics_io::{io_err, io_err_msg, io_ok};
use crate::string::{bytes_of, new_bytes, new_string, str_of};
use crate::task::TaskCtx;
use crate::{list, make_record, make_variant};

fn some(ret: &'static TypeDesc, w: u64) -> u64 {
    make_variant(ret, "some", &[w])
}

fn none(ret: &'static TypeDesc) -> u64 {
    make_variant(ret, "none", &[])
}

fn opt_string(ret: &'static TypeDesc, s: Option<String>) -> u64 {
    match s {
        Some(s) => {
            let w = new_string(&s);
            some(ret, w)
        }
        None => none(ret),
    }
}

fn strings(ret: &'static TypeDesc, v: Vec<String>) -> u64 {
    let r = Roots::new();
    let l = r.add(list::new(ret));
    for s in v {
        let w = r.add(new_string(&s));
        list::push(l, w);
    }
    l
}

/// Deterministic random source state (xorshift64*).
pub struct Rng(u64);

impl Rng {
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

static RNGS: std::sync::Mutex<Vec<Rng>> = std::sync::Mutex::new(Vec::new());

/// Manual (controlled) clock for tests: `None` = real monotonic clock.
pub static MANUAL_CLOCK: std::sync::Mutex<Option<i64>> = std::sync::Mutex::new(None);

macro_rules! os_intrinsics {
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

/// One HTTP request: (status, headers, body). Redirects are not followed.
fn http_fetch(
    method: &str,
    url: &str,
    body: &str,
    content_type: &str,
    timeout_ms: u64,
) -> Result<(u16, Vec<(String, String)>, String), (String, String)> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_millis(timeout_ms.max(1))))
        .max_redirects(0)
        .http_status_as_error(false)
        .build();
    let agent: ureq::Agent = config.into();
    let fail = |e: ureq::Error| {
        let kind = match &e {
            ureq::Error::Timeout(_) => "io/timed_out",
            ureq::Error::Io(_) | ureq::Error::ConnectionFailed | ureq::Error::HostNotFound => {
                "io/connection_failed"
            }
            _ => "io/other",
        };
        (kind.to_string(), format!("{method} {url}: {e}"))
    };
    let resp = match method {
        "GET" => agent.get(url).call(),
        "DELETE" => agent.delete(url).call(),
        "POST" => agent.post(url).header("content-type", content_type).send(body),
        "PUT" => agent.put(url).header("content-type", content_type).send(body),
        other => return Err(("io/invalid_input".into(), format!("unsupported method {other}"))),
    };
    let mut resp = resp.map_err(fail)?;
    let status = resp.status().as_u16();
    let headers = resp
        .headers()
        .iter()
        .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    let text = resp.body_mut().with_config().limit(16 << 20).read_to_string().map_err(fail)?;
    Ok((status, headers, text))
}

os_intrinsics! {
    fn fetch_origin(ctx, targs, ret, url) {
        match origin_of(str_of(url)) {
            Ok(o) => { let w = new_string(&o); io_ok(rdesc(ret), w) }
            Err(m) => io_err_msg(rdesc(ret), "io/invalid_input", &m),
        }
    }
    fn fetch_join(ctx, targs, ret, origin, path) {
        match join_origin(str_of(origin), str_of(path)) {
            Ok(u) => { let w = new_string(&u); io_ok(rdesc(ret), w) }
            Err(m) => io_err_msg(rdesc(ret), "io/permission_denied", &m),
        }
    }
    fn fetch_request(ctx, targs, ret, method, url, body, content_type, timeout_ms) {
        let (m, u, b, c) = (str_of(method).to_string(), str_of(url).to_string(), str_of(body).to_string(), str_of(content_type).to_string());
        // the URL must start with a valid http(s) origin
        let authority_end = u.find("://").map(|k| k + 3).map(|k| k + u[k..].find(['/', '?', '#']).unwrap_or(u.len() - k));
        if authority_end.is_none_or(|end| origin_of(&u[..end]).is_err()) {
            return io_err_msg(rdesc(ret), "io/invalid_input", &format!("`{u}` is not an http(s) URL"));
        }
        let r = crate::task::run_blocking(move || http_fetch(&m, &u, &b, &c, timeout_ms));
        match r {
            Ok((status, headers, text)) => {
                let g = Roots::new();
                let rec = rdesc(ret).variant(0).unwrap().fields[0].desc;
                let list_desc = rec.field("headers").map(|f| f.desc).unwrap();
                let pair = match &list_desc.kind { crate::desc::Kind::List { elem } => *elem, _ => list_desc };
                let hl = g.add(list::new(list_desc));
                for (k, v) in headers {
                    let kw = g.add(new_string(&k));
                    let vw = g.add(new_string(&v));
                    let p = g.add(make_record(pair, &[("0", kw), ("1", vw)]));
                    list::push(hl, p);
                }
                let bw = g.add(new_string(&text));
                let v = g.add(make_record(rec, &[("status", status as u64), ("headers", hl), ("body", bw)]));
                io_ok(rdesc(ret), v)
            }
            Err((k, msg)) => io_err_msg(rdesc(ret), &k, &msg),
        }
    }
    // ---------------------------------------------------------------- path
    fn path_join(ctx, targs, ret, a, b) {
        new_string(&std::path::Path::new(str_of(a)).join(str_of(b)).to_string_lossy())
    }
    fn path_parent(ctx, targs, ret, p) {
        opt_string(rdesc(ret), std::path::Path::new(str_of(p)).parent().map(|x| x.to_string_lossy().into_owned()))
    }
    fn path_file_name(ctx, targs, ret, p) {
        opt_string(rdesc(ret), std::path::Path::new(str_of(p)).file_name().map(|x| x.to_string_lossy().into_owned()))
    }
    fn path_extension(ctx, targs, ret, p) {
        opt_string(rdesc(ret), std::path::Path::new(str_of(p)).extension().map(|x| x.to_string_lossy().into_owned()))
    }
    fn path_components(ctx, targs, ret, p) {
        let v = std::path::Path::new(str_of(p)).components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        strings(rdesc(ret), v)
    }
    fn fs_list_dir(ctx, targs, ret, p) {
        // Result<List<String>, io.Error>
        let path = str_of(p).to_string();
        let r = crate::task::run_blocking(move || {
            std::fs::read_dir(&path).map(|it| {
                let mut v: Vec<String> = it.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
                v.sort();
                v
            })
        });
        match r {
            Ok(v) => {
                let g = Roots::new();
                let list_desc = rdesc(ret).variant(0).unwrap().fields[0].desc;
                let l = g.add(strings(list_desc, v));
                io_ok(rdesc(ret), l)
            }
            Err(e) => io_err(rdesc(ret), &e),
        }
    }

    // ---------------------------------------------------------------- process
    // Run a subprocess to completion on the blocking pool:
    // Result<{status, stdout, stderr}, io.Error>
    fn process_run(ctx, targs, ret, program, args) {
        let prog = str_of(program).to_string();
        let argv: Vec<String> = list::to_vec(args).into_iter().map(|w| str_of(w).to_string()).collect();
        let r = crate::task::run_blocking(move || std::process::Command::new(&prog).args(&argv).output());
        match r {
            Ok(out) => {
                let g = Roots::new();
                let rec = rdesc(ret).variant(0).unwrap().fields[0].desc;
                let so = g.add(new_string(&String::from_utf8_lossy(&out.stdout)));
                let se = g.add(new_string(&String::from_utf8_lossy(&out.stderr)));
                let code = out.status.code().unwrap_or(-1) as i64 as u64;
                let v = g.add(make_record(rec, &[("status", code), ("stdout", so), ("stderr", se)]));
                io_ok(rdesc(ret), v)
            }
            Err(e) => io_err(rdesc(ret), &e),
        }
    }
    fn process_exit(ctx, targs, ret, code) {
        use std::io::Write;
        if EXIT_FORBIDDEN.load(std::sync::atomic::Ordering::Relaxed) {
            crate::task::panic_current(
                "process.exit is not available here (the process is a shared session)".into(),
                crate::task::caller_location(),
            );
        }
        let _ = std::io::stdout().flush();
        run_exit_hook();
        crate::profile::finish();
        cleanup_at_exit();
        unsafe { libc::_exit(code as i32) }
    }
    fn process_id(ctx, targs, ret) { std::process::id() as u64 }

    // ---------------------------------------------------------------- bytes
    fn bytes_len(ctx, targs, ret, b) { bytes_of(b).len() as u64 }
    fn bytes_slice(ctx, targs, ret, b, s, e) {
        let bs = bytes_of(b);
        let (s, e) = (s as usize, e as usize);
        if s > e || e > bs.len() { none(rdesc(ret)) } else { let w = new_bytes(&bs[s..e]); some(rdesc(ret), w) }
    }
    fn bytes_concat(ctx, targs, ret, a, b) {
        let mut v = bytes_of(a).to_vec();
        v.extend_from_slice(bytes_of(b));
        new_bytes(&v)
    }
    fn bytes_to_list(ctx, targs, ret, b) {
        let g = Roots::new();
        g.add(b);
        let l = g.add(list::new(rdesc(ret)));
        for x in bytes_of(b).to_vec() {
            list::push(l, x as u64);
        }
        l
    }
    fn bytes_hex(ctx, targs, ret, b) {
        new_string(&bytes_of(b).iter().map(|x| format!("{x:02x}")).collect::<String>())
    }

    // ---------------------------------------------------------------- random
    fn random_seeded(ctx, targs, ret, seed) {
        let mut g = RNGS.lock().unwrap();
        g.push(Rng(seed.max(1)));
        (g.len() - 1) as u64
    }
    fn random_os(ctx, targs, ret) {
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(7) ^ (std::process::id() as u64) << 32;
        let mut g = RNGS.lock().unwrap();
        g.push(Rng(seed | 1));
        (g.len() - 1) as u64
    }
    fn random_next(ctx, targs, ret, id) {
        RNGS.lock().unwrap()[id as usize].next()
    }

    // ---------------------------------------------------------------- regex
    fn regex_is_match(ctx, targs, ret, pat, s) {
        match regex::Regex::new(str_of(pat)) { Ok(r) => r.is_match(str_of(s)) as u64, Err(_) => 0 }
    }
    // Result<List<String>, String>: capture groups of the first match
    fn regex_captures(ctx, targs, ret, pat, s) {
        match regex::Regex::new(str_of(pat)) {
            Ok(r) => {
                let g = Roots::new();
                let list_desc = rdesc(ret).variant(0).unwrap().fields[0].desc;
                let caps: Vec<String> = match r.captures(str_of(s)) {
                    Some(c) => c.iter().map(|m| m.map(|m| m.as_str().to_string()).unwrap_or_default()).collect(),
                    None => vec![],
                };
                let l = g.add(strings(list_desc, caps));
                make_variant(rdesc(ret), "ok", &[l])
            }
            Err(e) => { let m = new_string(&e.to_string()); make_variant(rdesc(ret), "err", &[m]) }
        }
    }
    fn regex_replace_all(ctx, targs, ret, pat, s, rep) {
        match regex::Regex::new(str_of(pat)) {
            Ok(r) => new_string(&r.replace_all(str_of(s), str_of(rep))),
            Err(_) => s,
        }
    }
    fn regex_find_all(ctx, targs, ret, pat, s) {
        let v = match regex::Regex::new(str_of(pat)) {
            Ok(r) => r.find_iter(str_of(s)).map(|m| m.as_str().to_string()).collect(),
            Err(_) => vec![],
        };
        strings(rdesc(ret), v)
    }

    // ---------------------------------------------------------------- test clock
    fn clock_manual(ctx, targs, ret, start) { *MANUAL_CLOCK.lock().unwrap() = Some(start as i64); 0 }
    fn clock_advance(ctx, targs, ret, d) {
        let now = {
            let mut g = MANUAL_CLOCK.lock().unwrap();
            match g.as_mut() {
                Some(t) => {
                    *t += d as i64;
                    *t
                }
                None => return 0,
            }
        };
        crate::task::clock_advanced(now);
        0
    }
    fn clock_real(ctx, targs, ret) {
        *MANUAL_CLOCK.lock().unwrap() = None;
        // pending controlled sleeps end
        crate::task::clock_advanced(i64::MAX);
        0
    }
    // controlled scheduling
    fn sched_seed(ctx, targs, ret, seed) {
        let mut c = crate::task::SCHED.lock().unwrap();
        c.seed = Some(seed);
        c.replay.clear();
        0
    }
    fn sched_record(ctx, targs, ret) {
        let mut c = crate::task::SCHED.lock().unwrap();
        c.record = true;
        c.decisions.clear();
        0
    }
    fn sched_recorded(ctx, targs, ret) {
        let ds = crate::task::SCHED.lock().unwrap().decisions.clone();
        let g = crate::gc::Roots::new();
        let l = g.add(crate::list::new(rdesc(ret)));
        for d in ds {
            crate::list::push(l, d as u64);
        }
        l
    }
    fn sched_replay(ctx, targs, ret, decisions) {
        let ds: Vec<u32> = crate::list::to_vec(decisions).into_iter().map(|w| w as u32).collect();
        let mut c = crate::task::SCHED.lock().unwrap();
        c.seed = None;
        c.replay = ds.into_iter().collect();
        0
    }
    fn sched_reset(ctx, targs, ret) {
        let mut c = crate::task::SCHED.lock().unwrap();
        c.seed = None;
        c.record = false;
        c.replay.clear();
        0
    }
    fn test_fail(ctx, targs, ret, msg) {
        let loc = crate::task::current().map(|t| t.caller_loc).filter(|l| *l != 0).map(|l| str_of(l).to_string()).unwrap_or_default();
        crate::task::panic_current(str_of(msg).to_string(), loc)
    }
}

#[allow(dead_code)]
fn unused(ret: &'static TypeDesc) -> u64 {
    io_err_msg(ret, "x", "y")
}

/// A tool hook run when the program ends itself with `process.exit` (for
/// example `lang run --bench` reporting the run time).
static EXIT_HOOK: std::sync::Mutex<Option<Box<dyn Fn() + Send>>> = std::sync::Mutex::new(None);

pub fn set_exit_hook(f: Box<dyn Fn() + Send>) {
    *EXIT_HOOK.lock().unwrap() = Some(f);
}

static EXIT_FORBIDDEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Make `process.exit` a task defect (a session that serves others, such
/// as `lang mcp`, outlives the code it runs).
pub fn forbid_exit() {
    EXIT_FORBIDDEN.store(true, std::sync::atomic::Ordering::Relaxed);
}

static REMOVE_AT_EXIT: std::sync::Mutex<Vec<std::path::PathBuf>> = std::sync::Mutex::new(Vec::new());

/// Remove `dir` (a private temporary directory) when the process exits.
pub fn remove_at_exit(dir: std::path::PathBuf) {
    REMOVE_AT_EXIT.lock().unwrap().push(dir);
}

/// Remove the directories registered with [`remove_at_exit`].
pub fn cleanup_at_exit() {
    for d in std::mem::take(&mut *REMOVE_AT_EXIT.lock().unwrap()) {
        let _ = std::fs::remove_dir_all(d);
    }
}

fn run_exit_hook() {
    if let Some(f) = EXIT_HOOK.lock().unwrap().as_ref() {
        f();
    }
}
