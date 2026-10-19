//! Module collection, import resolution, and AST → HIR lowering.

use badger_base::{Diagnostic, FileId, Interner, Span, Symbol};
use badger_syntax::ast;
use la_arena::Idx;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::hir::*;

/// A parsed source module handed to lowering.
#[derive(Clone)]
pub struct ParsedModule {
    pub path: Vec<Symbol>,
    pub file: FileId,
    pub ast: ast::Ast,
    pub is_std: bool,
    /// A module of a dependency package (its tests are not the program's).
    pub dependency: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Res {
    Def(DefId),
    Module(ModuleId),
}

/// The standard modules (other than the prelude) declaring a public item
/// named `name`, read from their sources.
fn std_homes(name: &str) -> Vec<&'static str> {
    let mut out = vec![];
    for (m, src) in crate::stdlib::STD_MODULES {
        if *m == "prelude" {
            continue;
        }
        let declares = src.lines().any(|l| {
            let Some(rest) = l.strip_prefix("pub ") else { return false };
            let rest = rest.trim_start_matches("extern \"intrinsic\" ");
            ["fn ", "struct ", "enum ", "type ", "let ", "provider ", "stage ", "protocol "].iter().any(
                |kw| {
                    rest.strip_prefix(kw).is_some_and(|r| {
                        r.strip_prefix(name).is_some_and(|after| {
                            !after.starts_with(|c: char| c.is_alphanumeric() || c == '_')
                        })
                    })
                },
            )
        });
        if declares {
            out.push(*m);
        }
    }
    out
}

/// Levenshtein distance (in characters).
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let sub = prev[j] + usize::from(ca != cb);
            cur.push(sub.min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

struct ModScope {
    names: FxHashMap<Symbol, (Res, Span)>,
}

/// Attributes the compiler recognises. The first built-in attributes are a
/// separate phase; until a name is listed here it is unknown, which is an
/// error in standard-library sources and a warning elsewhere.
const KNOWN_ATTRIBUTES: &[&str] = &["derive", "allow", "fusion"];

/// Lower all modules into a single HIR. `entry` is the index of the user's
/// entry module within `modules`.
pub fn lower_program(
    modules: &[ParsedModule],
    entry: Option<usize>,
    interner: &mut Interner,
    diags: &mut Vec<Diagnostic>,
) -> Hir {
    let mut cx = Cx {
        hir: Hir::default(),
        interner,
        diags,
        scopes: vec![],
        item_defs: vec![],
        const_locals: FxHashSet::default(),
        const_checks: vec![],
        failed_imports: FxHashSet::default(),
    };
    cx.collect(modules);
    if let Some(e) = entry {
        cx.hir.entry = Some(Idx::from_raw((e as u32).into()));
    }
    cx.register_lang_items(modules);
    cx.resolve_imports(modules);
    cx.check_import_cycles(modules);
    for (mi, m) in modules.iter().enumerate() {
        cx.lower_module(mi, m);
    }
    cx.resolve_late_ctors();
    cx.check_consts();
    cx.hir
}

struct Cx<'a> {
    hir: Hir,
    interner: &'a mut Interner,
    diags: &'a mut Vec<Diagnostic>,
    scopes: Vec<ModScope>,
    /// For each module, the DefId of each AST item (None for imports).
    item_defs: Vec<Vec<Option<DefId>>>,
    /// Locals bound by `const`, and `const` initializers to validate.
    const_locals: FxHashSet<LocalId>,
    const_checks: Vec<ExprId>,
    /// Names a failing import would have bound, per module: their unknown
    /// uses are not reported again (the import's error explains them).
    failed_imports: FxHashSet<(ModuleId, Symbol)>,
}

impl Cx<'_> {
    /// Import cycles may join modules of types and functions into one
    /// checked group, but not modules that initialize module-level values.
    fn check_import_cycles(&mut self, modules: &[ParsedModule]) {
        let n = modules.len();
        let mut edges: Vec<Vec<usize>> = vec![vec![]; n];
        for (mi, m) in modules.iter().enumerate() {
            if m.is_std {
                continue;
            }
            for item in &m.ast.items {
                let ast::ItemKind::Import(imp) = &item.kind else { continue };
                let path: Vec<Symbol> = imp.path.iter().map(|x| x.sym).collect();
                if let Some(t) = self.find_module(&path) {
                    let ti = t.into_raw().into_u32() as usize;
                    if !modules[ti].is_std {
                        edges[mi].push(ti);
                    }
                }
            }
        }
        let has_values = |mi: usize| {
            modules[mi].ast.items.iter().any(|it| matches!(it.kind, ast::ItemKind::Global { .. }))
        };
        // modules on a cycle: reachable from themselves
        let mut reported = vec![false; n];
        for start in 0..n {
            if modules[start].is_std || reported[start] || !has_values(start) {
                continue;
            }
            let mut seen = vec![false; n];
            let mut stack: Vec<usize> = edges[start].clone();
            let mut cyclic = false;
            while let Some(x) = stack.pop() {
                if x == start {
                    cyclic = true;
                    break;
                }
                if !seen[x] {
                    seen[x] = true;
                    stack.extend(edges[x].iter().copied());
                }
            }
            if cyclic {
                reported[start] = true;
                let m = &modules[start];
                let name = m
                    .path
                    .iter()
                    .map(|p| self.interner.resolve(*p).to_string())
                    .collect::<Vec<_>>()
                    .join(".");
                let span = m
                    .ast
                    .items
                    .iter()
                    .find(|it| matches!(it.kind, ast::ItemKind::Global { .. }))
                    .map(|it| it.span)
                    .unwrap_or(Span::dummy());
                self.err(
                    "E0134",
                    span,
                    format!("module `{name}` initializes values but is part of an import cycle"),
                    "this value is initialized by a module on an import cycle",
                );
            }
        }
    }

    /// `Enum.variant` naming an enum of a module lowered later than its use
    /// is a constructor too (its variants were unknown at that point).
    fn resolve_late_ctors(&mut self) {
        let ids: Vec<ExprId> = self.hir.exprs.iter().map(|(id, _)| id).collect();
        for id in ids {
            let ExprKind::AssocFn { ty, name, type_args, .. } = &self.hir.exprs[id].kind else { continue };
            let HTyKind::Named { def, args } = &ty.kind else { continue };
            let DefKind::Enum(e) = &self.hir.defs[*def].kind else { continue };
            let Some(i) = e.variants.iter().position(|v| v.name == *name) else { continue };
            let targs = if type_args.is_empty() { args.clone() } else { type_args.clone() };
            let def = *def;
            self.hir.exprs[id].kind = ExprKind::Ctor { enum_def: def, variant: i as u32, type_args: targs };
        }
    }

    /// `const` initializers are literal/constructor aggregates combined with
    /// pure operators and other constants; no function is evaluated.
    fn check_consts(&mut self) {
        for e in std::mem::take(&mut self.const_checks) {
            self.hir.const_inits.push(e);
            if let Some(bad) = self.non_const(e) {
                self.diags.push(
                    Diagnostic::error("expression is not constant")
                        .with_code("E0136")
                        .with_label(bad, "not allowed in a `const` initializer")
                        .with_note("`const` permits literals, constructors, aggregates, operators, and other constants; use `let` to evaluate functions"),
                );
            }
        }
    }

    fn non_const(&self, e: ExprId) -> Option<Span> {
        let x = &self.hir.exprs[e];
        let all = |es: &mut dyn Iterator<Item = ExprId>| -> Option<Span> {
            for c in es {
                if let Some(s) = self.non_const(c) {
                    return Some(s);
                }
            }
            None
        };
        match &x.kind {
            ExprKind::Lit(_) | ExprKind::Ctor { .. } | ExprKind::Error => None,
            ExprKind::Local(l) if self.const_locals.contains(l) => None,
            ExprKind::Def { def, .. } => match &self.hir.defs[*def].kind {
                DefKind::Global(g) if g.is_const => None,
                DefKind::Fn(_) => None,
                _ => Some(x.span),
            },
            ExprKind::Call { callee, args }
                if matches!(self.hir.exprs[*callee].kind, ExprKind::Ctor { .. }) =>
            {
                all(&mut args.iter().copied())
            }
            ExprKind::Field { base, .. } => self.non_const(*base),
            ExprKind::Unary { operand, .. } => self.non_const(*operand),
            ExprKind::Binary { lhs, rhs, .. } => self.non_const(*lhs).or_else(|| self.non_const(*rhs)),
            ExprKind::Tuple(xs) | ExprKind::List(xs) | ExprKind::Set(xs) => all(&mut xs.iter().copied()),
            ExprKind::Record { spread, fields } => {
                spread.and_then(|s| self.non_const(s)).or_else(|| all(&mut fields.iter().map(|f| f.2)))
            }
            ExprKind::Map(kvs) => all(&mut kvs.iter().flat_map(|(k, v)| [*k, *v])),
            ExprKind::Block { stmts, tail: Some(t) } if stmts.is_empty() => self.non_const(*t),
            _ => Some(x.span),
        }
    }
}

fn module_id(i: usize) -> ModuleId {
    Idx::from_raw((i as u32).into())
}

impl Cx<'_> {
    fn sym(&self, s: Symbol) -> &str {
        self.interner.resolve(s)
    }

    fn err(&mut self, code: &'static str, span: Span, msg: impl Into<String>, label: impl Into<String>) {
        self.diags.push(Diagnostic::error(msg).with_code(code).with_label(span, label));
    }

    // ------------------------------------------------------------ collection

    fn collect(&mut self, modules: &[ParsedModule]) {
        for m in modules {
            let mid = self.hir.modules.alloc(Module {
                path: m.path.clone(),
                file: m.file,
                is_std: m.is_std,
                dependency: m.dependency,
                items: FxHashMap::default(),
                tests: vec![],
            });
            let mut defs = vec![];
            for item in &m.ast.items {
                let name_span = match &item.kind {
                    ast::ItemKind::Import(_) => None,
                    ast::ItemKind::TypeAlias { name, .. } => Some((name.sym, name.span)),
                    ast::ItemKind::Struct(s) => Some((s.name.sym, s.name.span)),
                    ast::ItemKind::Enum(e) => Some((e.name.sym, e.name.span)),
                    ast::ItemKind::Protocol(p) => Some((p.name.sym, p.name.span)),
                    ast::ItemKind::Fn(f) => Some((f.name.sym, f.name.span)),
                    ast::ItemKind::Extern { decl, .. } => Some((decl.name.sym, decl.name.span)),
                    ast::ItemKind::ExternType { name, .. } => Some((name.sym, name.span)),
                    ast::ItemKind::Global { pat, .. } => match &m.ast.pats[*pat].kind {
                        ast::PatKind::Bind(n) => Some((n.sym, n.span)),
                        _ => {
                            let span = m.ast.pats[*pat].span;
                            self.err(
                                "E0110",
                                span,
                                "module-level bindings must bind a single name",
                                "use a plain name",
                            );
                            None
                        }
                    },
                    ast::ItemKind::Impl(_) => {
                        let s = self.interner.intern("<impl>");
                        Some((s, item.span))
                    }
                };
                let Some((name, span)) = name_span else {
                    defs.push(None);
                    continue;
                };
                let def = self.hir.defs.alloc(Def {
                    name,
                    module: mid,
                    span,
                    is_pub: item.is_pub,
                    kind: DefKind::Pending,
                });
                if !matches!(item.kind, ast::ItemKind::Impl(_))
                    && let Some(prev) = self.hir.modules[mid].items.insert(name, def)
                {
                    let prev_span = self.hir.defs[prev].span;
                    let n = self.sym(name).to_string();
                    self.diags.push(
                        Diagnostic::error(format!("`{n}` is defined more than once in this module"))
                            .with_code("E0111")
                            .with_label(span, "redefined here")
                            .with_label(prev_span, "first defined here"),
                    );
                }
                if let ast::ItemKind::Global { .. } = item.kind {
                    self.hir.globals.push(def);
                }
                defs.push(Some(def));
            }
            self.item_defs.push(defs);
        }
    }

    fn register_lang_items(&mut self, modules: &[ParsedModule]) {
        for (mi, m) in modules.iter().enumerate() {
            if !m.is_std {
                continue;
            }
            let path: Vec<String> = m.path.iter().skip(1).map(|s| self.sym(*s).to_string()).collect();
            let prefix = if path == ["prelude"] { String::new() } else { format!("{}.", path.join(".")) };
            let items: Vec<(Symbol, DefId)> =
                self.hir.modules[module_id(mi)].items.iter().map(|(k, v)| (*k, *v)).collect();
            for (name, def) in items {
                let key = format!("{prefix}{}", self.sym(name));
                self.hir.lang.insert(key, def);
            }
        }
    }

    /// `std.result` and `std.option` re-export their prelude types and
    /// helpers (`Result`, `ok`, `err`; `Option`, `some`, `none`).
    fn reexport(&self, m: ModuleId, name: Symbol) -> Option<DefId> {
        let module = &self.hir.modules[m];
        if !module.is_std || module.path.len() != 2 {
            return None;
        }
        let allowed: &[&str] = match self.sym(module.path[1]) {
            "result" => &["Result", "ok", "err"],
            "option" => &["Option", "some", "none"],
            _ => return None,
        };
        if !allowed.contains(&self.sym(name)) {
            return None;
        }
        let prelude = self
            .hir
            .modules
            .iter()
            .find(|(_, x)| x.is_std && x.path.len() == 2 && self.sym(x.path[1]) == "prelude")
            .map(|(id, _)| id)?;
        self.hir.modules[prelude].items.get(&name).copied()
    }

    fn find_module(&self, path: &[Symbol]) -> Option<ModuleId> {
        self.hir.modules.iter().find(|(_, m)| m.path == path).map(|(id, _)| id)
    }

    fn resolve_imports(&mut self, modules: &[ParsedModule]) {
        let prelude_path = [self.interner.intern("std"), self.interner.intern("prelude")];
        let prelude = self.find_module(&prelude_path);
        for (mi, m) in modules.iter().enumerate() {
            let mid = module_id(mi);
            let mut names: FxHashMap<Symbol, (Res, Span)> = FxHashMap::default();
            // prelude first (lowest priority; later inserts override)
            if let Some(p) = prelude
                && p != mid
            {
                for (k, v) in &self.hir.modules[p].items {
                    if self.hir.defs[*v].is_pub {
                        names.insert(*k, (Res::Def(*v), self.hir.defs[*v].span));
                    }
                }
            }
            // names this module's own imports bind (one name, one meaning)
            let mut imported: FxHashMap<Symbol, (Res, Span)> = FxHashMap::default();
            let mut conflicts: Vec<(Symbol, Span, Span)> = vec![];
            let mut bind = |names: &mut FxHashMap<Symbol, (Res, Span)>, name: Symbol, r: Res, span: Span| {
                match imported.get(&name) {
                    Some((prev, at)) if *prev != r => conflicts.push((name, span, *at)),
                    _ => {
                        imported.insert(name, (r, span));
                    }
                }
                names.insert(name, (r, span));
            };
            for item in &m.ast.items {
                let ast::ItemKind::Import(imp) = &item.kind else { continue };
                let path: Vec<Symbol> = imp.path.iter().map(|n| n.sym).collect();
                let span = imp.path.last().unwrap().span.to(imp.path[0].span);
                let target = self.find_module(&path);
                match (&imp.items, target) {
                    (None, Some(t)) => {
                        let alias = imp.alias.map(|a| a.sym).unwrap_or(*path.last().unwrap());
                        bind(&mut names, alias, Res::Module(t), span);
                    }
                    (Some(items), Some(t)) => {
                        for (n, alias) in items {
                            let found = self.hir.modules[t]
                                .items
                                .get(&n.sym)
                                .copied()
                                .or_else(|| self.reexport(t, n.sym));
                            match found {
                                Some(d) if self.hir.defs[d].is_pub => {
                                    bind(&mut names, alias.unwrap_or(*n).sym, Res::Def(d), n.span);
                                }
                                Some(_) => {
                                    let s = self.sym(n.sym).to_string();
                                    self.err(
                                        "E0112",
                                        n.span,
                                        format!("`{s}` is private"),
                                        "not marked `pub`",
                                    );
                                }
                                None => {
                                    self.failed_imports.insert((mid, alias.unwrap_or(*n).sym));
                                    let s = self.sym(n.sym).to_string();
                                    let mp = self.path_str(&path);
                                    self.err(
                                        "E0113",
                                        n.span,
                                        format!("module `{mp}` has no item `{s}`"),
                                        "not found",
                                    );
                                }
                            }
                        }
                    }
                    (items, None) => {
                        let mp = self.path_str(&path);
                        self.err("E0114", span, format!("unknown module `{mp}`"), "no such module");
                        match items {
                            None => {
                                let alias = imp.alias.map(|a| a.sym).unwrap_or(*path.last().unwrap());
                                self.failed_imports.insert((mid, alias));
                            }
                            Some(items) => {
                                for (n, alias) in items {
                                    self.failed_imports.insert((mid, alias.unwrap_or(*n).sym));
                                }
                            }
                        }
                    }
                }
            }
            for (name, span, first) in conflicts {
                let s = self.sym(name).to_string();
                let d = Diagnostic::error(format!("`{s}` is imported twice, naming different things"))
                    .with_code("E0141")
                    .with_label(span, "imported again here")
                    .with_label(first, "first imported here")
                    .with_note("import one of them under another name (`import m.{x as y}`), or use the module name (`m.x`)");
                self.diags.push(d);
            }
            // own items (highest priority)
            for (k, v) in &self.hir.modules[mid].items {
                names.insert(*k, (Res::Def(*v), self.hir.defs[*v].span));
            }
            self.scopes.push(ModScope { names });
        }
    }

    fn path_str(&self, path: &[Symbol]) -> String {
        path.iter().map(|s| self.sym(*s)).collect::<Vec<_>>().join(".")
    }

    // ------------------------------------------------------------ items

    fn lower_module(&mut self, mi: usize, m: &ParsedModule) {
        let mid = module_id(mi);
        for (ii, item) in m.ast.items.iter().enumerate() {
            let Some(def) = self.item_defs[mi][ii] else { continue };
            let mut lx = BodyCx::new(self, &m.ast, mid);
            let kind = lx.lower_item(def, item);
            self.hir.defs[def].kind = kind;
            if let DefKind::Fn(f) = &self.hir.defs[def].kind {
                let name = self.sym(self.hir.defs[def].name).to_string();
                if !m.is_std && !m.dependency && name.starts_with("test_") {
                    if f.params.is_empty() {
                        self.hir.modules[mid].tests.push(def);
                    } else {
                        let span = self.hir.defs[def].span;
                        self.diags.push(
                            Diagnostic::warning(format!(
                                "`{name}` takes parameters, so `badger test` does not run it"
                            ))
                            .with_code("E0142")
                            .with_label(span, "a test takes no parameters"),
                        );
                    }
                }
            }
        }
    }
}

// ------------------------------------------------------------------ bodies

struct OwnerFrame {
    owner: Owner,
    captures: Vec<LocalId>,
}

struct BodyCx<'c, 'a> {
    cx: &'c mut Cx<'a>,
    ast: &'c ast::Ast,
    module: ModuleId,
    /// Lexical value scopes (innermost last).
    locals: Vec<FxHashMap<Symbol, LocalId>>,
    /// Generic/type parameter scopes.
    generics: Vec<FxHashMap<Symbol, GenericId>>,
    owners: Vec<OwnerFrame>,
    loops: Vec<ExprId>,
    in_impl: bool,
    /// Current owning definition for generic params.
    cur_def: Option<DefId>,
    /// Active system configuration context: node locals by plan label.
    system: Option<FxHashMap<Symbol, LocalId>>,
    system_outputs: Option<LocalId>,
    /// Lowering a contract clause (which cannot return from the function).
    in_contract: bool,
    /// Patterns being lowered (nested ones are part of their outermost),
    /// and whether they are a parameter list (checked as a whole).
    pat_depth: u32,
    in_params: bool,
}

impl<'c, 'a> BodyCx<'c, 'a> {
    fn new(cx: &'c mut Cx<'a>, ast: &'c ast::Ast, module: ModuleId) -> Self {
        BodyCx {
            cx,
            ast,
            module,
            locals: vec![],
            generics: vec![],
            owners: vec![],
            loops: vec![],
            in_impl: false,
            cur_def: None,
            system: None,
            system_outputs: None,
            pat_depth: 0,
            in_params: false,
            in_contract: false,
        }
    }

    fn sym(&self, s: Symbol) -> String {
        self.cx.interner.resolve(s).to_string()
    }

    fn intern(&mut self, s: &str) -> Symbol {
        self.cx.interner.intern(s)
    }

    fn err(&mut self, code: &'static str, span: Span, msg: impl Into<String>, label: impl Into<String>) {
        self.cx.err(code, span, msg, label);
    }

    /// E0127 for `name`, unless a failing import of this module would have
    /// bound it (that import is already an error).
    fn unknown_name(&mut self, name: Symbol, span: Span) {
        if self.cx.failed_imports.contains(&(self.module, name)) {
            return;
        }
        let s = self.sym(name);
        let mut d = Diagnostic::error(format!("unknown name `{s}`"))
            .with_code("E0127")
            .with_label(span, "not found in this scope");
        // a standard module used without its import
        if crate::stdlib::STD_MODULES.iter().any(|(m, _)| *m == s && *m != "prelude") {
            d = d.with_note(format!("`{s}` is a standard module: add `import std.{s}`"));
        } else {
            // a standard module's member used bare
            let homes = std_homes(&s);
            if !homes.is_empty() {
                let m = homes[0];
                let others = if homes.len() > 1 {
                    format!(
                        " (also in {})",
                        homes[1..].iter().map(|h| format!("std.{h}")).collect::<Vec<_>>().join(", ")
                    )
                } else {
                    String::new()
                };
                d = d.with_note(format!(
                    "`{s}` is in std.{m}{others}: `import std.{m}` and write `{m}.{s}`, or `import std.{m}.{{{s}}}`"
                ));
            }
            // a near miss of a name in scope
            if let Some(near) = self.nearest_name(&s) {
                d = d.with_note(format!("did you mean `{near}`?"));
            }
        }
        self.cx.diags.push(d);
    }

    /// The name in scope (locals, then the module's names) closest to
    /// `s`, within one edit for short names and two for longer ones.
    fn nearest_name(&self, s: &str) -> Option<String> {
        let limit = if s.chars().count() <= 3 { 1 } else { 2 };
        let mut best: Option<(usize, String)> = None;
        let mut consider = |n: &str| {
            // a value is not mistaken for a type, or the other way round
            let upper = |x: &str| x.starts_with(|c: char| c.is_uppercase());
            if n == s || n.starts_with('%') || n.starts_with('_') || upper(n) != upper(s) {
                return;
            }
            let d = edit_distance(s, n);
            if d <= limit && best.as_ref().is_none_or(|(bd, bn)| d < *bd || (d == *bd && n < bn.as_str())) {
                best = Some((d, n.to_string()));
            }
        };
        for scope in &self.locals {
            for k in scope.keys() {
                consider(self.cx.interner.resolve(*k));
            }
        }
        for k in self.cx.scopes[self.module.into_raw().into_u32() as usize].names.keys() {
            consider(self.cx.interner.resolve(*k));
        }
        best.map(|b| b.1)
    }

    fn alloc(&mut self, kind: ExprKind, span: Span) -> ExprId {
        self.cx.hir.exprs.alloc(Expr { kind, span })
    }

    fn alloc_pat(&mut self, kind: PatKind, span: Span) -> PatId {
        self.cx.hir.pats.alloc(Pat { kind, span })
    }

    fn lookup_module_name(&self, name: Symbol) -> Option<Res> {
        self.cx.scopes[self.module.into_raw().into_u32() as usize].names.get(&name).map(|r| r.0)
    }

    // ------------------------------------------------------------ items

    /// Record the item's attributes and report any the compiler does not
    /// know (an error in standard-library sources, a warning elsewhere).
    fn record_attributes(&mut self, def: DefId, item: &ast::Item) {
        if item.attributes.is_empty() {
            return;
        }
        let is_std = self.cx.hir.modules[self.cx.hir.defs[def].module].is_std;
        for attr in &item.attributes {
            let name = self.cx.interner.resolve(attr.name).to_string();
            if !KNOWN_ATTRIBUTES.contains(&name.as_str()) {
                let msg = format!("unknown attribute `@{name}`");
                let mut d = Diagnostic::error(msg).with_code("E0191");
                if !is_std {
                    d = Diagnostic::warning(format!("unknown attribute `@{name}`")).with_code("E0191");
                }
                self.cx.diags.push(d.with_label(attr.span, "not recognised by the compiler"));
                continue;
            }
            // `@allow(<code>, ...)` silences those warnings inside this item
            if name == "allow" {
                for arg in &attr.args {
                    let code = match arg {
                        ast::AttrArg::Name(s) => Some(self.cx.interner.resolve(*s).to_string()),
                        ast::AttrArg::Str(s) => Some(s.clone()),
                        _ => None,
                    };
                    if let Some(code) = code {
                        self.cx.hir.allows.push((code, item.span));
                    }
                }
            }
        }
        self.cx.hir.attributes.entry(def).or_default().extend(item.attributes.iter().cloned());
    }

    fn lower_item(&mut self, def: DefId, item: &ast::Item) -> DefKind {
        self.cur_def = Some(def);
        self.record_attributes(def, item);
        match &item.kind {
            ast::ItemKind::Import(_) => unreachable!(),
            ast::ItemKind::TypeAlias { generics, ty, .. } => {
                let generics = self.push_generics(generics, Some(def));
                let ty = self.lower_ty(*ty);
                self.generics.pop();
                DefKind::Alias(AliasDef { generics, ty })
            }
            ast::ItemKind::ExternType { generics, .. } => {
                let generics = self.push_generics(generics, Some(def));
                self.generics.pop();
                DefKind::Opaque(OpaqueDef { generics })
            }
            ast::ItemKind::Struct(s) => {
                let generics = self.push_generics(&s.generics, Some(def));
                let mut seen = FxHashSet::default();
                for f in &s.fields {
                    if !seen.insert(f.name.sym) {
                        let n = self.sym(f.name.sym);
                        self.err(
                            "E0103",
                            f.name.span,
                            format!("duplicate field `{n}`"),
                            "declared again here",
                        );
                    }
                }
                let fields = s
                    .fields
                    .iter()
                    .map(|f| FieldDef {
                        name: f.name.sym,
                        ty: self.lower_ty(f.ty),
                        is_pub: f.is_pub,
                        span: f.name.span,
                    })
                    .collect();
                let derives = self.lower_derives(&s.derives);
                self.generics.pop();
                DefKind::Struct(StructDef { generics, fields, derives, repr_c: s.repr_c })
            }
            ast::ItemKind::Enum(e) => {
                let generics = self.push_generics(&e.generics, Some(def));
                let mut seen = FxHashSet::default();
                let mut variants = vec![];
                for v in &e.variants {
                    if !seen.insert(v.name.sym) {
                        let n = self.sym(v.name.sym);
                        self.err("E0111", v.name.span, format!("duplicate variant `{n}`"), "redefined here");
                    }
                    let fields =
                        v.fields.iter().map(|(l, t)| (l.map(|l| l.sym), self.lower_ty(*t))).collect();
                    variants.push(VariantDef { name: v.name.sym, fields, span: v.span });
                }
                let derives = self.lower_derives(&e.derives);
                self.generics.pop();
                DefKind::Enum(EnumDef { generics, variants, derives })
            }
            ast::ItemKind::Protocol(p) => {
                let generics = self.push_generics(&p.generics, Some(def));
                let supers = p.supers.iter().map(|t| self.lower_ty(*t)).collect();
                self.in_impl = true;
                let mut methods = vec![];
                for m in &p.methods {
                    let mdef = self.alloc_method_def(m, def, true);
                    methods.push(mdef);
                }
                self.in_impl = false;
                let assoc_types = p.assoc_types.iter().map(|(n, _)| n.sym).collect();
                self.generics.pop();
                DefKind::Protocol(ProtocolDef { generics, supers, methods, assoc_types })
            }
            ast::ItemKind::Impl(i) => {
                let generics = self.push_generics(&i.generics, Some(def));
                self.in_impl = true;
                let protocol = i.protocol.map(|t| self.lower_ty(t));
                let self_ty = self.lower_ty(i.self_ty);
                let where_preds = self.lower_where(&i.where_preds);
                let assoc_types =
                    i.assoc_types.iter().filter_map(|(n, t)| t.map(|t| (n.sym, self.lower_ty(t)))).collect();
                let mut methods = vec![];
                for m in &i.methods {
                    // inherent methods are private unless `pub`; protocol
                    // methods are as visible as the protocol
                    let mdef = self.alloc_method_def(m, def, protocol.is_some() || m.is_pub);
                    methods.push(mdef);
                }
                self.in_impl = false;
                self.generics.pop();
                DefKind::Impl(ImplDef { generics, protocol, self_ty, where_preds, methods, assoc_types })
            }
            ast::ItemKind::Fn(f) => DefKind::Fn(self.lower_fn(f, def, None, FnAbi::Lang)),
            ast::ItemKind::Extern { abi, decl } => {
                let mut words = abi.split_whitespace();
                let mut contract = crate::hir::FfiContract::default();
                let mut native = None;
                let abi = match words.next().unwrap_or("") {
                    "intrinsic" => FnAbi::Intrinsic,
                    "c" => {
                        for w in words.by_ref() {
                            match w {
                                "blocking" => contract.blocking = true,
                                "pinned" => contract.pinned = true,
                                "callbacks" => self.err(
                                    "E0115",
                                    decl.name.span,
                                    "C code cannot call back into the language",
                                    "callback reentry is not part of any supported adapter contract",
                                ),
                                other => self.err(
                                    "E0115",
                                    decl.name.span,
                                    format!("unknown foreign contract `{other}`"),
                                    "expected `blocking` or `pinned` after \"c\"",
                                ),
                            }
                        }
                        FnAbi::C
                    }
                    "native" => {
                        // `MODULE`, or `MODULE.FUNCTION` for a function the
                        // declaration names differently
                        let lib = words.next().unwrap_or("");
                        let ident = |x: &str| {
                            !x.is_empty() && x.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                        };
                        let valid = match lib.split_once('.') {
                            Some((m, f)) => ident(m) && ident(f),
                            None => ident(lib),
                        };
                        if !valid {
                            self.err(
                                "E0115",
                                decl.name.span,
                                "a native function names its module: `extern \"native NAME\"` or `extern \"native NAME.FUNCTION\"`",
                                "names are letters, digits, and `_`",
                            );
                        } else if let Some(w) = words.next() {
                            self.err(
                                "E0115",
                                decl.name.span,
                                format!("unexpected `{w}` after the native module's name"),
                                "the module declares how each of its functions runs",
                            );
                        }
                        native = Some(lib.to_string());
                        FnAbi::Native
                    }
                    other => {
                        self.err(
                            "E0115",
                            decl.name.span,
                            format!("unknown extern ABI `{other}`"),
                            "expected \"c\" or \"native NAME\"",
                        );
                        FnAbi::C
                    }
                };
                if abi == FnAbi::Intrinsic && !self.cx.hir.modules[self.module].is_std {
                    self.err(
                        "E0116",
                        decl.name.span,
                        "intrinsics are reserved for the standard library",
                        "here",
                    );
                }
                let mut f = self.lower_fn(decl, def, None, abi);
                f.ffi = contract;
                if abi == FnAbi::Native && !f.generics.is_empty() {
                    self.err(
                        "E0115",
                        decl.name.span,
                        "a native function cannot be generic",
                        "its argument and result types are fixed by the module",
                    );
                }
                f.native = native;
                DefKind::Fn(f)
            }
            ast::ItemKind::Global { kind, pat, ty, value } => {
                if *kind == ast::LetKind::Var {
                    let span = self.ast.pats[*pat].span;
                    self.err(
                        "E0110",
                        span,
                        "module-level values are immutable",
                        "use `let` or `const`; mutable state belongs in a `Cell`, `Atom`, or a function",
                    );
                }
                self.owners.push(OwnerFrame { owner: Owner::Global(def), captures: vec![] });
                self.locals.push(FxHashMap::default());
                let ty = ty.map(|t| self.lower_ty(t));
                // public values have explicit types (as public signatures do);
                // the standard library's own values are exempt
                let d = &self.cx.hir.defs[def];
                if d.is_pub
                    && !self.cx.hir.modules[d.module].is_std
                    && (ty.is_none() || ty.as_ref().is_some_and(contains_hole))
                {
                    let span = self.ast.pats[*pat].span;
                    let provider = matches!(&self.ast.exprs[*value].kind,
                        ast::ExprKind::Call { callee, .. } if matches!(self.ast.exprs[*callee].kind, ast::ExprKind::Std("system.provider")));
                    if provider {
                        self.err(
                            "E0118",
                            span,
                            "a public provider declares its configuration type and `-> Result<Output, Error>`",
                            "e.g. `pub provider store(scope, config: StoreConfig) -> Result<Store, AppError>`",
                        );
                    } else {
                        self.err("E0118", span, "public values must declare their type", "add `: Type`");
                    }
                }
                let value = self.lower_expr(*value);
                self.locals.pop();
                self.owners.pop();
                let (name, span) = match &self.ast.pats[*pat].kind {
                    ast::PatKind::Bind(n) => (n.sym, n.span),
                    _ => (self.intern("_"), self.ast.pats[*pat].span),
                };
                let local = self.cx.hir.locals.alloc(Local {
                    name,
                    span,
                    mutable: false,
                    owner: Some(Owner::Global(def)),
                });
                let pat = self.alloc_pat(PatKind::Bind(local), span);
                if *kind == ast::LetKind::Const {
                    self.cx.const_checks.push(value);
                }
                DefKind::Global(GlobalDef { pat, ty, value, is_const: *kind == ast::LetKind::Const, local })
            }
        }
    }

    fn alloc_method_def(&mut self, m: &ast::FnDecl, owner: DefId, is_pub: bool) -> DefId {
        let def = self.cx.hir.defs.alloc(Def {
            name: m.name.sym,
            module: self.module,
            span: m.name.span,
            is_pub,
            kind: DefKind::Pending,
        });
        let saved = self.cur_def;
        self.cur_def = Some(def);
        let f = self.lower_fn(m, def, Some(owner), FnAbi::Lang);
        self.cur_def = saved;
        self.cx.hir.defs[def].kind = DefKind::Fn(f);
        def
    }

    fn lower_derives(&mut self, ds: &[ast::TypeId]) -> Vec<Derive> {
        let mut out = vec![];
        for &t in ds {
            let h = self.lower_ty(t);
            match h.kind {
                HTyKind::Named { def, .. }
                    if matches!(self.cx.hir.defs[def].kind, DefKind::Protocol(_) | DefKind::Pending) =>
                {
                    out.push(Derive { protocol: def, span: h.span })
                }
                HTyKind::Error => {}
                _ => self.err("E0117", h.span, "only protocols can be derived", "not a protocol"),
            }
        }
        out
    }

    fn push_generics(&mut self, gs: &[ast::GenericParam], owner: Option<DefId>) -> Vec<GenericId> {
        let mut frame = FxHashMap::default();
        let mut ids = vec![];
        for g in gs {
            let id = self.cx.hir.generics.alloc(GenericParam {
                name: g.name.sym,
                span: g.name.span,
                bounds: vec![],
                owner,
            });
            frame.insert(g.name.sym, id);
            ids.push(id);
        }
        self.generics.push(frame);
        for (g, &id) in gs.iter().zip(&ids) {
            let bounds = g.bounds.iter().map(|b| self.lower_ty(*b)).collect();
            self.cx.hir.generics[id].bounds = bounds;
        }
        ids
    }

    fn lower_where(&mut self, ws: &[ast::WherePred]) -> Vec<WherePred> {
        ws.iter()
            .map(|w| WherePred {
                ty: self.lower_ty(w.ty),
                bounds: w.bounds.iter().map(|b| self.lower_ty(*b)).collect(),
            })
            .collect()
    }

    fn lower_fn(&mut self, f: &ast::FnDecl, def: DefId, owner: Option<DefId>, abi: FnAbi) -> FnDef {
        let generics = self.push_generics(&f.generics, Some(def));
        let where_preds = self.lower_where(&f.where_preds);
        self.owners.push(OwnerFrame { owner: Owner::Fn(def), captures: vec![] });
        self.locals.push(FxHashMap::default());
        let mut params = vec![];
        let mut has_self = false;
        let pats: Vec<ast::PatId> = f.params.iter().map(|p| p.pat).collect();
        self.check_bound_once(&pats);
        self.in_params = true;
        for (i, p) in f.params.iter().enumerate() {
            if i == 0
                && let ast::PatKind::Bind(n) = &self.ast.pats[p.pat].kind
            {
                has_self = self.cx.interner.resolve(n.sym) == "self";
            }
            let ty = p.ty.map(|t| self.lower_ty(t));
            let pat = self.lower_pat(p.pat);
            params.push(Param { pat, ty });
        }
        self.in_params = false;
        let ret = f.ret.map(|t| self.lower_ty(t));
        let effect = f.effect.as_ref().map(|e| self.lower_effect(e));
        let mut body = f.body.map(|b| self.lower_expr(b));
        // contracts: `requires` become checks at the start of the body;
        // `ensures` read `result`, bound for them alone
        let fname = self.sym(f.name.sym).to_string();
        let mut checks = vec![];
        let mut ensures = vec![];
        let mut requires = vec![];
        let mut result = None;
        let mut caller_local = None;
        let mut contracts = vec![];
        if f.body.is_none() && !f.contracts.is_empty() {
            self.err(
                "E0140",
                f.contracts[0].span,
                "contracts need a function body",
                "put them on each implementation's method",
            );
        }
        // preconditions first: `result` is bound for postconditions only
        let mut ordered: Vec<&ast::Contract> = f.contracts.iter().filter(|c| !c.ensures).collect();
        ordered.extend(f.contracts.iter().filter(|c| c.ensures));
        self.in_contract = true;
        contracts.extend(f.contracts.iter().filter(|_| f.body.is_some()).map(|c| c.text.clone()));
        for c in ordered {
            if f.body.is_none() {
                break;
            }
            let cond_text = &c.cond_text;
            let what = if c.ensures { "postcondition" } else { "precondition" };
            let mut message = format!("{what} of `{fname}` failed: {cond_text}");
            if let Some(m) = &c.message {
                message = format!("{message} ({m})");
            }
            if c.ensures {
                let r = *result.get_or_insert_with(|| {
                    let r = self.new_local("result", c.span);
                    let sym = self.intern("result");
                    self.locals.last_mut().unwrap().insert(sym, r);
                    r
                });
                let _ = r;
                let cond = self.lower_expr(c.cond);
                ensures.push(Postcondition { cond, message, span: c.span });
            } else {
                // the call site, recorded by the caller (read before
                // anything else can record another)
                let caller = *caller_local.get_or_insert_with(|| {
                    let l = self.new_local("$caller", c.span);
                    let site = self.call_lang("contract_caller", vec![], c.span);
                    let pat = self.alloc_pat(PatKind::Bind(l), c.span);
                    checks.push(Stmt::Let { pat, ty: None, init: site, span: c.span });
                    l
                });
                // `if !(cond) { precondition_failed(message, caller) }`
                let cond = self.lower_expr(c.cond);
                let not = self.alloc(ExprKind::Unary { op: UnOp::Not, operand: cond }, c.span);
                let msg = self.alloc(ExprKind::Lit(Lit::Str(message)), c.span);
                let at = self.alloc(ExprKind::Local(caller), c.span);
                let fail = self.call_lang("precondition_failed", vec![msg, at], c.span);
                let then = self.alloc(
                    ExprKind::Block { stmts: vec![Stmt::Expr { expr: fail, semi: true }], tail: None },
                    c.span,
                );
                let check = self.alloc(ExprKind::If { cond: not, then, els: None }, c.span);
                requires.push(Precondition { cond, check, text: cond_text.clone(), span: c.span });
                checks.push(Stmt::Expr { expr: check, semi: true });
            }
        }
        self.in_contract = false;
        if !checks.is_empty()
            && let Some(b) = body
        {
            let span = self.cx.hir.exprs[b].span;
            body = Some(self.alloc(ExprKind::Block { stmts: checks, tail: Some(b) }, span));
        }
        self.locals.pop();
        self.owners.pop();
        self.generics.pop();
        if f.is_pub && abi == FnAbi::Lang && owner.is_none() {
            // Public functions declare their argument and result types.
            for (p, ap) in params.iter().zip(&f.params) {
                if p.ty.is_none() || p.ty.as_ref().is_some_and(contains_hole) {
                    let span = self.ast.pats[ap.pat].span;
                    self.err(
                        "E0118",
                        span,
                        "public functions must declare parameter types",
                        "add a type annotation",
                    );
                }
            }
            // a `stage` factory's `pipeline.Stage` type follows from its callbacks
            if ret.is_none() && f.body.is_some() && !f.result_inferred {
                let n = self.sym(f.name.sym);
                if n != "main" {
                    self.err(
                        "E0118",
                        f.name.span,
                        "public functions must declare a result type",
                        "add `-> Type`",
                    );
                }
            } else if ret.as_ref().is_some_and(contains_hole) {
                self.err(
                    "E0118",
                    ret.as_ref().unwrap().span,
                    "public signatures cannot contain `_`",
                    "name the type",
                );
            }
        }
        FnDef {
            generics,
            params,
            ret,
            effect,
            where_preds,
            body,
            abi,
            owner,
            has_self,
            ffi: Default::default(),
            native: None,
            requires,
            ensures,
            result,
            contracts,
        }
    }

    fn lower_effect(&mut self, e: &ast::Effect) -> HEffect {
        match e {
            ast::Effect::Set(ts, span) => {
                let mut labels = vec![];
                let mut rest = None;
                for &t in ts {
                    match &self.ast.types[t].kind {
                        // an effect variable extends the row: `!{Io, Eff}`
                        ast::TypeKind::Path { module, name, args }
                            if module.is_empty()
                                && args.is_empty()
                                && self.find_generic(name.sym).is_some() =>
                        {
                            if rest.is_some() {
                                let span = self.ast.types[t].span;
                                self.err(
                                    "E0119",
                                    span,
                                    "an effect row has at most one effect variable",
                                    "keep one variable and list the labels",
                                );
                            }
                            rest = self.find_generic(name.sym);
                        }
                        ast::TypeKind::Path { module, name, args }
                            if module.is_empty() && args.is_empty() =>
                        {
                            labels.push(name.sym)
                        }
                        _ => {
                            let span = self.ast.types[t].span;
                            self.err("E0119", span, "expected an effect name", "such as `Io` or `Wait`");
                        }
                    }
                }
                HEffect::Set { labels, rest, span: *span }
            }
            ast::Effect::Var(n) => match self.find_generic(n.sym) {
                Some(g) => HEffect::Var(g),
                None => {
                    let s = self.sym(n.sym);
                    self.err(
                        "E0120",
                        n.span,
                        format!("unknown effect variable `{s}`"),
                        "declare it as a generic parameter",
                    );
                    HEffect::Set { labels: vec![], rest: None, span: n.span }
                }
            },
        }
    }

    fn find_generic(&self, s: Symbol) -> Option<GenericId> {
        self.generics.iter().rev().find_map(|f| f.get(&s).copied())
    }

    // ------------------------------------------------------------ types

    fn lower_ty(&mut self, t: ast::TypeId) -> HTy {
        let te = &self.ast.types[t];
        let span = te.span;
        let kind = match &te.kind {
            ast::TypeKind::Hole => HTyKind::Hole,
            // only in a tree recovered from syntax errors
            ast::TypeKind::Error => HTyKind::Error,
            ast::TypeKind::Unit => HTyKind::Unit,
            ast::TypeKind::Tuple(ts) => HTyKind::Tuple(ts.iter().map(|t| self.lower_ty(*t)).collect()),
            ast::TypeKind::Record { fields, rest } => {
                let fields = fields.iter().map(|(l, t)| (l.sym, self.lower_ty(*t))).collect();
                let rest = rest.and_then(|r| match self.find_generic(r.sym) {
                    Some(g) => Some(g),
                    None => {
                        let s = self.sym(r.sym);
                        self.err(
                            "E0120",
                            r.span,
                            format!("unknown row variable `{s}`"),
                            format!("declare `{s}` as a generic parameter"),
                        );
                        None
                    }
                });
                HTyKind::Record { fields, rest }
            }
            ast::TypeKind::Fn { once, params, ret, effect } => HTyKind::Fn {
                once: *once,
                params: params.iter().map(|t| self.lower_ty(*t)).collect(),
                ret: Box::new(self.lower_ty(*ret)),
                effect: effect.as_ref().map(|e| self.lower_effect(e)),
            },
            ast::TypeKind::Forall { generics, body } => {
                let params = self.push_generics(generics, self.cur_def);
                let body = self.lower_ty(*body);
                self.generics.pop();
                HTyKind::Forall { params, body: Box::new(body) }
            }
            ast::TypeKind::Ref { mutable, inner } => {
                HTyKind::Ref { mutable: *mutable, inner: Box::new(self.lower_ty(*inner)) }
            }
            ast::TypeKind::Effect(e) => HTyKind::Effect(self.lower_effect(e)),
            ast::TypeKind::Dyn { module, name, args } => {
                let args: Vec<HTy> = args.iter().map(|t| self.lower_ty(*t)).collect();
                match self.resolve_type_path(module, *name) {
                    Some(TypeRes::Def(d)) => HTyKind::Dyn { protocol: d, args },
                    _ => HTyKind::Error,
                }
            }
            ast::TypeKind::Path { module, name, args }
                if module.len() == 1
                    && args.is_empty()
                    && (self.find_generic(module[0].sym).is_some()
                        || (self.sym(module[0].sym) == "Self" && self.in_impl)) =>
            {
                // associated type of a generic parameter or `Self`
                let base = match self.find_generic(module[0].sym) {
                    Some(g) => HTyKind::Generic(g),
                    None => HTyKind::SelfTy,
                };
                HTyKind::Assoc { base: Box::new(HTy { kind: base, span: module[0].span }), name: name.sym }
            }
            // a standard-library type a desugared declaration names
            ast::TypeKind::Std { path, args } => {
                let args: Vec<HTy> = args.iter().map(|t| self.lower_ty(*t)).collect();
                match self.cx.hir.try_lang(path) {
                    Some(def) => HTyKind::Named { def, args },
                    None => {
                        self.err(
                            "E0190",
                            span,
                            format!("missing standard library item `{path}`"),
                            "required here",
                        );
                        HTyKind::Error
                    }
                }
            }
            ast::TypeKind::Path { module, name, args } => {
                let args: Vec<HTy> = args.iter().map(|t| self.lower_ty(*t)).collect();
                match self.resolve_type_path(module, *name) {
                    Some(TypeRes::Def(def)) => HTyKind::Named { def, args },
                    Some(TypeRes::Generic(g)) => {
                        if !args.is_empty() {
                            self.err(
                                "E0121",
                                span,
                                "type parameters cannot take arguments",
                                "higher-kinded types are not supported",
                            );
                        }
                        HTyKind::Generic(g)
                    }
                    Some(TypeRes::SelfTy) => HTyKind::SelfTy,
                    Some(TypeRes::Effect(s)) => HTyKind::EffectLabel(s),
                    None => HTyKind::Error,
                }
            }
        };
        HTy { kind, span }
    }

    fn resolve_type_path(&mut self, module: &[ast::Name], name: ast::Name) -> Option<TypeRes> {
        let text = self.sym(name.sym);
        if module.is_empty() {
            if let Some(g) = self.find_generic(name.sym) {
                return Some(TypeRes::Generic(g));
            }
            if text == "Self" && self.in_impl {
                return Some(TypeRes::SelfTy);
            }
            if matches!(text.as_str(), "Io" | "Wait" | "State" | "Foreign") {
                return Some(TypeRes::Effect(name.sym));
            }
            match self.lookup_module_name(name.sym) {
                Some(Res::Def(d)) => return Some(TypeRes::Def(d)),
                _ => {
                    self.err("E0122", name.span, format!("unknown type `{text}`"), "not found in this scope");
                    return None;
                }
            }
        }
        let m = self.resolve_module_path(module)?;
        self.module_item(m, name).map(TypeRes::Def)
    }

    fn resolve_module_path(&mut self, path: &[ast::Name]) -> Option<ModuleId> {
        match self.lookup_module_name(path[0].sym) {
            Some(Res::Module(m)) if path.len() == 1 => Some(m),
            _ => {
                // full path, e.g. std.io
                let syms: Vec<Symbol> = path.iter().map(|n| n.sym).collect();
                match self.cx.find_module(&syms) {
                    Some(m) => Some(m),
                    None => {
                        let s = self.cx.path_str(&syms);
                        let span = path[0].span.to(path.last().unwrap().span);
                        self.err("E0114", span, format!("unknown module `{s}`"), "import it first");
                        None
                    }
                }
            }
        }
    }

    fn module_item(&mut self, m: ModuleId, name: ast::Name) -> Option<DefId> {
        match self.cx.hir.modules[m].items.get(&name.sym).copied() {
            Some(d) => {
                if !self.cx.hir.defs[d].is_pub && m != self.module {
                    let s = self.sym(name.sym);
                    self.err("E0112", name.span, format!("`{s}` is private"), "not marked `pub`");
                }
                Some(d)
            }
            None => {
                let s = self.sym(name.sym);
                let mp = self.cx.path_str(&self.cx.hir.modules[m].path.clone());
                self.err("E0113", name.span, format!("module `{mp}` has no item `{s}`"), "not found");
                None
            }
        }
    }

    // ------------------------------------------------------------ locals

    fn bind_local(&mut self, name: ast::Name, mutable: bool) -> LocalId {
        let owner = self.owners.last().map(|o| o.owner);
        let id = self.cx.hir.locals.alloc(Local { name: name.sym, span: name.span, mutable, owner });
        self.locals.last_mut().unwrap().insert(name.sym, id);
        id
    }

    fn lookup_local(&mut self, name: Symbol) -> Option<LocalId> {
        let id = self.locals.iter().rev().find_map(|s| s.get(&name).copied())?;
        // record captures in every lambda between the use and the binding
        let owner = self.cx.hir.locals[id].owner;
        for frame in self.owners.iter_mut().rev() {
            if Some(frame.owner) == owner {
                break;
            }
            if matches!(frame.owner, Owner::Lambda(_)) && !frame.captures.contains(&id) {
                frame.captures.push(id);
            }
        }
        Some(id)
    }

    // ------------------------------------------------------------ expressions

    fn lower_expr(&mut self, e: ast::ExprId) -> ExprId {
        let expr = &self.ast.exprs[e];
        let span = expr.span;
        let kind = match &expr.kind {
            ast::ExprKind::Lit(l) => ExprKind::Lit(self.lower_lit(l)),
            ast::ExprKind::Error => ExprKind::Error,
            ast::ExprKind::Std(path) => return self.lang_def(path, span),
            // `#module/name`: the declaration's qualified name
            ast::ExprKind::DeclName(n) => {
                let module: Vec<String> =
                    self.cx.hir.modules[self.module].path.iter().map(|p| self.sym(*p)).collect();
                let text = format!("{}/{}", module.join("."), self.sym(n.sym));
                ExprKind::Lit(Lit::Symbol(self.intern(&text)))
            }
            ast::ExprKind::Interp(parts) => ExprKind::Interp(
                parts
                    .iter()
                    .map(|p| match p {
                        ast::InterpPart::Text(t) => InterpPart::Text(t.clone()),
                        ast::InterpPart::Expr(e) => InterpPart::Expr(self.lower_expr(*e)),
                    })
                    .collect(),
            ),
            ast::ExprKind::Name(n) => return self.lower_name(*n, span),
            ast::ExprKind::TypeName(n) => {
                let s = self.sym(n.sym);
                self.err(
                    "E0123",
                    n.span,
                    format!("type `{s}` is not a value"),
                    "use `Type.member` to access constructors",
                );
                ExprKind::Error
            }
            ast::ExprKind::Field { base, name } => return self.lower_field(*base, *name, span, &[]),
            ast::ExprKind::Index { base, index } => {
                ExprKind::Index { base: self.lower_expr(*base), index: self.lower_expr(*index) }
            }
            ast::ExprKind::Call { callee, type_args, args } => {
                if let Some(stage) = self.cx.hir.try_lang("pipeline.stage")
                    && args.len() == 1
                    && self.resolves_to(*callee, stage)
                {
                    return self.lower_stage(args[0], span);
                }
                if let Some(select) = self.cx.hir.try_lang("system.select")
                    && args.len() == 2
                    && self.resolves_to(*callee, select)
                {
                    return self.lower_select(args[0], args[1], span);
                }
                if let Some(replace) = self.cx.hir.try_lang("system.replace")
                    && args.len() == 3
                    && self.resolves_to(*callee, replace)
                    && let ast::ExprKind::Lit(ast::Lit::Symbol(key)) = &self.ast.exprs[args[1]].kind
                {
                    // `pick` checks the replacement's output against the key's
                    let (key, kspan) = (*key, self.ast.exprs[args[1]].span);
                    let plan = self.lower_expr(args[0]);
                    let key_e = self.lower_expr(args[1]);
                    let provider = self.lower_expr(args[2]);
                    let pick = self.synth_runner(kspan, vec![], |this, c| {
                        let base = this.alloc(ExprKind::Local(c), kspan);
                        this.alloc(ExprKind::Field { base, label: key, label_span: kspan }, kspan)
                    });
                    // and `pick_config` its configuration against the key's
                    let pick_config = self.synth_runner(kspan, vec![], |this, c| {
                        let base = this.alloc(ExprKind::Local(c), kspan);
                        this.alloc(ExprKind::Field { base, label: key, label_span: kspan }, kspan)
                    });
                    return self.call_lang(
                        "system.replace_node",
                        vec![plan, key_e, provider, pick, pick_config],
                        span,
                    );
                }
                return self.lower_call(*callee, type_args, args, span, None);
            }
            ast::ExprKind::Try(inner) => {
                if self.in_contract && matches!(self.owners.last().map(|o| o.owner), Some(Owner::Fn(_))) {
                    self.err(
                        "E0140",
                        span,
                        "a contract clause cannot propagate an error with `?`",
                        "inside this clause",
                    );
                }
                ExprKind::Try(self.lower_expr(*inner))
            }
            // `-<literal>` is one literal, so `-128I8` and `i64` minimums fit
            ast::ExprKind::Unary { op: ast::UnOp::Neg, operand }
                if matches!(
                    self.ast.exprs[*operand].kind,
                    ast::ExprKind::Lit(ast::Lit::Int { .. } | ast::Lit::Float { .. })
                ) =>
            {
                let ast::ExprKind::Lit(l) = &self.ast.exprs[*operand].kind else { unreachable!() };
                match self.lower_lit(l) {
                    Lit::Float { value, suffix } => ExprKind::Lit(Lit::Float { value: -value, suffix }),
                    Lit::Int { value, suffix } => {
                        ExprKind::Lit(Lit::Int { value: (value as i128).wrapping_neg() as u128, suffix })
                    }
                    other => ExprKind::Lit(other),
                }
            }
            ast::ExprKind::Unary { op, operand } => {
                ExprKind::Unary { op: *op, operand: self.lower_expr(*operand) }
            }
            ast::ExprKind::Binary { op: ast::BinOp::Pipe, lhs, rhs } => {
                return self.lower_pipe(*lhs, *rhs, span);
            }
            ast::ExprKind::Binary { op, lhs, rhs } => {
                ExprKind::Binary { op: *op, lhs: self.lower_expr(*lhs), rhs: self.lower_expr(*rhs) }
            }
            ast::ExprKind::Tuple(items) => {
                ExprKind::Tuple(items.iter().map(|e| self.lower_expr(*e)).collect())
            }
            ast::ExprKind::List(items) => ExprKind::List(items.iter().map(|e| self.lower_expr(*e)).collect()),
            ast::ExprKind::Record { spread, entries } => return self.lower_record(*spread, entries, span),
            ast::ExprKind::Map(entries) => ExprKind::Map(
                entries.iter().map(|(k, v)| (self.lower_expr(*k), self.lower_expr(*v))).collect(),
            ),
            ast::ExprKind::Set(items) => ExprKind::Set(items.iter().map(|e| self.lower_expr(*e)).collect()),
            ast::ExprKind::Lambda(l) => {
                let l = l.clone();
                return self.lower_lambda(
                    &l.params,
                    l.ret,
                    l.effect.as_ref(),
                    l.once,
                    LambdaBody::Expr(l.body),
                    span,
                );
            }
            ast::ExprKind::If { cond, then, els } => {
                let cond = self.lower_expr(*cond);
                let then = self.lower_expr(*then);
                let els = els.map(|e| self.lower_expr(e));
                ExprKind::If { cond, then, els }
            }
            ast::ExprKind::Match { scrutinee, arms } => {
                let scrutinee = self.lower_expr(*scrutinee);
                let arms = arms
                    .iter()
                    .map(|a| {
                        self.locals.push(FxHashMap::default());
                        let pat = self.lower_pat(a.pat);
                        let guard = a.guard.map(|g| self.lower_expr(g));
                        let body = self.lower_expr(a.body);
                        self.locals.pop();
                        Arm { pat, guard, body, span: a.span }
                    })
                    .collect();
                ExprKind::Match { scrutinee, arms }
            }
            ast::ExprKind::While { cond, body } => {
                let id = self.alloc(ExprKind::Error, span);
                let cond = self.lower_expr(*cond);
                self.loops.push(id);
                let body = self.lower_expr(*body);
                self.loops.pop();
                self.cx.hir.exprs[id].kind = ExprKind::While { cond, body };
                return id;
            }
            ast::ExprKind::For { pat, iter, body } => {
                let id = self.alloc(ExprKind::Error, span);
                let iter = self.lower_expr(*iter);
                self.locals.push(FxHashMap::default());
                let pat = self.lower_pat(*pat);
                self.loops.push(id);
                let body = self.lower_expr(*body);
                self.loops.pop();
                self.locals.pop();
                self.cx.hir.exprs[id].kind = ExprKind::For { pat, iter, body };
                return id;
            }
            ast::ExprKind::Loop { body } => {
                let id = self.alloc(ExprKind::Error, span);
                self.loops.push(id);
                let body = self.lower_expr(*body);
                self.loops.pop();
                self.cx.hir.exprs[id].kind = ExprKind::Loop { body };
                return id;
            }
            ast::ExprKind::Block { stmts } => {
                self.locals.push(FxHashMap::default());
                let (stmts, tail) = self.lower_stmts(stmts);
                self.locals.pop();
                ExprKind::Block { stmts, tail }
            }
            ast::ExprKind::Return(v) => {
                if self.in_contract && matches!(self.owners.last().map(|o| o.owner), Some(Owner::Fn(_))) {
                    self.err("E0140", span, "a contract clause cannot `return`", "inside this clause");
                }
                let value = v.map(|v| self.lower_expr(v));
                let target = self.owners.last().map(|o| o.owner);
                match target {
                    Some(t @ (Owner::Fn(_) | Owner::Lambda(_))) => ExprKind::Return { value, target: t },
                    _ => {
                        self.err(
                            "E0124",
                            span,
                            "`return` outside of a function",
                            "not inside a function body",
                        );
                        ExprKind::Error
                    }
                }
            }
            ast::ExprKind::Break(v) => {
                let value = v.map(|v| self.lower_expr(v));
                match self.loops.last() {
                    Some(&target) => ExprKind::Break { value, target },
                    None => {
                        self.err(
                            "E0125",
                            span,
                            "`break` outside of a loop",
                            "not inside `loop`, `while`, or `for`",
                        );
                        ExprKind::Error
                    }
                }
            }
            ast::ExprKind::Continue => match self.loops.last() {
                Some(&target) => ExprKind::Continue { target },
                None => {
                    self.err("E0125", span, "`continue` outside of a loop", "not inside a loop");
                    ExprKind::Error
                }
            },
            ast::ExprKind::System(entries) => return self.lower_system(entries, span),
            ast::ExprKind::Pipeline(stages) => return self.lower_pipeline(stages, span),
            ast::ExprKind::Ref(label) => match &self.system {
                Some(nodes) => match nodes.get(&label.sym).copied() {
                    Some(node) => {
                        let outputs = self.system_outputs.unwrap();
                        self.lookup_local_raw(outputs);
                        self.lookup_local_raw(node);
                        let f = self.lang_def("system.output", span);
                        let a = self.alloc(ExprKind::Local(outputs), span);
                        let b = self.alloc(ExprKind::Local(node), span);
                        ExprKind::Call { callee: f, args: vec![a, b] }
                    }
                    None => ExprKind::Error,
                },
                None => {
                    self.err(
                        "E0602",
                        span,
                        "`ref` is only valid inside a system configuration",
                        "outside `system { ... }`",
                    );
                    ExprKind::Error
                }
            },
            ast::ExprKind::Unsafe(b) => ExprKind::Unsafe(self.lower_expr(*b)),
        };
        self.alloc(kind, span)
    }

    /// Record a capture of a synthetic local from nested lambdas.
    fn lookup_local_raw(&mut self, id: LocalId) {
        let owner = self.cx.hir.locals[id].owner;
        for frame in self.owners.iter_mut().rev() {
            if Some(frame.owner) == owner {
                break;
            }
            if matches!(frame.owner, Owner::Lambda(_)) && !frame.captures.contains(&id) {
                frame.captures.push(id);
            }
        }
    }

    fn lower_lit(&mut self, l: &ast::Lit) -> Lit {
        match l {
            ast::Lit::Int { value, suffix } => Lit::Int { value: *value, suffix: *suffix },
            ast::Lit::Float { value, suffix } => Lit::Float { value: *value, suffix: *suffix },
            ast::Lit::Duration { value, unit } => Lit::Duration(value * unit.nanos()),
            ast::Lit::Str(s) => Lit::Str(s.clone()),
            ast::Lit::Char(c) => Lit::Char(*c),
            ast::Lit::Symbol(s) => Lit::Symbol(*s),
            ast::Lit::Bool(b) => Lit::Bool(*b),
            ast::Lit::Unit => Lit::Unit,
        }
    }

    fn lower_name(&mut self, n: ast::Name, span: Span) -> ExprId {
        if let Some(l) = self.lookup_local(n.sym) {
            return self.alloc(ExprKind::Local(l), span);
        }
        let kind = match self.lookup_module_name(n.sym) {
            Some(Res::Def(d)) => self.def_value(d, n, vec![]),
            Some(Res::Module(_)) => {
                let s = self.sym(n.sym);
                self.err(
                    "E0126",
                    n.span,
                    format!("module `{s}` is not a value"),
                    "access one of its members",
                );
                ExprKind::Error
            }
            None => {
                self.unknown_name(n.sym, n.span);
                ExprKind::Error
            }
        };
        self.alloc(kind, span)
    }

    fn def_value(&mut self, d: DefId, n: ast::Name, type_args: Vec<HTy>) -> ExprKind {
        match &self.cx.hir.defs[d].kind {
            DefKind::Struct(_)
            | DefKind::Enum(_)
            | DefKind::Alias(_)
            | DefKind::Opaque(_)
            | DefKind::Protocol(_) => {
                let s = self.sym(n.sym);
                self.err("E0123", n.span, format!("type `{s}` is not a value"), "types are not values");
                ExprKind::Error
            }
            _ => ExprKind::Def { def: d, type_args },
        }
    }

    /// `base.name` where base may be a module or type namespace.
    fn lower_field(
        &mut self,
        base: ast::ExprId,
        name: ast::Name,
        span: Span,
        type_args: &[ast::TypeId],
    ) -> ExprId {
        let targs: Vec<HTy> = type_args.iter().map(|t| self.lower_ty(*t)).collect();
        match self.namespace_of(base) {
            Some(Ns::Module(m)) => {
                let kind = match self.module_item(m, name) {
                    Some(d) => self.def_value(d, name, targs),
                    None => ExprKind::Error,
                };
                self.alloc(kind, span)
            }
            Some(Ns::Type(ty)) => {
                let kind = self.type_member(ty, name, targs);
                self.alloc(kind, span)
            }
            None => {
                if !targs.is_empty() {
                    self.err("E0128", span, "field access cannot take type arguments", "remove `<...>`");
                }
                let b = self.lower_expr(base);
                self.alloc(ExprKind::Field { base: b, label: name.sym, label_span: name.span }, span)
            }
        }
    }

    fn type_member(&mut self, ty: HTy, name: ast::Name, type_args: Vec<HTy>) -> ExprKind {
        if let HTyKind::Named { def, .. } = &ty.kind
            && let DefKind::Enum(e) = &self.cx.hir.defs[*def].kind
            && let Some(i) = e.variants.iter().position(|v| v.name == name.sym)
        {
            let targs = if type_args.is_empty() {
                match &ty.kind {
                    HTyKind::Named { args, .. } => args.clone(),
                    _ => vec![],
                }
            } else {
                type_args
            };
            return ExprKind::Ctor { enum_def: *def, variant: i as u32, type_args: targs };
        }
        ExprKind::AssocFn { ty, name: name.sym, name_span: name.span, type_args }
    }

    /// Classify an expression used as the base of `.member` as a module or
    /// type namespace, if it is one.
    fn namespace_of(&mut self, base: ast::ExprId) -> Option<Ns> {
        match &self.ast.exprs[base].kind {
            ast::ExprKind::Name(n) => {
                if self.locals.iter().rev().any(|s| s.contains_key(&n.sym)) {
                    return None;
                }
                match self.lookup_module_name(n.sym) {
                    Some(Res::Module(m)) => Some(Ns::Module(m)),
                    _ => None,
                }
            }
            ast::ExprKind::TypeName(n) => {
                let span = self.ast.exprs[base].span;
                let n = *n;
                let res = self.resolve_type_path(&[], n)?;
                let kind = match res {
                    TypeRes::Def(d) => HTyKind::Named { def: d, args: vec![] },
                    TypeRes::Generic(g) => HTyKind::Generic(g),
                    TypeRes::SelfTy => HTyKind::SelfTy,
                    TypeRes::Effect(_) => HTyKind::Error,
                };
                Some(Ns::Type(HTy { kind, span }))
            }
            ast::ExprKind::Field { base: inner, name } => {
                // module.Type namespaces, e.g. `channel.Mode.fast`
                let Some(Ns::Module(m)) = self.namespace_of(*inner) else { return None };
                let text = self.sym(name.sym);
                if text.starts_with(|c: char| c.is_ascii_uppercase()) {
                    let d = self.module_item(m, *name)?;
                    let span = self.ast.exprs[base].span;
                    Some(Ns::Type(HTy { kind: HTyKind::Named { def: d, args: vec![] }, span }))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn lower_call(
        &mut self,
        callee: ast::ExprId,
        type_args: &[ast::TypeId],
        args: &[ast::ExprId],
        span: Span,
        extra_first: Option<ExprId>,
    ) -> ExprId {
        self.lower_call_with(callee, type_args, span, |this| {
            let mut out: Vec<ExprId> = extra_first.into_iter().collect();
            out.extend(args.iter().map(|a| this.lower_expr(*a)));
            out
        })
    }

    fn lower_call_with(
        &mut self,
        callee: ast::ExprId,
        type_args: &[ast::TypeId],
        span: Span,
        args: impl FnOnce(&mut Self) -> Vec<ExprId>,
    ) -> ExprId {
        // method call: value.method(args)
        if let ast::ExprKind::Field { base, name } = &self.ast.exprs[callee].kind {
            let (base, name) = (*base, *name);
            if self.namespace_of(base).is_none() {
                let recv = self.lower_expr(base);
                let targs = type_args.iter().map(|t| self.lower_ty(*t)).collect();
                let args = args(self);
                return self.alloc(
                    ExprKind::MethodCall {
                        recv,
                        method: name.sym,
                        method_span: name.span,
                        type_args: targs,
                        args,
                    },
                    span,
                );
            }
            let f = self.lower_field(base, name, self.ast.exprs[callee].span, type_args);
            let args = args(self);
            return self.alloc(ExprKind::Call { callee: f, args }, span);
        }
        let f = if type_args.is_empty() {
            self.lower_expr(callee)
        } else {
            match &self.ast.exprs[callee].kind {
                ast::ExprKind::Name(n) if self.lookup_local(n.sym).is_none() => {
                    let n = *n;
                    let targs = type_args.iter().map(|t| self.lower_ty(*t)).collect();
                    let kind = match self.lookup_module_name(n.sym) {
                        Some(Res::Def(d)) => self.def_value(d, n, targs),
                        _ => {
                            self.unknown_name(n.sym, n.span);
                            ExprKind::Error
                        }
                    };
                    self.alloc(kind, self.ast.exprs[callee].span)
                }
                _ => {
                    let cs = self.ast.exprs[callee].span;
                    self.err("E0128", cs, "explicit type arguments require a named function", "here");
                    self.lower_expr(callee)
                }
            }
        };
        let args = args(self);
        self.alloc(ExprKind::Call { callee: f, args }, span)
    }

    fn lower_pipe(&mut self, lhs: ast::ExprId, rhs: ast::ExprId, span: Span) -> ExprId {
        let l = self.lower_expr(lhs);
        match &self.ast.exprs[rhs].kind {
            ast::ExprKind::Call { callee, type_args, args } => {
                let (callee, type_args, args) = (*callee, type_args.clone(), args.clone());
                self.lower_call(callee, &type_args, &args, span, Some(l))
            }
            _ => {
                let f = self.lower_expr(rhs);
                self.alloc(ExprKind::Call { callee: f, args: vec![l] }, span)
            }
        }
    }

    fn lower_record(
        &mut self,
        spread: Option<ast::ExprId>,
        entries: &[ast::RecordEntry],
        span: Span,
    ) -> ExprId {
        if spread.is_none() {
            // a dotted path updates an existing nested record
            for e in entries {
                if let ast::RecordEntry::Field { path, .. } = e
                    && path.len() > 1
                {
                    let p: Vec<String> = path.iter().map(|l| self.sym(l.sym)).collect();
                    let p = p.join(".");
                    self.err(
                            "E0105",
                            path[0].span,
                            format!("dotted path `{p}` needs a base record"),
                            "dotted updates change fields of an existing record: write `{ ..base, a.b: value }`, or a nested literal `{ a: { b: value } }`",
                        );
                }
            }
        }
        let spread_val = spread.map(|s| self.lower_expr(s));
        // Group dotted paths by their first label, preserving order.
        let mut groups: Vec<(Symbol, Span, Vec<(Vec<ast::Label>, ExprId)>)> = vec![];
        for e in entries {
            let (path, value) = match e {
                ast::RecordEntry::Field { path, value } => (path.clone(), self.lower_expr(*value)),
                ast::RecordEntry::Shorthand(n) => {
                    let v = self.lower_name(*n, n.span);
                    (vec![ast::Label { sym: n.sym, span: n.span }], v)
                }
            };
            let first = path[0];
            match groups.iter_mut().find(|g| g.0 == first.sym) {
                Some(g) => g.2.push((path[1..].to_vec(), value)),
                None => groups.push((first.sym, first.span, vec![(path[1..].to_vec(), value)])),
            }
        }
        let needs_tmp = spread_val.is_some() && groups.iter().any(|g| g.2.iter().any(|(p, _)| !p.is_empty()));
        if !needs_tmp {
            let fields = groups
                .into_iter()
                .map(|(l, ls, mut vs)| {
                    let v = if vs.len() == 1 && vs[0].0.is_empty() {
                        vs.pop().unwrap().1
                    } else {
                        self.nested_record(None, vs, span)
                    };
                    (l, ls, v)
                })
                .collect();
            return self.alloc(ExprKind::Record { spread: spread_val, fields }, span);
        }
        // { ..base, a.b: v } => { let tmp = base; { ..tmp, a: { ..tmp.a, b: v } } }
        let tmp_name = self.intern("__record_base");
        self.locals.push(FxHashMap::default());
        let tmp = self.bind_local(ast::Name { sym: tmp_name, span }, false);
        let tmp_pat = self.alloc_pat(PatKind::Bind(tmp), span);
        let mut fields = vec![];
        for (l, ls, mut vs) in groups {
            let v = if vs.len() == 1 && vs[0].0.is_empty() {
                vs.pop().unwrap().1
            } else {
                let base_ref = self.alloc(ExprKind::Local(tmp), span);
                let sub = self.alloc(ExprKind::Field { base: base_ref, label: l, label_span: ls }, ls);
                self.nested_record(Some(sub), vs, span)
            };
            fields.push((l, ls, v));
        }
        self.locals.pop();
        let base_ref = self.alloc(ExprKind::Local(tmp), span);
        let rec = self.alloc(ExprKind::Record { spread: Some(base_ref), fields }, span);
        let stmts = vec![Stmt::Let { pat: tmp_pat, ty: None, init: spread_val.unwrap(), span }];
        self.alloc(ExprKind::Block { stmts, tail: Some(rec) }, span)
    }

    /// Build a nested record from dotted path suffixes; `base` is the
    /// expression whose fields are preserved (if any).
    fn nested_record(
        &mut self,
        base: Option<ExprId>,
        entries: Vec<(Vec<ast::Label>, ExprId)>,
        span: Span,
    ) -> ExprId {
        let mut groups: Vec<(Symbol, Span, Vec<(Vec<ast::Label>, ExprId)>)> = vec![];
        for (path, v) in entries {
            if path.is_empty() {
                continue;
            }
            let first = path[0];
            match groups.iter_mut().find(|g| g.0 == first.sym) {
                Some(g) => g.2.push((path[1..].to_vec(), v)),
                None => groups.push((first.sym, first.span, vec![(path[1..].to_vec(), v)])),
            }
        }
        let mut fields = vec![];
        for (l, ls, mut vs) in groups {
            let v = if vs.len() == 1 && vs[0].0.is_empty() {
                vs.pop().unwrap().1
            } else {
                let sub = base.map(|b| {
                    let b2 = self.clone_field_base(b);
                    self.alloc(ExprKind::Field { base: b2, label: l, label_span: ls }, ls)
                });
                self.nested_record(sub, vs, span)
            };
            fields.push((l, ls, v));
        }
        self.alloc(ExprKind::Record { spread: base, fields }, span)
    }

    /// Re-create a pure field-access chain (`tmp.a.b`) so it can be used twice.
    fn clone_field_base(&mut self, e: ExprId) -> ExprId {
        let ex = self.cx.hir.exprs[e].clone();
        match ex.kind {
            ExprKind::Field { base, label, label_span } => {
                let b = self.clone_field_base(base);
                self.alloc(ExprKind::Field { base: b, label, label_span }, ex.span)
            }
            k => self.alloc(k, ex.span),
        }
    }

    fn lower_lambda(
        &mut self,
        params: &[ast::Param],
        ret: Option<ast::TypeId>,
        effect: Option<&ast::Effect>,
        once: bool,
        body: LambdaBody<'_>,
        span: Span,
    ) -> ExprId {
        let id = self.alloc(ExprKind::Error, span);
        self.owners.push(OwnerFrame { owner: Owner::Lambda(id), captures: vec![] });
        self.locals.push(FxHashMap::default());
        let saved_loops = std::mem::take(&mut self.loops);
        let pats: Vec<ast::PatId> = params.iter().map(|p| p.pat).collect();
        self.check_bound_once(&pats);
        let was_params = std::mem::replace(&mut self.in_params, true);
        let mut params: Vec<Param> = params
            .iter()
            .map(|p| {
                let ty = p.ty.map(|t| self.lower_ty(t));
                Param { pat: self.lower_pat(p.pat), ty }
            })
            .collect();
        self.in_params = was_params;
        let ret = ret.map(|t| self.lower_ty(t));
        let effect = effect.map(|e| self.lower_effect(e));
        let body = match body {
            LambdaBody::Expr(b) => self.lower_expr(b),
            LambdaBody::Stmts(stmts, bspan) => {
                self.locals.push(FxHashMap::default());
                let (stmts, tail) = self.lower_stmts(stmts);
                self.locals.pop();
                self.alloc(ExprKind::Block { stmts, tail }, bspan)
            }
            LambdaBody::SystemConfig(cfg, nodes) => {
                let outputs_name = self.intern("__outputs");
                let outputs = self.bind_local(ast::Name { sym: outputs_name, span }, false);
                let pat = self.alloc_pat(PatKind::Bind(outputs), span);
                params.push(Param { pat, ty: None });
                let saved = self.system.replace(nodes);
                let saved_out = self.system_outputs.replace(outputs);
                let body = match cfg {
                    Some(cfg) => self.lower_expr(cfg),
                    // `provider()`: the empty configuration
                    None => self.alloc(ExprKind::Record { spread: None, fields: vec![] }, span),
                };
                self.system = saved;
                self.system_outputs = saved_out;
                body
            }
        };
        self.loops = saved_loops;
        self.locals.pop();
        let frame = self.owners.pop().unwrap();
        // Captures of this lambda that are owned further out must also be
        // captured by enclosing lambdas.
        for &c in &frame.captures {
            let owner = self.cx.hir.locals[c].owner;
            for f in self.owners.iter_mut().rev() {
                if Some(f.owner) == owner {
                    break;
                }
                if matches!(f.owner, Owner::Lambda(_)) && !f.captures.contains(&c) {
                    f.captures.push(c);
                }
            }
        }
        self.cx.hir.exprs[id].kind =
            ExprKind::Lambda(LambdaExpr { once, params, ret, effect, body, captures: frame.captures });
        id
    }

    fn lower_stmts(&mut self, stmts: &[ast::Stmt]) -> (Vec<Stmt>, Option<ExprId>) {
        let mut out = vec![];
        let mut tail = None;
        let n = stmts.len();
        let mut i = 0;
        while i < n {
            match &stmts[i] {
                ast::Stmt::Let { kind, pat, ty, value, span } => {
                    let init = self.lower_expr(*value);
                    let ty = ty.map(|t| self.lower_ty(t));
                    // bind after lowering the initializer: let is not recursive
                    let pat = self.lower_pat_mut(*pat, *kind == ast::LetKind::Var);
                    if *kind == ast::LetKind::Const {
                        self.cx.const_checks.push(init);
                        if let PatKind::Bind(l) = self.cx.hir.pats[pat].kind {
                            self.cx.const_locals.insert(l);
                            self.cx.hir.const_local_inits.push((l, init));
                        }
                    }
                    out.push(Stmt::Let { pat, ty, init, span: *span });
                }
                ast::Stmt::Use { pats, call, span } => {
                    // `use p <- f(args)` + rest  ==>  f(args, fn(p) { rest })
                    let rest = &stmts[i + 1..];
                    let span = *span;
                    let call_id = *call;
                    let lam_params: Vec<ast::Param> =
                        pats.iter().map(|&p| ast::Param { pat: p, ty: None }).collect();
                    let body_span = rest
                        .first()
                        .map(|s| stmt_span(self.ast, s))
                        .unwrap_or(span)
                        .to(rest.last().map(|s| stmt_span(self.ast, s)).unwrap_or(span));
                    let build_lambda = |this: &mut Self| -> ExprId {
                        this.lower_lambda(
                            &lam_params,
                            None,
                            None,
                            false,
                            LambdaBody::Stmts(rest, body_span),
                            span,
                        )
                    };
                    let callee_expr = &self.ast.exprs[call_id];
                    let call = match &callee_expr.kind {
                        ast::ExprKind::Call { callee, type_args, args } => {
                            let (callee, type_args, args) = (*callee, type_args.clone(), args.clone());
                            let cspan = callee_expr.span;
                            self.lower_call_with(callee, &type_args, cspan, |this| {
                                let mut out: Vec<ExprId> = args.iter().map(|a| this.lower_expr(*a)).collect();
                                out.push(build_lambda(this));
                                out
                            })
                        }
                        ast::ExprKind::Name(_) | ast::ExprKind::Field { .. } => {
                            let cspan = callee_expr.span;
                            self.lower_call_with(call_id, &[], cspan, |this| vec![build_lambda(this)])
                        }
                        _ => {
                            self.err(
                                "E0129",
                                span,
                                "`use` requires a call or a callable name",
                                "e.g. `use x <- helper(args)`",
                            );
                            self.alloc(ExprKind::Error, span)
                        }
                    };
                    tail = Some(call);
                    break;
                }
                ast::Stmt::Assign { base, path, value, span } => {
                    let span = *span;
                    let Some(local) = self.lookup_local(base.sym) else {
                        let s = self.sym(base.sym);
                        self.err(
                            "E0127",
                            base.span,
                            format!("unknown name `{s}`"),
                            "not found in this scope",
                        );
                        i += 1;
                        continue;
                    };
                    if !self.cx.hir.locals[local].mutable {
                        let s = self.sym(base.sym);
                        let decl = self.cx.hir.locals[local].span;
                        self.cx.diags.push(
                            Diagnostic::error(format!("cannot assign to immutable binding `{s}`"))
                                .with_code("E0130")
                                .with_label(base.span, "assignment to `let` binding")
                                .with_label(decl, "declared here; use `var` to allow reassignment"),
                        );
                    }
                    let owner = self.owners.last().map(|o| o.owner);
                    if self.cx.hir.locals[local].owner != owner {
                        let s = self.sym(base.sym);
                        self.err(
                            "E0131",
                            base.span,
                            format!("cannot assign to captured variable `{s}`"),
                            "closures capture values; use a `Cell` for shared mutable state",
                        );
                    }
                    let v = self.lower_expr(*value);
                    match path.split_last() {
                        None => out.push(Stmt::Assign { local, value: v, span }),
                        Some((ast::PlaceElem::Index(ix), prefix)) => {
                            // x.a[i] = v  ==>  x.a.set(i, v)
                            let recv = self.place_expr(local, prefix, span);
                            let ixe = self.lower_expr(*ix);
                            let set = self.intern("set");
                            let call = self.alloc(
                                ExprKind::MethodCall {
                                    recv,
                                    method: set,
                                    method_span: span,
                                    type_args: vec![],
                                    args: vec![ixe, v],
                                },
                                span,
                            );
                            out.push(Stmt::Expr { expr: call, semi: true });
                        }
                        Some(_) => {
                            // x.a.b = v  ==>  x = { ..x, a.b: v }
                            if path.iter().any(|p| matches!(p, ast::PlaceElem::Index(_))) {
                                self.err(
                                    "E0132",
                                    span,
                                    "cannot assign through an index inside a field path",
                                    "assign the field directly",
                                );
                            }
                            let labels: Vec<ast::Label> = path
                                .iter()
                                .filter_map(|p| match p {
                                    ast::PlaceElem::Field(l) => Some(*l),
                                    _ => None,
                                })
                                .collect();
                            let base_e = self.alloc(ExprKind::Local(local), span);
                            let rec = self.nested_record(Some(base_e), vec![(labels, v)], span);
                            out.push(Stmt::Assign { local, value: rec, span });
                        }
                    }
                }
                ast::Stmt::Expr { expr, semi } => {
                    let e = self.lower_expr(*expr);
                    if i == n - 1 && !semi {
                        tail = Some(e);
                    } else {
                        out.push(Stmt::Expr { expr: e, semi: *semi });
                    }
                }
            }
            i += 1;
        }
        (out, tail)
    }

    fn place_expr(&mut self, local: LocalId, path: &[ast::PlaceElem], span: Span) -> ExprId {
        let mut e = self.alloc(ExprKind::Local(local), span);
        for p in path {
            e = match p {
                ast::PlaceElem::Field(l) => {
                    self.alloc(ExprKind::Field { base: e, label: l.sym, label_span: l.span }, span)
                }
                ast::PlaceElem::Index(ix) => {
                    let i = self.lower_expr(*ix);
                    self.alloc(ExprKind::Index { base: e, index: i }, span)
                }
            };
        }
        e
    }

    /// Does the callee expression (a name or `module.name`) resolve to `def`?
    fn resolves_to(&mut self, callee: ast::ExprId, def: DefId) -> bool {
        match &self.ast.exprs[callee].kind {
            ast::ExprKind::Name(n) => {
                !self.locals.iter().any(|s| s.contains_key(&n.sym))
                    && self.lookup_module_name(n.sym) == Some(Res::Def(def))
            }
            ast::ExprKind::Field { base, name } => {
                let name = *name;
                matches!(self.namespace_of(*base), Some(Ns::Module(m)) if self.cx.hir.modules[m].items.get(&name.sym) == Some(&def))
            }
            ast::ExprKind::Std(path) => self.cx.hir.try_lang(path) == Some(def),
            _ => false,
        }
    }

    fn lang_def(&mut self, name: &str, span: Span) -> ExprId {
        match self.cx.hir.try_lang(name) {
            Some(def) => self.alloc(ExprKind::Def { def, type_args: vec![] }, span),
            None => {
                self.err("E0190", span, format!("missing standard library item `{name}`"), "required here");
                self.alloc(ExprKind::Error, span)
            }
        }
    }

    fn call_lang(&mut self, name: &str, args: Vec<ExprId>, span: Span) -> ExprId {
        let f = self.lang_def(name, span);
        self.alloc(ExprKind::Call { callee: f, args }, span)
    }

    fn new_local(&mut self, name: &str, span: Span) -> LocalId {
        let sym = self.intern(name);
        let owner = self.owners.last().map(|o| o.owner);
        self.cx.hir.locals.alloc(Local { name: sym, span, mutable: false, owner })
    }

    /// `system { k: component(provider, config), ... }` desugars to typed node
    /// handles bound in dependency order, followed by `system.plan([...])`:
    ///
    /// ```text
    /// { let __node_k = system.node(#k, index, [deps], provider, fn(__outputs) { config })
    ///   ...
    ///   system.plan([system.erase(__node_a), ...]) }
    /// ```
    ///
    /// `ref(#k)` inside a configuration becomes `system.output(__outputs, __node_k)`,
    /// so reference types follow from ordinary inference.
    fn lower_system(&mut self, entries: &[(ast::Label, ast::ExprId)], span: Span) -> ExprId {
        let component = self.cx.hir.try_lang("system.component");
        let mut parsed: Vec<(ast::Label, ast::ExprId, Option<ast::ExprId>, Vec<(Symbol, Span)>)> = vec![];
        for (label, e) in entries {
            let expr = &self.ast.exprs[*e];
            let espan = expr.span;
            let ok = match &expr.kind {
                ast::ExprKind::Call { callee, args, .. } if args.len() == 2 => {
                    let is_component = match &self.ast.exprs[*callee].kind {
                        ast::ExprKind::Name(n) => {
                            self.lookup_module_name(n.sym) == component.map(Res::Def)
                                && !self.locals.iter().any(|s| s.contains_key(&n.sym))
                        }
                        ast::ExprKind::Field { base, name } => {
                            matches!(self.namespace_of(*base), Some(Ns::Module(m)) if self.cx.hir.modules[m].items.get(&name.sym).copied() == component)
                        }
                        _ => false,
                    };
                    is_component.then(|| (args[0], Some(args[1])))
                }
                _ => None,
            };
            // `key: provider(config)` applies a provider directly (as
            // `component(provider, config)`; `provider()` has the empty config)
            let ok = ok.or_else(|| match &expr.kind {
                ast::ExprKind::Call { callee, type_args, args }
                    if type_args.is_empty() && args.len() <= 1 =>
                {
                    Some((*callee, args.first().copied()))
                }
                _ => None,
            });
            let Some((provider, config)) = ok else {
                self.err(
                    "E0603",
                    espan,
                    "system entries apply a provider to its configuration: `key: provider(config)`",
                    "expected a provider application",
                );
                continue;
            };
            // keys that would collide with the running system's own fields
            let key = self.sym(label.sym);
            if matches!(key.as_str(), "brand" | "started" | "wait_for_shutdown") {
                self.err(
                    "E0607",
                    label.span,
                    format!("`{key}` is reserved in a running system"),
                    "rename this plan key",
                );
                continue;
            }
            let mut refs = vec![];
            if let Some(config) = config {
                collect_refs(self.ast, config, &mut refs);
            }
            parsed.push((*label, provider, config, refs));
        }
        // missing references
        let labels: Vec<Symbol> = parsed.iter().map(|p| p.0.sym).collect();
        let mut bad = false;
        for p in &parsed {
            for (r, rspan) in &p.3 {
                if !labels.contains(r) {
                    let s = self.sym(*r);
                    let keys: Vec<String> = labels.iter().map(|l| format!("#{}", self.sym(*l))).collect();
                    self.cx.diags.push(
                        Diagnostic::error(format!("system reference `#{s}` names no plan key"))
                            .with_code("E0601")
                            .with_label(*rspan, "missing reference")
                            .with_note(format!("plan keys: {}", keys.join(", "))),
                    );
                    bad = true;
                }
            }
        }
        if bad || self.check_system_cycles(&parsed) {
            return self.alloc(ExprKind::Error, span);
        }
        // topological order, ties broken by source order
        let n = parsed.len();
        let mut done = vec![false; n];
        let mut order = vec![];
        while order.len() < n {
            let next = (0..n)
                .find(|&i| {
                    !done[i]
                        && parsed[i]
                            .3
                            .iter()
                            .all(|(r, _)| labels.iter().position(|l| l == r).is_some_and(|j| done[j]))
                })
                .unwrap();
            done[next] = true;
            order.push(next);
        }
        self.locals.push(FxHashMap::default());
        let mut stmts = vec![];
        let mut nodes: FxHashMap<Symbol, LocalId> = FxHashMap::default();
        let mut node_list = vec![];
        for (pos, &i) in order.iter().enumerate() {
            let (label, provider, config, refs) = parsed[i].clone();
            let lspan = label.span;
            let provider = self.lower_expr(provider);
            let cfg = self.lower_lambda(
                &[],
                None,
                None,
                false,
                LambdaBody::SystemConfig(config, nodes.clone()),
                lspan,
            );
            let label_e = self.alloc(ExprKind::Lit(Lit::Symbol(label.sym)), lspan);
            let index = self.alloc(ExprKind::Lit(Lit::Int { value: pos as u128, suffix: None }), lspan);
            let mut deps: Vec<usize> = refs
                .iter()
                .map(|(r, _)| order.iter().position(|&o| parsed[o].0.sym == *r).unwrap())
                .collect();
            deps.sort();
            deps.dedup();
            let dep_items = deps
                .iter()
                .map(|d| self.alloc(ExprKind::Lit(Lit::Int { value: *d as u128, suffix: None }), lspan))
                .collect();
            let deps_e = self.alloc(ExprKind::List(dep_items), lspan);
            // the node's source location, for inspection and diagnostics
            let unit_e = self.alloc(ExprKind::Lit(Lit::Unit), lspan);
            let at = self.call_lang("located", vec![unit_e], lspan);
            let call = self.call_lang("system.node", vec![label_e, index, deps_e, provider, cfg, at], lspan);
            let name = format!("__node_{}", self.sym(label.sym).replace('/', "_"));
            let local = self.new_local(&name, lspan);
            let pat = self.alloc_pat(PatKind::Bind(local), lspan);
            stmts.push(Stmt::Let { pat, ty: None, init: call, span: lspan });
            nodes.insert(label.sym, local);
            let r = self.alloc(ExprKind::Local(local), lspan);
            node_list.push(self.call_lang("system.erase", vec![r], lspan));
        }
        self.locals.pop();
        let list = self.alloc(ExprKind::List(node_list), span);
        // typed output view: fn(outputs) { { key: system.output(outputs, __node_key), ... } }
        let node_locals: Vec<(Symbol, Span, LocalId)> =
            parsed.iter().map(|p| (p.0.sym, p.0.span, nodes[&p.0.sym])).collect();
        let view = self.synth_runner(span, node_locals.iter().map(|n| n.2).collect(), |this, outputs| {
            let fields = node_locals
                .iter()
                .map(|(label, lspan, node)| {
                    let o = this.alloc(ExprKind::Local(outputs), *lspan);
                    let n = this.alloc(ExprKind::Local(*node), *lspan);
                    (*label, *lspan, this.call_lang("system.output", vec![o, n], *lspan))
                })
                .collect();
            this.alloc(ExprKind::Record { spread: None, fields }, span)
        });
        // the providers' configuration types, for the plan's type (the view
        // is never called): fn(outputs) { { key: system.config_of(__node_key), ... } }
        let configs = self.synth_runner(span, node_locals.iter().map(|n| n.2).collect(), |this, _| {
            let fields = node_locals
                .iter()
                .map(|(label, lspan, node)| {
                    let n = this.alloc(ExprKind::Local(*node), *lspan);
                    (*label, *lspan, this.call_lang("system.config_of", vec![n], *lspan))
                })
                .collect();
            this.alloc(ExprKind::Record { spread: None, fields }, span)
        });
        let plan = self.call_lang("system.plan", vec![list, view, configs], span);
        self.alloc(ExprKind::Block { stmts, tail: Some(plan) }, span)
    }

    /// Report a dependency cycle (with its path); returns whether one exists.
    fn check_system_cycles(
        &mut self,
        entries: &[(ast::Label, ast::ExprId, Option<ast::ExprId>, Vec<(Symbol, Span)>)],
    ) -> bool {
        let n = entries.len();
        let idx = |l: Symbol| entries.iter().position(|e| e.0.sym == l);
        fn dfs(
            u: usize,
            entries: &[(ast::Label, ast::ExprId, Option<ast::ExprId>, Vec<(Symbol, Span)>)],
            idx: &dyn Fn(Symbol) -> Option<usize>,
            color: &mut [u8],
            stack: &mut Vec<usize>,
        ) -> Option<Vec<usize>> {
            color[u] = 1;
            stack.push(u);
            for (r, _) in &entries[u].3 {
                let Some(v) = idx(*r) else { continue };
                if color[v] == 1 {
                    let pos = stack.iter().position(|&x| x == v).unwrap();
                    let mut cyc = stack[pos..].to_vec();
                    cyc.push(v);
                    return Some(cyc);
                }
                if color[v] == 0
                    && let Some(c) = dfs(v, entries, idx, color, stack)
                {
                    return Some(c);
                }
            }
            stack.pop();
            color[u] = 2;
            None
        }
        let mut color = vec![0u8; n];
        for u in 0..n {
            if color[u] != 0 {
                continue;
            }
            let mut stack = vec![];
            if let Some(cyc) = dfs(u, entries, &idx, &mut color, &mut stack) {
                let path: Vec<String> =
                    cyc.iter().map(|&i| format!("#{}", self.sym(entries[i].0.sym))).collect();
                let mut d = Diagnostic::error(format!("system dependency cycle: {}", path.join(" -> ")))
                    .with_code("E0604");
                for w in cyc.windows(2) {
                    let (a, b) = (w[0], w[1]);
                    let span = entries[a].3.iter().find(|r| r.0 == entries[b].0.sym).map(|r| r.1).unwrap();
                    let (la, lb) = (self.sym(entries[a].0.sym), self.sym(entries[b].0.sym));
                    d = d.with_label(span, format!("`#{la}` depends on `#{lb}`"));
                }
                self.cx.diags.push(d);
                return true;
            }
        }
        false
    }

    /// `pipeline { s1, ..., sn, handler }` desugars to nested stage runners:
    ///
    /// ```text
    /// { let __stage0 = s1; ...; let __stageN = handler
    ///   pipeline.plan(fn(c) { pipeline.run_stage(__stage0, fn(c) { ... pipeline.run_handler(__stageN, c) }, c) }) }
    /// ```
    ///
    /// Each runner returns its input-shaped context with the latest values of
    /// its prefix fields (a typed projection), so earlier unwind callbacks see
    /// later writes but never later-only fields.
    fn lower_pipeline(&mut self, stages: &[ast::ExprId], span: Span) -> ExprId {
        if stages.is_empty() {
            self.err("E0701", span, "a pipeline needs a terminal handler", "add `handle(...)`");
            return self.alloc(ExprKind::Error, span);
        }
        let mut stmts = vec![];
        let mut locals = vec![];
        for (i, e) in stages.iter().enumerate() {
            let espan = self.ast.exprs[*e].span;
            let v = self.lower_expr(*e);
            let l = self.new_local(&format!("__stage{i}"), espan);
            let pat = self.alloc_pat(PatKind::Bind(l), espan);
            stmts.push(Stmt::Let { pat, ty: None, init: v, span: espan });
            locals.push((l, espan));
        }
        let n = locals.len();
        let (h, hspan) = locals[n - 1];
        // innermost runner: fn(c) { pipeline.run_handler(h, c) }
        let mut inner = self.synth_runner(hspan, vec![h], |this, c| {
            let hl = this.alloc(ExprKind::Local(h), hspan);
            let cl = this.alloc(ExprKind::Local(c), hspan);
            this.call_lang("pipeline.run_handler", vec![hl, cl], hspan)
        });
        for i in (0..n - 1).rev() {
            let (s_i, sspan) = locals[i];
            let caps: Vec<LocalId> = locals[i..].iter().map(|x| x.0).collect();
            let rest = inner;
            inner = self.synth_runner(sspan, caps, move |this, c| {
                let sl = this.alloc(ExprKind::Local(s_i), sspan);
                let cl = this.alloc(ExprKind::Local(c), sspan);
                this.call_lang("pipeline.run_stage", vec![sl, rest, cl], sspan)
            });
        }
        // stage names make the plan inspectable
        let name_sym = self.intern("name");
        let names: Vec<ExprId> = locals[..n - 1]
            .iter()
            .map(|(s_i, sspan)| {
                let base = self.alloc(ExprKind::Local(*s_i), *sspan);
                self.alloc(ExprKind::Field { base, label: name_sym, label_span: *sspan }, *sspan)
            })
            .collect();
        let names = self.alloc(ExprKind::List(names), span);
        let plan = self.call_lang("pipeline.plan", vec![inner, names], span);
        self.alloc(ExprKind::Block { stmts, tail: Some(plan) }, span)
    }

    /// Build `fn(__ctx) { body }` capturing `caps` (all owned outside).
    fn synth_runner(
        &mut self,
        span: Span,
        caps: Vec<LocalId>,
        body: impl FnOnce(&mut Self, LocalId) -> ExprId,
    ) -> ExprId {
        let id = self.alloc(ExprKind::Error, span);
        let ctx_name = self.intern("__ctx");
        let c = self.cx.hir.locals.alloc(Local {
            name: ctx_name,
            span,
            mutable: false,
            owner: Some(Owner::Lambda(id)),
        });
        let pat = self.alloc_pat(PatKind::Bind(c), span);
        // re-own nested runner lambdas' parameter references: the body is
        // built while this lambda is the innermost owner
        self.owners.push(OwnerFrame { owner: Owner::Lambda(id), captures: vec![] });
        let b = body(self, c);
        self.owners.pop();
        for &cap in &caps {
            self.lookup_local_raw(cap);
        }
        self.cx.hir.exprs[id].kind = ExprKind::Lambda(LambdaExpr {
            once: false,
            params: vec![Param { pat, ty: None }],
            ret: None,
            effect: None,
            body: b,
            captures: caps,
        });
        id
    }

    /// `system.select(plan, [#a, #b])` becomes
    /// `system.select_keys(plan, [#a, #b], fn(all) { { a: all.a, b: all.b } })`:
    /// the projection types the selected output record (and rejects unknown
    /// keys statically); the runtime starts only the keys' dependency closure.
    fn lower_select(&mut self, plan: ast::ExprId, keys: ast::ExprId, span: Span) -> ExprId {
        let plan = self.lower_expr(plan);
        let kspan = self.ast.exprs[keys].span;
        let labels: Option<Vec<(Symbol, Span)>> = match &self.ast.exprs[keys].kind {
            ast::ExprKind::List(items) => items
                .iter()
                .map(|i| match &self.ast.exprs[*i].kind {
                    ast::ExprKind::Lit(ast::Lit::Symbol(s)) => Some((*s, self.ast.exprs[*i].span)),
                    _ => None,
                })
                .collect(),
            _ => None,
        };
        let Some(labels) = labels else {
            self.err(
                "E0605",
                kspan,
                "`system.select` takes a literal list of plan keys",
                "e.g. `[#database, #api]`",
            );
            return self.alloc(ExprKind::Error, span);
        };
        let keys = self.lower_expr(keys);
        let pick = self.synth_runner(kspan, vec![], |this, c| {
            let fields = labels
                .iter()
                .map(|(l, lspan)| {
                    let base = this.alloc(ExprKind::Local(c), *lspan);
                    let f = this.alloc(ExprKind::Field { base, label: *l, label_span: *lspan }, *lspan);
                    (*l, *lspan, f)
                })
                .collect();
            this.alloc(ExprKind::Record { spread: None, fields }, kspan)
        });
        self.call_lang("system.select_keys", vec![plan, keys, pick], span)
    }

    /// `pipeline.stage({name, enter, leave, error})` normalizes omitted
    /// callbacks: it becomes `pipeline.make_stage(name, enter?, leave?, error?)`
    /// with `Option` callbacks, or `pipeline.make_passthrough(name, leave?, error?)`
    /// when `enter` is omitted (its output context equals its input).
    fn lower_stage(&mut self, arg: ast::ExprId, span: Span) -> ExprId {
        let ast::ExprKind::Record { spread: None, entries } = &self.ast.exprs[arg].kind else {
            self.err(
                "E0702",
                span,
                "`pipeline.stage` expects a record literal",
                "e.g. `{ name: #app/stage, enter: ... }`",
            );
            return self.alloc(ExprKind::Error, span);
        };
        let mut fields: FxHashMap<String, ExprId> = FxHashMap::default();
        for e in entries.clone() {
            let (label, value) = match e {
                ast::RecordEntry::Field { path, value } if path.len() == 1 => {
                    (path[0], self.lower_expr(value))
                }
                ast::RecordEntry::Shorthand(n) => {
                    (ast::Label { sym: n.sym, span: n.span }, self.lower_name(n, n.span))
                }
                ast::RecordEntry::Field { path, .. } => {
                    self.err(
                        "E0702",
                        path[0].span,
                        "unexpected stage field",
                        "expected name, enter, leave, or error",
                    );
                    continue;
                }
            };
            let key = self.sym(label.sym);
            if !matches!(key.as_str(), "name" | "enter" | "leave" | "error") {
                self.err(
                    "E0702",
                    label.span,
                    format!("unknown stage field `{key}`"),
                    "expected name, enter, leave, or error",
                );
                continue;
            }
            fields.insert(key, value);
        }
        let name = match fields.remove("name") {
            Some(n) => n,
            None => {
                self.err("E0702", span, "a stage needs a `name`", "add `name: #app/stage`");
                self.alloc(ExprKind::Error, span)
            }
        };
        let opt_def = self.cx.hir.lang("Option");
        let wrap = |this: &mut Self, v: Option<ExprId>| -> ExprId {
            match v {
                Some(v) => {
                    let ctor =
                        this.alloc(ExprKind::Ctor { enum_def: opt_def, variant: 1, type_args: vec![] }, span);
                    this.alloc(ExprKind::Call { callee: ctor, args: vec![v] }, span)
                }
                None => this.alloc(ExprKind::Ctor { enum_def: opt_def, variant: 0, type_args: vec![] }, span),
            }
        };
        let enter = fields.remove("enter");
        let leave = fields.remove("leave");
        let error = fields.remove("error");
        let leave = wrap(self, leave);
        let error = wrap(self, error);
        match enter {
            Some(e) => {
                let enter = wrap(self, Some(e));
                self.call_lang("pipeline.make_stage", vec![name, enter, leave, error], span)
            }
            None => self.call_lang("pipeline.make_passthrough", vec![name, leave, error], span),
        }
    }

    // ------------------------------------------------------------ patterns

    fn lower_pat(&mut self, p: ast::PatId) -> PatId {
        self.lower_pat_mut(p, false)
    }

    fn lower_pat_mut(&mut self, p: ast::PatId, mutable: bool) -> PatId {
        if self.pat_depth == 0 && !self.in_params {
            self.check_bound_once(&[p]);
        }
        self.pat_depth += 1;
        let r = self.lower_pat_inner(p, mutable);
        self.pat_depth -= 1;
        r
    }

    /// The names a pattern binds, in order (a record field without a
    /// pattern binds its label).
    fn pat_names(&self, p: ast::PatId, out: &mut Vec<(Symbol, Span)>) {
        match &self.ast.pats[p].kind {
            ast::PatKind::Bind(n) => out.push((n.sym, n.span)),
            ast::PatKind::Tuple(ps) => ps.iter().for_each(|q| self.pat_names(*q, out)),
            ast::PatKind::Record { fields, .. } => {
                for (l, q) in fields {
                    match q {
                        Some(q) => self.pat_names(*q, out),
                        None => out.push((l.sym, l.span)),
                    }
                }
            }
            ast::PatKind::Variant { args: Some(args), .. } => {
                args.iter().for_each(|q| self.pat_names(*q, out))
            }
            _ => {}
        }
    }

    /// A name bound twice by one pattern, or by one parameter list, is an
    /// error (the second would silently shadow the first).
    fn check_bound_once(&mut self, pats: &[ast::PatId]) {
        let mut names = vec![];
        for p in pats {
            self.pat_names(*p, &mut names);
        }
        let mut seen: FxHashMap<Symbol, Span> = FxHashMap::default();
        for (n, sp) in names {
            if self.cx.interner.resolve(n) == "_" {
                continue;
            }
            if let Some(first) = seen.get(&n).copied() {
                let name = self.sym(n);
                let what = if pats.len() > 1 { "parameter list" } else { "pattern" };
                self.cx.diags.push(
                    Diagnostic::error(format!("`{name}` is bound twice in one {what}"))
                        .with_code("E0148")
                        .with_label(sp, "bound again here")
                        .with_label(first, "first bound here"),
                );
            } else {
                seen.insert(n, sp);
            }
        }
    }

    fn lower_pat_inner(&mut self, p: ast::PatId, mutable: bool) -> PatId {
        let pat = &self.ast.pats[p];
        let span = pat.span;
        let kind = match &pat.kind {
            ast::PatKind::Discard => PatKind::Discard,
            ast::PatKind::Error => PatKind::Error,
            ast::PatKind::Bind(n) => PatKind::Bind(self.bind_local(*n, mutable)),
            ast::PatKind::Lit { lit, negative } => {
                let mut l = self.lower_lit(lit);
                if *negative {
                    l = match l {
                        Lit::Float { value, suffix } => Lit::Float { value: -value, suffix },
                        Lit::Int { value, suffix } => {
                            // encode negative integers by two's complement in u128
                            Lit::Int { value: (value as i128).wrapping_neg() as u128, suffix }
                        }
                        other => other,
                    };
                }
                PatKind::Lit(l)
            }
            ast::PatKind::Tuple(ps) => {
                let ps = ps.clone();
                PatKind::Tuple(ps.iter().map(|p| self.lower_pat_mut(*p, mutable)).collect())
            }
            ast::PatKind::Record { fields, open } => {
                let (fields, open) = (fields.clone(), *open);
                let fields = fields
                    .iter()
                    .map(|(l, sub)| {
                        let sp = match sub {
                            Some(s) => self.lower_pat_mut(*s, mutable),
                            None => {
                                let local = self.bind_local(ast::Name { sym: l.sym, span: l.span }, mutable);
                                self.alloc_pat(PatKind::Bind(local), l.span)
                            }
                        };
                        (l.sym, sp)
                    })
                    .collect();
                PatKind::Record { fields, open }
            }
            ast::PatKind::Variant { ty, name, args } => {
                let (ty, name, args) = (ty.clone(), *name, args.clone());
                let enum_def =
                    ty.and_then(|(module, tname)| match self.resolve_type_path(&module, tname)? {
                        TypeRes::Def(d) if matches!(self.cx.hir.defs[d].kind, DefKind::Enum(_)) => Some(d),
                        TypeRes::Def(_) => {
                            let s = self.sym(tname.sym);
                            self.err(
                                "E0133",
                                tname.span,
                                format!("`{s}` is not an enum"),
                                "variant patterns need an enum",
                            );
                            None
                        }
                        _ => None,
                    });
                let has_args = args.is_some();
                let args = args.unwrap_or_default().iter().map(|p| self.lower_pat_mut(*p, mutable)).collect();
                PatKind::Variant { enum_def, name: name.sym, name_span: name.span, args, has_args }
            }
        };
        self.alloc_pat(kind, span)
    }
}

enum Ns {
    Module(ModuleId),
    Type(HTy),
}

enum TypeRes {
    Def(DefId),
    Generic(GenericId),
    SelfTy,
    Effect(Symbol),
}

enum LambdaBody<'s> {
    Expr(ast::ExprId),
    Stmts(&'s [ast::Stmt], Span),
    /// System node configuration with earlier node handles in scope for `ref`.
    SystemConfig(Option<ast::ExprId>, FxHashMap<Symbol, LocalId>),
}

/// Collect `ref(#label)` occurrences in an expression tree.
fn collect_refs(ast: &ast::Ast, e: ast::ExprId, out: &mut Vec<(Symbol, Span)>) {
    use ast::ExprKind as K;
    let mut go = |x: ast::ExprId| collect_refs(ast, x, out);
    match &ast.exprs[e].kind {
        K::Ref(l) => out.push((l.sym, l.span)),
        K::Lit(_) | K::Name(_) | K::TypeName(_) | K::Continue | K::Error | K::Std(_) | K::DeclName(_) => {}
        K::Interp(parts) => parts.iter().for_each(|p| {
            if let ast::InterpPart::Expr(x) = p {
                go(*x)
            }
        }),
        K::Field { base, .. } => go(*base),
        K::Index { base, index } => {
            go(*base);
            go(*index)
        }
        K::Call { callee, args, .. } => {
            go(*callee);
            args.iter().for_each(|a| go(*a))
        }
        K::Try(x) | K::Unary { operand: x, .. } | K::Unsafe(x) | K::Loop { body: x } => go(*x),
        K::Binary { lhs, rhs, .. } => {
            go(*lhs);
            go(*rhs)
        }
        K::Tuple(xs) | K::List(xs) | K::Set(xs) | K::Pipeline(xs) => xs.iter().for_each(|a| go(*a)),
        K::Record { spread, entries } => {
            spread.iter().for_each(|s| go(*s));
            for en in entries {
                if let ast::RecordEntry::Field { value, .. } = en {
                    go(*value)
                }
            }
        }
        K::Map(kv) => kv.iter().for_each(|(k, v)| {
            go(*k);
            go(*v)
        }),
        K::Lambda(l) => go(l.body),
        K::If { cond, then, els } => {
            go(*cond);
            go(*then);
            els.iter().for_each(|x| go(*x))
        }
        K::Match { scrutinee, arms } => {
            go(*scrutinee);
            arms.iter().for_each(|a| {
                a.guard.iter().for_each(|g| go(*g));
                go(a.body)
            })
        }
        K::While { cond, body } => {
            go(*cond);
            go(*body)
        }
        K::For { iter, body, .. } => {
            go(*iter);
            go(*body)
        }
        K::Block { stmts } => {
            for st in stmts {
                match st {
                    ast::Stmt::Let { value, .. } | ast::Stmt::Assign { value, .. } => go(*value),
                    ast::Stmt::Use { call, .. } => go(*call),
                    ast::Stmt::Expr { expr, .. } => go(*expr),
                }
            }
        }
        K::Return(x) | K::Break(x) => x.iter().for_each(|v| go(*v)),
        K::System(_) => {}
    }
}

fn stmt_span(ast: &ast::Ast, s: &ast::Stmt) -> Span {
    match s {
        ast::Stmt::Let { span, .. } | ast::Stmt::Use { span, .. } | ast::Stmt::Assign { span, .. } => *span,
        ast::Stmt::Expr { expr, .. } => ast.exprs[*expr].span,
    }
}

fn contains_hole(t: &HTy) -> bool {
    match &t.kind {
        HTyKind::Hole => true,
        HTyKind::Named { args, .. } | HTyKind::Dyn { args, .. } => args.iter().any(contains_hole),
        HTyKind::Tuple(ts) => ts.iter().any(contains_hole),
        HTyKind::Record { fields, .. } => fields.iter().any(|f| contains_hole(&f.1)),
        HTyKind::Fn { params, ret, .. } => params.iter().any(contains_hole) || contains_hole(ret),
        HTyKind::Forall { body, .. } => contains_hole(body),
        HTyKind::Ref { inner, .. } => contains_hole(inner),
        _ => false,
    }
}
