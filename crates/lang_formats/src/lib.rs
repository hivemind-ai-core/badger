//! `lang_formats`: TOML, YAML, and CSV for `std.toml`, `std.yaml`, and
//! `std.csv`. Documents parse to `Data` (TOML dates and times as their
//! text) and encode from it; CSV is rows of text. Every function is pure.

use lang_native::{Args, Error, Handler, ModuleDef, Outcome, Value};
use serde_json::Value as J;

fn to_value(j: J) -> Value {
    match j {
        J::Null => Value::Null,
        J::Bool(b) => Value::Bool(b),
        J::Number(n) => match n.as_i64() {
            Some(i) => Value::Int(i),
            None => Value::Float(n.as_f64().unwrap_or(0.0)),
        },
        J::String(s) => Value::Str(s),
        J::Array(xs) => Value::List(xs.into_iter().map(to_value).collect()),
        J::Object(m) => {
            // a TOML date or time, as serde sees it
            if m.len() == 1
                && let Some(J::String(s)) = m.get("$__toml_private_datetime")
            {
                return Value::Str(s.clone());
            }
            Value::Map(m.into_iter().map(|(k, v)| (k, to_value(v))).collect())
        }
    }
}

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

fn arg(a: &Args, i: usize) -> Result<&Value, Error> {
    a.get(i).ok_or_else(|| Error::new("format/argument", format!("missing argument {}", i + 1)))
}

fn toml_parse(a: &Args) -> Outcome {
    toml::from_str::<J>(a.str(0)?)
        .map(to_value)
        .map_err(|e| Error::new("format/toml", e.to_string().trim().to_string()))
}

fn toml_encode(a: &Args) -> Outcome {
    let j = to_json(arg(a, 0)?);
    if !j.is_object() {
        return Err(Error::new("format/toml", "a TOML document is a table (an object)"));
    }
    toml::to_string_pretty(&j).map(Value::Str).map_err(|e| Error::new("format/toml", e.to_string()))
}

fn yaml_parse(a: &Args) -> Outcome {
    serde_yaml_ng::from_str::<J>(a.str(0)?)
        .map(to_value)
        .map_err(|e| Error::new("format/yaml", e.to_string()))
}

fn yaml_encode(a: &Args) -> Outcome {
    serde_yaml_ng::to_string(&to_json(arg(a, 0)?))
        .map(Value::Str)
        .map_err(|e| Error::new("format/yaml", e.to_string()))
}

fn delimiter(a: &Args, i: usize) -> Result<u8, Error> {
    let d = a.str(i)?;
    match d.as_bytes() {
        [b] => Ok(*b),
        _ => Err(Error::new("format/csv", format!("the delimiter is one ASCII character, not `{d}`"))),
    }
}

/// Rows of fields (`csv_rows(text, delimiter)`).
fn csv_rows(a: &Args) -> Outcome {
    let mut r = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter(a, 1)?)
        .from_reader(a.str(0)?.as_bytes());
    let mut rows = vec![];
    for rec in r.records() {
        let rec = rec.map_err(|e| Error::new("format/csv", e.to_string()))?;
        rows.push(Value::List(rec.iter().map(Value::str).collect()));
    }
    Ok(Value::List(rows))
}

/// Records keyed by the header row (`csv_records(text, delimiter)`).
fn csv_records(a: &Args) -> Outcome {
    let mut r = csv::ReaderBuilder::new()
        .has_headers(true)
        .delimiter(delimiter(a, 1)?)
        .from_reader(a.str(0)?.as_bytes());
    let header: Vec<String> = r
        .headers()
        .map_err(|e| Error::new("format/csv", e.to_string()))?
        .iter()
        .map(str::to_string)
        .collect();
    let mut out = vec![];
    for rec in r.records() {
        let rec = rec.map_err(|e| Error::new("format/csv", e.to_string()))?;
        out.push(Value::Map(header.iter().cloned().zip(rec.iter().map(Value::str)).collect()));
    }
    Ok(Value::List(out))
}

/// Text of rows (`csv_encode(rows, delimiter)`), quoting fields as needed.
fn csv_encode(a: &Args) -> Outcome {
    let mut w = csv::WriterBuilder::new().flexible(true).delimiter(delimiter(a, 1)?).from_writer(vec![]);
    for row in a.list(0)? {
        let Value::List(fields) = row else {
            return Err(Error::new("format/csv", "a row is a list of text"));
        };
        let fields: Vec<String> = fields
            .iter()
            .map(|f| match f {
                Value::Str(s) => s.clone(),
                other => to_json(other).to_string(),
            })
            .collect();
        w.write_record(&fields).map_err(|e| Error::new("format/csv", e.to_string()))?;
    }
    let bytes = w.into_inner().map_err(|e| Error::new("format/csv", e.to_string()))?;
    Ok(Value::Str(String::from_utf8_lossy(&bytes).into_owned()))
}

/// CommonMark (with tables, strikethrough, task lists, and footnotes) as
/// HTML. `safe` escapes raw HTML in the input and drops link and image
/// URLs with a script scheme, for text from untrusted sources.
fn markdown_html(a: &Args) -> Outcome {
    use pulldown_cmark::{CowStr, Event, Options, Parser, Tag};
    let (text, safe) = (a.str(0)?, a.bool(1)?);
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    // browsers ignore whitespace and control characters in a scheme
    // (`java&#9;script:`), so compare without them
    let unsafe_url = |u: &str| {
        let plain: String = u.chars().filter(|c| !c.is_whitespace() && !c.is_control()).collect();
        let scheme = plain.split(':').next().unwrap_or("").to_ascii_lowercase();
        plain.contains(':') && matches!(scheme.as_str(), "javascript" | "vbscript" | "data")
    };
    let events = Parser::new_ext(text, options).map(|e| {
        if !safe {
            return e;
        }
        match e {
            Event::Html(h) | Event::InlineHtml(h) => Event::Text(h),
            Event::Start(Tag::Link { link_type, dest_url, title, id }) if unsafe_url(&dest_url) => {
                Event::Start(Tag::Link { link_type, dest_url: CowStr::Borrowed("#"), title, id })
            }
            Event::Start(Tag::Image { link_type, dest_url, title, id }) if unsafe_url(&dest_url) => {
                Event::Start(Tag::Image { link_type, dest_url: CowStr::Borrowed(""), title, id })
            }
            e => e,
        }
    });
    let mut out = String::new();
    pulldown_cmark::html::push_html(&mut out, events);
    Ok(Value::Str(out))
}

pub static DEF: ModuleDef = ModuleDef {
    name: "lang_formats",
    functions: &[
        ("toml_parse", Handler::Inline(toml_parse)),
        ("toml_encode", Handler::Inline(toml_encode)),
        ("yaml_parse", Handler::Inline(yaml_parse)),
        ("yaml_encode", Handler::Inline(yaml_encode)),
        ("csv_rows", Handler::Inline(csv_rows)),
        ("csv_records", Handler::Inline(csv_records)),
        ("csv_encode", Handler::Inline(csv_encode)),
        ("markdown_html", Handler::Inline(markdown_html)),
    ],
    drop_handle: None,
    cancel: None,
    recorded: &[],
    pure: &[
        "markdown_html",
        "toml_parse",
        "toml_encode",
        "yaml_parse",
        "yaml_encode",
        "csv_rows",
        "csv_records",
        "csv_encode",
    ],
};

lang_native::export!("lang_formats", DEF);
