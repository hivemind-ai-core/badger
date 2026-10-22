//! `badger_template`: Jinja templates (minijinja) for `stdx.template`. A
//! template renders a value (`Data`); HTML rendering escapes it. A set of
//! named templates is compiled once (to a handle) and may `include` and
//! `extends` its members.

use std::sync::Arc;

use badger_native::{Args, Error, Handler, Handles, ModuleDef, Outcome, Value};
use minijinja::{AutoEscape, Environment, UndefinedBehavior};
use serde_json::Value as J;

fn to_json(v: &Value) -> J {
    match v {
        Value::Null | Value::Unit => J::Null,
        Value::Bool(b) => J::Bool(*b),
        Value::Int(i) => J::from(*i),
        Value::Float(f) => serde_json::Number::from_f64(*f).map(J::Number).unwrap_or(J::Null),
        Value::Str(s) | Value::Symbol(s) => J::String(s.clone()),
        Value::Char(c) => J::String(c.to_string()),
        Value::Bytes(b) => J::String(String::from_utf8_lossy(b).into_owned()),
        Value::List(xs) => J::Array(xs.iter().map(to_json).collect()),
        Value::Map(fs) => J::Object(fs.iter().map(|(k, v)| (k.clone(), to_json(v))).collect()),
        Value::Variant(n, xs) => {
            J::Object([(n.clone(), J::Array(xs.iter().map(to_json).collect()))].into_iter().collect())
        }
        Value::Handle(h) => J::from(*h),
    }
}

fn error(e: minijinja::Error) -> Error {
    let mut msg = e.to_string();
    let mut src = std::error::Error::source(&e);
    while let Some(s) = src {
        msg.push_str(&format!(": {s}"));
        src = s.source();
    }
    Error::new("template/error", msg)
}

fn environment(html: bool) -> Environment<'static> {
    let mut env = Environment::new();
    // a misspelled variable is an error, not empty output
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env.set_auto_escape_callback(move |_| if html { AutoEscape::Html } else { AutoEscape::None });
    env
}

/// `render(text, context, html)`
fn render(a: &Args) -> Outcome {
    let env = environment(a.bool(2)?);
    let ctx = to_json(a.get(1).unwrap_or(&Value::Null));
    env.render_str(a.str(0)?, ctx).map(Value::Str).map_err(error)
}

static SETS: Handles<Environment<'static>> = Handles::new();

/// `compile(templates: Map<String, String>, html)`: a handle.
fn compile(a: &Args) -> Outcome {
    let mut env = environment(a.bool(1)?);
    let Some(Value::Map(ts)) = a.get(0) else {
        return Err(Error::new("template/argument", "templates by name"));
    };
    for (name, text) in ts {
        let Value::Str(text) = text else {
            return Err(Error::new("template/argument", "a template is text"));
        };
        env.add_template_owned(name.clone(), text.clone()).map_err(error)?;
    }
    Ok(SETS.insert(env))
}

/// `render_named(set, name, context)`
fn render_named(a: &Args) -> Outcome {
    let env: Arc<Environment<'static>> = SETS.get(a.handle(0)?)?;
    let t = env.get_template(a.str(1)?).map_err(error)?;
    t.render(to_json(a.get(2).unwrap_or(&Value::Null))).map(Value::Str).map_err(error)
}

fn drop_handle(h: u64) {
    SETS.remove(h);
}

pub static DEF: ModuleDef = ModuleDef {
    name: "badger_template",
    functions: &[
        ("render", Handler::Inline(render)),
        ("compile", Handler::Inline(compile)),
        ("render_named", Handler::Inline(render_named)),
    ],
    drop_handle: Some(drop_handle),
    cancel: None,
    recorded: &[],
    pure: &["render", "compile", "render_named"],
};

badger_native::export!("badger_template", DEF);
