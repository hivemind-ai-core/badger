//! Typed SQL: literal statements given to `std.sqlite` are prepared (not
//! run) when the program is checked, against a scratch database holding the
//! program's schema: the project's `[database] schema` file, then every
//! literal `CREATE`/`ALTER` statement of the program in source order. A
//! statement that does not compile is an error (E0180): with a known schema
//! that includes unknown tables and columns; without one, only syntax
//! errors. The checks also cover the call: one statement where only one
//! runs, `execute` with parameters on a statement that returns rows (it
//! fails at run time), `query_one` on one that returns none, the number of
//! parameters given as a list literal (E0181), and for `query_as` /
//! `query_one_as`, the row type's fields against the result's columns:
//! each field has a column, a column that can be NULL is an `Option`, and
//! a declared column type fits the field (E0182).

use crate::sqlcheck::{Db, Prepared, split};
use lang_base::{Diagnostic, Interner, Span};
use lang_hir::{DefId, DefKind, ExprId, ExprKind, HTyKind, Hir, Lit};
use std::collections::HashMap as FxHashMap;
use std::path::Path;

#[derive(Clone, Copy, PartialEq)]
enum Call {
    Execute,
    Query,
    QueryOne,
    QueryAs,
    QueryOneAs,
}

impl Call {
    fn name(self) -> &'static str {
        match self {
            Call::Execute => "sqlite.execute",
            Call::Query => "sqlite.query",
            Call::QueryOne => "sqlite.query_one",
            Call::QueryAs => "sqlite.query_as",
            Call::QueryOneAs => "sqlite.query_one_as",
        }
    }
}

struct Site {
    call: ExprId,
    kind: Call,
    sql: String,
    span: Span,
    /// The number of parameters, when they are a list literal.
    params: Option<usize>,
}

fn is_ddl(stmt: &str) -> bool {
    let w = stmt.split_whitespace().next().unwrap_or("").to_ascii_uppercase();
    matches!(w.as_str(), "CREATE" | "ALTER" | "DROP")
}

fn syntax_error(msg: &str) -> bool {
    msg.contains("syntax error") || msg.contains("incomplete input") || msg.contains("unrecognized token")
}

/// Whether the program uses `std.sqlite` (the check needs recorded types).
pub fn wanted(hir: &Hir) -> bool {
    hir.try_lang("sqlite.execute").is_some()
}

pub fn check(
    hir: &Hir,
    types: &lang_typeck::TypeIndex,
    interner: &Interner,
    schema: Option<&Path>,
) -> Vec<Diagnostic> {
    let calls: FxHashMap<DefId, Call> = [
        ("sqlite.execute", Call::Execute),
        ("sqlite.query", Call::Query),
        ("sqlite.query_one", Call::QueryOne),
        ("sqlite.query_as", Call::QueryAs),
        ("sqlite.query_one_as", Call::QueryOneAs),
    ]
    .iter()
    .filter_map(|(n, k)| hir.try_lang(n).map(|d| (d, *k)))
    .collect();
    if calls.is_empty() {
        return vec![];
    }
    let std_files: Vec<lang_base::FileId> =
        hir.modules.iter().filter(|(_, m)| m.is_std).map(|(_, m)| m.file).collect();
    let mut sites = vec![];
    for (e, ex) in hir.exprs.iter() {
        let ExprKind::Call { callee, args } = &ex.kind else { continue };
        if ex.span.is_dummy() || std_files.contains(&ex.span.file) {
            continue;
        }
        let def = match &hir.exprs[*callee].kind {
            ExprKind::Def { def, .. } => Some(*def),
            _ => types.targets.get(callee).copied(),
        };
        let Some(kind) = def.and_then(|d| calls.get(&d)).copied() else { continue };
        let Some(sql) = args.get(1) else { continue };
        let ExprKind::Lit(Lit::Str(text)) = &hir.exprs[*sql].kind else { continue };
        let params = args.get(2).and_then(|p| match &hir.exprs[*p].kind {
            ExprKind::List(items) => Some(items.len()),
            _ => None,
        });
        sites.push(Site { call: e, kind, sql: text.clone(), span: hir.exprs[*sql].span, params });
    }
    if sites.is_empty() {
        return vec![];
    }
    sites.sort_by_key(|s| (s.span.file, s.span.start));
    let mut out = vec![];
    let Ok(db) = Db::new() else { return out };
    let mut strict = false;
    if let Some(path) = schema {
        strict = true;
        match std::fs::read_to_string(path) {
            Ok(text) => {
                if let Err(e) = db.run(&text) {
                    out.push(
                        Diagnostic::error(format!(
                            "the database schema {} does not load: {e}",
                            path.display()
                        ))
                        .with_code("E0180"),
                    );
                    return out;
                }
            }
            Err(e) => {
                out.push(
                    Diagnostic::error(format!("cannot read the database schema {}: {e}", path.display()))
                        .with_code("E0180"),
                );
                return out;
            }
        }
    }
    // the program's own schema statements, in source order
    for s in &sites {
        for stmt in split(&s.sql).into_iter().filter(|t| is_ddl(t)) {
            match db.run(&stmt) {
                Ok(()) => strict = true,
                Err(e) if syntax_error(&e) => out.push(sql_error(s, &e)),
                Err(_) => {}
            }
        }
    }
    for s in &sites {
        let stmts = split(&s.sql);
        let batch = s.kind == Call::Execute && s.params == Some(0);
        if stmts.len() > 1 && !batch {
            out.push(
                Diagnostic::error(format!(
                    "`{}` runs one statement, and this text has {}",
                    s.kind.name(),
                    stmts.len()
                ))
                .with_code("E0180")
                .with_label(s.span, "several statements")
                .with_note("`sqlite.execute` with no parameters runs a batch of statements"),
            );
            continue;
        }
        for stmt in &stmts {
            if is_ddl(stmt) {
                continue;
            }
            let p = match db.prepare(stmt) {
                Ok(p) => p,
                Err(e) => {
                    if strict || syntax_error(&e) {
                        out.push(sql_error(s, &e));
                    }
                    continue;
                }
            };
            out.extend(check_call(hir, types, interner, s, &p, batch));
        }
    }
    out
}

fn sql_error(s: &Site, msg: &str) -> Diagnostic {
    Diagnostic::error(format!("SQL does not compile: {msg}"))
        .with_code("E0180")
        .with_label(s.span, "in this statement")
        .with_note("checked against the project's `[database] schema` and the program's literal CREATE/ALTER statements")
}

fn check_call(
    hir: &Hir,
    types: &lang_typeck::TypeIndex,
    interner: &Interner,
    s: &Site,
    p: &Prepared,
    batch: bool,
) -> Vec<Diagnostic> {
    let mut out = vec![];
    let rows = !p.columns.is_empty();
    match s.kind {
        Call::Execute if rows && !batch => out.push(
            Diagnostic::error("`sqlite.execute` with parameters fails on a statement that returns rows")
                .with_code("E0180")
                .with_label(s.span, "returns rows")
                .with_note("use `sqlite.query` (or `query_one`) to read them"),
        ),
        Call::QueryOne | Call::QueryOneAs if !rows => out.push(
            Diagnostic::error(format!(
                "`{}` needs a statement that returns a row; this one returns none",
                s.kind.name()
            ))
            .with_code("E0180")
            .with_label(s.span, "returns no rows")
            .with_note("use `sqlite.execute`, or add `RETURNING`"),
        ),
        _ => {}
    }
    if let Some(n) = s.params
        && !batch
        && n != p.params
    {
        out.push(
            Diagnostic::error(format!(
                "this statement takes {} parameter{}, but {n} {} given",
                p.params,
                if p.params == 1 { "" } else { "s" },
                if n == 1 { "is" } else { "are" }
            ))
            .with_code("E0181")
            .with_label(s.span, "the statement"),
        );
    }
    if matches!(s.kind, Call::QueryAs | Call::QueryOneAs)
        && rows
        && let Some((name, fields)) = row_type(hir, types, interner, s)
    {
        out.extend(check_row(&name, &fields, s, p));
    }
    out
}

/// A row type's field: its name, whether it is an `Option`, the type name
/// (inside the `Option`) when known, and where it is declared.
struct Field {
    name: String,
    optional: bool,
    base: Option<String>,
    span: Option<Span>,
}

/// The fields of a record type written out (`{a: Int, b: Option<String>}`).
fn record_fields(text: &str) -> Option<Vec<Field>> {
    let inner = text.trim().strip_prefix('{')?.strip_suffix('}')?;
    let mut parts = vec![];
    let (mut depth, mut cur) = (0i32, String::new());
    for c in inner.chars() {
        match c {
            '<' | '{' | '(' | '[' => depth += 1,
            '>' | '}' | ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    parts.push(cur);
    let mut out = vec![];
    for p in parts.iter().map(|p| p.trim()).filter(|p| !p.is_empty()) {
        let (name, ty) = p.split_once(':')?;
        let ty = ty.trim();
        let (optional, base) = match ty.strip_prefix("Option<").and_then(|t| t.strip_suffix('>')) {
            Some(t) => (true, t),
            None => (false, ty),
        };
        let base =
            base.split('<').next().unwrap_or(base).rsplit('.').next().unwrap_or(base).trim().to_string();
        out.push(Field { name: name.trim().to_string(), optional, base: Some(base), span: None });
    }
    Some(out)
}

/// The row type `T` of a `query_as`/`query_one_as` call, from its type
/// (`Result<List<T>, Error>` or `Result<T, Error>`): a struct, or a record
/// type written out. Returns its name and fields.
fn row_type(
    hir: &Hir,
    types: &lang_typeck::TypeIndex,
    interner: &Interner,
    s: &Site,
) -> Option<(String, Vec<Field>)> {
    let ty = types.exprs.get(&s.call)?;
    let inner = ty.strip_prefix("Result<")?;
    let inner = if s.kind == Call::QueryAs { inner.strip_prefix("List<")? } else { inner };
    if inner.starts_with('{') {
        // up to the matching brace
        let mut depth = 0;
        let end = inner.char_indices().find_map(|(i, c)| {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i + 1);
                    }
                }
                _ => {}
            }
            None
        })?;
        return Some(("the row record".into(), record_fields(&inner[..end])?));
    }
    let end = inner.find([',', '<', '>']).unwrap_or(inner.len());
    let path: Vec<&str> = inner[..end].trim().split('.').collect();
    let (name, module) = path.split_last()?;
    let def = hir.defs.iter().find_map(|(d, def)| {
        let DefKind::Struct(_) = &def.kind else { return None };
        if interner.resolve(def.name) != *name {
            return None;
        }
        let m: Vec<&str> = hir.modules[def.module].path.iter().map(|x| interner.resolve(*x)).collect();
        (module.is_empty() || m.ends_with(module)).then_some(d)
    })?;
    let DefKind::Struct(st) = &hir.defs[def].kind else { return None };
    let fields = st
        .fields
        .iter()
        .map(|f| {
            let (optional, base) = field_type(hir, interner, &f.ty);
            Field { name: interner.resolve(f.name).to_string(), optional, base, span: Some(f.span) }
        })
        .collect();
    Some((format!("`{}`", interner.resolve(hir.defs[def].name)), fields))
}

/// A field's type: its outer name, whether it is an `Option`, and the
/// name inside the `Option`.
fn field_type(hir: &Hir, interner: &Interner, ty: &lang_hir::HTy) -> (bool, Option<String>) {
    let name = |t: &lang_hir::HTy| match &t.kind {
        HTyKind::Named { def, .. } => Some(interner.resolve(hir.defs[*def].name).to_string()),
        _ => None,
    };
    match &ty.kind {
        HTyKind::Named { def, args }
            if interner.resolve(hir.defs[*def].name) == "Option" && args.len() == 1 =>
        {
            (true, name(&args[0]))
        }
        _ => (false, name(ty)),
    }
}

/// SQLite type affinity of a declared column type.
fn affinity(decl: &str) -> &'static str {
    let d = decl.to_ascii_uppercase();
    if d.contains("INT") {
        "INTEGER"
    } else if d.contains("CHAR") || d.contains("CLOB") || d.contains("TEXT") {
        "TEXT"
    } else if d.contains("BLOB") || d.is_empty() {
        "BLOB"
    } else if d.contains("REAL") || d.contains("FLOA") || d.contains("DOUB") {
        "REAL"
    } else {
        "NUMERIC"
    }
}

/// Whether a field type can hold values of a column affinity (types the
/// check does not know, such as records decoded from JSON text, fit).
fn fits(field: &str, affinity: &str) -> bool {
    let int = matches!(field, "Int" | "I8" | "I16" | "I32" | "I64" | "U8" | "U16" | "U32" | "U64" | "Bool");
    let float = matches!(field, "Float" | "F32" | "F64");
    let text = matches!(field, "String" | "Symbol");
    match affinity {
        "INTEGER" => !(text || float) || field == "Float",
        "TEXT" => !(int || float),
        "REAL" => !(int || text),
        _ => true,
    }
}

fn check_row(tname: &str, fields: &[Field], s: &Site, p: &Prepared) -> Vec<Diagnostic> {
    let columns: Vec<&str> = p.columns.iter().map(|c| c.name.as_str()).collect();
    let mut out = vec![];
    let at = |d: Diagnostic, f: &Field| {
        let d = d.with_code("E0182").with_label(s.span, "the statement");
        match f.span {
            Some(sp) => d.with_label(sp, "this field"),
            None => d,
        }
    };
    for f in fields {
        let fname = f.name.as_str();
        let shown = match (&f.base, f.optional) {
            (Some(b), true) => format!("Option<{b}>"),
            (Some(b), false) => b.clone(),
            (None, _) => "its type".into(),
        };
        let Some(c) = p.columns.iter().find(|c| c.name == fname) else {
            out.push(at(
                Diagnostic::error(format!(
                    "the result has no column `{fname}` for field `{fname}` of {tname}"
                ))
                .with_note(format!("the columns are: {}", columns.join(", "))),
                f,
            ));
            continue;
        };
        if c.nullable == Some(true) && !f.optional {
            out.push(at(
                Diagnostic::error(format!(
                    "column `{fname}` can be NULL, but field `{fname}` of {tname} is `{shown}`"
                ))
                .with_note(format!(
                    "declare the field `Option<{}>`, or the column NOT NULL",
                    f.base.as_deref().unwrap_or("...")
                )),
                f,
            ));
        }
        if let (Some(decl), Some(b)) = (&c.decl, &f.base) {
            let a = affinity(decl);
            if !fits(b, a) {
                out.push(at(
                    Diagnostic::error(format!(
                        "column `{fname}` is {decl}, which does not fit field `{fname}: {shown}` of {tname}"
                    )),
                    f,
                ));
            }
        }
    }
    out
}
