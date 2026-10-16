//! `lang_native_demo`: a native module covering the ABI for the tests:
//! every mode (inline, blocking, async with cancellation), the value
//! shapes, expected errors, defects (a panic, a result of the wrong
//! type), and handles released when unreachable.

use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use lang_native::{Args, CallId, Completion, Error, Handler, Handles, ModuleDef, Outcome, Value};

static CANCELS: AtomicI64 = AtomicI64::new(0);
static DROPS: AtomicI64 = AtomicI64::new(0);
static COUNTERS: Handles<AtomicI64> = Handles::new();

fn add(a: &Args) -> Outcome {
    a.int(0)?
        .checked_add(a.int(1)?)
        .map(Value::Int)
        .ok_or_else(|| Error::new("demo/overflow", "sum overflows"))
}

fn greet(a: &Args) -> Outcome {
    Ok(Value::str(format!("hello, {}!", a.str(0)?)))
}

fn echo(a: &Args) -> Outcome {
    Ok(a.get(0).cloned().unwrap_or(Value::Null))
}

fn point(a: &Args) -> Outcome {
    Ok(Value::Map(vec![("x".into(), Value::Int(a.int(0)?)), ("y".into(), Value::Int(a.int(1)?))]))
}

/// A struct argument arrives as a map by field name.
fn scale(a: &Args) -> Outcome {
    let p = a.get(0).ok_or_else(|| Error::new("demo/argument", "no point"))?;
    let k = a.int(1)?;
    let get = |n: &str| match p.field(n) {
        Some(Value::Int(v)) => Ok(*v * k),
        _ => Err(Error::new("demo/argument", format!("no field `{n}`"))),
    };
    Ok(Value::Map(vec![("x".into(), Value::Int(get("x")?)), ("y".into(), Value::Int(get("y")?))]))
}

fn divide(a: &Args) -> Outcome {
    match a.int(1)? {
        0 => Err(Error::new("demo/division_by_zero", "cannot divide by zero")),
        d => Ok(Value::Int(a.int(0)? / d)),
    }
}

fn boom(_: &Args) -> Outcome {
    panic!("the demo module exploded")
}

/// Declared `-> Int` by the tests, but returns text.
fn wrong_type(_: &Args) -> Outcome {
    Ok(Value::str("not a number"))
}

fn sum(a: &Args) -> Outcome {
    let mut t = 0i64;
    for v in a.list(0)? {
        if let Value::Int(i) = v {
            t += i;
        }
    }
    Ok(Value::Int(t))
}

fn reverse(a: &Args) -> Outcome {
    let mut b = a.bytes(0)?.to_vec();
    b.reverse();
    Ok(Value::Bytes(b))
}

fn maybe(a: &Args) -> Outcome {
    Ok(match a.int(0)? {
        0 => Value::Null,
        n => Value::str(format!("#{n}")),
    })
}

/// A shape enum: `circle(r)` or `square(side)`.
fn area(a: &Args) -> Outcome {
    match a.get(0) {
        Some(Value::Variant(n, xs)) => match (n.as_str(), xs.first()) {
            ("circle", Some(Value::Float(r))) => Ok(Value::Float(3.0 * r * r)),
            ("square", Some(Value::Float(s))) => Ok(Value::Float(s * s)),
            _ => Err(Error::new("demo/argument", format!("unknown shape {n}"))),
        },
        v => Err(Error::new("demo/argument", format!("not a shape: {v:?}"))),
    }
}

fn shapes(_: &Args) -> Outcome {
    Ok(Value::List(vec![
        Value::Variant("circle".into(), vec![Value::Float(1.0)]),
        Value::Variant("square".into(), vec![Value::Float(2.0)]),
    ]))
}

/// Complete after `ms` milliseconds on another thread, with `value`.
fn later(a: Args, done: Completion) {
    let (Ok(ms), Ok(v)) = (a.int(0), a.int(1)) else {
        done.complete(Err(Error::new("demo/argument", "later(ms, value)")));
        return;
    };
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(ms as u64));
        done.complete(Ok(Value::Int(v)));
    });
}

fn cancel(_: CallId) {
    CANCELS.fetch_add(1, Ordering::SeqCst);
}

fn cancels(_: &Args) -> Outcome {
    Ok(Value::Int(CANCELS.load(Ordering::SeqCst)))
}

fn counter(_: &Args) -> Outcome {
    Ok(COUNTERS.insert(AtomicI64::new(0)))
}

fn bump(a: &Args) -> Outcome {
    let c = COUNTERS.get(a.handle(0)?)?;
    Ok(Value::Int(c.fetch_add(1, Ordering::SeqCst) + 1))
}

fn drop_handle(h: u64) {
    if COUNTERS.remove(h).is_some() {
        DROPS.fetch_add(1, Ordering::SeqCst);
    }
}

fn drops(_: &Args) -> Outcome {
    Ok(Value::Int(DROPS.load(Ordering::SeqCst)))
}

pub static DEF: ModuleDef = ModuleDef {
    name: "lang_native_demo",
    functions: &[
        ("add", Handler::Inline(add)),
        ("greet", Handler::Inline(greet)),
        ("echo", Handler::Inline(echo)),
        ("point", Handler::Inline(point)),
        ("scale", Handler::Inline(scale)),
        ("divide", Handler::Inline(divide)),
        ("boom", Handler::Inline(boom)),
        ("wrong_type", Handler::Inline(wrong_type)),
        ("sum", Handler::Blocking(sum)),
        ("reverse", Handler::Inline(reverse)),
        ("maybe", Handler::Inline(maybe)),
        ("area", Handler::Inline(area)),
        ("shapes", Handler::Inline(shapes)),
        ("later", Handler::Async(later)),
        ("cancels", Handler::Inline(cancels)),
        ("counter", Handler::Inline(counter)),
        ("bump", Handler::Inline(bump)),
        ("drops", Handler::Inline(drops)),
    ],
    drop_handle: Some(drop_handle),
    cancel: Some(cancel),
    recorded: &[("later", "", 1)],
    pure: &[],
};

lang_native::export!("lang_native_demo", DEF);
