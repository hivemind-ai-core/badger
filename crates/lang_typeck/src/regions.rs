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

use lang_base::{Diagnostic, Span};
use lang_hir::{ExprId, ExprKind, LocalId, PatKind, Stmt};
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
        r.stores(l.body, &outer);
    }
    let lambdas = std::mem::take(&mut inf.region_lambdas);
    for (lam, regions) in lambdas {
        let hir = inf.g.hir;
        let ExprKind::Lambda(l) = &hir.exprs[lam].kind else { continue };
        let body = l.body;
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
        let captures = l.captures.clone();
        r.stores(body, &captures);
    }
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
    fn opaque(&mut self, t: &Ty, depth: usize) -> bool {
        if depth > 6 {
            return false;
        }
        match &**t {
            TyKind::Fn(_) | TyKind::Dyn(..) => true,
            TyKind::Skolem(s) => !self.inf.icx.skolems.get(*s as usize).is_some_and(|i| i.region),
            TyKind::Ref(_, x) | TyKind::Record(x) => self.opaque(x, depth + 1),
            TyKind::RowExt(_, a, rest) => self.opaque(a, depth + 1) || self.opaque(rest, depth + 1),
            TyKind::Tuple(ts) => ts.iter().any(|x| self.opaque(x, depth + 1)),
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
                    return args.iter().any(|x| self.opaque(x, depth + 1));
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
                match fields {
                    // user and library structs/enums: by their contents
                    Some(fs) => fs
                        .iter()
                        .any(|x| !matches!(&**x, TyKind::Con(y, _) if *y == d) && self.opaque(x, depth + 1)),
                    // runtime handles (pools, transactions, tasks, endpoints)
                    None => true,
                }
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
        let opaque = self.opaque(&t, 0);
        match &hir.exprs[e].kind {
            ExprKind::Local(l) => self.tainted.get(l).copied().unwrap_or(false) && opaque,
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
            ExprKind::Try(x) | ExprKind::Unsafe(x) => self.tainted(*x),
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
        if let ExprKind::Call { callee, args } = &hir.exprs[e].kind
            && let ExprKind::Def { def, .. } = &hir.exprs[*callee].kind
        {
            let flows = store_summaries(self.inf.g).get(def).cloned().unwrap_or_default();
            for (from, into) in flows {
                let (Some(&src), Some(&dst)) = (args.get(from), args.get(into)) else { continue };
                if place_in(hir, dst, captures) && self.tainted(src) {
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
                ExprKind::Unary { op: lang_hir::UnOp::Ref | lang_hir::UnOp::RefMut, operand } => x = *operand,
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
        self.inf.push_diag(
            Diagnostic::error("a value derived from a scoped value escapes its callback")
                .with_code("E0401")
                .with_label(span, "this result can still reach a value bound to the callback's scope")
                .with_note("closures, lazy sequences, tasks, and resource handles built from a scoped value cannot outlive the callback that introduced its scope; return plain data instead"),
        );
    }
}

pub(crate) fn collect(hir: &lang_hir::Hir, p: lang_hir::PatId, out: &mut Vec<LocalId>) {
    match &hir.pats[p].kind {
        PatKind::Bind(l) => out.push(*l),
        PatKind::Tuple(ps) => ps.iter().for_each(|x| collect(hir, *x, out)),
        PatKind::Record { fields, .. } => fields.iter().for_each(|(_, x)| collect(hir, *x, out)),
        PatKind::Variant { args, .. } => args.iter().for_each(|x| collect(hir, *x, out)),
        _ => {}
    }
}

pub(crate) fn sub_exprs(kind: &ExprKind) -> Vec<ExprId> {
    let mut v = vec![];
    match kind {
        ExprKind::Interp(parts) => {
            for p in parts {
                if let lang_hir::InterpPart::Expr(x) = p {
                    v.push(*x);
                }
            }
        }
        ExprKind::Field { base, .. } => v.push(*base),
        ExprKind::Index { base, index } => v.extend([*base, *index]),
        ExprKind::Call { callee, args } => {
            v.push(*callee);
            v.extend(args.iter().copied());
        }
        ExprKind::MethodCall { recv, args, .. } => {
            v.push(*recv);
            v.extend(args.iter().copied());
        }
        ExprKind::Try(x) | ExprKind::Unsafe(x) | ExprKind::Loop { body: x } => v.push(*x),
        ExprKind::Unary { operand, .. } => v.push(*operand),
        ExprKind::Binary { lhs, rhs, .. } => v.extend([*lhs, *rhs]),
        ExprKind::Tuple(xs) | ExprKind::List(xs) | ExprKind::Set(xs) => v.extend(xs.iter().copied()),
        ExprKind::Map(kv) => kv.iter().for_each(|(k, x)| v.extend([*k, *x])),
        ExprKind::Record { spread, fields } => {
            v.extend(spread.iter().copied());
            v.extend(fields.iter().map(|f| f.2));
        }
        ExprKind::Lambda(l) => v.push(l.body),
        ExprKind::If { cond, then, els } => {
            v.extend([*cond, *then]);
            v.extend(els.iter().copied());
        }
        ExprKind::Match { scrutinee, arms } => {
            v.push(*scrutinee);
            for a in arms {
                v.extend(a.guard.iter().copied());
                v.push(a.body);
            }
        }
        ExprKind::While { cond, body } => v.extend([*cond, *body]),
        ExprKind::For { iter, body, .. } => v.extend([*iter, *body]),
        ExprKind::Block { stmts, tail } => {
            for s in stmts {
                match s {
                    Stmt::Let { init, .. } => v.push(*init),
                    Stmt::Assign { value, .. } => v.push(*value),
                    Stmt::Expr { expr, .. } => v.push(*expr),
                }
            }
            v.extend(tail.iter().copied());
        }
        ExprKind::Return { value, .. } | ExprKind::Break { value, .. } => v.extend(value.iter().copied()),
        _ => {}
    }
    v
}

/// Whether a place (`x`, `x.f`, `&x`) is rooted in one of `locals`.
fn place_in(hir: &lang_hir::Hir, mut x: ExprId, locals: &[LocalId]) -> bool {
    loop {
        match &hir.exprs[x].kind {
            ExprKind::Local(l) => return locals.contains(l),
            ExprKind::Field { base, .. } | ExprKind::Index { base, .. } => x = *base,
            ExprKind::Unary { op: lang_hir::UnOp::Ref | lang_hir::UnOp::RefMut, operand } => x = *operand,
            _ => return false,
        }
    }
}

/// For each user function, the parameter pairs `(from, into)` where the
/// body stores (a value derived from) parameter `from` into a cell, atom,
/// collection, or channel reached through parameter `into` (directly or
/// through other such helpers), computed once to a fixpoint.
pub(crate) fn store_summaries(
    g: &crate::program::Global<'_>,
) -> FxHashMap<lang_hir::DefId, Vec<(usize, usize)>> {
    if let Some(s) = g.store_summaries.borrow().as_ref() {
        return s.clone();
    }
    let hir = g.hir;
    let storing = |name: &str| {
        matches!(
            name,
            "set" | "update" | "swap" | "push" | "append" | "try_append" | "send" | "try_send" | "insert"
        )
    };
    let mut out: FxHashMap<lang_hir::DefId, Vec<(usize, usize)>> = FxHashMap::default();
    // does `e` mention parameter local `p` (also inside closures)?
    fn mentions(hir: &lang_hir::Hir, e: ExprId, p: LocalId) -> bool {
        let mut found = false;
        crate::program::walk_exprs(hir, e, &mut |x| match &hir.exprs[x].kind {
            ExprKind::Local(l) if *l == p => found = true,
            ExprKind::Lambda(l) if l.captures.contains(&p) => found = true,
            _ => {}
        });
        found
    }
    for _ in 0..4 {
        let mut changed = false;
        for (id, def) in hir.defs.iter() {
            if hir.modules[def.module].is_std {
                continue;
            }
            let lang_hir::DefKind::Fn(f) = &def.kind else { continue };
            let Some(body) = f.body else { continue };
            let params: Vec<Option<LocalId>> = f
                .params
                .iter()
                .map(|p| match &hir.pats[p.pat].kind {
                    PatKind::Bind(l) => Some(*l),
                    _ => None,
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
                    ExprKind::Call { callee, args } => {
                        if let ExprKind::Def { def: cd, .. } = &hir.exprs[*callee].kind
                            && let Some(flows) = out.get(cd)
                        {
                            for (from, into) in flows {
                                if let (Some(a), Some(b)) = (args.get(*from), args.get(*into)) {
                                    pairs.push((*a, *b));
                                }
                            }
                        }
                    }
                    _ => {}
                }
                for (val, place) in pairs {
                    for (j, pj) in params.iter().enumerate() {
                        let Some(pj) = pj else { continue };
                        if !place_in(hir, place, &[*pj]) {
                            continue;
                        }
                        for (i, pi) in params.iter().enumerate() {
                            if let Some(pi) = pi
                                && i != j
                                && mentions(hir, val, *pi)
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
    *g.store_summaries.borrow_mut() = Some(out.clone());
    out
}
