//! Restricted projects (`[package] restricted = true`): only the entry
//! module may use ambient authority, the standard functions that reach the
//! filesystem, processes, the environment, databases, or the network by
//! name. Every other module must be given capabilities (such as `fs.Dir`).
//! The check follows calls and function references through user code, so a
//! wrapper (in any module, the entry's included) does not hide an ambient
//! use; it reports the path from the restricted function to the ambient one.
//! Foreign (`extern "c"`) functions count as ambient authority too.
//!
//! Beyond the listed functions, every standard free function or module
//! value (such as a provider) that reaches an intrinsic touching the
//! operating system, through other standard free functions and values, is
//! ambient as well. Methods are not: they act on a value someone was given
//! (a capability such as `fs.Dir`, `env.Vars`, or `fetch.Client`).

use lang_base::{Diagnostic, Interner, Span};
use lang_hir::{DefId, DefKind, ExprKind, Hir};
use std::collections::{HashMap as FxHashMap, HashSet as FxHashSet};

/// Standard functions that grant ambient authority.
const AMBIENT: &[&str] = &[
    "fs.read_text",
    "fs.write_text",
    "fs.exists",
    "fs.list",
    "fs.root",
    "io.read_text",
    "process.run",
    "process.run_input",
    "process.allow",
    "process.exit",
    "env.get",
    "env.args",
    "env.vars",
    "sqlite.open",
    "jobs.open",
    "http.listen",
    "fetch.get",
    "fetch.post",
    "fetch.client",
    "http.listen_with",
];

/// Intrinsics that reach the operating system.
const AMBIENT_INTRINSICS: &[&str] = &[
    "fs_read_text",
    "fs_write_text",
    "fs_exists",
    "fs_root",
    "fs_list",
    "env_get",
    "env_args",
    "config_from_env",
    "config_load",
    "process_run",
    "process_run_input",
    "process_exit",
    "sql_open",
    "http_listen",
    "fetch_request",
];

/// Standard free functions and module values that reach an ambient
/// intrinsic through other standard free functions and values.
pub fn std_ambient(hir: &Hir, interner: &Interner) -> FxHashSet<DefId> {
    let std_def = |d: DefId| hir.modules[hir.defs[d].module].is_std;
    // the definitions whose code contains each expression, by span
    let mut ranges: Vec<(lang_base::FileId, u32, u32, DefId)> = vec![];
    for (d, def) in hir.defs.iter() {
        if !std_def(d) {
            continue;
        }
        let body = match &def.kind {
            DefKind::Fn(f) => f.body,
            DefKind::Global(g) => Some(g.value),
            _ => None,
        };
        if let Some(b) = body {
            let sp = hir.exprs[b].span;
            ranges.push((def.span.file, def.span.start.min(sp.start), def.span.end.max(sp.end), d));
        }
    }
    let owner = |sp: Span| {
        ranges
            .iter()
            .filter(|(f, a, b, _)| *f == sp.file && *a <= sp.start && sp.end <= *b)
            .min_by_key(|(_, a, b, _)| b - a)
            .map(|r| r.3)
    };
    let mut refs: FxHashMap<DefId, Vec<DefId>> = FxHashMap::default();
    for (_, ex) in hir.exprs.iter() {
        if let ExprKind::Def { def, .. } = &ex.kind
            && !ex.span.is_dummy()
            && let Some(o) = owner(ex.span)
        {
            refs.entry(o).or_default().push(*def);
        }
    }
    // free functions and values only: a method acts on a given value
    let candidate = |d: DefId| match &hir.defs[d].kind {
        DefKind::Fn(f) => f.owner.is_none() && f.abi == lang_hir::FnAbi::Lang,
        DefKind::Global(_) => true,
        _ => false,
    };
    let mut ambient: FxHashSet<DefId> = FxHashSet::default();
    for (d, def) in hir.defs.iter() {
        if let DefKind::Fn(f) = &def.kind
            && std_def(d)
            && (f.abi == lang_hir::FnAbi::Native
                || f.abi == lang_hir::FnAbi::Intrinsic
                    && AMBIENT_INTRINSICS.contains(&interner.resolve(def.name)))
        {
            ambient.insert(d);
        }
    }
    loop {
        let mut grew = false;
        for (d, rs) in &refs {
            if !ambient.contains(d) && candidate(*d) && rs.iter().any(|r| ambient.contains(r)) {
                ambient.insert(*d);
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    ambient.retain(|d| candidate(*d));
    ambient
}

pub fn check(hir: &Hir, types: &lang_typeck::TypeIndex, interner: &Interner) -> Vec<Diagnostic> {
    let Some(entry) = hir.entry else { return vec![] };
    let mut sinks: FxHashMap<DefId, String> =
        AMBIENT.iter().filter_map(|n| hir.try_lang(n).map(|d| (d, n.to_string()))).collect();
    for d in std_ambient(hir, interner) {
        sinks.entry(d).or_insert_with(|| {
            let def = &hir.defs[d];
            let m: Vec<&str> = hir.modules[def.module].path.iter().map(|s| interner.resolve(*s)).collect();
            let m = m.join(".");
            format!("{}.{}", m.strip_prefix("std.").unwrap_or(&m), interner.resolve(def.name))
        });
    }
    let user = |d: DefId| !hir.modules[hir.defs[d].module].is_std;
    let name = |d: DefId| {
        let def = &hir.defs[d];
        let m: Vec<&str> = hir.modules[def.module].path.iter().map(|s| interner.resolve(*s)).collect();
        format!("{}.{}", m.join("."), interner.resolve(def.name))
    };
    // user functions and the spans they cover (bodies include lambdas)
    let mut ranges: Vec<(lang_base::FileId, u32, u32, DefId)> = vec![];
    for (d, def) in hir.defs.iter() {
        if let DefKind::Fn(f) = &def.kind
            && let Some(b) = f.body
            && user(d)
        {
            let body = hir.exprs[b].span;
            ranges.push((def.span.file, def.span.start.min(body.start), def.span.end.max(body.end), d));
        }
    }
    let owner = |sp: Span| {
        ranges
            .iter()
            .filter(|(f, a, b, _)| *f == sp.file && *a <= sp.start && sp.end <= *b)
            .min_by_key(|(_, a, b, _)| b - a)
            .map(|r| r.3)
    };
    // edges: a function to what it calls or names, with the site
    let mut edges: FxHashMap<DefId, Vec<(DefId, Span)>> = FxHashMap::default();
    let mut add = |from: Option<DefId>, to: DefId, sp: Span| {
        if let Some(f) = from {
            let v = edges.entry(f).or_default();
            if !v.iter().any(|(t, _)| *t == to) {
                v.push((to, sp));
            }
        }
    };
    for (e, t) in &types.targets {
        let sp = hir.exprs[*e].span;
        add(owner(sp), *t, sp);
    }
    for (_, ex) in hir.exprs.iter() {
        if let ExprKind::Def { def, .. } = &ex.kind
            && !ex.span.is_dummy()
        {
            add(owner(ex.span), *def, ex.span);
        }
    }
    // the ambient function each user function reaches, and through whom
    let mut out = vec![];
    for &(_, _, _, f) in &ranges {
        if hir.defs[f].module == entry {
            continue;
        }
        // depth-first over user functions
        let mut seen: FxHashSet<DefId> = FxHashSet::default();
        let mut stack: Vec<(DefId, Vec<DefId>, Span)> = vec![(f, vec![], Span::dummy())];
        let mut found = None;
        while let Some((d, path, first)) = stack.pop() {
            if !seen.insert(d) {
                continue;
            }
            for (t, sp) in edges.get(&d).map(|v| v.as_slice()).unwrap_or(&[]) {
                let site = if d == f { *sp } else { first };
                // a foreign (C) or native module function can do anything
                let foreign = match &hir.defs[*t].kind {
                    DefKind::Fn(x) if x.abi == lang_hir::FnAbi::C => {
                        Some(format!("extern \"c\" fn {}", name(*t)))
                    }
                    DefKind::Fn(x) if x.abi == lang_hir::FnAbi::Native => Some(format!(
                        "extern \"native {}\" fn {}",
                        x.native.as_deref().unwrap_or(""),
                        name(*t)
                    )),
                    _ => None,
                };
                if let Some(s) = sinks.get(t).map(|s| s.to_string()).or(foreign) {
                    found = Some((path.clone(), s, site));
                    break;
                }
                if user(*t) {
                    let mut p = path.clone();
                    p.push(*t);
                    stack.push((*t, p, site));
                }
            }
            if found.is_some() {
                break;
            }
        }
        if let Some((path, sink, site)) = found {
            let via = if path.is_empty() {
                String::new()
            } else {
                format!(
                    " through {}",
                    path.iter().map(|d| format!("`{}`", name(*d))).collect::<Vec<_>>().join(", ")
                )
            };
            out.push(
                Diagnostic::error(format!("`{}` uses ambient authority `{sink}`{via}", name(f)))
                    .with_code("E0170")
                    .with_label(site, "reaches it here")
                    .with_note("this project is restricted (`[package] restricted = true`): only the entry module may use the filesystem, processes, the environment, databases, the network, or foreign and native code directly; give this function a capability instead, such as an `fs.Dir` or `fs.ReadDir` for files"),
            );
        }
    }
    out
}
