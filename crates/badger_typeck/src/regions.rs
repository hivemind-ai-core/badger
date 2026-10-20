//! Region escape through values that can hide a scoped one.
//!
//! The unifier rejects a callback result whose *type* mentions the region
//! brand of its `for<S>` scope. A value can also carry a scoped value without
//! naming the brand in its type: a lazy `Seq` built from a closure that
//! captured a transaction, a task handle running such a closure, a resource
//! handle read from a running system, a `dyn` value. This pass tracks which
//! values derive from region-bound ones (results of calls taking them,
//! fields of them, closures capturing them) and rejects a callback that
//! returns such a value through a type able to hide it (closures, tasks,
//! resources and other runtime handles; plain data cannot). It also rejects
//! spawning a child task that uses a scoped value into a task scope older
//! than that value's region: the child could outlive the resource.
//!
//! A `region R { ... }` block is checked like such a callback: its body is
//! the callback's, and the locals it uses from outside are its captures.

use badger_base::{Diagnostic, Span};
use badger_hir::{ExprId, ExprKind, LocalId, PatId, PatKind, Stmt};
use rustc_hash::FxHashMap;

use crate::infer::Infer;
use crate::ty::*;

pub fn check(inf: &mut Infer<'_, '_>) {
    // a provider initializer may not stash its scope or anything derived from
    // its configuration (which carries dependency outputs) in state that
    // outlives the running system; returning them as its output is the point
    for lam in std::mem::take(&mut inf.init_lambdas) {
        let hir = inf.g.hir;
        let ExprKind::Lambda(l) = &hir.exprs[lam].kind else { continue };
        let mut tainted = FxHashMap::default();
        let mut params = vec![];
        for p in &l.params {
            let mut ls = vec![];
            collect(hir, p.pat, &mut ls);
            for x in ls {
                tainted.insert(x, true);
                params.push(x);
            }
        }
        let mut r = Regions { inf, regions: vec![], tainted };
        for _ in 0..3 {
            r.scan(l.body);
        }
        // channels and cells reached through the configuration may come
        // from outside the system as well as captured ones
        let mut outer = l.captures.clone();
        outer.extend(params);
        let outer = derived_from(hir, l.body, &outer);
        r.stores(l.body, &outer);
    }
    let lambdas = std::mem::take(&mut inf.region_lambdas);
    for (lam, regions) in lambdas {
        let hir = inf.g.hir;
        let (body, captures) = match &hir.exprs[lam].kind {
            ExprKind::Lambda(l) => (l.body, l.captures.clone()),
            ExprKind::Block { .. } => (lam, outer_locals(hir, lam)),
            _ => continue,
        };
        let mut r = Regions { inf, regions, tainted: FxHashMap::default() };
        // fixpoint over let-bound locals (loops and reassignment)
        for _ in 0..3 {
            r.scan(body);
        }
        let mut exits = vec![];
        r.exits(body, &mut exits);
        for x in exits {
            // a result whose type names the brand is reported by unification
            let named = match r.ty(x) {
                Some(t) => r.mentions(&t),
                None => true,
            };
            if !named && r.tainted(x) {
                let span = r.inf.g.hir.exprs[x].span;
                r.report(span);
            }
        }
        r.spawns(body);
        // storing a scoped value into state that outlives the callback
        // (reached through a capture or a local alias of one)
        let captures = derived_from(hir, body, &captures);
        r.stores(body, &captures);
    }
}

/// The locals a block uses (or assigns) that are bound outside it.
fn outer_locals(hir: &badger_hir::Hir, block: ExprId) -> Vec<LocalId> {
    let span = hir.exprs[block].span;
    let inside = |l: LocalId| {
        let s = hir.locals[l].span;
        s.file == span.file && s.start >= span.start && s.end <= span.end
    };
    let mut out = vec![];
    crate::program::walk_exprs(hir, block, &mut |x| {
        let used: Vec<LocalId> = match &hir.exprs[x].kind {
            ExprKind::Local(l) => vec![*l],
            ExprKind::Block { stmts, .. } => stmts
                .iter()
                .filter_map(|s| match s {
                    Stmt::Assign { local, .. } => Some(*local),
                    _ => None,
                })
                .collect(),
            _ => vec![],
        };
        for l in used {
            if !inside(l) && !out.contains(&l) {
                out.push(l);
            }
        }
    });
    out
}

struct Regions<'i, 'g, 'a> {
    inf: &'i mut Infer<'g, 'a>,
    regions: Vec<u32>,
    tainted: FxHashMap<LocalId, bool>,
}

impl Regions<'_, '_, '_> {
    fn ty(&mut self, e: ExprId) -> Option<Ty> {
        let t = self.inf.exprs.get(&e).cloned()?;
        Some(self.inf.icx.resolve(&t))
    }

    /// The type names one of this callback's regions.
    fn mentions(&mut self, t: &Ty) -> bool {
        let regions = self.regions.clone();
        if ty_any(t, &mut |k| matches!(k, TyKind::Skolem(s) if regions.contains(s))) {
            return true;
        }
        let labels: Vec<String> = regions.iter().map(|s| format!("%region{s}")).collect();
        let mut found = false;
        ty_any(t, &mut |k| {
            if let TyKind::Fn(f) = k {
                for (l, _) in row_fields(&f.eff).0 {
                    if labels.iter().any(|x| x == self.inf.g.interner.resolve(l)) {
                        found = true;
                    }
                }
            }
            false
        });
        found
    }

    /// Can a value of this type hold on to a scoped value without its type
    /// saying so? Closures, `dyn`, and runtime handles can; plain data
    /// (numbers, text, collections of data, records of data) cannot.
    fn opaque(&mut self, t: &Ty) -> bool {
        self.opaque_in(t, &mut vec![])
    }

    fn opaque_in(&mut self, t: &Ty, seen: &mut Vec<Ty>) -> bool {
        match &**t {
            TyKind::Fn(_) | TyKind::Dyn(..) => true,
            TyKind::Skolem(s) => !self.inf.icx.skolems.get(*s as usize).is_some_and(|i| i.region),
            TyKind::Ref(_, x) | TyKind::Record(x) => self.opaque_in(x, seen),
            TyKind::RowExt(_, a, rest) => self.opaque_in(a, seen) || self.opaque_in(rest, seen),
            TyKind::Tuple(ts) => ts.iter().any(|x| self.opaque_in(x, seen)),
            TyKind::Con(d, args) => {
                let d = *d;
                let name = self.inf.lang_name(d).unwrap_or_default();
                let data = [
                    "Bool",
                    "I8",
                    "I16",
                    "I32",
                    "I64",
                    "U8",
                    "U16",
                    "U32",
                    "U64",
                    "F32",
                    "F64",
                    "Char",
                    "Symbol",
                    "String",
                    "Bytes",
                    "Duration",
                    "Instant",
                    "Timestamp",
                    "Data",
                    "List",
                    "Map",
                    "Set",
                    "Array",
                    "Option",
                    "Result",
                    "Never",
                    "Unit",
                ];
                if data.contains(&name.as_str()) {
                    return args.iter().any(|x| self.opaque_in(x, seen));
                }
                match expand(seen, t) {
                    Expand::Cycle => return false,
                    Expand::Deep => return args.iter().any(|x| self.opaque_in(x, seen)),
                    Expand::Go => {}
                }
                let fields: Option<Vec<Ty>> = if let Some(e) = self.inf.g.env.enums.get(&d) {
                    Some(
                        (0..e.variants.len())
                            .flat_map(|v| {
                                self.inf.g.env.variant_fields(d, v as u32, args).into_iter().map(|f| f.1)
                            })
                            .collect(),
                    )
                } else if self.inf.g.env.structs.contains_key(&d) {
                    Some(self.inf.g.env.struct_fields(d, args).into_iter().map(|f| f.1).collect())
                } else {
                    None
                };
                let r = match fields {
                    // user and library structs/enums: by their contents
                    Some(fs) => fs.iter().any(|x| self.opaque_in(x, seen)),
                    // runtime handles (pools, transactions, tasks, endpoints)
                    None => true,
                };
                seen.pop();
                r
            }
            _ => false,
        }
    }

    fn tainted(&mut self, e: ExprId) -> bool {
        let hir = self.inf.g.hir;
        let Some(t) = self.ty(e) else { return false };
        if self.mentions(&t) {
            return true;
        }
        let opaque = self.opaque(&t);
        match &hir.exprs[e].kind {
            ExprKind::Local(l) => self.tainted.get(l).copied().unwrap_or(false) && opaque,
            // a `for<S>` callback's result was checked where the callback
            // was written: it cannot reach a value of the scope it is given
            ExprKind::Call { callee, .. }
                if self.ty(*callee).is_some_and(|t| matches!(&*t, TyKind::Forall(..)))
                    && !self.tainted(*callee) =>
            {
                false
            }
            ExprKind::Call { callee, args } => {
                opaque && (self.tainted(*callee) || args.clone().into_iter().any(|a| self.tainted(a)))
            }
            // what a child task returns crossed a task boundary: it can only
            // reach the scope if its type says so (checked above)
            ExprKind::MethodCall { recv, method, .. }
                if matches!(self.inf.sym(*method), "join" | "try_join")
                    && self.ty(*recv).is_some_and(|t| {
                        matches!(&*t, TyKind::Con(d, _) if Some(*d) == self.inf.g.hir.try_lang("task.Task"))
                    }) =>
            {
                false
            }
            ExprKind::MethodCall { recv, args, .. } => {
                opaque && (self.tainted(*recv) || args.clone().into_iter().any(|a| self.tainted(a)))
            }
            ExprKind::Field { base, .. } | ExprKind::Index { base, .. } => opaque && self.tainted(*base),
            // `r?` is `r`'s success value: plain data cannot hold on to the
            // scope even when the `Result` could (an error with a `dyn` cause)
            ExprKind::Try(x) => opaque && self.tainted(*x),
            ExprKind::Unsafe(x) => self.tainted(*x),
            ExprKind::Lambda(l) => l.captures.iter().any(|c| self.tainted.get(c).copied().unwrap_or(false)),
            ExprKind::Tuple(xs) | ExprKind::List(xs) | ExprKind::Set(xs) => {
                opaque && xs.clone().into_iter().any(|x| self.tainted(x))
            }
            ExprKind::Record { spread, fields } => {
                opaque
                    && (spread.is_some_and(|s| self.tainted(s))
                        || fields.clone().into_iter().any(|f| self.tainted(f.2)))
            }
            ExprKind::Map(kv) => {
                opaque && kv.clone().into_iter().any(|(k, v)| self.tainted(k) || self.tainted(v))
            }
            ExprKind::Block { tail: Some(t), .. } => self.tainted(*t),
            ExprKind::If { then, els, .. } => self.tainted(*then) || els.is_some_and(|x| self.tainted(x)),
            ExprKind::Match { arms, .. } => arms.clone().into_iter().any(|a| self.tainted(a.body)),
            _ => false,
        }
    }

    /// Mark let-bound locals whose values derive from scoped ones.
    fn scan(&mut self, e: ExprId) {
        let hir = self.inf.g.hir;
        if let ExprKind::Block { stmts, .. } = &hir.exprs[e].kind {
            for s in stmts {
                match s {
                    Stmt::Let { pat, init, .. } => {
                        if self.tainted(*init) {
                            let mut ls = vec![];
                            collect(hir, *pat, &mut ls);
                            for l in ls {
                                self.tainted.insert(l, true);
                            }
                        }
                    }
                    Stmt::Assign { local, value, .. } => {
                        if self.tainted(*value) {
                            self.tainted.insert(*local, true);
                        }
                    }
                    Stmt::Expr { .. } => {}
                }
            }
        }
        // lambda parameters typed with the region are tainted through types
        for c in sub_exprs(&hir.exprs[e].kind) {
            self.scan(c);
        }
    }

    /// Expressions whose value leaves the callback.
    fn exits(&self, e: ExprId, out: &mut Vec<ExprId>) {
        let hir = self.inf.g.hir;
        match &hir.exprs[e].kind {
            ExprKind::Block { tail: Some(t), .. } => self.exits(*t, out),
            ExprKind::If { then, els: Some(x), .. } => {
                self.exits(*then, out);
                self.exits(*x, out);
            }
            ExprKind::Match { arms, .. } => arms.iter().for_each(|a| self.exits(a.body, out)),
            _ => out.push(e),
        }
        self.returns(e, out);
    }

    fn returns(&self, e: ExprId, out: &mut Vec<ExprId>) {
        let hir = self.inf.g.hir;
        match &hir.exprs[e].kind {
            ExprKind::Lambda(_) => {}
            ExprKind::Return { value: Some(v), .. } => out.push(*v),
            k => {
                for c in sub_exprs(k) {
                    self.returns(c, out);
                }
            }
        }
    }

    /// A scoped value written into a cell, atom, collection, or channel that
    /// the callback captured from outside (or assigned to an outer `var`)
    /// outlives the region.
    fn stores(&mut self, e: ExprId, captures: &[LocalId]) {
        let hir = self.inf.g.hir;
        // a helper that stores one parameter into another's cell, atom,
        // collection, or channel stores the caller's argument there
        let summaries = store_summaries(self.inf.g);
        for (flows, args) in helper_calls(self.inf.g, &summaries, e) {
            for (from, into) in flows {
                let (Some(&src), Some(&dst)) = (args.get(from), args.get(into)) else { continue };
                if aliases(hir, dst, captures) && self.tainted(src) {
                    let span = hir.exprs[src].span;
                    self.report(span);
                }
            }
        }
        // the local a place expression (`ch.sender`, `state.items[0]`) is rooted in
        let captured = |mut x: ExprId| loop {
            match &hir.exprs[x].kind {
                ExprKind::Local(l) => break captures.contains(l),
                ExprKind::Field { base, .. } | ExprKind::Index { base, .. } => x = *base,
                ExprKind::Unary { op: badger_hir::UnOp::Ref | badger_hir::UnOp::RefMut, operand } => {
                    x = *operand
                }
                _ => break false,
            }
        };
        match &hir.exprs[e].kind {
            ExprKind::MethodCall { recv, method, args, .. } => {
                let m = self.inf.sym(*method).to_string();
                if matches!(
                    m.as_str(),
                    "set"
                        | "update"
                        | "swap"
                        | "push"
                        | "append"
                        | "try_append"
                        | "send"
                        | "try_send"
                        | "insert"
                ) && captured(*recv)
                {
                    for a in args.clone() {
                        // `update(f)` stores what `f` returns: plain data
                        // cannot carry the scope
                        if m == "update"
                            && let Some(TyKind::Fn(f)) = self.ty(a).as_deref()
                        {
                            let ret = f.ret.clone();
                            if !self.mentions(&ret) && !self.opaque(&ret) {
                                continue;
                            }
                        }
                        if self.tainted(a) {
                            let span = hir.exprs[a].span;
                            self.report(span);
                        }
                    }
                }
            }
            // a send operation or case naming an outer sender (`Op.send`,
            // `channel.on_send`) delivers its payload outside the region
            ExprKind::Call { args, .. } => {
                let outer_sender = args.iter().any(|a| {
                    captured(*a)
                        && self.ty(*a).is_some_and(|t| {
                            matches!(&*t, TyKind::Con(d, _) if [
                                self.inf.g.hir.try_lang("channel.Sender"),
                                self.inf.g.hir.try_lang("channel.BatchSender"),
                            ].contains(&Some(*d)))
                        })
                });
                if outer_sender {
                    for a in args.clone() {
                        if self.tainted(a) {
                            let span = hir.exprs[a].span;
                            self.report(span);
                        }
                    }
                }
            }
            ExprKind::Block { stmts, .. } => {
                for st in stmts {
                    if let Stmt::Assign { local, value, .. } = st
                        && captures.contains(local)
                        && self.tainted(*value)
                    {
                        let span = hir.exprs[*value].span;
                        self.report(span);
                    }
                }
            }
            _ => {}
        }
        for c in sub_exprs(&hir.exprs[e].kind) {
            self.stores(c, captures);
        }
    }

    /// Child tasks using a scoped value must belong to a scope inside the
    /// value's region (they then finish before it closes).
    fn spawns(&mut self, e: ExprId) {
        let hir = self.inf.g.hir;
        if let ExprKind::MethodCall { recv, method, args, .. } = &hir.exprs[e].kind {
            let m = self.inf.sym(*method).to_string();
            if matches!(m.as_str(), "spawn" | "capture")
                && args.len() == 1
                && let Some(rt) = self.ty(*recv)
            {
                let scope_skolem = match &*rt {
                    TyKind::Con(d, a) if Some(*d) == self.inf.g.hir.try_lang("task.TaskScope") => {
                        match a.first().map(|x| &**x) {
                            Some(TyKind::Skolem(s)) => Some(*s),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                if let Some(s) = scope_skolem {
                    let level = self.inf.icx.skolems.get(s as usize).map(|i| i.level).unwrap_or(0);
                    let newest = self
                        .regions
                        .iter()
                        .filter_map(|r| self.inf.icx.skolems.get(*r as usize))
                        .map(|i| i.level)
                        .max()
                        .unwrap_or(0);
                    if !self.regions.contains(&s) && level < newest && self.tainted(args[0]) {
                        let span = hir.exprs[args[0]].span;
                        self.inf.push_diag(
                                Diagnostic::error("a child task could outlive the scoped value it uses")
                                    .with_code("E0401")
                                    .with_label(span, "this task uses a value bound to an inner scope")
                                    .with_note("spawn it into a task scope opened inside that scope, so it finishes before the value closes"),
                            );
                    }
                }
            }
        }
        for c in sub_exprs(&hir.exprs[e].kind) {
            self.spawns(c);
        }
    }

    fn report(&mut self, span: Span) {
        if let Some(&r) = self.regions.first()
            && self.inf.icx.skolems[r as usize].lexical
        {
            let name = self.inf.g.interner.resolve(self.inf.icx.skolems[r as usize].name).to_string();
            self.inf.push_diag(
                Diagnostic::error(format!("a value derived from region `{name}` outlives its region"))
                    .with_code("E0414")
                    .with_label(span, format!("this can still reach a value of region `{name}`"))
                    .with_note("closures, lazy sequences, tasks, and handles built from a region's values cannot leave its `region` block; return plain data instead"),
            );
            return;
        }
        self.inf.push_diag(
            Diagnostic::error("a value derived from a scoped value escapes its callback")
                .with_code("E0401")
                .with_label(span, "this result can still reach a value bound to the callback's scope")
                .with_note("closures, lazy sequences, tasks, and resource handles built from a scoped value cannot outlive the callback that introduced its scope; return plain data instead"),
        );
    }
}

pub(crate) fn collect(hir: &badger_hir::Hir, p: badger_hir::PatId, out: &mut Vec<LocalId>) {
    hir.pat_binders(p, out)
}

/// The direct sub-expressions of `kind`, inline for the common short lists
/// (the walks over a body call this once per node).
pub(crate) fn sub_exprs(kind: &ExprKind) -> smallvec::SmallVec<[ExprId; 8]> {
    let mut v = smallvec::SmallVec::new();
    kind.for_each_child(|c| v.push(c));
    v
}

/// `seeds` and the locals of `body` bound or assigned to an alias of them
/// (the value itself, a part of it, or a record, list, option, or closure
/// holding it), transitively.
fn derived_from(hir: &badger_hir::Hir, body: ExprId, seeds: &[LocalId]) -> Vec<LocalId> {
    let mut out = seeds.to_vec();
    loop {
        let mut grew = false;
        crate::program::walk_exprs(hir, body, &mut |x| {
            // bindings destructuring a derived value (match arms, loop items)
            let bound: Vec<(PatId, ExprId)> = match &hir.exprs[x].kind {
                ExprKind::Match { scrutinee, arms } => arms.iter().map(|a| (a.pat, *scrutinee)).collect(),
                ExprKind::For { pat, iter, .. } => vec![(*pat, *iter)],
                _ => vec![],
            };
            for (pat, from) in bound {
                if aliases(hir, from, &out) {
                    let mut ls = vec![];
                    collect(hir, pat, &mut ls);
                    for t in ls {
                        if !out.contains(&t) {
                            out.push(t);
                            grew = true;
                        }
                    }
                }
            }
            let ExprKind::Block { stmts, .. } = &hir.exprs[x].kind else { return };
            for st in stmts {
                let (targets, from) = match st {
                    Stmt::Let { pat, init, .. } => {
                        let mut ls = vec![];
                        collect(hir, *pat, &mut ls);
                        (ls, *init)
                    }
                    Stmt::Assign { local, value, .. } => (vec![*local], *value),
                    Stmt::Expr { .. } => continue,
                };
                if aliases(hir, from, &out) {
                    for t in targets {
                        if !out.contains(&t) {
                            out.push(t);
                            grew = true;
                        }
                    }
                }
            }
        });
        if !grew {
            return out;
        }
    }
}

/// Can the value of `e` be (or hold) one of `locals` itself, rather than a
/// new value computed from it?
fn aliases(hir: &badger_hir::Hir, e: ExprId, locals: &[LocalId]) -> bool {
    match &hir.exprs[e].kind {
        ExprKind::Local(l) => locals.contains(l),
        ExprKind::Lambda(l) => l.captures.iter().any(|c| locals.contains(c)),
        ExprKind::Field { base, .. } | ExprKind::Index { base, .. } => aliases(hir, *base, locals),
        ExprKind::Unary { op: badger_hir::UnOp::Ref | badger_hir::UnOp::RefMut, operand } => {
            aliases(hir, *operand, locals)
        }
        ExprKind::Tuple(xs) | ExprKind::List(xs) => xs.iter().any(|x| aliases(hir, *x, locals)),
        ExprKind::Record { fields, .. } => fields.iter().any(|f| aliases(hir, f.2, locals)),
        // constructors and the prelude's wrapping functions hold their argument
        ExprKind::Call { callee, args }
            if match &hir.exprs[*callee].kind {
                ExprKind::Ctor { .. } => true,
                ExprKind::Def { def, .. } => {
                    ["some", "ok", "err"].iter().any(|n| hir.try_lang(n) == Some(*def))
                }
                _ => false,
            } =>
        {
            args.iter().any(|a| aliases(hir, *a, locals))
        }
        ExprKind::Block { tail: Some(t), .. } => aliases(hir, *t, locals),
        ExprKind::If { then, els, .. } => {
            aliases(hir, *then, locals) || els.is_some_and(|x| aliases(hir, x, locals))
        }
        ExprKind::Match { arms, .. } => arms.iter().any(|a| aliases(hir, a.body, locals)),
        _ => false,
    }
}

/// Does `e` mention any of `locals` (also as closure captures)?
fn mentions_any(hir: &badger_hir::Hir, e: ExprId, locals: &[LocalId]) -> bool {
    let mut found = false;
    crate::program::walk_exprs(hir, e, &mut |x| match &hir.exprs[x].kind {
        ExprKind::Local(l) if locals.contains(l) => found = true,
        ExprKind::Lambda(l) if l.captures.iter().any(|c| locals.contains(c)) => found = true,
        _ => {}
    });
    found
}

/// Calls of storing helpers at `e`: each candidate's flows with the call's
/// arguments in parameter order (a method call's receiver first; methods
/// are matched by name and arity, which over-approximates the target).
fn helper_calls(
    g: &crate::program::Global<'_>,
    summaries: &FxHashMap<badger_hir::DefId, Vec<(usize, usize)>>,
    e: ExprId,
) -> Vec<(Vec<(usize, usize)>, Vec<ExprId>)> {
    let hir = g.hir;
    match &hir.exprs[e].kind {
        ExprKind::Call { callee, args } => match &hir.exprs[*callee].kind {
            ExprKind::Def { def, .. } => {
                summaries.get(def).map(|f| vec![(f.clone(), args.clone())]).unwrap_or_default()
            }
            _ => vec![],
        },
        ExprKind::MethodCall { recv, method, args, .. } => {
            let mut all = vec![*recv];
            all.extend(args.iter().copied());
            summaries
                .iter()
                .filter(|(d, _)| {
                    hir.defs[**d].name == *method
                        && matches!(&hir.defs[**d].kind, badger_hir::DefKind::Fn(f)
                            if f.has_self && f.params.len() == all.len())
                })
                .map(|(_, f)| (f.clone(), all.clone()))
                .collect()
        }
        _ => vec![],
    }
}

/// For each user function, the parameter pairs `(from, into)` where the
/// body stores (a value derived from) parameter `from` into a cell, atom,
/// collection, or channel reached through parameter `into` (directly or
/// through other such helpers), computed once to a fixpoint.
pub(crate) fn store_summaries(
    g: &crate::program::Global<'_>,
) -> std::rc::Rc<FxHashMap<badger_hir::DefId, Vec<(usize, usize)>>> {
    if let Some(s) = g.store_summaries.borrow().as_ref() {
        return s.clone();
    }
    let hir = g.hir;
    let is_std = |d: badger_hir::DefId| hir.modules[hir.defs[d].module].is_std;
    // with a seeded prefix the standard helpers are already summarized; only
    // the definitions outside the standard library are recomputed
    let seeded = g.store_seed.is_some();
    let storing = |name: &str| {
        matches!(
            name,
            "set" | "update" | "swap" | "push" | "append" | "try_append" | "send" | "try_send" | "insert"
        )
    };
    let mut out: FxHashMap<badger_hir::DefId, Vec<(usize, usize)>> = match &g.store_seed {
        Some(seed) => (**seed).clone(),
        None => FxHashMap::default(),
    };
    loop {
        let mut changed = false;
        for (id, def) in hir.defs.iter() {
            if seeded && is_std(id) {
                continue;
            }
            let badger_hir::DefKind::Fn(f) = &def.kind else { continue };
            let Some(body) = f.body else { continue };
            let params: Vec<Option<LocalId>> = f
                .params
                .iter()
                .map(|p| match &hir.pats[p.pat].kind {
                    PatKind::Bind(l) => Some(*l),
                    _ => None,
                })
                .collect();
            // each parameter with the locals derived from it in the body
            let derived: Vec<Vec<LocalId>> = params
                .iter()
                .map(|p| match p {
                    Some(p) => derived_from(hir, body, &[*p]),
                    None => vec![],
                })
                .collect();
            let mut found: Vec<(usize, usize)> = vec![];
            crate::program::walk_exprs(hir, body, &mut |e| {
                // (stored values, target place)
                let mut pairs: Vec<(ExprId, ExprId)> = vec![];
                match &hir.exprs[e].kind {
                    ExprKind::MethodCall { recv, method, args, .. }
                        if storing(g.interner.resolve(*method)) =>
                    {
                        for a in args {
                            pairs.push((*a, *recv));
                        }
                    }
                    _ => {}
                }
                for (flows, args) in helper_calls(g, &out, e) {
                    for (from, into) in flows {
                        if let (Some(a), Some(b)) = (args.get(from), args.get(into)) {
                            pairs.push((*a, *b));
                        }
                    }
                }
                for (val, place) in pairs {
                    for (j, dj) in derived.iter().enumerate() {
                        if dj.is_empty() || !aliases(hir, place, dj) {
                            continue;
                        }
                        for (i, di) in derived.iter().enumerate() {
                            if i != j
                                && !di.is_empty()
                                && mentions_any(hir, val, di)
                                && !found.contains(&(i, j))
                            {
                                found.push((i, j));
                            }
                        }
                    }
                }
            });
            if !found.is_empty() {
                let e = out.entry(id).or_default();
                for x in found {
                    if !e.contains(&x) {
                        e.push(x);
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    let out = std::rc::Rc::new(out);
    *g.store_summaries.borrow_mut() = Some(out.clone());
    out
}
