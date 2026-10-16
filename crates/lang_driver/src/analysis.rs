//! Semantic analysis for editor and agent tooling (the language server and
//! MCP mode): diagnostics, document symbols, references with hover text and
//! definition targets, and completion data, from one check of a program
//! with unsaved buffers overlaid. Everything is owned data with byte
//! offsets, so callers keep it across edits and convert positions
//! themselves.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use lang_base::{Severity, Span};
use lang_hir::{DefKind, ExprKind, HTy, HTyKind, PatKind, Stmt};

use crate::Session;

/// A location: file path and byte range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loc {
    pub path: String,
    pub start: usize,
    pub end: usize,
}

/// What a reference denotes, for finding every reference to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    Local(u32),
    Def(u32),
}

#[derive(Debug, Clone)]
pub struct Ref {
    pub start: usize,
    pub end: usize,
    pub target: Option<Loc>,
    pub key: Option<Key>,
    /// Markdown-free hover text (a type or signature).
    pub hover: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    Method,
    Struct,
    Enum,
    Protocol,
    Type,
    Value,
    Impl,
}

#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: SymbolKind,
    pub detail: String,
    /// The whole declaration and its name.
    pub start: usize,
    pub end: usize,
    pub name_start: usize,
    pub name_end: usize,
    pub children: Vec<SymbolInfo>,
}

/// A completion candidate.
#[derive(Debug, Clone)]
pub struct Item {
    pub name: String,
    pub detail: String,
    pub kind: SymbolKind,
}

/// A local binding, for completion of names and `name.` members.
#[derive(Debug, Clone)]
pub struct LocalInfo {
    pub name: String,
    pub ty: String,
    /// Where it is declared and the body it is visible in.
    pub start: usize,
    pub scope: (usize, usize),
    pub members: Vec<Item>,
}

#[derive(Debug, Clone, Default)]
pub struct FileIndex {
    pub path: String,
    pub text: String,
    pub symbols: Vec<SymbolInfo>,
    pub refs: Vec<Ref>,
    pub locals: Vec<LocalInfo>,
}

#[derive(Debug, Clone)]
pub struct FileDiag {
    pub path: String,
    pub start: usize,
    pub end: usize,
    pub severity: Severity,
    pub code: Option<String>,
    pub message: String,
    /// Secondary labels and notes, already rendered.
    pub related: Vec<(Loc, String)>,
    pub notes: Vec<String>,
}

#[derive(Debug, Default)]
pub struct Analysis {
    /// User (non-library) files of the program.
    pub files: Vec<FileIndex>,
    pub diagnostics: Vec<FileDiag>,
    /// Members of each module by the name it is used under (`io`, `list`).
    pub modules: HashMap<String, Vec<Item>>,
    /// Names visible at the top of every user module (its own items).
    pub globals: HashMap<String, Vec<Item>>,
}

impl Analysis {
    pub fn file(&self, path: &str) -> Option<&FileIndex> {
        self.files.iter().find(|f| f.path == path)
    }

    /// The innermost reference containing byte `offset` of `path`.
    pub fn ref_at(&self, path: &str, offset: usize) -> Option<&Ref> {
        self.file(path)?
            .refs
            .iter()
            .filter(|r| r.start <= offset && offset <= r.end)
            .min_by_key(|r| (r.end - r.start, r.target.is_none()))
    }

    /// Every reference to `key` across the program (declarations included).
    pub fn references(&self, key: Key) -> Vec<Loc> {
        let mut out: Vec<Loc> = vec![];
        for f in &self.files {
            for r in &f.refs {
                if r.key == Some(key) {
                    let l = Loc { path: f.path.clone(), start: r.start, end: r.end };
                    if !out.contains(&l) {
                        out.push(l);
                    }
                }
            }
        }
        out
    }
}

fn kind_of(k: &DefKind, method: bool) -> Option<SymbolKind> {
    Some(match k {
        DefKind::Fn(_) if method => SymbolKind::Method,
        DefKind::Fn(_) => SymbolKind::Function,
        DefKind::Global(_) => SymbolKind::Value,
        DefKind::Struct(_) => SymbolKind::Struct,
        DefKind::Enum(_) => SymbolKind::Enum,
        DefKind::Protocol(_) => SymbolKind::Protocol,
        DefKind::Alias(_) | DefKind::Opaque(_) => SymbolKind::Type,
        DefKind::Impl(_) => SymbolKind::Impl,
        DefKind::Pending => return None,
    })
}

/// Check the program at `path` (an entry file or project directory) with
/// `overlays` (canonical path -> unsaved text) and index its user code.
pub fn analyze(path: &Path, overlays: HashMap<PathBuf, String>) -> Analysis {
    let mut s = Session::new();
    s.overlays = overlays;
    let mut out = Analysis::default();
    let loaded = s.load(path);
    let mut hir = None;
    if let Some(l) = &loaded
        && !s.diagnostics.has_errors()
    {
        let h = s.lower(l);
        if !s.diagnostics.has_errors() {
            hir = Some(h);
        } else {
            s.annotate_scopes(&h);
        }
    }
    if let Some(hir) = &hir {
        lang_typeck::set_record_types(true);
        let (checked, diags) = lang_typeck::check_program(hir, &s.interner);
        lang_typeck::set_record_types(false);
        s.diagnostics.extend(diags);
        index(&s, hir, &checked, &mut out);
        drop(checked);
        s.annotate_scopes(hir);
    }
    s.diagnostics.dedupe(&s.sources);
    for d in s.diagnostics.iter() {
        let Some(p) = d.primary() else { continue };
        let f = s.sources.get(p.file);
        let loc = |sp: Span| {
            let f = s.sources.get(sp.file);
            Loc { path: f.name.clone(), start: sp.start as usize, end: sp.end as usize }
        };
        let mut message = d.message.clone();
        if let Some(l) = d.labels.iter().find(|l| l.span == p)
            && !l.message.is_empty()
        {
            message = format!("{message}: {}", l.message);
        }
        let mut notes = d.notes.clone();
        if let (Some(e), Some(a)) = (&d.expected, &d.actual) {
            notes.push(format!("expected {e}, found {a}"));
        }
        out.diagnostics.push(FileDiag {
            path: f.name.clone(),
            start: p.start as usize,
            end: p.end as usize,
            severity: d.severity,
            code: d.code.map(str::to_string),
            message,
            related: d
                .labels
                .iter()
                .filter(|l| l.span != p && !l.span.is_dummy())
                .map(|l| (loc(l.span), l.message.clone()))
                .collect(),
            notes,
        });
    }
    // files that did not get as far as checking still appear (diagnostics,
    // and their text for position conversion)
    if let Some(l) = &loaded {
        for m in &l.modules {
            if m.is_std {
                continue;
            }
            let f = s.sources.get(m.file);
            if out.file(&f.name).is_none() {
                out.files.push(FileIndex {
                    path: f.name.clone(),
                    text: f.text.clone(),
                    ..Default::default()
                });
            }
        }
    }
    out
}

struct Ix<'a> {
    s: &'a Session,
    hir: &'a lang_hir::Hir,
    checked: &'a lang_typeck::Checked<'a>,
    types: &'a lang_typeck::TypeIndex,
    sigs: HashMap<u32, String>,
}

impl Ix<'_> {
    fn path(&self, sp: Span) -> String {
        self.s.sources.get(sp.file).name.clone()
    }

    fn text(&self, sp: Span) -> &str {
        self.s.sources.get(sp.file).text.get(sp.range()).unwrap_or("")
    }

    /// The span of a definition's name inside its declaration.
    fn name_span(&self, d: lang_hir::DefId) -> Span {
        let def = &self.hir.defs[d];
        let name = self.s.interner.resolve(def.name);
        let text = self.text(def.span);
        let mut from = 0;
        while let Some(i) = text[from..].find(name) {
            let at = from + i;
            let before = text[..at].chars().next_back();
            let after = text[at + name.len()..].chars().next();
            let ident = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
            if !ident(before) && !ident(after) {
                let start = def.span.start + at as u32;
                return Span { file: def.span.file, start, end: start + name.len() as u32 };
            }
            from = at + name.len();
        }
        def.span
    }

    fn def_loc(&self, d: lang_hir::DefId) -> Option<Loc> {
        let sp = self.name_span(d);
        if sp.is_dummy() {
            return None;
        }
        Some(Loc { path: self.path(sp), start: sp.start as usize, end: sp.end as usize })
    }

    fn signature(&mut self, d: lang_hir::DefId) -> String {
        if let Some(s) = self.sigs.get(&d.into_raw().into_u32()) {
            return s.clone();
        }
        let def = &self.hir.defs[d];
        let name = self.s.interner.resolve(def.name).to_string();
        let sig = match crate::describe_def(&self.checked.g, self.hir, &self.s.interner, &self.s.sources, d) {
            // a function type already shows its effects
            Some((ty, _)) => match &def.kind {
                DefKind::Fn(_) | DefKind::Global(_) => format!("{name}: {ty}"),
                _ => ty,
            },
            None => name,
        };
        self.sigs.insert(d.into_raw().into_u32(), sig.clone());
        sig
    }

    fn local_hover(&self, l: lang_hir::LocalId) -> String {
        let name = self.s.interner.resolve(self.hir.locals[l].name);
        match self.types.locals.get(&l) {
            Some(t) => format!("{name}: {t}"),
            None => name.to_string(),
        }
    }

    fn add_ref(&self, out: &mut HashMap<String, Vec<Ref>>, sp: Span, r: Ref) {
        if sp.is_dummy() {
            return;
        }
        out.entry(self.path(sp)).or_default().push(Ref {
            start: sp.start as usize,
            end: sp.end as usize,
            ..r
        });
    }

    fn hty(&mut self, t: &HTy, out: &mut HashMap<String, Vec<Ref>>) {
        match &t.kind {
            HTyKind::Named { def, args } => {
                let r = Ref {
                    start: 0,
                    end: 0,
                    target: self.def_loc(*def),
                    key: Some(Key::Def(def.into_raw().into_u32())),
                    hover: self.signature(*def),
                };
                self.add_ref(out, t.span, r);
                for a in args {
                    self.hty(a, out);
                }
            }
            HTyKind::Dyn { protocol, args } => {
                let r = Ref {
                    start: 0,
                    end: 0,
                    target: self.def_loc(*protocol),
                    key: Some(Key::Def(protocol.into_raw().into_u32())),
                    hover: self.signature(*protocol),
                };
                self.add_ref(out, t.span, r);
                for a in args {
                    self.hty(a, out);
                }
            }
            HTyKind::Assoc { base, .. } => self.hty(base, out),
            HTyKind::Tuple(ts) => ts.iter().for_each(|x| self.hty(x, out)),
            HTyKind::Record { fields, .. } => fields.iter().for_each(|(_, x)| self.hty(x, out)),
            HTyKind::Fn { params, ret, .. } => {
                params.iter().for_each(|x| self.hty(x, out));
                self.hty(ret, out);
            }
            HTyKind::Forall { body, .. } => self.hty(body, out),
            HTyKind::Ref { inner, .. } => self.hty(inner, out),
            _ => {}
        }
    }
}

fn index(s: &Session, hir: &lang_hir::Hir, checked: &lang_typeck::Checked<'_>, out: &mut Analysis) {
    let empty = lang_typeck::TypeIndex::default();
    let types = checked.types.as_ref().unwrap_or(&empty);
    let mut ix = Ix { s, hir, checked, types, sigs: HashMap::new() };
    let user_files: Vec<lang_base::FileId> =
        hir.modules.iter().filter(|(_, m)| !m.is_std).map(|(_, m)| m.file).collect();
    let user = |sp: Span| !sp.is_dummy() && user_files.contains(&sp.file);
    let mut refs: HashMap<String, Vec<Ref>> = HashMap::new();

    // expressions
    for (e, ex) in hir.exprs.iter() {
        if !user(ex.span) {
            continue;
        }
        let ty = types.exprs.get(&e).cloned().unwrap_or_default();
        let target_def = types.targets.get(&e).copied();
        match &ex.kind {
            ExprKind::Local(l) => {
                let decl = hir.locals[*l].span;
                let r = Ref {
                    start: 0,
                    end: 0,
                    target: Some(Loc {
                        path: ix.path(decl),
                        start: decl.start as usize,
                        end: decl.end as usize,
                    }),
                    key: Some(Key::Local(l.into_raw().into_u32())),
                    hover: ix.local_hover(*l),
                };
                ix.add_ref(&mut refs, ex.span, r);
            }
            ExprKind::Def { def, type_args } => {
                let r = Ref {
                    start: 0,
                    end: 0,
                    target: ix.def_loc(*def),
                    key: Some(Key::Def(def.into_raw().into_u32())),
                    hover: ix.signature(*def),
                };
                ix.add_ref(&mut refs, ex.span, r);
                for t in type_args {
                    ix.hty(t, &mut refs);
                }
            }
            ExprKind::Ctor { enum_def, variant, .. } => {
                let vname = match &hir.defs[*enum_def].kind {
                    DefKind::Enum(ed) => {
                        ed.variants.get(*variant as usize).map(|v| s.interner.resolve(v.name))
                    }
                    _ => None,
                };
                let ename = s.interner.resolve(hir.defs[*enum_def].name);
                let r = Ref {
                    start: 0,
                    end: 0,
                    target: ix.def_loc(*enum_def),
                    key: Some(Key::Def(enum_def.into_raw().into_u32())),
                    hover: format!("{ename}.{}: {ty}", vname.unwrap_or("?")),
                };
                ix.add_ref(&mut refs, ex.span, r);
            }
            ExprKind::AssocFn { name_span, .. } | ExprKind::MethodCall { method_span: name_span, .. } => {
                let (target, key, hover) = match target_def {
                    Some(d) => (ix.def_loc(d), Some(Key::Def(d.into_raw().into_u32())), ix.signature(d)),
                    None => (None, None, ty.clone()),
                };
                ix.add_ref(&mut refs, *name_span, Ref { start: 0, end: 0, target, key, hover });
                ix.add_ref(&mut refs, ex.span, Ref { start: 0, end: 0, target: None, key: None, hover: ty });
            }
            ExprKind::Field { label, label_span, .. } => {
                let hover = format!("{}: {ty}", s.interner.resolve(*label));
                ix.add_ref(&mut refs, *label_span, Ref { start: 0, end: 0, target: None, key: None, hover });
            }
            ExprKind::Block { stmts, .. } => {
                for st in stmts {
                    match st {
                        Stmt::Let { ty: Some(t), .. } => ix.hty(t, &mut refs),
                        Stmt::Assign { local, span, .. } => {
                            let name = s.interner.resolve(hir.locals[*local].name);
                            let sp = Span {
                                file: span.file,
                                start: span.start,
                                end: span.start + name.len() as u32,
                            };
                            let decl = hir.locals[*local].span;
                            let r = Ref {
                                start: 0,
                                end: 0,
                                target: Some(Loc {
                                    path: ix.path(decl),
                                    start: decl.start as usize,
                                    end: decl.end as usize,
                                }),
                                key: Some(Key::Local(local.into_raw().into_u32())),
                                hover: ix.local_hover(*local),
                            };
                            ix.add_ref(&mut refs, sp, r);
                        }
                        _ => {}
                    }
                }
            }
            ExprKind::Lambda(lam) => {
                for p in &lam.params {
                    if let Some(t) = &p.ty {
                        ix.hty(t, &mut refs);
                    }
                }
                if let Some(t) = &lam.ret {
                    ix.hty(t, &mut refs);
                }
            }
            _ => {}
        }
        // every expression: its type, for hovers anywhere inside it
        if !matches!(ex.kind, ExprKind::Block { .. } | ExprKind::Lambda(_) | ExprKind::Error)
            && let Some(t) = types.exprs.get(&e)
        {
            ix.add_ref(
                &mut refs,
                ex.span,
                Ref { start: 0, end: 0, target: None, key: None, hover: t.clone() },
            );
        }
    }
    // bindings: declaration sites
    for (_, p) in hir.pats.iter() {
        if let PatKind::Bind(l) = &p.kind
            && user(p.span)
        {
            let decl = hir.locals[*l].span;
            let r = Ref {
                start: 0,
                end: 0,
                target: Some(Loc { path: ix.path(decl), start: decl.start as usize, end: decl.end as usize }),
                key: Some(Key::Local(l.into_raw().into_u32())),
                hover: ix.local_hover(*l),
            };
            ix.add_ref(&mut refs, decl, r);
        }
    }
    // definitions: names, signature types, and symbols
    let mut symbols: HashMap<String, Vec<SymbolInfo>> = HashMap::new();
    let mut impl_children: HashMap<u32, Vec<SymbolInfo>> = HashMap::new();
    for (d, def) in hir.defs.iter() {
        if !user(def.span) {
            continue;
        }
        let name_sp = ix.name_span(d);
        let method_owner = match &def.kind {
            DefKind::Fn(f) => f.owner,
            _ => None,
        };
        let r = Ref {
            start: 0,
            end: 0,
            target: ix.def_loc(d),
            key: Some(Key::Def(d.into_raw().into_u32())),
            hover: ix.signature(d),
        };
        if !matches!(def.kind, DefKind::Impl(_)) {
            ix.add_ref(&mut refs, name_sp, r);
        }
        match &def.kind {
            DefKind::Fn(f) => {
                for p in &f.params {
                    if let Some(t) = &p.ty {
                        ix.hty(t, &mut refs);
                    }
                }
                if let Some(t) = &f.ret {
                    ix.hty(t, &mut refs);
                }
            }
            DefKind::Global(g) => {
                if let Some(t) = &g.ty {
                    ix.hty(t, &mut refs);
                }
            }
            DefKind::Struct(sd) => sd.fields.iter().for_each(|fd| ix.hty(&fd.ty, &mut refs)),
            DefKind::Enum(ed) => {
                for v in &ed.variants {
                    v.fields.iter().for_each(|(_, t)| ix.hty(t, &mut refs));
                }
            }
            DefKind::Impl(im) => {
                ix.hty(&im.self_ty, &mut refs);
                if let Some(p) = &im.protocol {
                    ix.hty(p, &mut refs);
                }
            }
            _ => {}
        }
        let Some(kind) = kind_of(&def.kind, method_owner.is_some()) else { continue };
        let name = match &def.kind {
            DefKind::Impl(im) => format!("impl {}", ix.text(im.self_ty.span)),
            _ => s.interner.resolve(def.name).to_string(),
        };
        let detail = match &def.kind {
            DefKind::Impl(_) => String::new(),
            // the type alone (the symbol carries the name)
            _ => {
                let sig = ix.signature(d);
                sig.strip_prefix(&format!("{}: ", s.interner.resolve(def.name)))
                    .map(str::to_string)
                    .unwrap_or(sig)
            }
        };
        let mut end = def.span.end as usize;
        if let DefKind::Fn(f) = &def.kind
            && let Some(b) = f.body
        {
            end = end.max(hir.exprs[b].span.end as usize);
        }
        let sym = SymbolInfo {
            name,
            kind,
            detail,
            start: def.span.start as usize,
            end,
            name_start: name_sp.start as usize,
            name_end: name_sp.end as usize,
            children: vec![],
        };
        match method_owner {
            Some(o) if matches!(hir.defs[o].kind, DefKind::Impl(_)) => {
                impl_children.entry(o.into_raw().into_u32()).or_default().push(sym)
            }
            _ => symbols.entry(ix.path(def.span)).or_default().push(sym),
        }
    }
    for syms in symbols.values_mut() {
        for sym in syms.iter_mut() {
            if sym.kind == SymbolKind::Impl {
                // children were keyed by the impl's definition id
                let id = hir
                    .defs
                    .iter()
                    .find(|(_, d)| d.span.start as usize == sym.start && matches!(d.kind, DefKind::Impl(_)))
                    .map(|(i, _)| i.into_raw().into_u32());
                if let Some(c) = id.and_then(|i| impl_children.remove(&i)) {
                    sym.end = c.iter().map(|x| x.end).max().unwrap_or(sym.end).max(sym.end);
                    sym.children = c;
                }
            }
        }
        syms.sort_by_key(|x| x.start);
    }
    // locals: completion data
    let mut locals: HashMap<String, Vec<LocalInfo>> = HashMap::new();
    for (l, loc) in hir.locals.iter() {
        if !user(loc.span) {
            continue;
        }
        let scope = match loc.owner {
            Some(lang_hir::Owner::Fn(d)) => match &hir.defs[d].kind {
                DefKind::Fn(f) => f.body.map(|b| hir.exprs[b].span),
                _ => None,
            },
            Some(lang_hir::Owner::Lambda(e)) => match &hir.exprs[e].kind {
                ExprKind::Lambda(lam) => Some(hir.exprs[lam.body].span),
                _ => None,
            },
            _ => None,
        };
        let scope = scope.map(|sp| (sp.start as usize, sp.end as usize)).unwrap_or((0, usize::MAX));
        let members = types
            .members
            .get(&l)
            .map(|ms| {
                ms.iter()
                    .map(|m| Item {
                        name: m.name.clone(),
                        detail: m.detail.clone(),
                        kind: if m.method { SymbolKind::Method } else { SymbolKind::Value },
                    })
                    .collect()
            })
            .unwrap_or_default();
        locals.entry(ix.path(loc.span)).or_default().push(LocalInfo {
            name: s.interner.resolve(loc.name).to_string(),
            ty: types.locals.get(&l).cloned().unwrap_or_default(),
            start: loc.span.start as usize,
            scope: (scope.0.min(loc.span.start as usize), scope.1),
            members,
        });
    }
    // modules: members by the name code uses (`io.println`)
    for (_, m) in hir.modules.iter() {
        let Some(last) = m.path.last() else { continue };
        let mut items: Vec<Item> = m
            .items
            .iter()
            .filter(|(_, d)| !m.is_std || hir.defs[**d].is_pub)
            .filter_map(|(n, d)| {
                let def = &hir.defs[*d];
                let kind = kind_of(&def.kind, false)?;
                if kind == SymbolKind::Impl {
                    return None;
                }
                Some(Item { name: s.interner.resolve(*n).to_string(), detail: ix.signature(*d), kind })
            })
            .collect();
        items.sort_by(|a, b| a.name.cmp(&b.name));
        let file = s.sources.get(m.file).name.clone();
        if !m.is_std {
            out.globals.insert(file, items.clone());
        }
        out.modules.insert(s.interner.resolve(*last).to_string(), items);
    }
    for f in &user_files {
        let sf = s.sources.get(*f);
        let mut r = refs.remove(&sf.name).unwrap_or_default();
        r.sort_by_key(|x| (x.start, x.end));
        out.files.push(FileIndex {
            path: sf.name.clone(),
            text: sf.text.clone(),
            symbols: symbols.remove(&sf.name).unwrap_or_default(),
            refs: r,
            locals: locals.remove(&sf.name).unwrap_or_default(),
        });
    }
}
