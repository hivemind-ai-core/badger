//! Match exhaustiveness (usefulness, after Maranget). Guarded arms do not
//! establish exhaustiveness. Literal domains other than `Bool` are infinite,
//! so they need a wildcard or binding.

use lang_base::{Diagnostic, Symbol};
use lang_hir::{DefId, DefKind, ExprId, ExprKind, Lit, PatId, PatKind};

use crate::infer::Infer;
use crate::program::walk_exprs;
use crate::ty::*;

#[derive(Clone, Debug)]
enum P {
    Wild,
    /// Constructor index within the type's constructor set, with fields.
    Ctor(Ctor, Vec<P>),
}

#[derive(Clone, Debug, PartialEq)]
enum Ctor {
    Variant(u32),
    Bool(bool),
    /// The single constructor of a product type (tuple, record, struct).
    Product,
    /// A literal of an infinite domain.
    Lit(String),
}

/// Constructors of a type: `None` means infinite or unknown.
fn ctors(inf: &mut Infer<'_, '_>, t: &Ty) -> Option<Vec<(Ctor, Vec<Ty>)>> {
    let t = inf.deref(t);
    let t = inf.icx.resolve(&t);
    match &*t {
        TyKind::Con(d, args) if inf.g.env.enums.contains_key(d) => {
            let n = inf.g.env.enums[d].variants.len();
            Some(
                (0..n)
                    .map(|i| {
                        (
                            Ctor::Variant(i as u32),
                            inf.g.env.variant_fields(*d, i as u32, args).into_iter().map(|f| f.1).collect(),
                        )
                    })
                    .collect(),
            )
        }
        TyKind::Con(d, _) if *d == inf.b().bool_ => {
            Some(vec![(Ctor::Bool(true), vec![]), (Ctor::Bool(false), vec![])])
        }
        TyKind::Con(d, args) if inf.g.env.structs.contains_key(d) => {
            let mut fs = inf.g.env.struct_fields(*d, args);
            fs.sort_by(|a, b| inf.sym(a.0).cmp(inf.sym(b.0)));
            Some(vec![(Ctor::Product, fs.into_iter().map(|f| f.1).collect())])
        }
        TyKind::Tuple(ts) => Some(vec![(Ctor::Product, ts.clone())]),
        TyKind::Record(r) => {
            let (mut fs, _) = row_fields(r);
            fs.sort_by(|a, b| inf.sym(a.0).cmp(inf.sym(b.0)));
            Some(vec![(Ctor::Product, fs.into_iter().map(|f| f.1).collect())])
        }
        _ => None,
    }
}

/// Field labels of a product type in canonical order.
fn product_labels(inf: &mut Infer<'_, '_>, t: &Ty) -> Vec<Symbol> {
    let t = inf.deref(t);
    let t = inf.icx.resolve(&t);
    let mut ls: Vec<Symbol> = match &*t {
        TyKind::Con(d, args) if inf.g.env.structs.contains_key(d) => {
            inf.g.env.struct_fields(*d, args).into_iter().map(|f| f.0).collect()
        }
        TyKind::Record(r) => row_fields(r).0.into_iter().map(|f| f.0).collect(),
        _ => vec![],
    };
    ls.sort_by(|a, b| inf.sym(*a).cmp(inf.sym(*b)));
    ls
}

fn lower(inf: &mut Infer<'_, '_>, p: PatId, t: &Ty) -> P {
    let pat = inf.g.hir.pats[p].clone();
    match pat.kind {
        PatKind::Discard | PatKind::Bind(_) | PatKind::Error => P::Wild,
        PatKind::Lit(Lit::Bool(b)) => P::Ctor(Ctor::Bool(b), vec![]),
        PatKind::Lit(Lit::Unit) => P::Wild,
        PatKind::Lit(l) => P::Ctor(Ctor::Lit(format!("{l:?}")), vec![]),
        PatKind::Tuple(ps) => {
            let tys = match ctors(inf, t) {
                Some(cs) if cs.len() == 1 => cs[0].1.clone(),
                _ => vec![error_ty(); ps.len()],
            };
            P::Ctor(Ctor::Product, ps.iter().zip(tys).map(|(sp, st)| lower(inf, *sp, &st)).collect())
        }
        PatKind::Record { fields, .. } => {
            let labels = product_labels(inf, t);
            let tys = match ctors(inf, t) {
                Some(cs) if cs.len() == 1 => cs[0].1.clone(),
                _ => vec![],
            };
            let mut sub = vec![P::Wild; labels.len()];
            for (l, sp) in fields {
                if let Some(i) = labels.iter().position(|x| *x == l) {
                    let st = tys.get(i).cloned().unwrap_or_else(error_ty);
                    sub[i] = lower(inf, sp, &st);
                }
            }
            P::Ctor(Ctor::Product, sub)
        }
        PatKind::Variant { name, args, .. } => {
            let t2 = inf.deref(t);
            let t2 = inf.icx.resolve(&t2);
            let TyKind::Con(d, targs) = &*t2 else { return P::Wild };
            let Some(info) = inf.g.env.enums.get(d) else { return P::Wild };
            let Some(vi) = info.variants.iter().position(|v| v.name == name) else { return P::Wild };
            let ftys: Vec<Ty> =
                inf.g.env.variant_fields(*d, vi as u32, targs).into_iter().map(|f| f.1).collect();
            let mut sub: Vec<P> = args.iter().zip(&ftys).map(|(sp, st)| lower(inf, *sp, st)).collect();
            while sub.len() < ftys.len() {
                sub.push(P::Wild);
            }
            P::Ctor(Ctor::Variant(vi as u32), sub)
        }
    }
}

/// Is `v` useful with respect to `rows` (all of types `tys`)? Returns a
/// witness of an unmatched value when it is.
fn useful(inf: &mut Infer<'_, '_>, rows: &[Vec<P>], v: &[P], tys: &[Ty]) -> Option<Vec<String>> {
    if v.is_empty() {
        return if rows.is_empty() { Some(vec![]) } else { None };
    }
    let t = tys[0].clone();
    let rest_tys = &tys[1..];
    let specialize = |rows: &[Vec<P>], c: &Ctor, arity: usize| -> Vec<Vec<P>> {
        rows.iter()
            .filter_map(|r| match &r[0] {
                P::Wild => {
                    let mut n = vec![P::Wild; arity];
                    n.extend_from_slice(&r[1..]);
                    Some(n)
                }
                P::Ctor(c2, sub) if c2 == c => {
                    let mut n = sub.clone();
                    n.resize(arity, P::Wild);
                    n.extend_from_slice(&r[1..]);
                    Some(n)
                }
                _ => None,
            })
            .collect()
    };
    match &v[0] {
        P::Ctor(c, sub) => {
            let cs = ctors(inf, &t).unwrap_or_default();
            let ftys = cs
                .iter()
                .find(|x| &x.0 == c)
                .map(|x| x.1.clone())
                .unwrap_or_else(|| vec![error_ty(); sub.len()]);
            let srows = specialize(rows, c, ftys.len());
            let mut nv = sub.clone();
            nv.resize(ftys.len(), P::Wild);
            nv.extend_from_slice(&v[1..]);
            let mut ntys = ftys;
            ntys.extend_from_slice(rest_tys);
            useful(inf, &srows, &nv, &ntys)
        }
        P::Wild => {
            let cs = ctors(inf, &t);
            let used: Vec<Ctor> = rows
                .iter()
                .filter_map(|r| match &r[0] {
                    P::Ctor(c, _) => Some(c.clone()),
                    P::Wild => None,
                })
                .collect();
            let complete =
                matches!(&cs, Some(cs) if !cs.is_empty() && cs.iter().all(|c| used.contains(&c.0)));
            if complete {
                for (c, ftys) in cs.unwrap() {
                    let srows = specialize(rows, &c, ftys.len());
                    let mut nv = vec![P::Wild; ftys.len()];
                    nv.extend_from_slice(&v[1..]);
                    let mut ntys = ftys.clone();
                    ntys.extend_from_slice(rest_tys);
                    if let Some(w) = useful(inf, &srows, &nv, &ntys) {
                        let (args, rest) = w.split_at(ftys.len().min(w.len()));
                        let mut out = vec![show_ctor(inf, &t, &c, args)];
                        out.extend_from_slice(rest);
                        return Some(out);
                    }
                }
                None
            } else {
                // default matrix
                let drows: Vec<Vec<P>> =
                    rows.iter().filter(|r| matches!(r[0], P::Wild)).map(|r| r[1..].to_vec()).collect();
                let w = useful(inf, &drows, &v[1..], rest_tys)?;
                let missing = match &cs {
                    Some(cs) if !cs.is_empty() => cs
                        .iter()
                        .find(|c| !used.contains(&c.0))
                        .map(|(c, f)| show_ctor(inf, &t, c, &vec!["_".to_string(); f.len()]))
                        .unwrap_or_else(|| "_".into()),
                    _ => "_".into(),
                };
                let mut out = vec![missing];
                out.extend(w);
                Some(out)
            }
        }
    }
}

fn show_ctor(inf: &mut Infer<'_, '_>, t: &Ty, c: &Ctor, args: &[String]) -> String {
    let t = inf.deref(t);
    let t = inf.icx.resolve(&t);
    match c {
        Ctor::Variant(i) => {
            let TyKind::Con(d, _) = &*t else { return "_".into() };
            let name = inf.sym(inf.g.env.enums[d].variants[*i as usize].name).to_string();
            if args.is_empty() { format!(".{name}") } else { format!(".{name}({})", args.join(", ")) }
        }
        Ctor::Bool(b) => b.to_string(),
        Ctor::Product => {
            if let TyKind::Tuple(_) = &*t {
                format!("({})", args.join(", "))
            } else {
                let labels = product_labels(inf, &t);
                let parts: Vec<String> =
                    labels.iter().zip(args).map(|(l, a)| format!("{}: {a}", inf.sym(*l))).collect();
                format!("{{ {} }}", parts.join(", "))
            }
        }
        Ctor::Lit(_) => "_".into(),
    }
}

pub fn check_def(inf: &mut Infer<'_, '_>, d: DefId) {
    let hir = inf.g.hir;
    let body = match &hir.defs[d].kind {
        DefKind::Fn(f) => f.body,
        DefKind::Global(g) => Some(g.value),
        _ => None,
    };
    let Some(body) = body else { return };
    let mut matches: Vec<ExprId> = vec![];
    walk_exprs(hir, body, &mut |e| {
        if let ExprKind::Match { .. } = hir.exprs[e].kind {
            matches.push(e);
        }
    });
    for m in matches {
        let ExprKind::Match { scrutinee, arms } = &hir.exprs[m].kind else { continue };
        let Some(st) = inf.exprs.get(scrutinee).cloned() else { continue };
        let st = inf.icx.resolve(&st);
        if ty_any(&st, &mut |k| matches!(k, TyKind::Error)) {
            continue;
        }
        let mut rows: Vec<Vec<P>> = vec![];
        for a in arms {
            let row = vec![lower(inf, a.pat, &st)];
            if a.guard.is_none() {
                rows.push(row);
            }
        }
        if let Some(w) = useful(inf, &rows, &[P::Wild], std::slice::from_ref(&st)) {
            let missing = w.first().cloned().unwrap_or_else(|| "_".into());
            let span = hir.exprs[*scrutinee].span;
            let mut d = Diagnostic::error("non-exhaustive match")
                .with_code("E0342")
                .with_label(span, format!("pattern `{missing}` is not covered"))
                .with_note(
                    "add an arm for it, or a wildcard `_` arm; guarded arms do not establish exhaustiveness",
                );
            // the scrutinee's type declaration lists the cases
            if let Some(def) = inf.head_def(&st) {
                let dd = &hir.defs[def];
                if !hir.modules[dd.module].is_std {
                    d = d.with_related(inf.g.interner.resolve(dd.name).to_string(), dd.span);
                }
            }
            inf.push_diag(d);
        }
    }
}

/// Generic providers must yield a concrete output at each system node: an
/// output type still holding inference variables is rejected (E0606).
pub fn check_system_nodes(inf: &mut Infer<'_, '_>, d: DefId, generalized: &[Ty]) {
    let hir = inf.g.hir;
    let Some(node_fn) = hir.try_lang("system.node") else { return };
    let body = match &hir.defs[d].kind {
        DefKind::Fn(f) => f.body,
        DefKind::Global(g) => Some(g.value),
        _ => None,
    };
    let Some(body) = body else { return };
    let mut calls: Vec<ExprId> = vec![];
    walk_exprs(hir, body, &mut |e| {
        if let ExprKind::Call { callee, .. } = &hir.exprs[e].kind
            && matches!(hir.exprs[*callee].kind, ExprKind::Def { def, .. } if def == node_fn)
        {
            calls.push(e);
        }
    });
    // variables of shape positions; region arguments are fixed when the
    // system runs, and effect rows of function outputs are not part of the
    // output's shape
    fn shape_vars(env: &crate::env::TypeEnv, hir: &lang_hir::Hir, t: &Ty, out: &mut Vec<TyVid>) {
        match &**t {
            TyKind::Var(v) => out.push(*v),
            TyKind::Fn(ft) => {
                for p in &ft.params {
                    shape_vars(env, hir, p, out);
                }
                shape_vars(env, hir, &ft.ret, out);
            }
            TyKind::Con(def, args) => {
                for (i, a) in args.iter().enumerate() {
                    if !env.param_is_region(hir, *def, i) {
                        shape_vars(env, hir, a, out);
                    }
                }
            }
            TyKind::Tuple(args) | TyKind::Dyn(_, args) => {
                for a in args {
                    shape_vars(env, hir, a, out);
                }
            }
            TyKind::Record(r) | TyKind::Ref(_, r) | TyKind::Forall(_, r) | TyKind::Proj(r, _) => {
                shape_vars(env, hir, r, out)
            }
            TyKind::RowExt(_, a, rest) => {
                shape_vars(env, hir, a, out);
                shape_vars(env, hir, rest, out);
            }
            _ => {}
        }
    }
    // variables still determined elsewhere: deferred constraints and module
    // values whose types are being inferred together with this definition
    let mut live: Vec<TyVid> = vec![];
    let mut tys: Vec<Ty> = inf.pending.iter().flat_map(|c| c.types()).collect();
    tys.extend(inf.g.pending_schemes.borrow().values().map(|s| s.ty.clone()));
    // the group's own types are about to be generalized: their variables
    // are the definition's (inferred) generic parameters
    tys.extend(generalized.iter().cloned());
    for t in tys {
        let t = inf.icx.resolve(&t);
        inf.icx.free_vars(&t, &mut live);
    }
    let live: Vec<TyVid> = live.into_iter().map(|v| inf.icx.root(v)).collect();
    for c in calls {
        let Some(t) = inf.exprs.get(&c).cloned() else { continue };
        let t = inf.icx.resolve(&t);
        let TyKind::Con(_, args) = &*t else { continue };
        let Some(out) = args.first() else { continue };
        if ty_any(out, &mut |k| matches!(k, TyKind::Error)) {
            continue;
        }
        let mut vs = vec![];
        shape_vars(&inf.g.env, hir, out, &mut vs);
        if vs.into_iter().all(|v| live.contains(&inf.icx.root(v))) {
            continue;
        }
        let ExprKind::Call { args: cargs, .. } = &hir.exprs[c].kind else { continue };
        let key = match cargs.first().map(|a| &hir.exprs[*a].kind) {
            Some(ExprKind::Lit(Lit::Symbol(s))) => format!(":{}", inf.sym(*s)),
            _ => "this node".into(),
        };
        let shown = inf.show(out);
        let span = hir.exprs[c].span;
        inf.push_diag(
            Diagnostic::error(format!("the output of system node `{key}` is not concrete"))
                .with_code("E0606")
                .with_label(span, format!("output type `{shown}` is not fully determined"))
                .with_note("a generic provider must yield a concrete output at each node; annotate the provider's type arguments"),
        );
    }
}

/// Rows hold unique labels: instantiating an open row `{x: T, ..Rest}` with
/// a `Rest` that already has `x` (the implied `Rest lacks x`) is rejected.
pub fn check_row_labels(inf: &mut Infer<'_, '_>, d: DefId) {
    let hir = inf.g.hir;
    let body = match &hir.defs[d].kind {
        DefKind::Fn(f) => f.body,
        DefKind::Global(g) => Some(g.value),
        _ => None,
    };
    let Some(body) = body else { return };
    let mut exprs: Vec<ExprId> = vec![];
    walk_exprs(hir, body, &mut |e| exprs.push(e));
    fn dup(t: &Ty) -> Option<Symbol> {
        match &**t {
            TyKind::Record(r) => {
                let mut seen: Vec<Symbol> = vec![];
                let mut cur = r.clone();
                loop {
                    match &*cur.clone() {
                        TyKind::RowExt(l, ft, rest) => {
                            if seen.contains(l) {
                                return Some(*l);
                            }
                            seen.push(*l);
                            if let Some(x) = dup(ft) {
                                return Some(x);
                            }
                            cur = rest.clone();
                        }
                        _ => return None,
                    }
                }
            }
            // effect rows are label sets, not records
            TyKind::Fn(ft) => ft.params.iter().find_map(dup).or_else(|| dup(&ft.ret)),
            TyKind::Con(_, args) | TyKind::Tuple(args) | TyKind::Dyn(_, args) => args.iter().find_map(dup),
            TyKind::Ref(_, x) | TyKind::Forall(_, x) | TyKind::Proj(x, _) => dup(x),
            _ => None,
        }
    }
    for e in exprs {
        let Some(t) = inf.exprs.get(&e).cloned() else { continue };
        let t = inf.icx.resolve(&t);
        if let Some(l) = dup(&t) {
            let (ls, ts) = (inf.sym(l).to_string(), inf.show(&t));
            let span = hir.exprs[e].span;
            inf.push_diag(
                Diagnostic::error(format!("a record would hold the label `{ls}` twice"))
                    .with_code("E0303")
                    .with_label(span, format!("this has type `{ts}`"))
                    .with_note(format!("an open row `{{{ls}: T, ..Rest}}` requires that `Rest` lacks `{ls}`; pass a record without `{ls}`")),
            );
            return;
        }
    }
}

/// `const` initializers are evaluated at check time: integer arithmetic
/// that overflows its type, divides by zero, or shifts too far is an error
/// here rather than a defect on first use (E0139).
pub fn check_const_arithmetic(inf: &mut Infer<'_, '_>) {
    use lang_hir::BinOp;
    let hir = inf.g.hir;
    let mut local_inits: rustc_hash::FxHashMap<lang_hir::LocalId, ExprId> =
        hir.const_local_inits.iter().copied().collect();
    // module constants referenced through their binding
    for (_, d) in hir.defs.iter() {
        if let DefKind::Global(g) = &d.kind
            && g.is_const
        {
            local_inits.insert(g.local, g.value);
        }
    }
    // the integer type of an expression: (bits, signed)
    fn int_ty(inf: &mut Infer<'_, '_>, e: ExprId) -> Option<(u32, bool)> {
        let t = inf.exprs.get(&e).cloned()?;
        let t = inf.icx.resolve(&t);
        let TyKind::Con(d, _) = &*t else { return None };
        let i = inf.g.env.b.ints.iter().position(|x| x == d)?;
        Some(([8, 16, 32, 64, 8, 16, 32, 64][i], i < 4))
    }
    fn fits(v: i128, (bits, signed): (u32, bool)) -> bool {
        if signed {
            let max = (1i128 << (bits - 1)) - 1;
            v >= -max - 1 && v <= max
        } else {
            v >= 0 && v < (1i128 << bits)
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn eval(
        inf: &mut Infer<'_, '_>,
        e: ExprId,
        locals: &rustc_hash::FxHashMap<lang_hir::LocalId, ExprId>,
        depth: usize,
        errs: &mut Vec<(lang_base::Span, String)>,
        ctx: Option<(u32, bool)>,
    ) -> Option<i128> {
        let hir = inf.g.hir;
        if depth > 32 {
            return None;
        }
        // a referenced constant's own expression takes the use's type
        let ty = int_ty(inf, e).or(ctx)?;
        match &hir.exprs[e].kind {
            // negative literals are two's complement: the cast keeps the sign
            ExprKind::Lit(Lit::Int { value, .. }) => Some(*value as i128),
            ExprKind::Def { def, .. } => match &hir.defs[*def].kind {
                DefKind::Global(g) if g.is_const => eval(inf, g.value, locals, depth + 1, errs, Some(ty)),
                _ => None,
            },
            ExprKind::Local(l) => {
                let init = *locals.get(l)?;
                eval(inf, init, locals, depth + 1, errs, Some(ty))
            }
            ExprKind::Unary { op: lang_hir::UnOp::Neg, operand } => {
                let v = eval(inf, *operand, locals, depth + 1, errs, Some(ty))?;
                let r = -v;
                if !fits(r, ty) {
                    errs.push((hir.exprs[e].span, format!("`-{v}` does not fit its type")));
                    return None;
                }
                Some(r)
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let (op, lhs, rhs) = (*op, *lhs, *rhs);
                let a = eval(inf, lhs, locals, depth + 1, errs, Some(ty))?;
                let b = eval(inf, rhs, locals, depth + 1, errs, Some(ty))?;
                let span = hir.exprs[e].span;
                let r = match op {
                    BinOp::Add => a.checked_add(b),
                    BinOp::Sub => a.checked_sub(b),
                    BinOp::Mul => a.checked_mul(b),
                    BinOp::Div | BinOp::Rem if b == 0 => {
                        errs.push((span, "division by zero".into()));
                        return None;
                    }
                    BinOp::Div => a.checked_div(b),
                    BinOp::Rem => a.checked_rem(b),
                    BinOp::Shl | BinOp::Shr if b < 0 || b >= ty.0 as i128 => {
                        errs.push((span, format!("shift by {b} is out of range for a {}-bit integer", ty.0)));
                        return None;
                    }
                    BinOp::Shl => {
                        // shifting out set bits overflows (as at run time)
                        let r = a.checked_shl(b as u32);
                        r.filter(|r| (r >> b) == a)
                    }
                    BinOp::Shr => Some(a >> b),
                    BinOp::BitAnd => Some(a & b),
                    BinOp::BitOr => Some(a | b),
                    BinOp::BitXor => Some(a ^ b),
                    _ => return None,
                };
                match r {
                    Some(r) if fits(r, ty) => Some(r),
                    _ => {
                        errs.push((span, format!("`{a} {} {b}` overflows its type", op.as_str())));
                        None
                    }
                }
            }
            _ => None,
        }
    }
    let mut errs = vec![];
    for &e in &hir.const_inits.clone() {
        let mut sub = vec![];
        walk_exprs(hir, e, &mut |x| sub.push(x));
        // evaluate each maximal integer expression once (from the root)
        let _ = eval(inf, e, &local_inits, 0, &mut errs, None);
        for x in sub {
            if matches!(hir.exprs[x].kind, ExprKind::Binary { .. }) && int_ty(inf, x).is_some() {
                let _ = eval(inf, x, &local_inits, 0, &mut errs, None);
            }
        }
    }
    errs.sort_by_key(|(s, _)| (s.file, s.start));
    errs.dedup_by_key(|(s, _)| (s.file, s.start));
    for (span, msg) in errs {
        inf.push_diag(
            Diagnostic::error(format!("constant arithmetic fails: {msg}"))
                .with_code("E0139")
                .with_label(span, "evaluated at check time")
                .with_note(
                    "a `const` is computed when the program is checked; this would be a defect when it runs",
                ),
        );
    }
}
