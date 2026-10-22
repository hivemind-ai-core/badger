//! `badger generate sql`: typed functions from SQL files (SQL stays SQL, in
//! the style of sqlc and pgtyped). A file holds named queries:
//!
//! ```sql
//! -- name: find_user :one
//! -- Find a user by id.
//! SELECT id, name, email FROM users WHERE id = :id;
//! ```
//!
//! `:one` returns exactly one row, `:opt` an optional row, `:many` a list
//! of rows, `:exec` the changed row count. Parameters are written `:name`
//! (`:name?` for one that may be NULL); the same name used twice is one
//! parameter. Each query becomes a function taking a connection (a pool
//! or a transaction) and its parameters, returning rows as a record type
//! (`FindUserRow`). Types come from the database: Postgres describes each
//! statement (parameter and column types, and NOT NULL for columns read
//! from tables); SQLite prepares it against the schema (column types and
//! nullability; parameters are `Data` unless given). A `-- params: id:
//! Int, name: String` line sets parameter types. An expression column
//! (whose nullability is unknown) is an `Option` unless its alias ends in
//! `!` (`count(*) AS "total!"`); `?` makes any column an `Option`.

use std::path::Path;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    One,
    Opt,
    Many,
    Exec,
}

#[derive(Debug)]
pub struct Query {
    pub name: String,
    pub kind: Kind,
    pub doc: Vec<String>,
    /// The SQL as written (named parameters).
    pub written: String,
    /// `-- params:` types by name.
    pub hints: Vec<(String, String)>,
    /// 1-based line of the `-- name:` line.
    pub line: usize,
}

/// Where types come from.
pub enum Backend<'a> {
    Postgres { url: &'a str },
    Sqlite { schema: Option<&'a Path> },
}

/// A parameter after rewriting: its name and whether it may be NULL.
#[derive(Debug, PartialEq)]
pub struct Param {
    pub name: String,
    pub nullable: bool,
}

/// Column overrides from aliases: `"total!"` (never NULL), `"note?"`
/// (may be NULL), `"total!: Int"` (and of this type).
#[derive(Debug, PartialEq)]
pub enum Mark {
    NotNull,
    Nullable,
    Unmarked,
}

#[derive(Debug, PartialEq)]
pub struct Override {
    pub column: String,
    pub mark: Mark,
    pub ty: Option<String>,
}

/// An alias's override, if it has a mark or a type.
fn alias_override(inner: &str) -> Option<Override> {
    // language types start with a capital (`Int`, `Option<String>`)
    let (head, ty) = match inner.split_once(':') {
        Some((h, t)) if t.trim().starts_with(|c: char| c.is_ascii_uppercase()) => {
            (h.trim_end(), Some(t.trim().to_string()))
        }
        _ => (inner, None),
    };
    let (column, mark) = if let Some(n) = head.strip_suffix('!') {
        (n, Mark::NotNull)
    } else if let Some(n) = head.strip_suffix('?') {
        (n, Mark::Nullable)
    } else {
        (head, Mark::Unmarked)
    };
    let plain = !column.is_empty() && column.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    (plain && (mark != Mark::Unmarked || ty.is_some())).then(|| Override {
        column: column.to_string(),
        mark,
        ty,
    })
}

/// The queries of a SQL file.
pub fn parse(text: &str) -> Result<Vec<Query>, String> {
    let mut out: Vec<Query> = vec![];
    let mut in_header = false;
    for (i, line) in text.lines().enumerate() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("--").map(str::trim).and_then(|r| r.strip_prefix("name:")) {
            let mut words = rest.split_whitespace();
            let name = words.next().ok_or(format!("line {}: a query needs a name", i + 1))?;
            let kind = match words.next() {
                Some(":one") => Kind::One,
                Some(":opt") => Kind::Opt,
                Some(":many") => Kind::Many,
                Some(":exec") => Kind::Exec,
                Some(other) => {
                    return Err(format!(
                        "line {}: unknown result `{other}` (use :one, :opt, :many, or :exec)",
                        i + 1
                    ));
                }
                None => {
                    return Err(format!(
                        "line {}: say what `{name}` returns (:one, :opt, :many, or :exec)",
                        i + 1
                    ));
                }
            };
            let name = snake(name);
            if !valid_ident(&name) {
                return Err(format!("line {}: `{name}` is not a function name", i + 1));
            }
            if out.iter().any(|q| q.name == name) {
                return Err(format!("line {}: a second query named `{name}`", i + 1));
            }
            out.push(Query { name, kind, doc: vec![], written: String::new(), hints: vec![], line: i + 1 });
            in_header = true;
            continue;
        }
        let Some(q) = out.last_mut() else { continue };
        if in_header && let Some(c) = t.strip_prefix("--") {
            let c = c.strip_prefix(' ').unwrap_or(c);
            if let Some(ps) = c.trim().strip_prefix("params:") {
                for p in split_top(ps) {
                    let (n, ty) = p
                        .split_once(':')
                        .ok_or(format!("line {}: write parameter types as `name: Type`", i + 1))?;
                    q.hints.push((n.trim().to_string(), ty.trim().to_string()));
                }
            } else {
                q.doc.push(c.trim_end().to_string());
            }
            continue;
        }
        if t.is_empty() && q.written.is_empty() {
            continue;
        }
        in_header = false;
        q.written.push_str(line);
        q.written.push('\n');
    }
    for q in &mut out {
        let s = q.written.trim().trim_end_matches(';').trim_end().to_string();
        if s.is_empty() {
            return Err(format!("line {}: query `{}` has no SQL", q.line, q.name));
        }
        q.written = s;
    }
    Ok(out)
}

/// Split at commas outside `<>` (`a: Map<String, Int>, b: Int`).
fn split_top(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut depth) = (vec![], String::new(), 0);
    for c in s.chars() {
        match c {
            '<' | '(' | '{' => depth += 1,
            '>' | ')' | '}' => depth -= 1,
            ',' if depth == 0 => {
                out.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    out.push(cur);
    out.into_iter().map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect()
}

fn valid_ident(s: &str) -> bool {
    let mut cs = s.chars();
    cs.next().is_some_and(|c| c.is_ascii_lowercase() || c == '_')
        && cs.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !badger_syntax::RESERVED.contains(&s)
        // the generated functions' connection parameter
        && s != "db"
}

/// `FindUser` → `find_user`.
fn snake(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 && !out.ends_with('_') {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// `find_user` → `FindUser`.
fn pascal(s: &str) -> String {
    s.split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut cs = w.chars();
            cs.next().map(|c| c.to_ascii_uppercase().to_string() + cs.as_str()).unwrap_or_default()
        })
        .collect()
}

/// Rewrite `:name` parameters to positional ones (`$1` for Postgres, `?1`
/// for SQLite) and strip `!`/`?` alias marks, outside strings, quoted
/// identifiers (but for the marks), comments, and `$tag$` bodies.
pub fn rewrite(sql: &str, postgres: bool) -> (String, Vec<Param>, Vec<Override>) {
    let cs: Vec<char> = sql.chars().collect();
    let (mut out, mut params, mut marks) = (String::new(), Vec::<Param>::new(), vec![]);
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        match c {
            '\'' => {
                let j = skip_quoted(&cs, i, '\'');
                out.extend(&cs[i..j]);
                i = j;
            }
            '"' => {
                let j = skip_quoted(&cs, i, '"');
                let inner: String = cs[i + 1..j.saturating_sub(1).max(i + 1)].iter().collect();
                // only an alias (`AS "name!"`) carries an override
                let before = out.trim_end();
                let after_as = out.len() > before.len()
                    && before.len() >= 2
                    && before.as_bytes()[before.len() - 2..].eq_ignore_ascii_case(b"as")
                    && before[..before.len() - 2].ends_with(|c: char| c.is_whitespace() || c == ')');
                match alias_override(&inner).filter(|_| after_as) {
                    Some(o) => {
                        out.push('"');
                        out.push_str(&o.column);
                        out.push('"');
                        marks.push(o);
                    }
                    None => out.extend(&cs[i..j]),
                }
                i = j;
            }
            '-' if cs.get(i + 1) == Some(&'-') => {
                while i < cs.len() && cs[i] != '\n' {
                    out.push(cs[i]);
                    i += 1;
                }
            }
            '/' if cs.get(i + 1) == Some(&'*') => {
                let start = i;
                i += 2;
                while i + 1 < cs.len() && !(cs[i] == '*' && cs[i + 1] == '/') {
                    i += 1;
                }
                i = (i + 2).min(cs.len());
                out.extend(&cs[start..i]);
            }
            '$' if postgres => {
                // a dollar-quoted body: $$ ... $$ or $tag$ ... $tag$
                let mut j = i + 1;
                while j < cs.len() && (cs[j].is_alphanumeric() || cs[j] == '_') {
                    j += 1;
                }
                if j < cs.len() && cs[j] == '$' && !cs[i + 1..j].first().is_some_and(|c| c.is_ascii_digit()) {
                    let tag: String = cs[i..=j].iter().collect();
                    let rest: String = cs[j + 1..].iter().collect();
                    let end = rest
                        .find(&tag)
                        .map_or(cs.len(), |k| j + 1 + rest[..k].chars().count() + tag.chars().count());
                    out.extend(&cs[i..end]);
                    i = end;
                } else {
                    out.push(c);
                    i += 1;
                }
            }
            ':' if cs.get(i + 1) == Some(&':') => {
                out.push_str("::");
                i += 2;
            }
            ':' if cs.get(i + 1).is_some_and(|c| c.is_ascii_alphabetic() || *c == '_')
                && !(i > 0 && (cs[i - 1].is_alphanumeric() || cs[i - 1] == '_')) =>
            {
                let mut j = i + 1;
                while j < cs.len() && (cs[j].is_ascii_alphanumeric() || cs[j] == '_') {
                    j += 1;
                }
                let name: String = cs[i + 1..j].iter().collect();
                let nullable = cs.get(j) == Some(&'?');
                if nullable {
                    j += 1;
                }
                let n = match params.iter().position(|p| p.name == name) {
                    Some(k) => {
                        params[k].nullable |= nullable;
                        k + 1
                    }
                    None => {
                        params.push(Param { name, nullable });
                        params.len()
                    }
                };
                out.push_str(&format!("{}{n}", if postgres { '$' } else { '?' }));
                i = j;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    (out, params, marks)
}

/// The index after the quoted run starting at `i` (a doubled quote
/// continues it).
fn skip_quoted(cs: &[char], i: usize, q: char) -> usize {
    let mut j = i + 1;
    while j < cs.len() {
        if cs[j] == q {
            if cs.get(j + 1) == Some(&q) {
                j += 2;
                continue;
            }
            return j + 1;
        }
        j += 1;
    }
    cs.len()
}

/// A Postgres type as a language type, or why it has none.
fn pg_type(t: &str) -> Result<String, String> {
    if let Some(e) = t.strip_suffix("[]") {
        return Ok(format!("List<{}>", pg_type(e)?));
    }
    if t.starts_with("enum ") {
        return Ok("String".into());
    }
    Ok(match t {
        "int2" | "int4" | "int8" | "oid" => "Int",
        "float4" | "float8" => "Float",
        "bool" => "Bool",
        "json" | "jsonb" => "Data",
        "numeric" | "text" | "varchar" | "bpchar" | "name" | "citext" | "char" | "uuid" | "date" | "time"
        | "timestamp" | "timestamptz" | "bytea" | "xml" | "unknown" => "String",
        other => return Err(format!("type {other} is not supported; cast it (`::text`)")),
    }
    .to_string())
}

/// A SQLite declared column type as a language type.
fn sqlite_type(decl: Option<&str>) -> String {
    let Some(d) = decl else { return "Data".into() };
    let d = d.to_ascii_uppercase();
    if d.contains("INT") {
        "Int"
    } else if d.contains("CHAR") || d.contains("CLOB") || d.contains("TEXT") {
        "String"
    } else if d.contains("REAL") || d.contains("FLOA") || d.contains("DOUB") {
        "Float"
    } else {
        "Data"
    }
    .into()
}

struct Column {
    name: String,
    ty: String,
}

struct Typed {
    sql: String,
    params: Vec<(String, String)>,
    columns: Vec<Column>,
}

fn optional(ty: String, nullable: Option<bool>, mark: Option<&Mark>) -> String {
    let null = match mark {
        Some(Mark::NotNull) => false,
        Some(Mark::Nullable) => true,
        Some(Mark::Unmarked) | None => nullable.unwrap_or(true),
    };
    // `Data` holds NULL itself
    if null && ty != "Data" { format!("Option<{ty}>") } else { ty }
}

fn param_type(q: &Query, p: &Param, inferred: Option<String>) -> Result<String, String> {
    if let Some((_, t)) = q.hints.iter().find(|(n, _)| *n == p.name) {
        return Ok(t.clone());
    }
    let t = inferred.unwrap_or_else(|| "Data".into());
    Ok(if p.nullable && t != "Data" { format!("Option<{t}>") } else { t })
}

fn check_names(q: &Query, params: &[Param]) -> Result<(), String> {
    for p in params {
        if !valid_ident(&p.name) {
            return Err(format!("parameter `:{}` cannot be a function parameter name; rename it", p.name));
        }
    }
    for (n, _) in &q.hints {
        if !params.iter().any(|p| p.name == *n) {
            return Err(format!("`-- params:` names `{n}`, which the query does not use"));
        }
    }
    Ok(())
}

fn type_queries(queries: &[Query], backend: &Backend) -> Result<Vec<Typed>, Vec<String>> {
    let postgres = matches!(backend, Backend::Postgres { .. });
    let rewritten: Vec<_> = queries.iter().map(|q| rewrite(&q.written, postgres)).collect();
    let mut errors = vec![];
    let mut out = vec![];
    match backend {
        Backend::Postgres { url } => {
            let sqls: Vec<String> = rewritten.iter().map(|(s, _, _)| s.clone()).collect();
            let described =
                badger_postgres::describe_statements(url, &sqls).map_err(|e| vec![format!("{url}: {e}")])?;
            for ((q, (sql, params, marks)), d) in queries.iter().zip(rewritten).zip(described) {
                let r = (|| -> Result<Typed, String> {
                    check_names(q, &params)?;
                    let d = d?;
                    let ps = params
                        .iter()
                        .zip(&d.params)
                        .map(|(p, t)| {
                            Ok((
                                p.name.clone(),
                                param_type(
                                    q,
                                    p,
                                    Some(pg_type(t).map_err(|e| format!("parameter `{}`: {e}", p.name))?),
                                )?,
                            ))
                        })
                        .collect::<Result<Vec<_>, String>>()?;
                    let mut columns = vec![];
                    for c in &d.columns {
                        let o = marks.iter().find(|o| o.column == c.name);
                        let ty = match o.and_then(|o| o.ty.clone()) {
                            Some(t) => t,
                            None => pg_type(&c.sql_type).map_err(|e| format!("column `{}`: {e}", c.name))?,
                        };
                        columns.push(Column {
                            name: c.name.clone(),
                            ty: optional(ty, c.nullable, o.map(|o| &o.mark)),
                        });
                    }
                    Ok(Typed { sql, params: ps, columns })
                })();
                match r {
                    Ok(t) => out.push(t),
                    Err(e) => errors.push(format!("line {}: `{}`: {e}", q.line, q.name)),
                }
            }
        }
        Backend::Sqlite { schema } => {
            let db = crate::sqlcheck::Db::new().map_err(|e| vec![e])?;
            if let Some(path) = schema {
                let text =
                    std::fs::read_to_string(path).map_err(|e| vec![format!("{}: {e}", path.display())])?;
                db.run(&text)
                    .map_err(|e| vec![format!("the schema {} does not load: {e}", path.display())])?;
            }
            for (q, (sql, params, marks)) in queries.iter().zip(rewritten) {
                let r = (|| -> Result<Typed, String> {
                    check_names(q, &params)?;
                    let p = db.prepare(&sql)?;
                    let ps = params
                        .iter()
                        .map(|p| Ok((p.name.clone(), param_type(q, p, None)?)))
                        .collect::<Result<Vec<_>, String>>()?;
                    let columns = p
                        .columns
                        .iter()
                        .map(|c| {
                            let o = marks.iter().find(|o| o.column == c.name);
                            let ty = o
                                .and_then(|o| o.ty.clone())
                                .unwrap_or_else(|| sqlite_type(c.decl.as_deref()));
                            Column { name: c.name.clone(), ty: optional(ty, c.nullable, o.map(|o| &o.mark)) }
                        })
                        .collect();
                    Ok(Typed { sql, params: ps, columns })
                })();
                match r {
                    Ok(t) => out.push(t),
                    Err(e) => errors.push(format!("line {}: `{}`: {e}", q.line, q.name)),
                }
            }
        }
    }
    for (q, t) in queries.iter().zip(&out) {
        if q.kind != Kind::Exec && t.columns.is_empty() {
            errors.push(format!("line {}: `{}` returns no rows; use :exec", q.line, q.name));
        }
        if let Some(c) = t.columns.iter().find(|c| !valid_ident(&c.name)) {
            errors.push(format!(
                "line {}: `{}`: column `{}` cannot be a field name; give it an alias (`AS some_name`)",
                q.line, q.name, c.name
            ));
        }
    }
    if errors.is_empty() { Ok(out) } else { Err(errors) }
}

fn string_literal(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => {}
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The generated module for the queries of `source` (a file name shown
/// in the header), or the problems found.
pub fn generate(source: &str, text: &str, backend: &Backend) -> Result<String, Vec<String>> {
    let queries = parse(text).map_err(|e| vec![e])?;
    let typed = type_queries(&queries, backend)?;
    let (module, db_name) = match backend {
        Backend::Postgres { .. } => ("postgres", "Postgres"),
        Backend::Sqlite { .. } => ("sqlite", "SQLite"),
    };
    let mut out = format!(
        "// Generated by `badger generate sql` from {source} ({db_name}); do not edit.\n// Regenerate after changing the queries or the schema.\n\nimport std.codec\nimport {}.{module}\n",
        crate::std_root_of(module).unwrap_or(badger_stdlib::STD)
    );
    for (q, t) in queries.iter().zip(&typed) {
        let row = format!("{}Row", pascal(&q.name));
        out.push('\n');
        if q.kind != Kind::Exec {
            let fields: Vec<String> = t.columns.iter().map(|c| format!("{}: {}", c.name, c.ty)).collect();
            out.push_str(&format!(
                "/// A row of `{}`.\npub type {row} = {{ {} }}\n\n",
                q.name,
                fields.join(", ")
            ));
        }
        for d in &q.doc {
            out.push_str(&format!("///{}{d}\n", if d.is_empty() { "" } else { " " }));
        }
        if !q.doc.is_empty() {
            out.push_str("///\n");
        }
        out.push_str("/// ```sql\n");
        for line in q.written.lines() {
            out.push_str(&format!("/// {line}\n"));
        }
        out.push_str("/// ```\n");
        // the signature lists `-- params:` in their order, then the rest as
        // they first appear; the arguments follow the placeholders
        let mut order: Vec<&(String, String)> =
            q.hints.iter().filter_map(|(h, _)| t.params.iter().find(|(n, _)| n == h)).collect();
        order.extend(t.params.iter().filter(|(n, _)| !q.hints.iter().any(|(h, _)| h == n)));
        let params: String = order.iter().map(|(n, ty)| format!(", {n}: {ty}")).collect();
        let args: Vec<String> = t.params.iter().map(|(n, _)| format!("codec.to_data({n})")).collect();
        let args = format!("[{}]", args.join(", "));
        let ret = match q.kind {
            Kind::One => row.clone(),
            Kind::Opt => format!("Option<{row}>"),
            Kind::Many => format!("List<{row}>"),
            Kind::Exec => "Int".into(),
        };
        out.push_str(&format!(
            "pub fn {}<C>(db: C{params}) -> Result<{ret}, {module}.Error> !{{Io, Wait}} where C: {module}.Connection {{\n",
            q.name
        ));
        let sql = string_literal(&t.sql);
        match q.kind {
            Kind::One => out.push_str(&format!("    {module}.query_one_as<{row}, C>(db, {sql}, {args})\n")),
            Kind::Many => out.push_str(&format!("    {module}.query_as<{row}, C>(db, {sql}, {args})\n")),
            Kind::Exec => out.push_str(&format!("    {module}.execute(db, {sql}, {args})\n")),
            Kind::Opt => out.push_str(&format!(
                "    let rows = {module}.query_as<{row}, C>(db, {sql}, {args})?\n    if rows.len() > 1 {{\n        return err({{ code: #{module}/row_count, message: $\"expected at most one row, got {{rows.len()}}\", cause: Option.none }})\n    }}\n    ok(rows.first())\n"
            )),
        }
        out.push_str("}\n");
    }
    Ok(out)
}

/// The `[database] schema` of the project a file belongs to (the nearest
/// `badger.toml` above it).
pub fn project_schema(file: &Path) -> Option<std::path::PathBuf> {
    let file = std::path::absolute(file).ok()?;
    let root = file.ancestors().skip(1).find(|d| d.join("badger.toml").is_file())?;
    let m = crate::manifest::Manifest::in_dir(root).ok()?;
    m.schema.map(|s| root.join(s))
}

/// Generate `NAME.bdg` next to each `NAME.sql`; with `check`, only
/// compare. The written (or stale) files, or the problems found.
pub fn run(
    files: &[std::path::PathBuf],
    database: Option<&str>,
    schema: Option<&Path>,
    check: bool,
) -> Result<Vec<String>, Vec<String>> {
    let mut done = vec![];
    let mut errors = vec![];
    for f in files {
        let text = match std::fs::read_to_string(f) {
            Ok(t) => t,
            Err(e) => {
                errors.push(format!("{}: {e}", f.display()));
                continue;
            }
        };
        let found = schema.map(Path::to_path_buf).or_else(|| project_schema(f));
        let backend = match database {
            Some(url)
                if url.starts_with("postgres://")
                    || url.starts_with("postgresql://")
                    || url.contains("host=") =>
            {
                Backend::Postgres { url }
            }
            Some(other) => {
                errors.push(format!(
                    "{other}: only Postgres URLs (postgres://...) are supported; for SQLite give --schema"
                ));
                continue;
            }
            None => Backend::Sqlite { schema: found.as_deref() },
        };
        let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let out = f.with_extension("bdg");
        match generate(&name, &text, &backend) {
            Ok(src) => {
                let src = badger_syntax::format::format(&src);
                if check {
                    if std::fs::read_to_string(&out).ok().as_deref() != Some(src.as_str()) {
                        errors.push(format!(
                            "{} is out of date (run `badger generate sql {}`)",
                            out.display(),
                            f.display()
                        ));
                    }
                } else if let Err(e) = std::fs::write(&out, src) {
                    errors.push(format!("{}: {e}", out.display()));
                } else {
                    done.push(out.display().to_string());
                }
            }
            Err(es) => errors.extend(es.into_iter().map(|e| format!("{}: {e}", f.display()))),
        }
    }
    if errors.is_empty() { Ok(done) } else { Err(errors) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_named_queries() {
        let qs = parse(
            "-- users\n\n-- name: FindUser :one\n-- Find a user.\n-- params: id: Int\nSELECT * FROM users\nWHERE id = :id;\n\n-- name: purge :exec\nDELETE FROM users;\n",
        )
        .unwrap();
        assert_eq!(qs.len(), 2);
        assert_eq!(qs[0].name, "find_user");
        assert_eq!(qs[0].kind, Kind::One);
        assert_eq!(qs[0].doc, vec!["Find a user."]);
        assert_eq!(qs[0].hints, vec![("id".to_string(), "Int".to_string())]);
        assert_eq!(qs[0].written, "SELECT * FROM users\nWHERE id = :id");
        assert_eq!(qs[1].written, "DELETE FROM users");
        assert!(parse("-- name: x :all\nSELECT 1").unwrap_err().contains("unknown result"));
        assert!(parse("-- name: x :one\n").unwrap_err().contains("no SQL"));
    }

    #[test]
    fn rewrites_parameters() {
        let (sql, ps, marks) = rewrite(
            "SELECT a::int, ':no', \"q:x\", count(*) AS \"n!\", $$:body$$ FROM t WHERE a = :a AND b = :b? OR a = :a -- :c\n",
            true,
        );
        assert_eq!(
            sql,
            "SELECT a::int, ':no', \"q:x\", count(*) AS \"n\", $$:body$$ FROM t WHERE a = $1 AND b = $2 OR a = $1 -- :c\n"
        );
        assert_eq!(
            ps,
            vec![Param { name: "a".into(), nullable: false }, Param { name: "b".into(), nullable: true }]
        );
        assert_eq!(marks, vec![Override { column: "n".into(), mark: Mark::NotNull, ty: None }]);
        let (sql, _, marks) = rewrite("SELECT sum(x) AS \"total!: Int\", y AS \"y: Float\"", true);
        assert_eq!(sql, "SELECT sum(x) AS \"total\", y AS \"y\"");
        assert_eq!(
            marks[0],
            Override { column: "total".into(), mark: Mark::NotNull, ty: Some("Int".into()) }
        );
        assert_eq!(marks[1], Override { column: "y".into(), mark: Mark::Unmarked, ty: Some("Float".into()) });
        let (sql, _, _) = rewrite("SELECT * FROM t WHERE x = :x", false);
        assert_eq!(sql, "SELECT * FROM t WHERE x = ?1");
    }

    #[test]
    fn maps_types() {
        assert_eq!(pg_type("int8[]").unwrap(), "List<Int>");
        assert_eq!(pg_type("enum mood").unwrap(), "String");
        assert!(pg_type("interval").is_err());
        assert_eq!(optional("Int".into(), None, None), "Option<Int>");
        assert_eq!(optional("Int".into(), None, Some(&Mark::NotNull)), "Int");
        assert_eq!(optional("Data".into(), Some(true), None), "Data");
    }

    #[test]
    fn generates_sqlite() {
        let dir = std::env::temp_dir().join(format!("badger-sqlgen-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let schema = dir.join("schema.sql");
        std::fs::write(
            &schema,
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT);",
        )
        .unwrap();
        let out = generate(
            "users.sql",
            "-- name: find :opt\n-- params: id: Int\nSELECT id, name, email FROM users WHERE id = :id\n",
            &Backend::Sqlite { schema: Some(&schema) },
        )
        .unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(out.contains("pub type FindRow = { id: Int, name: String, email: Option<String> }"), "{out}");
        assert!(
            out.contains("pub fn find<C>(db: C, id: Int) -> Result<Option<FindRow>, sqlite.Error>"),
            "{out}"
        );
        assert!(out.contains("\"SELECT id, name, email FROM users WHERE id = ?1\""), "{out}");
    }
}
